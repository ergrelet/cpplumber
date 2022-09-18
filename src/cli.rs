use std::path::PathBuf;

use structopt::StructOpt;

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Debug, StructOpt)]
#[structopt(name = PKG_NAME, about = "An information leak detector for C and C++ code bases")]
pub struct CpplumberOptions {
    /// Path to the output binary to scan for leaked information.
    #[structopt(parse(from_os_str), short, long = "bin")]
    pub binary_file_path: PathBuf,

    /// Additional include directories.
    /// Only used when project files aren't used.
    #[structopt(short = "I")]
    pub include_directories: Vec<String>,

    /// Additional preprocessor definitions.
    /// Only used when project files aren't used.
    #[structopt(short = "D")]
    pub compile_definitions: Vec<String>,

    /// Compilation database.
    #[structopt(parse(from_os_str), short, long = "project")]
    pub project_file_path: Option<PathBuf>,

    /// Path to a file containing rules to prevent certain errors from being
    /// generated.
    #[structopt(parse(from_os_str), short, long)]
    pub suppressions_list: Option<PathBuf>,

    /// Report leaked values only once, even when found in multiple locations.
    #[structopt(long)]
    pub ignore_multiple_locations: bool,

    /// Report leaks for data declared in system headers
    #[structopt(long)]
    pub report_system_headers: bool,

    /// Minimum required size in bytes, for a leak to be reported. Defaults to 4.
    /// Warning: Setting this to a lower value might greatly increase resource
    /// consumption and reports' sizes.
    #[structopt(short, long)]
    pub minimum_leak_size: Option<usize>,

    /// Generate output as JSON.
    #[structopt(short, long = "json")]
    pub json_output: bool,

    /// List of source files to scan for (can be glob expressions).
    pub source_path_globs: Vec<String>,
}
