use chumsky::{input::ValueInput, prelude::*};

use crate::ast::{Beat, SourceBpm, SourceLiteral, SourceSpan, TempoMap, TempoPoint};

use super::{
    input::{ChumskySpan, ParserExtra, source_span},
    token::{Keyword, Punctuation, Token},
};

pub(super) fn tempo_map_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, (TempoMap, SourceSpan), ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::TempoMap))
        .ignore_then(tempo_points_parser().delimited_by(
            just(Token::Punctuation(Punctuation::LeftBrace)),
            just(Token::Punctuation(Punctuation::RightBrace)),
        ))
        .map(|points| TempoMap { points })
        .map_with(|tempo_map, extra| (tempo_map, source_span(extra.span())))
}

fn tempo_points_parser<'tokens, I>()
-> impl Parser<'tokens, I, Vec<TempoPoint>, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    let beat = select! { Token::Literal(SourceLiteral::Beat(beat)) => beat };
    let signed_beat = just(Token::Punctuation(Punctuation::Minus))
        .or_not()
        .then(beat)
        .try_map(|(negative, beat), span| {
            if negative.is_some() {
                Beat::new(-beat.numerator(), beat.denominator())
                    .map_err(|_| chumsky::error::Rich::custom(span, "invalid tempo beat"))
            } else {
                Ok(beat)
            }
        });
    let bpm = select! { Token::TempoBpm(bpm) => bpm };
    let signed_bpm = just(Token::Punctuation(Punctuation::Minus))
        .or_not()
        .then(bpm)
        .map(|(negative, bpm)| {
            if negative.is_some() {
                SourceBpm::from_value(-bpm.get())
            } else {
                bpm
            }
        });
    signed_beat
        .then_ignore(just(Token::Punctuation(Punctuation::Arrow)))
        .then(signed_bpm)
        .then_ignore(just(Token::Punctuation(Punctuation::Semicolon)))
        .map(|(beat, bpm)| TempoPoint { beat, bpm })
        .repeated()
        .collect()
}
