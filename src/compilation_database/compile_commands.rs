use std::path::Path;
use std::{fs, rc::Rc};

use anyhow::{anyhow, Result};
use tempfile::TempDir;

use super::{CompilationDatabase, CompileCommand, CompileCommands};

pub struct CompileCommandsDatabase {
    clang_db: clang::CompilationDatabase,
}

impl CompileCommandsDatabase {
    pub fn new<P: AsRef<Path>>(db_file_path: P) -> Result<Self> {
        let fake_build_directory = move_database_file_into_tmp_dir(db_file_path)?;
        let clang_db = clang::CompilationDatabase::from_directory(fake_build_directory.path())
            .map_err(|_| anyhow!("Failed to parse compilation database"))?;

        Ok(Self { clang_db })
    }
}

impl CompilationDatabase for CompileCommandsDatabase {
    fn get_compile_commands(&self, file_path: &Path) -> Result<CompileCommands> {
        let clang_cmds = self.clang_db.get_compile_commands(file_path).map_err(|_| {
            anyhow!(format!(
                "Failed to get compile commands for {}",
                file_path.display()
            ))
        })?;

        Ok(convert_clang_compile_commands(clang_cmds))
    }

    fn get_all_compile_commands(&self) -> CompileCommands {
        let clang_cmds = self.clang_db.get_all_compile_commands();

        convert_clang_compile_commands(clang_cmds)
    }
}

/// Converts `clang`'s CompileCommands to our own `CompileCommands` type
fn convert_clang_compile_commands(clang_cmds: clang::CompileCommands) -> CompileCommands {
    clang_cmds
        .get_commands()
        .iter()
        .map(|cmd| {
            // Note: For some reason, having the file path in `arguments` when
            // passing the file path explicitly to libclang make the parser fail.
            // So we explicitely pop the last argument (which is the file path).
            let mut arguments = cmd.get_arguments();
            // Should contain at least compiler path and file path
            if arguments.len() > 1 {
                arguments.pop();
            }

            CompileCommand {
                directory: cmd.get_directory(),
                filename: cmd.get_filename(),
                arguments: Rc::new(arguments),
            }
        })
        .collect()
}

/// Move the database file with the name clang expects, into a temporary directory
fn move_database_file_into_tmp_dir<P: AsRef<Path>>(db_file_path: P) -> Result<TempDir> {
    let tmp_directory = tempfile::tempdir()?;
    let dest_path = tmp_directory.path().join("compile_commands.json");
    _ = fs::copy(db_file_path, dest_path)?;

    Ok(tmp_directory)
}
