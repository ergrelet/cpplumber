mod compile_commands;
mod file_list;

use std::{path::PathBuf, sync::Arc};

pub use compile_commands::CompileCommandsDatabase;
pub use file_list::FileListDatabase;

#[derive(Debug)]
pub struct CompileCommand {
    pub directory: PathBuf,
    pub filename: PathBuf,
    pub arguments: Arc<Vec<String>>,
}

pub type CompileCommands = Vec<CompileCommand>;

pub trait CompilationDatabase {
    /// Indicates if the file path can be found in the argument list.
    fn is_file_path_in_arguments(&self) -> bool;
    /// Returns all the compile commands stored in the database
    fn get_all_compile_commands(&self) -> CompileCommands;
}
