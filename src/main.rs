mod compilation_database;
mod information_leak;
mod suppressions;

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    vec,
};

use anyhow::{anyhow, Context, Result};
use clang::{Clang, Entity, EntityKind, Index};
use compilation_database::CompileCommands;
use glob::glob;
use information_leak::{BinaryLocation, ConfirmedLeak};
use rayon::prelude::*;
use structopt::StructOpt;
use suppressions::Suppressions;

use crate::{
    compilation_database::{CompilationDatabase, CompileCommandsDatabase, FileListDatabase},
    information_leak::{print_confirmed_leaks, PotentialLeak},
    suppressions::parse_suppressions_file,
};

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Debug, StructOpt)]
#[structopt(name = PKG_NAME, about = "An information leak detector for C and C++ code bases")]
struct CpplumberOptions {
    /// Path to the output binary to scan for leaked information.
    #[structopt(parse(from_os_str), short, long = "bin")]
    binary_file_path: PathBuf,

    /// Additional include directories.
    /// Only used when project files aren't used.
    #[structopt(short = "I")]
    include_directories: Vec<String>,

    /// Additional preprocessor definitions.
    /// Only used when project files aren't used.
    #[structopt(short = "D")]
    compile_definitions: Vec<String>,

    /// Compilation database.
    #[structopt(parse(from_os_str), short, long = "project")]
    project_file_path: Option<PathBuf>,

    /// Path to a file containing rules to prevent certain errors from being
    /// generated.
    #[structopt(parse(from_os_str), short, long)]
    suppressions_list: Option<PathBuf>,

    /// Only report leaks once for artifacts declared in multiple locations.
    #[structopt(long)]
    ignore_multiple_declarations: bool,

    /// Report leaks for data declared in system headers
    #[structopt(long)]
    report_system_headers: bool,

    /// Minimum required size in bytes, for a leak to be reported. Defaults to 4.
    /// Warning: Setting this to a lower value might greatly increase resource
    /// consumption and reports' sizes.
    #[structopt(short, long)]
    minimum_leak_size: Option<usize>,

    /// Generate output as JSON.
    #[structopt(short, long = "json")]
    json_output: bool,

    /// List of source files to scan for (can be glob expressions).
    source_path_globs: Vec<String>,
}

fn main() -> Result<()> {
    // Default to 'info' if 'RUST_LOG' is not set
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Parse command-line options
    let options = CpplumberOptions::from_args();
    let minimum_leak_size = options.minimum_leak_size.unwrap_or(4);

    // Initial checks before starting work
    if !options.binary_file_path.is_file() {
        return Err(anyhow!(
            "'{}' is not a valid file path.",
            options.binary_file_path.display()
        ));
    }

    // Parse the suppression list if used
    let suppressions = if let Some(ref suppressions_list) = options.suppressions_list {
        log::info!("Parsing suppressions file...");
        Some(
            parse_suppressions_file(suppressions_list)
                .with_context(|| "Failed to parse suppressions list")?,
        )
    } else {
        None
    };

    log::info!("Gathering source files...");
    // Parse project file or process glob expressions
    let compilation_db = generate_compilation_database(&options)?;

    log::info!("Filtering suppressed files...");
    // Filter suppressed files from the list, to avoid parsing files we're not
    // interested in
    let compile_commands =
        filter_suppressed_files(compilation_db.get_all_compile_commands()?, &suppressions);

    log::info!("Extracting artifacts from source files...");
    // Parse source files and extract information that could leak
    let potential_leaks = extract_artifacts_from_source_files(
        compile_commands,
        compilation_db.is_file_path_in_arguments(),
        !options.report_system_headers,
        minimum_leak_size,
    )?;

    log::info!("Filtering suppressed artifacts...");
    // Filter suppressed artifacts by source location if needed
    // Note: We need to do this "again" because artifacts from suppressed
    // headers might have been included during the parsing of other files
    let potential_leaks = filter_suppressed_artifacts_by_origin(potential_leaks, &suppressions);
    // Filter suppressed artifacts by value if needed
    let potential_leaks = filter_suppressed_artifacts_by_value(potential_leaks, &suppressions);

    log::info!(
        "Looking for leaks in '{}'...",
        options.binary_file_path.display()
    );
    let leaks = if options.ignore_multiple_declarations {
        // Remove duplicated artifacts if needed
        let potential_leaks: HashSet<PotentialLeak> = HashSet::from_iter(potential_leaks);
        log::debug!("{:#?}", potential_leaks);
        find_leaks_in_binary_file(&options.binary_file_path, potential_leaks)?
    } else {
        log::debug!("{:#?}", potential_leaks);
        find_leaks_in_binary_file(&options.binary_file_path, potential_leaks)?
    };
    log::debug!("Done!");

    if leaks.is_empty() {
        // Nothing leaked, alright!
        Ok(())
    } else {
        // Print the result to stdout
        print_confirmed_leaks(leaks, options.json_output)?;

        // Return an error to indicate that leaks were found (useful for automation)
        Err(anyhow!("Leaks detected!"))
    }
}

fn gather_entities_by_kind<'tu>(
    root_entity: Entity<'tu>,
    entity_kind_filter: &[EntityKind],
    ignore_system_headers: bool,
) -> Vec<Entity<'tu>> {
    gather_entities_by_kind_rec(root_entity, entity_kind_filter, ignore_system_headers, 0)
}

fn gather_entities_by_kind_rec<'tu>(
    root_entity: Entity<'tu>,
    entity_kind_filter: &[EntityKind],
    ignore_system_headers: bool,
    current_depth: usize,
) -> Vec<Entity<'tu>> {
    let mut entities = vec![];

    let root_entity_kind = root_entity.get_kind();
    // Check the if entity's kind is one we're looking for
    if entity_kind_filter
        .iter()
        .any(|elem| elem == &root_entity_kind)
    {
        entities.push(root_entity);
    }

    for child in root_entity.get_children() {
        // Ignore entity if requested
        if ignore_system_headers && child.is_in_system_header() {
            continue;
        }

        let entities_sub = gather_entities_by_kind_rec(
            child,
            entity_kind_filter,
            ignore_system_headers,
            current_depth + 1,
        );
        entities.extend(entities_sub);
    }

    entities
}

fn generate_compilation_database(
    options: &CpplumberOptions,
) -> Result<Box<dyn CompilationDatabase>> {
    if let Some(ref project_file_path) = options.project_file_path {
        // Parse compile commands from the JSON database
        Ok(Box::new(CompileCommandsDatabase::new(project_file_path)?))
    } else {
        // Otherwise, process glob expressions
        let file_paths = options
            .source_path_globs
            .par_iter()
            .try_fold(
                Vec::new,
                |mut accum, glob_expression| -> Result<Vec<PathBuf>> {
                    if let Ok(paths) = glob(glob_expression) {
                        for path in paths {
                            accum.push(path?);
                        }
                    } else {
                        log::warn!(
                            "'{}' is not a valid path or glob expression, ignoring it",
                            glob_expression
                        );
                    }

                    Ok(accum)
                },
            )
            .try_reduce(Vec::new, |mut accum, mut other| {
                accum.append(&mut other);
                Ok(accum)
            })?;

        // Generate `arguments` from the CLI arguments
        let mut arguments = vec![];

        // Add include directories to the list of arguments
        for include_dir in options.include_directories.iter() {
            arguments.push(format!("-I{}", include_dir));
        }
        // Add preprocessor defitions to the list of arguments
        for compile_def in options.compile_definitions.iter() {
            arguments.push(format!("-D{}", compile_def));
        }

        log::debug!("Using arguments: {:?}", arguments);
        Ok(Box::new(FileListDatabase::new(&file_paths, arguments)))
    }
}

fn filter_suppressed_files(
    compile_cmds: CompileCommands,
    suppressions: &Option<Suppressions>,
) -> CompileCommands {
    if let Some(suppressions) = suppressions {
        compile_cmds
            .into_par_iter()
            .filter(|compile_cmd| {
                if let Some(file_path) = compile_cmd.filename.to_str() {
                    !suppressions
                        .files
                        .par_iter()
                        .any(|pattern| pattern.matches(file_path))
                } else {
                    true
                }
            })
            .collect()
    } else {
        compile_cmds
    }
}

fn extract_artifacts_from_source_files(
    compile_commands: CompileCommands,
    use_file_path_from_arguments: bool,
    ignore_system_headers: bool,
    minimum_leak_size: usize,
) -> Result<Vec<PotentialLeak>> {
    // Prepare the clang index
    let clang = Clang::new().map_err(|e| anyhow!(e))?;
    let index = Index::new(&clang, false, false);

    compile_commands
        .into_iter()
        // Populate indexes by parsing source files in parallel
        .try_fold(
            Vec::new(),
            |mut accum, compile_cmd| -> Result<Vec<PotentialLeak>> {
                // Note: For some reason, having the file path in `arguments` when
                // passing the file path explicitly to libclang make the parser fail.
                // So we explicitely avoid doing so.
                let file_path = if use_file_path_from_arguments {
                    PathBuf::default()
                } else {
                    compile_cmd.filename
                };
                let translation_unit = index
                    .parser(&file_path)
                    .arguments(&compile_cmd.arguments)
                    .parse()
                    .with_context(|| {
                        format!("Failed to parse source file '{}'", file_path.display())
                    })?;

                let string_literals = gather_entities_by_kind(
                    translation_unit.get_entity(),
                    &[EntityKind::StringLiteral],
                    ignore_system_headers,
                );

                accum.extend(string_literals.into_iter().filter_map(|literal| {
                    let leak_res: Result<PotentialLeak> = literal.try_into();
                    if let Ok(potential_leak) = leak_res {
                        if potential_leak.bytes.len() >= minimum_leak_size {
                            Some(potential_leak)
                        } else {
                            // Value is too small, ignore it
                            None
                        }
                    } else {
                        // Log failure and discard element
                        log::warn!(
                            "Failed to process entity '{:?}': {}",
                            literal,
                            leak_res.unwrap_err()
                        );
                        None
                    }
                }));

                Ok(accum)
            },
        )
}

fn filter_suppressed_artifacts_by_origin(
    potential_leaks: Vec<PotentialLeak>,
    suppressions: &Option<Suppressions>,
) -> Vec<PotentialLeak> {
    if let Some(suppressions) = suppressions {
        potential_leaks
            .into_par_iter()
            .filter(|leak| {
                let file_path = &leak.declaration_metadata.file;
                if let Some(file_path) = file_path.as_os_str().to_str() {
                    !suppressions
                        .files
                        .par_iter()
                        .any(|pattern| pattern.matches(file_path))
                } else {
                    true
                }
            })
            .collect()
    } else {
        potential_leaks
    }
}

fn filter_suppressed_artifacts_by_value(
    potential_leaks: Vec<PotentialLeak>,
    suppressions: &Option<Suppressions>,
) -> Vec<PotentialLeak> {
    if let Some(suppressions) = suppressions {
        potential_leaks
            .into_par_iter()
            .filter(|leak| !suppressions.artifacts.contains(&leak.leaked_information))
            .collect()
    } else {
        potential_leaks
    }
}

fn find_leaks_in_binary_file<PotentialLeakCollection>(
    binary_file_path: &Path,
    leak_desc: PotentialLeakCollection,
) -> Result<BTreeSet<ConfirmedLeak>>
where
    PotentialLeakCollection: IntoParallelIterator<Item = PotentialLeak>,
{
    // Read binary file's content
    let mut bin_file = File::open(binary_file_path)?;
    let mut bin_data = vec![];
    bin_file.read_to_end(&mut bin_data)?;

    // Build a map that allows to lookup "leaks' first byte -> leaks"
    let byte_to_leaks = leak_desc
        .into_par_iter()
        .fold(
            HashMap::new,
            |mut accum: HashMap<u8, Vec<PotentialLeak>>, potential_leak| {
                if let Some(key) = potential_leak.bytes.first() {
                    if let Some(value) = accum.get_mut(key) {
                        value.push(potential_leak);
                    } else {
                        accum.insert(*key, vec![potential_leak]);
                    }
                }

                accum
            },
        )
        // Reduce intermediate maps into one
        .reduce(HashMap::new, |mut accum, other| {
            for (other_key, mut other_value) in other {
                if let Some(value) = accum.get_mut(&other_key) {
                    value.append(&mut other_value);
                } else {
                    accum.insert(other_key, other_value);
                }
            }
            accum
        });

    // Go through the binary file byte by byte and try to match leaks that start
    // with each byte
    let shared_binary_file_path = Arc::new(binary_file_path.to_path_buf().canonicalize()?);
    let confirmed_leaks = bin_data
        .par_iter()
        .enumerate()
        // Find actual leaks
        .map(|(i, byte_value)| {
            let mut confirmed_leaks = BTreeSet::new();
            if let Some(potential_leaks) = byte_to_leaks.get(byte_value) {
                // Go through each candidate
                for leak in potential_leaks {
                    // Check bounds
                    if i + leak.bytes.len() <= bin_data.len() {
                        let byte_slice = &bin_data[i..i + leak.bytes.len()];
                        if byte_slice == leak.bytes {
                            // Bytes match, the leak is confirmed
                            confirmed_leaks.insert(ConfirmedLeak {
                                leaked_information: leak.leaked_information.clone(),
                                location: information_leak::LeakLocation {
                                    source: leak.declaration_metadata.clone(),
                                    binary: BinaryLocation {
                                        file: shared_binary_file_path.clone(),
                                        offset: i as u64,
                                    },
                                },
                            });
                        }
                    }
                }
            }

            confirmed_leaks
        })
        .reduce(BTreeSet::new, |mut accum, other| {
            accum.extend(other);
            accum
        });

    Ok(confirmed_leaks)
}

#[cfg(test)]
mod tests {
    use super::*;

    use serial_test::serial;

    const FILE_LIST_PROJ_PATH: &str = "tests/data/main/file_list_proj";

    #[test]
    #[serial]
    fn extract_artifacts_from_source_files_file_list() {
        let root_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FILE_LIST_PROJ_PATH);
        let file_list_db = FileListDatabase::new(
            &[
                root_dir_path.join("main.cc"),
                root_dir_path.join("header.h"),
            ],
            vec![
                "-DDEF_TEST".to_string(),
                format!("-I{}", FILE_LIST_PROJ_PATH),
            ],
        );
        let potential_leaks = extract_artifacts_from_source_files(
            file_list_db
                .get_all_compile_commands()
                .expect("get_all_compile_commands failed"),
            file_list_db.is_file_path_in_arguments(),
            true,
            0,
        )
        .expect("extract_artifacts_from_source_files failed");

        let expected_string_literals = vec![
            // header.h
            "\"included_string_literal\"",
            // main.cc
            "\"included_string_literal\"",
            "\"c_string\"",
            "u8\"utf8_string\"",
            "L\"wide_string\"",
            "u\"utf16_string\"",
            "U\"utf32_string\"",
            "\"raw_string\"",
            "u8\"raw_utf8_string\"",
            "L\"wide_raw_string\"",
            "u\"raw_utf16_string\"",
            "U\"raw_utf32_string\"",
            "\"def_test\"",
            "\"concatenated_string\"",
            r#""multiline\nstring""#,
            r#""'\"\n\t\a\b|\220|\220|\351\246\231|\351\246\231|\360\237\230\202""#,
            r#""%s\n""#,
            "\"preprocessor_string_literal\"",
            r#"L"%s\n""#,
            "L\"preprocessor_string_literal\"",
            r#""%s\n""#,
        ];

        // Check extracted string literals
        assert!(potential_leaks.iter().enumerate().all(|(i, leak)| {
            println!("{:?}", leak.leaked_information);
            *leak.leaked_information == expected_string_literals[i]
        }));
        assert_eq!(expected_string_literals.len(), potential_leaks.len());
    }

    #[test]
    #[serial]
    fn extract_artifacts_with_minimum_leak_size() {
        let root_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FILE_LIST_PROJ_PATH);
        let file_list_db = FileListDatabase::new(
            &[root_dir_path.join("main.cc")],
            vec![
                "-DDEF_TEST".to_string(),
                format!("-I{}", FILE_LIST_PROJ_PATH),
            ],
        );
        let potential_leaks = extract_artifacts_from_source_files(
            file_list_db
                .get_all_compile_commands()
                .expect("get_all_compile_commands failed"),
            file_list_db.is_file_path_in_arguments(),
            true,
            4,
        )
        .expect("extract_artifacts_from_source_files failed");

        // r#""%s\n""# should be removed
        let expected_string_literals = vec![
            // main.cc
            "\"included_string_literal\"",
            "\"c_string\"",
            "u8\"utf8_string\"",
            "L\"wide_string\"",
            "u\"utf16_string\"",
            "U\"utf32_string\"",
            "\"raw_string\"",
            "u8\"raw_utf8_string\"",
            "L\"wide_raw_string\"",
            "u\"raw_utf16_string\"",
            "U\"raw_utf32_string\"",
            "\"def_test\"",
            "\"concatenated_string\"",
            r#""multiline\nstring""#,
            r#""'\"\n\t\a\b|\220|\220|\351\246\231|\351\246\231|\360\237\230\202""#,
            "\"preprocessor_string_literal\"",
            r#"L"%s\n""#,
            "L\"preprocessor_string_literal\"",
        ];

        // Check extracted string literals
        assert!(potential_leaks.iter().enumerate().all(|(i, leak)| {
            println!("{:?}", leak.leaked_information);
            *leak.leaked_information == expected_string_literals[i]
        }));
        assert_eq!(expected_string_literals.len(), potential_leaks.len());
    }

    #[cfg(windows)]
    #[test]
    #[serial]
    fn find_leaks_in_binary_file_exe() {
        // Gather potential leaks
        let root_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FILE_LIST_PROJ_PATH);
        let file_list_db = FileListDatabase::new(
            &[root_dir_path.join("main.cc")],
            vec![
                "-DDEF_TEST".to_string(),
                format!("-I{}", FILE_LIST_PROJ_PATH),
            ],
        );
        let potential_leaks = extract_artifacts_from_source_files(
            file_list_db
                .get_all_compile_commands()
                .expect("get_all_compile_commands failed"),
            file_list_db.is_file_path_in_arguments(),
            true,
            0,
        )
        .expect("extract_artifacts_from_source_files failed");

        // Look for leaks present in the compiled binary
        let bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(FILE_LIST_PROJ_PATH)
            .join("a.exe");

        let confirmed_leaks = find_leaks_in_binary_file(&bin_path, potential_leaks)
            .expect("find_leaks_in_binary_file failed");

        let expected_string_literals = vec![
            // main.cc
            "\"included_string_literal\"",
            "\"preprocessor_string_literal\"",
            "\"%s\\n\"",
            "L\"preprocessor_string_literal\"",
            r#"L"%s\n""#,
            "\"%s\\n\"",
        ];

        // Check extracted string literals
        assert!(confirmed_leaks.iter().enumerate().all(|(i, leak)| {
            println!("{:?}", leak.leaked_information);
            *leak.leaked_information == expected_string_literals[i]
        }));
        assert_eq!(confirmed_leaks.len(), expected_string_literals.len());
    }

    #[cfg(unix)]
    #[test]
    #[serial]
    fn find_leaks_in_binary_file_elf() {
        // Gather potential leaks
        let root_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FILE_LIST_PROJ_PATH);
        let file_list_db = FileListDatabase::new(
            &[root_dir_path.join("main.cc")],
            vec!["-DDEF_TEST".to_string()],
        );
        let potential_leaks = extract_artifacts_from_source_files(
            file_list_db
                .get_all_compile_commands()
                .expect("get_all_compile_commands failed"),
            file_list_db.is_file_path_in_arguments(),
            true,
            0,
        )
        .expect("extract_artifacts_from_source_files failed");

        // Look for leaks present in the compiled binary
        let bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(FILE_LIST_PROJ_PATH)
            .join("a.out");

        let confirmed_leaks = find_leaks_in_binary_file(&bin_path, potential_leaks)
            .expect("find_leaks_in_binary_file failed");

        let expected_string_literals = vec![
            // main.cc
            "\"included_string_literal\"",
            "\"preprocessor_string_literal\"",
            r#"L"%s\n""#,
            "L\"preprocessor_string_literal\"",
        ];

        // Check extracted string literals
        assert!(confirmed_leaks.iter().enumerate().all(|(i, leak)| {
            println!("{:?}", leak.leaked_information);
            *leak.leaked_information == expected_string_literals[i]
        }));
        assert_eq!(confirmed_leaks.len(), expected_string_literals.len());
    }
}
