mod compilation_db;
mod information_leak;
mod suppressions;

use std::{
    collections::HashSet,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    vec,
};

use anyhow::{anyhow, Context, Result};
use clang::{Clang, Entity, EntityKind, Index};
use glob::glob;
use structopt::StructOpt;
use suppressions::Suppressions;

use crate::{
    compilation_db::get_file_paths_from_compile_database,
    information_leak::InformationLeakDescription, suppressions::parse_suppressions_file,
};

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Debug, StructOpt)]
#[structopt(name = PKG_NAME, about = "An information leak detector for C and C++ code bases")]
struct CpplumberOptions {
    /// Path to the output binary to scan for leaked information
    #[structopt(parse(from_os_str), short, long = "bin")]
    binary_file_path: PathBuf,

    /// Compilation database
    #[structopt(parse(from_os_str), short, long = "project")]
    project_file_path: Option<PathBuf>,

    /// Path to a file containing rules to prevent certain errors from being
    /// generated.
    #[structopt(parse(from_os_str), short, long)]
    suppressions_list: Option<PathBuf>,

    /// Only report leaks once for artifacts used in multiple locations
    #[structopt(long)]
    ignore_multiple_declarations: bool,

    /// List of source files to scan for (can be glob expressions)
    source_path_globs: Vec<String>,
}

fn main() -> Result<()> {
    env_logger::init();
    let options = CpplumberOptions::from_args();

    // Initial checks before starting work
    if !options.binary_file_path.is_file() {
        return Err(anyhow!(
            "'{}' is not a valid file path.",
            options.binary_file_path.display()
        ));
    }

    // Parse the suppression list if used
    let suppressions = if let Some(suppressions_list) = options.suppressions_list {
        Some(
            parse_suppressions_file(&suppressions_list)
                .with_context(|| "Failed to parse suppressions list")?,
        )
    } else {
        None
    };

    // Parse project file if used
    let file_paths = if let Some(project_file_path) = options.project_file_path {
        get_file_paths_from_compile_database(&project_file_path)?
    } else {
        // Process glob expressions otherwise
        let mut file_paths = vec![];
        for glob_expressions in options.source_path_globs {
            if let Ok(paths) = glob(&glob_expressions) {
                for path in paths {
                    file_paths.push(path?);
                }
            } else {
                log::warn!(
                    "'{}' is not a valid path or glob expression, ignoring it",
                    glob_expressions
                );
            }
        }

        file_paths
    };

    // Filter suppressed files
    let file_paths: Vec<PathBuf> = filter_suppressed_files(file_paths, &suppressions);

    let clang = Clang::new().map_err(|e| anyhow!(e))?;
    let index = Index::new(&clang, false, false);

    // Parse source files and extract information that could leak
    let mut potential_leaks: Vec<InformationLeakDescription> = vec![];
    for path in file_paths {
        let translation_unit = index
            .parser(&path)
            .visit_implicit_attributes(false)
            .parse()
            .with_context(|| format!("Failed to parse source file '{}'", path.display()))?;

        let string_literals =
            gather_entities_by_kind(translation_unit.get_entity(), &[EntityKind::StringLiteral]);

        potential_leaks.extend(
            string_literals
                .into_iter()
                .filter_map(|literal| literal.try_into().ok()),
        );
    }

    // Filter suppressed artifacts if needed
    let potential_leaks = filter_suppressed_artifacts(potential_leaks, &suppressions);

    log::info!(
        "Looking for leaks in '{}'...",
        options.binary_file_path.display()
    );

    if options.ignore_multiple_declarations {
        // Remove duplicated artifacts if needed
        let potential_leaks: HashSet<InformationLeakDescription> =
            HashSet::from_iter(potential_leaks);
        log::debug!("{:#?}", potential_leaks);
        check_for_leaks_in_binary_file(&options.binary_file_path, &potential_leaks)?;
    } else {
        log::debug!("{:#?}", potential_leaks);
        check_for_leaks_in_binary_file(&options.binary_file_path, &potential_leaks)?;
    }

    Ok(())
}

fn gather_entities_by_kind<'tu>(
    root_entity: Entity<'tu>,
    entity_kind_filter: &[EntityKind],
) -> Vec<Entity<'tu>> {
    gather_entities_by_kind_rec(root_entity, entity_kind_filter, 0)
}

fn gather_entities_by_kind_rec<'tu>(
    root_entity: Entity<'tu>,
    entity_kind_filter: &[EntityKind],
    current_depth: usize,
) -> Vec<Entity<'tu>> {
    let mut entities = vec![];

    let root_entity_kind = root_entity.get_kind();
    if entity_kind_filter
        .iter()
        .any(|elem| elem == &root_entity_kind)
    {
        entities.push(root_entity);
    }

    for child in root_entity.get_children() {
        // We're only interested in declarations made in the main files
        if child.is_in_main_file() {
            let entities_sub =
                gather_entities_by_kind_rec(child, entity_kind_filter, current_depth + 1);
            entities.extend(entities_sub);
        }
    }

    entities
}

fn filter_suppressed_files(
    file_paths: Vec<PathBuf>,
    suppressions: &Option<Suppressions>,
) -> Vec<PathBuf> {
    if let Some(suppressions) = suppressions {
        file_paths
            .iter()
            .filter(|file_path| {
                if let Some(file_path) = file_path.to_str() {
                    !suppressions
                        .files
                        .iter()
                        .any(|pattern| pattern.matches(file_path))
                } else {
                    true
                }
            })
            .cloned()
            .collect()
    } else {
        file_paths
    }
}

fn filter_suppressed_artifacts(
    potential_leaks: Vec<InformationLeakDescription>,
    suppressions: &Option<Suppressions>,
) -> Vec<InformationLeakDescription> {
    if let Some(suppressions) = suppressions {
        potential_leaks
            .iter()
            .filter(|leak| !suppressions.artifacts.contains(&leak.leaked_information))
            .cloned()
            .collect()
    } else {
        potential_leaks
    }
}

fn check_for_leaks_in_binary_file<'l, LeakDescCollection>(
    binary_file_path: &Path,
    leak_desc: LeakDescCollection,
) -> Result<()>
where
    LeakDescCollection: IntoIterator<Item = &'l InformationLeakDescription>,
{
    let mut bin_file = File::open(binary_file_path)?;

    let mut bin_data = vec![];
    bin_file.read_to_end(&mut bin_data)?;

    for leak in leak_desc {
        if let Some(offset) = bin_data
            .windows(leak.bytes.len())
            .position(|window| window == leak.bytes)
        {
            println!(
                "[{}:{}]: {} is leaked (offset=0x{:x})",
                leak.declaration_metadata.0.display(),
                leak.declaration_metadata.1,
                leak.leaked_information,
                offset,
            );
        }
    }

    Ok(())
}
