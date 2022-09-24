use std::collections::BTreeSet;

use anyhow::Result;
use serde::Serialize;

use crate::information_leak::{ConfirmedLeak, LeakedDataType};

const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const REPORT_FORMAT_VERSION: u32 = 1;

#[derive(Serialize)]
struct JsonReport<SortedConfirmedLeak: Into<ConfirmedLeak> + Ord + Eq + Serialize> {
    version: ReportVersion,
    leaks: BTreeSet<SortedConfirmedLeak>,
}

#[derive(Serialize)]
struct ReportVersion {
    executable: String,
    format: u32,
}

pub fn dump_confirmed_leaks<W, SortedConfirmedLeak>(
    writer: W,
    confirmed_leaks: BTreeSet<SortedConfirmedLeak>,
    json: bool,
) -> Result<()>
where
    W: std::io::Write,
    SortedConfirmedLeak: Into<ConfirmedLeak> + Ord + Eq + Serialize,
{
    if json {
        dump_confirmed_leaks_as_json(writer, confirmed_leaks)
    } else {
        dump_confirmed_leaks_as_text(writer, confirmed_leaks)
    }
}

fn dump_confirmed_leaks_as_json<W, SortedConfirmedLeak>(
    writer: W,
    confirmed_leaks: BTreeSet<SortedConfirmedLeak>,
) -> Result<()>
where
    W: std::io::Write,
    SortedConfirmedLeak: Into<ConfirmedLeak> + Ord + Eq + Serialize,
{
    let report = JsonReport {
        version: ReportVersion {
            executable: PKG_VERSION.into(),
            format: REPORT_FORMAT_VERSION,
        },
        leaks: confirmed_leaks,
    };

    Ok(serde_json::to_writer(writer, &report)?)
}

fn dump_confirmed_leaks_as_text<W, SortedConfirmedLeak>(
    mut writer: W,
    confirmed_leaks: BTreeSet<SortedConfirmedLeak>,
) -> Result<()>
where
    W: std::io::Write,
    SortedConfirmedLeak: Into<ConfirmedLeak> + Ord + Eq + Serialize,
{
    for leak in confirmed_leaks {
        let leak: ConfirmedLeak = leak.into();
        writeln!(
            &mut writer,
            "\"{}\" ({}) leaked at offset 0x{:x} in \"{}\" [declared at {}:{}]",
            leak.data,
            display_leaked_data_type(leak.data_type),
            leak.location.binary.offset,
            leak.location.binary.file.display(),
            leak.location.source.file.display(),
            leak.location.source.line,
        )?;
    }

    Ok(())
}

/// Returns a text representation of `LeakedDataType`
fn display_leaked_data_type(data_type: LeakedDataType) -> String {
    match data_type {
        LeakedDataType::StringLiteral => "string literal".to_string(),
        LeakedDataType::StructName => "struct name".to_string(),
        LeakedDataType::ClassName => "class name".to_string(),
    }
}
