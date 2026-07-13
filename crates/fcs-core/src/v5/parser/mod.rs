mod document;
mod header;
mod tempo;

use crate::v5::version::Version;

pub use document::parse_document;
pub use header::parse_header;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    MissingHeader,
    InvalidVersion,
    UnsupportedSourceVersion(Version),
    InvalidSyntax(&'static str),
}
