use std::path::PathBuf;
use std::{collections::BTreeSet, sync::Arc};

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
    fn get_all_compile_commands(&self) -> CompileCommands {
        self.file_paths
            .iter()
            .map(|file_path| CompileCommand {
                directory: file_path.parent().unwrap().to_owned(),
                filename: file_path.file_name().unwrap().into(),
                arguments: self.arguments.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn get_all_compile_commands_empty() {
        let database = FileListDatabase::new(&[], vec![]);
        // Result is empty
        assert!(database.get_all_compile_commands().is_empty());
    }

    #[test]
    fn get_all_compile_commands() {
        let arguments = vec!["arg1".to_string(), "arg2".to_string()];
        let database = FileListDatabase::new(
            &[
                PathBuf::from_str("file1.cc").unwrap(),
                PathBuf::from_str("file2.c").unwrap(),
            ],
            arguments.clone(),
        );

        let compile_commands = database.get_all_compile_commands();
        // Result is not empty
        assert_eq!(compile_commands.len(), 2);

        // File #1
        // Check `directory` value
        assert_eq!(compile_commands[0].directory.to_str().unwrap(), "");
        // Check `filename` value
        assert_eq!(compile_commands[0].filename.to_str().unwrap(), "file1.cc");
        // Check `arguments` value
        assert_eq!(*compile_commands[0].arguments, arguments);

        // File #2
        // Check `directory` value
        assert_eq!(compile_commands[1].directory.to_str().unwrap(), "");
        // Check `filename` value
        assert_eq!(compile_commands[1].filename.to_str().unwrap(), "file2.c");
        // Check `arguments` value
        assert_eq!(*compile_commands[1].arguments, arguments);
    }
}
