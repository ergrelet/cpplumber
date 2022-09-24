use std::{ops::Deref, sync::Arc};

use serde::Serialize;

use super::{LeakLocation, LeakedDataType};

/// Struct containing information on a piece of data that has leaked into a
/// binary file.
#[derive(Serialize)]
pub struct ConfirmedLeak {
    /// Type of data leaked
    pub data_type: LeakedDataType,
    /// Leaked data, as represented in the source code
    pub data: Arc<String>,
    /// Information on where the leaked data is declared in the source code as
    /// well as found in in the target binary
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
        self.0.data == other.0.data
    }
}

impl Eq for ConfirmedLeakWithUniqueValue {}

impl PartialOrd for ConfirmedLeakWithUniqueValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.data.partial_cmp(&other.0.data)
    }
}

impl Ord for ConfirmedLeakWithUniqueValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.data.cmp(&other.0.data)
    }
}
