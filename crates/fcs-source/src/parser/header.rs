use chumsky::{input::ValueInput, prelude::*};

use crate::{
    diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage, ParseOutput},
    version::Version,
};

use super::{
    ParseLimits,
    input::{ChumskySpan, ParserExtra, SpannedToken},
    lexer::lex,
    token::Token,
};

pub fn parse_header(input: &str) -> ParseOutput<Version> {
    match lex(input, ParseLimits::default()) {
        Ok(tokens) => parse_header_tokens(&tokens),
        Err(diagnostics) => ParseOutput::new(None, diagnostics),
    }
}

pub(super) fn header_parser<'tokens, I>()
-> impl Parser<'tokens, I, Version, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    select! { Token::Header(version) => version }.labelled("FCS source header")
}

pub(super) fn parse_header_tokens(tokens: &[SpannedToken]) -> ParseOutput<Version> {
    match tokens.first() {
        Some((Token::Header(version), _)) => ParseOutput::new(Some(*version), Vec::new()),
        Some(_) => missing_header(crate::ast::SourceSpan::new(0, 0)),
        None => missing_header(crate::ast::SourceSpan::new(0, 0)),
    }
}

fn missing_header(span: crate::ast::SourceSpan) -> ParseOutput<Version> {
    ParseOutput::new(
        None,
        vec![Diagnostic::new(
            DiagnosticCode::VERSION_MISSING_HEADER,
            DiagnosticStage::Parse,
            "source is missing an #fcs header",
            span,
        )],
    )
}
