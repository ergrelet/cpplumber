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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const INVALID_DATABASE_PATH: &str = "tests/data/compile_commands/invalid.json";
    const EMPTY_DATABASE_PATH: &str = "tests/data/compile_commands/empty.json";
    const DATABASE1_PATH: &str = "tests/data/compile_commands/db1.json";

    #[test]
    fn get_all_compile_commands_invalid() {
        let empty_db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(INVALID_DATABASE_PATH);
        assert!(CompileCommandsDatabase::new(empty_db_path).is_err());
    }

    #[test]
    #[should_panic]
    fn get_all_compile_commands_empty() {
        let empty_db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(EMPTY_DATABASE_PATH);
        // Unfortunately, the `clang` crate panics in `from_directory`
        // for empty databases.
        assert!(CompileCommandsDatabase::new(empty_db_path).is_err());
    }

    #[test]
    fn get_all_compile_commands() {
        let db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DATABASE1_PATH);
        let database = CompileCommandsDatabase::new(db_path).expect("Failed to parse database");

        let compile_commands = database.get_all_compile_commands();
        // Result is not empty
        assert_eq!(compile_commands.len(), 2);

        const DIRECTORY: &str = "C:\\Users\\user\\Documents\\cpplumber";

        // File #1
        // Check `directory` value
        assert_eq!(compile_commands[0].directory.to_str().unwrap(), DIRECTORY);
        // Check `filename` value
        assert_eq!(compile_commands[0].filename.to_str().unwrap(), "file.cc");
        // Check `arguments` value
        assert_eq!(
            *compile_commands[0].arguments,
            vec![
                "/usr/bin/clang++".to_string(),
                "--driver-mode=g++".to_string(),
                "-Irelative".to_string(),
                "-DSOMEDEF=With spaces, quotes.".to_string(),
                "-c".to_string(),
                "-o".to_string(),
                "file.o".to_string(),
            ]
        );

        // File #2
        // Check `directory` value
        assert_eq!(compile_commands[1].directory.to_str().unwrap(), DIRECTORY);
        // Check `filename` value
        assert_eq!(compile_commands[1].filename.to_str().unwrap(), "file2.cc");
        // Check `arguments` value
        assert_eq!(
            *compile_commands[1].arguments,
            vec![
                "/usr/bin/clang++".to_string(),
                "--driver-mode=g++".to_string(),
                "-Irelative".to_string(),
                "-DSOMEDEF=With spaces, quotes.".to_string(),
                "-c".to_string(),
                "-o".to_string(),
                "file2.o".to_string(),
            ]
        );
    }
}
