use chumsky::{error::Rich, prelude::SimpleSpan};

use super::token::Token;
use crate::ast::SourceSpan;

pub(crate) type ChumskySpan = SimpleSpan<usize>;
pub(crate) type SpannedToken = (Token, ChumskySpan);
pub(crate) type ParserExtra<'tokens> = chumsky::extra::Err<Rich<'tokens, Token, ChumskySpan>>;
pub(crate) fn source_span(span: ChumskySpan) -> SourceSpan {
    SourceSpan::new(span.start, span.end)
}
