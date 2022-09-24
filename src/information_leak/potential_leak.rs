use std::{borrow::Cow, hash::Hash, sync::Arc};

use anyhow::{anyhow, Result};
use clang::{Entity, EntityKind};
use widestring::{encode_utf16, encode_utf32};

use super::{LeakedDataType, SourceLocation};

/// Struct containing information on a piece of data from the source code, which
/// may leak into a binary file.
#[derive(Debug)]
pub struct PotentialLeak {
    /// Type of data leaked
    pub data_type: LeakedDataType,
    /// Leaked data, as represented in the source code
    pub data: Arc<String>,
    /// Byte pattern to match (i.e., leaked information, as represented in the
    /// binary file)
    pub bytes: Vec<u8>,
    /// Information on where the leaked data is declared in the source code
    pub declaration_metadata: Arc<SourceLocation>,
}

impl TryFrom<Entity<'_>> for PotentialLeak {
    type Error = anyhow::Error;

    fn try_from(entity: Entity) -> Result<Self, Self::Error> {
        let location = entity
            .get_location()
            .ok_or_else(|| anyhow!("Failed to get entity's location"))?
            .get_file_location();
        let file_location = location
            .file
            .ok_or_else(|| anyhow!("Failed to get entity's file location"))?
            .get_path();

        match entity.get_kind() {
            EntityKind::StringLiteral => {
                let leaked_information = entity
                    .get_display_name()
                    .ok_or_else(|| anyhow!("Failed to get entity's display name"))?;
                let (_, string_content) = parse_string_literal(&leaked_information)?;

                Ok(Self {
                    data_type: LeakedDataType::StringLiteral,
                    data: Arc::new(string_content.to_owned()),
                    bytes: string_literal_to_bytes(&leaked_information, None)?,
                    declaration_metadata: Arc::new(SourceLocation {
                        file: file_location.canonicalize()?,
                        line: location.line as u64,
                    }),
                })
            }
            entity_kind @ (EntityKind::StructDecl | EntityKind::ClassDecl) => {
                // Convert `EntityKind` to `LeakedDataType`
                let data_type = match entity_kind {
                    EntityKind::StructDecl => LeakedDataType::StructName,
                    EntityKind::ClassDecl => LeakedDataType::ClassName,
                    _ => unreachable!("This entity kind should not be matched"),
                };
                let leaked_information = entity.get_display_name().unwrap_or_default();

                Ok(Self {
                    data_type,
                    bytes: leaked_information.as_bytes().to_vec(),
                    data: Arc::new(leaked_information),
                    declaration_metadata: Arc::new(SourceLocation {
                        file: file_location.canonicalize()?,
                        line: location.line as u64,
                    }),
                })
            }
            _ => Err(anyhow!("Unsupported entity kind")),
        }
    }
}

impl PartialEq for PotentialLeak {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl Eq for PotentialLeak {}

impl Hash for PotentialLeak {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.data.hash(state);
    }
}

/// Kind of wide chars to use when encoding wide strings
pub enum WideCharMode {
    /// Wide strings are encoded as UTF-16LE
    Windows,
    /// Wide strings are encoded as UTF-32LE
    Unix,
}

/// Describes the string encoding specified for a string literal
enum StringLiteralEncoding {
    /// No encoding specified (i.e., typical "*" string)
    Unspecified,
    /// Wide string encoding specified (i.e., L"*" string)
    Wide,
    /// UTF-8 encoding (i.e., u8"*" string)
    Utf8,
    /// UTF-16LE encoding (i.e., u"*" string)
    Utf16,
    /// UTF-32LE encoding (i.e., U"*" string)
    Utf32,
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

    let (string_encoding, string_content) = parse_string_literal(string_literal)?;
    match string_encoding {
        // Unspecified (ASCII assumed)
        StringLiteralEncoding::Unspecified => Ok(process_escape_sequences(string_content)
            .ok_or_else(|| anyhow!("Failed to process escape sequences"))?
            .as_bytes()
            .to_owned()),

        // Wide
        StringLiteralEncoding::Wide => {
            match wide_char_mode {
                WideCharMode::Windows => {
                    // Encode as UTF-16LE on Windows
                    Ok(encode_utf16(
                        process_escape_sequences(string_content)
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
                        process_escape_sequences(string_content)
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

        // UTF-8
        StringLiteralEncoding::Utf8 => Ok(process_escape_sequences(string_content)
            .ok_or_else(|| anyhow!("Failed to process escape sequences"))?
            .as_bytes()
            .to_owned()),

        // UTF-16LE
        StringLiteralEncoding::Utf16 => Ok(encode_utf16(
            process_escape_sequences(string_content)
                .ok_or_else(|| anyhow!("Failed to process escape sequences"))?
                .chars(),
        )
        .map(u16::to_le_bytes)
        .fold(Vec::new(), |mut acc: Vec<u8>, e| {
            acc.extend(e);
            acc
        })),

        // UTF-32LE
        StringLiteralEncoding::Utf32 => Ok(encode_utf32(
            process_escape_sequences(string_content)
                .ok_or_else(|| anyhow!("Failed to process escape sequences"))?
                .chars(),
        )
        .map(u32::to_le_bytes)
        .fold(Vec::new(), |mut acc: Vec<u8>, e| {
            acc.extend(e);
            acc
        })),
    }
}

/// Takes in a string literal (e.g., "str", L"str") and returns the specified
/// encoding (extracted from the prefix) and the actual content of the string.
fn parse_string_literal(string_literal: &str) -> Result<(StringLiteralEncoding, &str)> {
    let mut char_it = string_literal.chars();
    let first_char = char_it.next();
    match first_char {
        None => Err(anyhow!("Empty string literal")),
        Some(first_char) => match first_char {
            // Ordinary string (we assume it'll be encoded to ASCII)
            '"' => Ok((
                StringLiteralEncoding::Unspecified,
                &string_literal[1..string_literal.len() - 1],
            )),

            // Wide string
            'L' => Ok((
                StringLiteralEncoding::Wide,
                &string_literal[2..string_literal.len() - 1],
            )),

            // UTF-32LE string
            'U' => Ok((
                StringLiteralEncoding::Utf32,
                &string_literal[2..string_literal.len() - 1],
            )),

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
                    Ok((
                        StringLiteralEncoding::Utf8,
                        &string_literal[3..string_literal.len() - 1],
                    ))
                } else {
                    // UTF-16LE
                    Ok((
                        StringLiteralEncoding::Utf16,
                        &string_literal[2..string_literal.len() - 1],
                    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_literal_to_bytes_empty_string() {
        // We consider empty string literals an error, as they should at least
        // contain two double-quotes.
        assert!(string_literal_to_bytes("", None).is_err());
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
