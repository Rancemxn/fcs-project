//! Nom-based parser for .fcs source files.

pub mod block;
pub mod expr;
pub mod literal;

pub use block::parse_document;
pub use expr::parse_expression;
pub use literal::{parse_literal, parse_string, parse_color, parse_bool, parse_numeric_literal, ws};
