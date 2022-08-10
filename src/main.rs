use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clang::{Clang, Entity, EntityKind, Index};
use structopt::StructOpt;
use widestring::{encode_utf16, encode_utf32};

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Debug, StructOpt)]
#[structopt(name = PKG_NAME, about = "TODO")]
struct CpplumberOptions {
    #[structopt(parse(from_os_str))]
    file_paths: Vec<PathBuf>,
}

#[derive(Debug)]
struct InformationLeakDescription {
    /// Leaked information, as represented in the source code
    leaked_information: String,
    /// Byte pattern to match (i.e., leaked information, as represented in the
    /// binary file)
    bytes: Vec<u8>,
    /// Data on where the leaked information is declared in the
    /// source code (file name, line number)
    declaration_metadata: (PathBuf, u32),
}

impl TryFrom<Entity<'_>> for InformationLeakDescription {
    type Error = ();

    fn try_from(entity: Entity) -> Result<Self, Self::Error> {
        match entity.get_kind() {
            EntityKind::StringLiteral => {
                let leaked_information = entity.get_display_name().unwrap();
                let location = entity.get_location().unwrap().get_file_location();
                let file_location = location.file.unwrap().get_path();
                let line_location = location.line;

                Ok(Self {
                    bytes: string_literal_to_bytes(&leaked_information),
                    leaked_information,
                    declaration_metadata: (file_location, line_location),
                })
            }
            _ => Err(()),
        }
    }
}

fn main() -> Result<()> {
    let options = CpplumberOptions::from_args();

    let clang = Clang::new().map_err(|e| anyhow!(e))?;
    let index = Index::new(&clang, false, false);

    let mut potential_leaks: Vec<InformationLeakDescription> = vec![];
    for file_path in options.file_paths {
        let translation_unit = index
            .parser(file_path)
            .visit_implicit_attributes(false)
            .parse()?;

        let string_literals =
            gather_entities_by_kind(translation_unit.get_entity(), &[EntityKind::StringLiteral]);

        potential_leaks.extend(
            string_literals
                .into_iter()
                .filter_map(|literal| literal.try_into().ok()),
        );
    }

    println!("{:?}", potential_leaks);

    Ok(())
}

fn gather_entities_by_kind<'tu>(
    root_entity: Entity<'tu>,
    entity_kind_filter: &[EntityKind],
) -> Vec<Entity<'tu>> {
    gather_entities_by_kind_rec(root_entity, entity_kind_filter, 0)
}

fn gather_entities_by_kind_rec<'tu>(
    root_entity: Entity<'tu>,
    entity_kind_filter: &[EntityKind],
    current_depth: usize,
) -> Vec<Entity<'tu>> {
    let mut entities = vec![];

    let root_entity_kind = root_entity.get_kind();
    if entity_kind_filter
        .iter()
        .any(|elem| elem == &root_entity_kind)
    {
        println!("{}", root_entity.get_name().unwrap());
        entities.push(root_entity);
    }

    for child in root_entity.get_children() {
        // We're only interested in declarations made in the main files
        if child.is_in_main_file() {
            let entities_sub =
                gather_entities_by_kind_rec(child, entity_kind_filter, current_depth + 1);
            entities.extend(entities_sub);
        }
    }

    entities
}

/// We have to reimplement this ourselves since the `clang` crate doesn't
/// provide an easy to get byte representations of `StringLiteral` entities.
fn string_literal_to_bytes(string_literal: &str) -> Vec<u8> {
    let mut char_it = string_literal.chars();
    let first_char = char_it.next();
    match first_char {
        None => return vec![],
        Some(first_char) => match first_char {
            // Ordinary string (we assume it'll be encoded to ASCII)
            '"' => string_literal[1..string_literal.len() - 1]
                .as_bytes()
                .to_owned(),
            // Wide string (we assume it'll be encoded to UTF-16LE)
            'L' => encode_utf16(string_literal[2..string_literal.len() - 1].chars())
                .map(u16::to_le_bytes)
                .fold(Vec::new(), |mut acc: Vec<u8>, e| {
                    acc.extend(e);
                    acc
                }),
            // UTF-32 string
            'U' => encode_utf32(string_literal[2..string_literal.len() - 1].chars())
                .map(u32::to_le_bytes)
                .fold(Vec::new(), |mut acc: Vec<u8>, e| {
                    acc.extend(e);
                    acc
                }),
            // UTF-8 or UTF-16LE string
            'u' => {
                let second_char = char_it.next().unwrap();
                let third_char = char_it.next().unwrap();
                if second_char == '8' && third_char == '"' {
                    // UTF-8
                    string_literal[3..string_literal.len() - 1]
                        .as_bytes()
                        .to_owned()
                } else {
                    // UTF-16LE
                    encode_utf16(string_literal[2..string_literal.len() - 1].chars())
                        .map(u16::to_le_bytes)
                        .fold(Vec::new(), |mut acc: Vec<u8>, e| {
                            acc.extend(e);
                            acc
                        })
                }
            }
            _ => unreachable!("New string literal prefix introduced in the standard?"),
        },
    }
}
