use std::{hash::Hash, sync::Arc};

use serde::Serialize;

use super::LeakLocation;

/// Struct containing information on a piece of data that has leaked into a
/// binary file.
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
