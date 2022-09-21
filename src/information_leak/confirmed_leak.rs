use std::{ops::Deref, sync::Arc};

use serde::Serialize;

use super::LeakLocation;

/// Struct containing information on a piece of data that has leaked into a
/// binary file.
#[derive(Serialize)]
pub struct ConfirmedLeak {
    pub leaked_information: Arc<String>,
    pub location: LeakLocation,
}

impl From<ConfirmedLeakWithUniqueLocation> for ConfirmedLeak {
    fn from(leak: ConfirmedLeakWithUniqueLocation) -> Self {
        leak.0
    }
}

impl From<ConfirmedLeakWithUniqueValue> for ConfirmedLeak {
    fn from(leak: ConfirmedLeakWithUniqueValue) -> Self {
        leak.0
    }
}

/// Wrapper struct used to deduplicate `ConfirmedLeak`s in `BTreeSet`s based on
/// the value of the `location` field.
#[derive(Serialize)]
pub struct ConfirmedLeakWithUniqueLocation(ConfirmedLeak);

impl From<ConfirmedLeak> for ConfirmedLeakWithUniqueLocation {
    fn from(leak: ConfirmedLeak) -> Self {
        ConfirmedLeakWithUniqueLocation(leak)
    }
}

impl Deref for ConfirmedLeakWithUniqueLocation {
    type Target = ConfirmedLeak;
    fn deref(&self) -> &ConfirmedLeak {
        &self.0
    }
}

impl PartialEq for ConfirmedLeakWithUniqueLocation {
    fn eq(&self, other: &Self) -> bool {
        self.0.location == other.0.location
    }
}

impl Eq for ConfirmedLeakWithUniqueLocation {}

impl PartialOrd for ConfirmedLeakWithUniqueLocation {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.location.partial_cmp(&other.0.location)
    }
}

impl Ord for ConfirmedLeakWithUniqueLocation {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.location.cmp(&other.0.location)
    }
}

/// Wrapper struct used to deduplicate `ConfirmedLeak`s in `BTreeSet`s based on
/// the value of the `leak_information` field.
#[derive(Serialize)]
pub struct ConfirmedLeakWithUniqueValue(ConfirmedLeak);

impl From<ConfirmedLeak> for ConfirmedLeakWithUniqueValue {
    fn from(leak: ConfirmedLeak) -> Self {
        ConfirmedLeakWithUniqueValue(leak)
    }
}

impl Deref for ConfirmedLeakWithUniqueValue {
    type Target = ConfirmedLeak;
    fn deref(&self) -> &ConfirmedLeak {
        &self.0
    }
}

impl PartialEq for ConfirmedLeakWithUniqueValue {
    fn eq(&self, other: &Self) -> bool {
        self.0.leaked_information == other.0.leaked_information
    }
}

impl Eq for ConfirmedLeakWithUniqueValue {}

impl PartialOrd for ConfirmedLeakWithUniqueValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0
            .leaked_information
            .partial_cmp(&other.0.leaked_information)
    }
}

impl Ord for ConfirmedLeakWithUniqueValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.leaked_information.cmp(&other.0.leaked_information)
    }
}
