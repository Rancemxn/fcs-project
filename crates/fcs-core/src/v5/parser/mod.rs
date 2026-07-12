mod header;

use crate::v5::version::Version;

pub use header::parse_header;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    MissingHeader,
    InvalidVersion,
    UnsupportedSourceVersion(Version),
    InvalidSyntax(&'static str),
}
