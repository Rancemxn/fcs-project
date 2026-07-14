use chumsky::{input::MappedInput, prelude::SimpleSpan};

use super::token::Token;
use crate::ast::SourceSpan;

pub(crate) type ChumskySpan = SimpleSpan<usize>;
pub(crate) type SpannedToken = (Token, ChumskySpan);
#[allow(dead_code)]
pub(crate) type TokenInput<'tokens> = MappedInput<
    Token,
    ChumskySpan,
    &'tokens [SpannedToken],
    fn(&SpannedToken) -> (&Token, ChumskySpan),
>;

pub(crate) fn source_span(span: ChumskySpan) -> SourceSpan {
    SourceSpan::new(span.start, span.end)
}
