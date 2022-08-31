mod compile_commands;
mod file_list;

use anyhow::Result;
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

pub use compile_commands::CompileCommandsDatabase;
pub use file_list::FileListDatabase;

pub struct CompileCommand {
    pub directory: PathBuf,
    pub filename: PathBuf,
    pub arguments: Rc<Vec<String>>,
}

pub type CompileCommands = Vec<CompileCommand>;

pub trait CompilationDatabase {
    /// Find the compile commands for the given file.
    fn get_compile_commands(&self, file_path: &Path) -> Result<CompileCommands>;
    /// Return all the compile commands stored in the database
    fn get_all_compile_commands(&self) -> CompileCommands;
}
