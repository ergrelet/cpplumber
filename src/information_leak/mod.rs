mod confirmed_leak;
mod leak_location;
mod potential_leak;

pub use confirmed_leak::*;
pub use leak_location::*;
pub use potential_leak::*;

use serde::Serialize;

/// Describes the kind of data that's leaked
#[derive(Debug, Serialize, Clone, Copy)]
pub enum LeakedDataType {
    /// Data comes from a string literal
    StringLiteral,
    /// Data represents the name of a C/C++ struct
    StructName,
    /// Data represents the name of a C++ class
    ClassName,
}
