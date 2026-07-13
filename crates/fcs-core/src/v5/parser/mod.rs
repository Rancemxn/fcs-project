mod document;
mod expression;
mod header;
mod lexer;
mod tempo;

use crate::v5::version::Version;

pub use document::parse_document;
pub use expression::{parse_expression, parse_type};
pub use header::parse_header;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    MissingHeader,
    InvalidVersion,
    UnsupportedSourceVersion(Version),
    MissingRequiredBlock(&'static str),
    InvalidTempoMap(&'static str),
    InvalidSyntax(&'static str),
}
