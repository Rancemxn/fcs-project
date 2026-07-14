mod definitions;
mod document;
mod entities;
mod expression;
mod header;
mod input;
mod lexer;
mod tempo;
mod token;

use crate::ast::SourceSpan;
use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage, ParseOutput};
use crate::version::Version;

pub use document::{parse_document, parse_document_with_limits};
pub use expression::{
    parse_expression, parse_expression_with_limits, parse_type, parse_type_with_limits,
};
pub use header::parse_header;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParseError {
    MissingHeader,
    InvalidVersion,
    UnsupportedSourceVersion(Version),
    InvalidSyntax(&'static str),
    Diagnostic(Diagnostic),
}

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

impl ParseError {
    fn diagnostic(self, input: &str) -> Diagnostic {
        let span = SourceSpan::new(0, input.len());
        match self {
            Self::Diagnostic(diagnostic) => diagnostic,
            Self::MissingHeader => Diagnostic::new(
                DiagnosticCode::VERSION_MISSING_HEADER,
                DiagnosticStage::Parse,
                "source is missing an #fcs header",
                SourceSpan::new(0, 0),
            ),
            Self::InvalidVersion => Diagnostic::new(
                DiagnosticCode::VERSION_INVALID,
                DiagnosticStage::Parse,
                "source version is not a valid semantic version",
                header_span(input),
            ),
            Self::UnsupportedSourceVersion(version) => Diagnostic::new(
                DiagnosticCode::VERSION_UNSUPPORTED,
                DiagnosticStage::Parse,
                format!("source version {version} is not supported"),
                header_span(input),
            ),
            Self::InvalidSyntax(reason) => Diagnostic::new(
                syntax_code(reason, input),
                DiagnosticStage::Parse,
                syntax_message(reason),
                span,
            ),
        }
    }
}

fn header_span(input: &str) -> SourceSpan {
    let end = input.find('\n').unwrap_or(input.len());
    SourceSpan::new(0, end)
}

fn syntax_code(reason: &str, input: &str) -> DiagnosticCode {
    if reason == "resource limit" {
        return DiagnosticCode::RESOURCE_LIMIT_EXCEEDED;
    }
    let (unclosed_comment, unclosed_string) = scan_unclosed_syntax(input);
    if reason.contains("comment") || unclosed_comment {
        return DiagnosticCode::SYNTAX_UNCLOSED_COMMENT;
    }
    if reason.contains("string") || unclosed_string {
        return DiagnosticCode::SYNTAX_UNCLOSED_STRING;
    }
    if reason == "trailing document input" {
        return DiagnosticCode::SYNTAX_TRAILING_INPUT;
    }
    DiagnosticCode::SYNTAX_INVALID_TOKEN
}

fn syntax_message(reason: &str) -> &'static str {
    match reason {
        "trailing document input" => "trailing non-trivia input",
        "format block" => "invalid or incomplete block syntax",
        "document profile" => "invalid document profile syntax",
        "expression" => "invalid expression syntax",
        "type" => "invalid type syntax",
        "resource limit" => "parser resource limit exceeded",
        _ => "invalid source syntax",
    }
}

fn scan_unclosed_syntax(input: &str) -> (bool, bool) {
    let mut escaped = false;
    let mut in_string = false;
    let mut in_line_comment = false;
    let mut block_comment_depth = 0usize;
    let mut characters = input.chars().peekable();

    while let Some(character) = characters.next() {
        if in_line_comment {
            if character == '\n' {
                in_line_comment = false;
            }
            continue;
        }
        if block_comment_depth > 0 {
            if character == '/' && characters.peek() == Some(&'*') {
                characters.next();
                block_comment_depth += 1;
            } else if character == '*' && characters.peek() == Some(&'/') {
                characters.next();
                block_comment_depth -= 1;
            }
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '/' if characters.peek() == Some(&'/') => {
                characters.next();
                in_line_comment = true;
            }
            '/' if characters.peek() == Some(&'*') => {
                characters.next();
                block_comment_depth = 1;
            }
            _ => {}
        }
    }

    (block_comment_depth > 0, in_string)
}

pub(crate) fn output_from_result<T>(input: &str, result: Result<T, ParseError>) -> ParseOutput<T> {
    match result {
        Ok(output) => ParseOutput::new(Some(output), Vec::new()),
        Err(error) => ParseOutput::new(None, vec![error.diagnostic(input)]),
    }
}
