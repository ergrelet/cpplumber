use std::{borrow::Cow, collections::BTreeSet, hash::Hash, path::PathBuf, sync::Arc};

use anyhow::{anyhow, Result};
use clang::{Entity, EntityKind};
use serde::Serialize;
use widestring::{encode_utf16, encode_utf32};

const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const REPORT_FORMAT_VERSION: u32 = 1;

/// Enum that the kind of wide chars to use
pub enum WideCharMode {
    /// Wide strings are encoded as UTF-16LE
    Windows,
    /// Wide strings are encoded as UTF-32LE
    Unix,
}

#[derive(Debug)]
pub struct PotentialLeak {
    /// Leaked information, as represented in the source code
    pub leaked_information: Arc<String>,
    /// Byte pattern to match (i.e., leaked information, as represented in the
    /// binary file)
    pub bytes: Vec<u8>,
    /// Data on where the leaked information is declared in the
    /// source code
    pub declaration_metadata: Arc<SourceLocation>,
}

impl TryFrom<Entity<'_>> for PotentialLeak {
    type Error = anyhow::Error;

    fn try_from(entity: Entity) -> Result<Self, Self::Error> {
        match entity.get_kind() {
            EntityKind::StringLiteral => {
                let leaked_information = entity
                    .get_display_name()
                    .ok_or_else(|| anyhow!("Failed to get entity's display name"))?;
                let location = entity
                    .get_location()
                    .ok_or_else(|| anyhow!("Failed to get entity's location"))?
                    .get_file_location();
                let file_location = location
                    .file
                    .ok_or_else(|| anyhow!("Failed to get entity's file location"))?
                    .get_path();
                let line_location = location.line;

                Ok(Self {
                    bytes: string_literal_to_bytes(&leaked_information, None)?,
                    leaked_information: Arc::new(leaked_information),
                    declaration_metadata: Arc::new(SourceLocation {
                        file: file_location.canonicalize()?,
                        line: line_location as u64,
                    }),
                })
            }
            _ => Err(anyhow!("Unsupported entity kind")),
        }
    }
}

impl PartialEq for PotentialLeak {
    fn eq(&self, other: &Self) -> bool {
        self.leaked_information == other.leaked_information
    }
}

impl Eq for PotentialLeak {}

impl Hash for PotentialLeak {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.leaked_information.hash(state);
    }
}

#[derive(Serialize)]
pub struct ConfirmedLeak {
    pub leaked_information: Arc<String>,
    pub location: LeakLocation,
}

impl PartialEq for ConfirmedLeak {
    fn eq(&self, other: &Self) -> bool {
        self.location == other.location
    }
}

impl Eq for ConfirmedLeak {}

impl PartialOrd for ConfirmedLeak {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.location.partial_cmp(&other.location)
    }
}

impl Ord for ConfirmedLeak {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.location.cmp(&other.location)
    }
}

impl Hash for ConfirmedLeak {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.location.hash(state);
    }
}

#[derive(Serialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LeakLocation {
    pub source: Arc<SourceLocation>,
    pub binary: BinaryLocation,
}

#[derive(Debug, Serialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceLocation {
    pub file: PathBuf,
    pub line: u64,
}

#[derive(Serialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BinaryLocation {
    pub file: Arc<PathBuf>,
    pub offset: u64,
}

#[derive(Serialize)]
struct JsonReport {
    version: ReportVersion,
    leaks: BTreeSet<ConfirmedLeak>,
}

#[derive(Serialize)]
struct ReportVersion {
    executable: String,
    format: u32,
}

/// We have to reimplement this ourselves since the `clang` crate doesn't
/// provide an easy way to get byte representations of `StringLiteral` entities.
fn string_literal_to_bytes(
    string_literal: &str,
    wide_char_mode: Option<WideCharMode>,
) -> Result<Vec<u8>> {
    let wide_char_mode = wide_char_mode.unwrap_or({
        // Pick the sensible default if not specified
        if cfg!(windows) {
            WideCharMode::Windows
        } else {
            WideCharMode::Unix
        }
    });

    let mut char_it = string_literal.chars();
    let first_char = char_it.next();
    match first_char {
        None => Ok(vec![]),
        Some(first_char) => match first_char {
            // Ordinary string (we assume it'll be encoded to ASCII)
            '"' => Ok(
                process_escape_sequences(&string_literal[1..string_literal.len() - 1])
                    .ok_or_else(|| anyhow!("Failed to process escape sequences"))?
                    .as_bytes()
                    .to_owned(),
            ),
            // Wide string
            'L' => {
                match wide_char_mode {
                    WideCharMode::Windows => {
                        // Encode as UTF-16LE on Windows
                        Ok(encode_utf16(
                            process_escape_sequences(&string_literal[2..string_literal.len() - 1])
                                .ok_or_else(|| anyhow!("Failed to process escape sequences"))?
                                .chars(),
                        )
                        .map(u16::to_le_bytes)
                        .fold(Vec::new(), |mut acc: Vec<u8>, e| {
                            acc.extend(e);
                            acc
                        }))
                    }
                    WideCharMode::Unix => {
                        // Encode as UTF-32LE on Unix platforms
                        Ok(encode_utf32(
                            process_escape_sequences(&string_literal[2..string_literal.len() - 1])
                                .ok_or_else(|| anyhow!("Failed to process escape sequences"))?
                                .chars(),
                        )
                        .map(u32::to_le_bytes)
                        .fold(Vec::new(), |mut acc: Vec<u8>, e| {
                            acc.extend(e);
                            acc
                        }))
                    }
                }
            }
            // UTF-32 string
            'U' => Ok(encode_utf32(
                process_escape_sequences(&string_literal[2..string_literal.len() - 1])
                    .ok_or_else(|| anyhow!("Failed to process escape sequences"))?
                    .chars(),
            )
            .map(u32::to_le_bytes)
            .fold(Vec::new(), |mut acc: Vec<u8>, e| {
                acc.extend(e);
                acc
            })),
            // UTF-8 or UTF-16LE string
            'u' => {
                let second_char = char_it
                    .next()
                    .ok_or_else(|| anyhow!("Invalid string literal"))?;
                let third_char = char_it
                    .next()
                    .ok_or_else(|| anyhow!("Invalid string literal"))?;
                if second_char == '8' && third_char == '"' {
                    // UTF-8
                    Ok(
                        process_escape_sequences(&string_literal[3..string_literal.len() - 1])
                            .ok_or_else(|| anyhow!("Failed to process escape sequences"))?
                            .as_bytes()
                            .to_owned(),
                    )
                } else {
                    // UTF-16LE
                    Ok(encode_utf16(
                        process_escape_sequences(&string_literal[2..string_literal.len() - 1])
                            .ok_or_else(|| anyhow!("Failed to process escape sequences"))?
                            .chars(),
                    )
                    .map(u16::to_le_bytes)
                    .fold(Vec::new(), |mut acc: Vec<u8>, e| {
                        acc.extend(e);
                        acc
                    }))
                }
            }
            _ => Err(anyhow!(
                "Invalid string literal or a new string literal prefix introduced in the standard."
            )),
        },
    }
}

fn process_escape_sequences(string: &str) -> Option<Cow<str>> {
    let mut owned: Option<String> = None;
    let mut skip_until: usize = 0;
    for (position, char) in string.chars().enumerate() {
        if position < skip_until {
            continue;
        }

        if char == '\\' {
            if owned.is_none() {
                owned = Some(string[..position].to_owned());
            }
            let b = owned.as_mut()?;
            let mut escape_char_it = string.chars();
            let first_char = escape_char_it.nth(position + 1);
            if let Some(first_char) = first_char {
                skip_until = position + 2;
                match first_char {
                    // Simple escape sequences
                    'a' => b.push('\x07'),
                    'b' => b.push('\x08'),
                    't' => b.push('\t'),
                    'n' => b.push('\n'),
                    'v' => b.push('\x0b'),
                    'f' => b.push('\x0c'),
                    'r' => b.push('\r'),
                    '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' => {
                        let start_position = position + 1;
                        let mut end_position = start_position + 1;
                        // Check following char
                        if let Some(second_char) = escape_char_it.next() {
                            if second_char.is_digit(8) {
                                end_position += 1;
                                // Check the next char
                                if let Some(third_char) = escape_char_it.next() {
                                    if third_char.is_digit(8) {
                                        end_position += 1;
                                    }
                                }
                            }
                        }

                        // Octal escape sequence (\nnn)
                        let octal_value =
                            u8::from_str_radix(&string[start_position..end_position], 8).ok()?;
                        b.push(octal_value as char);
                        skip_until = end_position;
                    }
                    a => b.push(a),
                }
            } else {
                return None;
            }
        } else if let Some(o) = owned.as_mut() {
            o.push(char);
        }
    }

    if let Some(owned) = owned {
        Some(Cow::Owned(owned))
    } else {
        Some(Cow::Borrowed(string))
    }
}

pub fn print_confirmed_leaks(confirmed_leaks: BTreeSet<ConfirmedLeak>, json: bool) -> Result<()> {
    if json {
        let report = JsonReport {
            version: ReportVersion {
                executable: PKG_VERSION.into(),
                format: REPORT_FORMAT_VERSION,
            },
            leaks: confirmed_leaks,
        };
        serde_json::to_writer(std::io::stdout(), &report)?;
    } else {
        for leak in confirmed_leaks {
            println!(
                "{} leaked at offset 0x{:x} in \"{}\" [declared at {}:{}]",
                leak.leaked_information,
                leak.location.binary.offset,
                leak.location.binary.file.display(),
                leak.location.source.file.display(),
                leak.location.source.line,
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_literal_to_bytes_empty_string() {
        assert!(string_literal_to_bytes("", None)
            .expect("string_literal_to_bytes failed")
            .is_empty());
    }

    #[test]
    fn string_literal_to_bytes_not_a_literal() {
        assert!(string_literal_to_bytes("not a literal", None).is_err());
    }

    #[test]
    fn string_literal_to_bytes_ascii_string_literal() {
        assert_eq!(
            string_literal_to_bytes("\"hello\"", None).expect("string_literal_to_bytes failed"),
            b"hello"
        );
    }

    #[test]
    fn string_literal_to_bytes_wide_string_literal_default() {
        // On Windows, wide chars are encoded as UTF-16LE
        #[cfg(windows)]
        assert_eq!(
            string_literal_to_bytes("L\"hello\"", None).expect("string_literal_to_bytes failed"),
            b"h\0e\0l\0l\0o\0"
        );

        // On Unix-like platforms, wide chars are encoded as UTF-32LE
        #[cfg(unix)]
        assert_eq!(
            string_literal_to_bytes("L\"hello\"", None).expect("string_literal_to_bytes failed"),
            b"h\0\0\0e\0\0\0l\0\0\0l\0\0\0o\0\0\0"
        );
    }

    #[test]
    fn string_literal_to_bytes_wide_string_literal_override() {
        // On Windows, wide chars are encoded as UTF-16LE
        assert_eq!(
            string_literal_to_bytes("L\"hello\"", Some(WideCharMode::Windows))
                .expect("string_literal_to_bytes failed"),
            b"h\0e\0l\0l\0o\0"
        );

        // On Unix-like platforms, wide chars are encoded as UTF-32LE
        assert_eq!(
            string_literal_to_bytes("L\"hello\"", Some(WideCharMode::Unix))
                .expect("string_literal_to_bytes failed"),
            b"h\0\0\0e\0\0\0l\0\0\0l\0\0\0o\0\0\0"
        );
    }

    #[test]
    fn string_literal_to_bytes_utf8_string_literal() {
        assert_eq!(
            string_literal_to_bytes("u8\"hello\"", None).expect("string_literal_to_bytes failed"),
            b"hello"
        );
    }

    #[test]
    fn string_literal_to_bytes_utf16_string_literal() {
        assert_eq!(
            string_literal_to_bytes("u\"hello\"", None).expect("string_literal_to_bytes failed"),
            b"h\0e\0l\0l\0o\0"
        );
    }

    #[test]
    fn string_literal_to_bytes_utf32_string_literal() {
        assert_eq!(
            string_literal_to_bytes("U\"hello\"", None).expect("string_literal_to_bytes failed"),
            b"h\0\0\0e\0\0\0l\0\0\0l\0\0\0o\0\0\0"
        );
    }

    #[test]
    fn process_escape_sequences_no_escape_sequence() {
        assert_eq!(
            process_escape_sequences("hello world!").expect("Failed to escape string"),
            "hello world!"
        );
    }

    #[test]
    fn process_escape_sequences_invalid_escape_sequence() {
        assert!(process_escape_sequences(r"invalid\").is_none());
    }

    #[test]
    fn process_escape_sequences_char_escape_sequences() {
        assert_eq!(
            process_escape_sequences(r"\a\b\t\n\v\f\r\ \\").expect("Failed to escape string"),
            "\x07\x08\t\n\x0B\x0C\r \\"
        );
    }

    #[test]
    fn process_escape_sequences_octal_escape_sequences() {
        assert_eq!(
            process_escape_sequences(r"\0\1\2\3\4\5\6\7\10\100").expect("Failed to escape string"),
            "\x00\x01\x02\x03\x04\x05\x06\x07\x08\x40"
        );
    }
}
