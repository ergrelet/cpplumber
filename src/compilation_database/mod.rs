mod compile_commands;
mod file_list;

use std::{path::PathBuf, rc::Rc};

pub use compile_commands::CompileCommandsDatabase;
pub use file_list::FileListDatabase;

#[derive(Debug)]
pub struct CompileCommand {
    pub directory: PathBuf,
    pub filename: PathBuf,
    pub arguments: Rc<Vec<String>>,
}

pub type CompileCommands = Vec<CompileCommand>;

pub trait CompilationDatabase {
    /// Return all the compile commands stored in the database
    fn get_all_compile_commands(&self) -> CompileCommands;
}
