use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::Result;
use glob::Pattern;
use serde::Deserialize;

pub struct Suppressions {
    pub files: Vec<Pattern>,
    pub artifacts: Vec<String>,
}

#[derive(Deserialize)]
struct SuppressionsListYaml {
    files: Option<Vec<String>>,
    artifacts: Option<Vec<String>>,
}

pub fn parse_suppressions_file(suppression_file_path: &Path) -> Result<Suppressions> {
    // Read file
    let mut suppression_data = vec![];
    let mut suppression_file = File::open(suppression_file_path)?;
    suppression_file.read_to_end(&mut suppression_data)?;

    // Parse YAML content
    let suppressions_yaml: SuppressionsListYaml = serde_yaml::from_slice(&suppression_data)?;

    // Compile glob patterns
    let files = suppressions_yaml
        .files
        .unwrap_or_default()
        .iter()
        .map(|pattern| {
            if let Ok(pattern) = Pattern::new(pattern) {
                pattern
            } else {
                log::warn!("Failed to compile '{}', ignoring ...", &pattern);
                Pattern::default()
            }
        })
        .collect();

    Ok(Suppressions {
        files,
        artifacts: suppressions_yaml.artifacts.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const FILE1_PATH: &str = "tests/data/suppressions/files_and_artifacts.yml";

    #[test]
    fn parse_suppressions_file_files_and_artifacts() {
        let file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FILE1_PATH);
        let suppressions =
            parse_suppressions_file(&file_path).expect("Failed parsing suppressions file");

        // Files
        assert_eq!(suppressions.files.len(), 1);
        assert_eq!(
            suppressions.files[0],
            glob::Pattern::new("*\\file2.cc").unwrap()
        );

        // Artifacts
        assert_eq!(suppressions.artifacts.len(), 2);
        assert_eq!(suppressions.artifacts[0], "\"c_string\"");
        assert_eq!(suppressions.artifacts[1], "U\"utf32_string\"");
    }
}
