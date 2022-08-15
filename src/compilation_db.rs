use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct CommandObject {
    pub directory: PathBuf,
    pub file: PathBuf,
    pub arguments: Option<Vec<String>>,
    pub command: Option<String>,
    pub output: Option<String>,
}

pub fn get_file_paths_from_compile_database(db_file_path: &Path) -> Result<Vec<PathBuf>> {
    let command_objects = parse_compile_database(db_file_path)?;

    // Transform command objects to file paths
    let file_paths = command_objects
        .iter()
        .cloned()
        .map(|obj| {
            if obj.file.is_absolute() {
                obj.file
            } else {
                obj.directory.join(obj.file)
            }
        })
        .collect();

    Ok(file_paths)
}

fn parse_compile_database(db_file_path: &Path) -> Result<Vec<CommandObject>> {
    let mut db_file = File::open(db_file_path)?;

    let mut db_data = vec![];
    db_file.read_to_end(&mut db_data)?;

    Ok(serde_json::from_slice(&db_data)?)
}
