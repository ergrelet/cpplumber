use std::path::PathBuf;
use std::{collections::BTreeSet, sync::Arc};

use anyhow::Result;
use rayon::prelude::*;

use super::{CompilationDatabase, CompileCommand, CompileCommands};

pub struct FileListDatabase {
    /// Set of file paths
    file_paths: BTreeSet<PathBuf>,
    /// Shared arguments for all files
    arguments: Arc<Vec<String>>,
}

impl FileListDatabase {
    pub fn new(file_paths: &[PathBuf], arguments: Vec<String>) -> Self {
        Self {
            file_paths: BTreeSet::from_iter(file_paths.iter().cloned()),
            arguments: Arc::new(arguments),
        }
    }
}

impl CompilationDatabase for FileListDatabase {
    fn is_file_path_in_arguments(&self) -> bool {
        false
    }

    fn get_all_compile_commands(&self) -> Result<CompileCommands> {
        self.file_paths
            .par_iter()
            .map(|file_path| {
                Ok(CompileCommand {
                    filename: file_path.canonicalize()?,
                    arguments: self.arguments.clone(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE_LIST_PROJ_PATH: &str = "tests/data/main/file_list_proj";

    #[test]
    fn is_file_path_in_arguments() {
        let database = FileListDatabase::new(&[], vec![]);

        // Not present in arguments
        assert!(!database.is_file_path_in_arguments());
    }

    #[test]
    fn get_all_compile_commands_empty() {
        let database = FileListDatabase::new(&[], vec![]);
        // Result is empty
        assert!(database
            .get_all_compile_commands()
            .expect("get_all_compile_commands failed")
            .is_empty());
    }

    #[test]
    fn get_all_compile_commands() {
        let root_dir_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FILE_LIST_PROJ_PATH);
        let arguments = vec!["arg1".to_string(), "arg2".to_string()];
        let database = FileListDatabase::new(
            &[
                root_dir_path.join("main.cc"),
                root_dir_path.join("header.h"),
            ],
            arguments.clone(),
        );

        let compile_commands = database
            .get_all_compile_commands()
            .expect("get_all_compile_commands failed");
        // Result is not empty
        assert_eq!(compile_commands.len(), 2);

        // File #1
        // Check `filename` value
        assert_eq!(
            compile_commands[0].filename,
            root_dir_path.join("header.h").canonicalize().unwrap()
        );
        // Check `arguments` value
        assert_eq!(*compile_commands[0].arguments, arguments);

        // File #2
        // Check `filename` value
        assert_eq!(
            compile_commands[1].filename,
            root_dir_path.join("main.cc").canonicalize().unwrap()
        );
        // Check `arguments` value
        assert_eq!(*compile_commands[1].arguments, arguments);
    }
}
