use std::{hash::Hash, path::PathBuf, sync::Arc};

use serde::Serialize;

/// Struct containing the source and binary locations of leaked data
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
