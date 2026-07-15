mod definitions;
mod document;
mod entities;
mod expression;
mod header;
mod input;
mod lexer;
mod tempo;
mod token;

pub use document::{
    parse_document, parse_document_bytes, parse_document_bytes_with_limits,
    parse_document_with_limits,
};
pub use expression::{
    parse_expression, parse_expression_with_limits, parse_type, parse_type_with_limits,
};
pub use header::parse_header;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLimits {
    pub max_source_bytes: usize,
    pub max_tokens: usize,
    pub max_nesting_depth: usize,
    pub max_comment_depth: usize,
    pub max_literal_bytes: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 16 * 1024 * 1024,
            max_tokens: 1_000_000,
            max_nesting_depth: 512,
            max_comment_depth: 256,
            max_literal_bytes: 1024 * 1024,
        }
    }
}

impl From<usize> for ParseLimits {
    fn from(max_source_bytes: usize) -> Self {
        Self {
            max_source_bytes,
            ..Self::default()
        }
    }
}
