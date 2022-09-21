mod compile_commands;
mod file_list;

use glob::glob;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use rayon::prelude::*;

pub use compile_commands::CompileCommandsDatabase;
pub use file_list::FileListDatabase;

pub enum ProjectConfiguration<'p> {
    CompilationDatabase {
        project_file_path: &'p Path,
    },
    Manual {
        source_path_globs: &'p [String],
        include_directories: &'p [String],
        compile_definitions: &'p [String],
    },
}

#[derive(Debug)]
pub struct CompileCommand {
    pub filename: PathBuf,
    pub arguments: Arc<Vec<String>>,
}

pub type CompileCommands = Vec<CompileCommand>;

pub trait CompilationDatabase {
    /// Indicates if the file path can be found in the argument list.
    fn is_file_path_in_arguments(&self) -> bool;
    /// Returns all the compile commands stored in the database
    fn get_all_compile_commands(&self) -> Result<CompileCommands>;
}

pub fn generate_compilation_database(
    project_config: ProjectConfiguration,
) -> Result<Box<dyn CompilationDatabase>> {
    match project_config {
        ProjectConfiguration::CompilationDatabase { project_file_path } => {
            // Parse compile commands from the JSON database
            Ok(Box::new(CompileCommandsDatabase::new(project_file_path)?))
        }

        ProjectConfiguration::Manual {
            source_path_globs,
            include_directories,
            compile_definitions,
        } => {
            // Otherwise, process glob expressions
            let file_paths = source_path_globs
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
            for include_dir in include_directories.iter() {
                arguments.push(format!("-I{}", include_dir));
            }
            // Add preprocessor defitions to the list of arguments
            for compile_def in compile_definitions.iter() {
                arguments.push(format!("-D{}", compile_def));
            }

            log::debug!("Using arguments: {:?}", arguments);
            Ok(Box::new(FileListDatabase::new(&file_paths, arguments)))
        }
    }
}
