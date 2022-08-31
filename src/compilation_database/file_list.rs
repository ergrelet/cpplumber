use std::collections::BTreeSet;
use std::path::PathBuf;

use std::path::Path;
use std::rc::Rc;

use anyhow::{anyhow, Result};

use super::{CompilationDatabase, CompileCommand, CompileCommands};

pub struct FileListDatabase {
    /// Set of file paths
    file_paths: BTreeSet<PathBuf>,
    /// Shared arguments for all files
    arguments: Rc<Vec<String>>,
}

impl FileListDatabase {
    pub fn new(file_paths: &[PathBuf], arguments: Vec<String>) -> Self {
        Self {
            file_paths: BTreeSet::from_iter(file_paths.iter().cloned()),
            arguments: Rc::new(arguments),
        }
    }
}

impl CompilationDatabase for FileListDatabase {
    fn get_compile_commands(&self, file_path: &Path) -> Result<CompileCommands> {
        if self.file_paths.contains(file_path) {
            Ok(vec![CompileCommand {
                directory: file_path.parent().unwrap().to_owned(),
                filename: file_path.file_name().unwrap().into(),
                arguments: self.arguments.clone(),
            }])
        } else {
            Err(anyhow!("File not found in the database"))
        }
    }

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
