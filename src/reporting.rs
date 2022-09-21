use std::collections::BTreeSet;

use anyhow::Result;
use serde::Serialize;

use crate::information_leak::ConfirmedLeak;

const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const REPORT_FORMAT_VERSION: u32 = 1;

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

pub fn dump_confirmed_leaks<W: std::io::Write>(
    mut writer: W,
    confirmed_leaks: BTreeSet<ConfirmedLeak>,
    json: bool,
) -> Result<()> {
    if json {
        let report = JsonReport {
            version: ReportVersion {
                executable: PKG_VERSION.into(),
                format: REPORT_FORMAT_VERSION,
            },
            leaks: confirmed_leaks,
        };
        serde_json::to_writer(writer, &report)?;
    } else {
        for leak in confirmed_leaks {
            writeln!(
                &mut writer,
                "{} leaked at offset 0x{:x} in \"{}\" [declared at {}:{}]",
                leak.leaked_information,
                leak.location.binary.offset,
                leak.location.binary.file.display(),
                leak.location.source.file.display(),
                leak.location.source.line,
            )?;
        }
    }

    Ok(())
}
