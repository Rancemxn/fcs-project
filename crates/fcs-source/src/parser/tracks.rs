use chumsky::{input::ValueInput, prelude::*};

use crate::ast::{
    DirectPoint, DirectSegment, FieldPath, GeneratorOwner, HalfOpenInterval, Interpolation,
    LineBodyItem, LineDeclaration, LinesBlock, ScrollTempoMap, ScrollTempoPoint, SegmentsBlock,
    SourceBpm, SourceExpression, SourceSpan, TrackDeclaration, TrackSegmentItem, TrackSetting,
    TracksBlock,
};

use super::{
    definitions::identifier_with_span,
    entities::{field_parser, field_path_parser, generator_parser},
    expression::{expression_parser, type_parser},
    input::{ChumskySpan, ParserExtra, source_span},
    token::{Keyword, Punctuation, Token},
};

pub(super) fn lines_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, LinesBlock, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Lines))
        .map_with(|_, extra| source_span(extra.span()))
        .then(
            just(left_brace())
                .ignore_then(line_declaration_parser().repeated().collect::<Vec<_>>())
                .then_ignore(just(right_brace())),
        )
        .map_with(|(keyword_span, lines), extra| LinesBlock {
            lines,
            keyword_span,
            span: source_span(extra.span()),
        })
}

fn line_declaration_parser<'tokens, I>()
-> impl Parser<'tokens, I, LineDeclaration, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Line))
        .ignore_then(identifier_with_span())
        .then(
            just(left_brace())
                .ignore_then(line_body_item_parser().repeated().collect::<Vec<_>>())
                .then_ignore(just(right_brace())),
        )
        .map_with(|((name, name_span), items), extra| LineDeclaration {
            name,
            name_span,
            items,
            span: source_span(extra.span()),
        })
}

fn line_body_item_parser<'tokens, I>()
-> impl Parser<'tokens, I, LineBodyItem, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    choice((
        tracks_block_parser().map(LineBodyItem::Tracks),
        scroll_tempo_map_block_parser().map(LineBodyItem::ScrollTempoMap),
        field_parser().map(LineBodyItem::Field),
    ))
}

fn scroll_tempo_map_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, ScrollTempoMap, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::ScrollTempoMap))
        .map_with(|_, extra| source_span(extra.span()))
        .then(
            scroll_tempo_point_parser()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(left_brace()), just(right_brace())),
        )
        .map_with(|(keyword_span, points), extra| ScrollTempoMap {
            points,
            span: source_span(extra.span()),
            keyword_span,
        })
}

fn scroll_tempo_point_parser<'tokens, I>()
-> impl Parser<'tokens, I, ScrollTempoPoint, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    expression_parser()
        .then_ignore(just(arrow()))
        .then(signed_scroll_bpm_parser())
        .then_ignore(just(semicolon()))
        .map_with(|(key, bpm), extra| ScrollTempoPoint {
            key,
            bpm,
            span: source_span(extra.span()),
        })
}

fn signed_scroll_bpm_parser<'tokens, I>()
-> impl Parser<'tokens, I, SourceBpm, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    select! { Token::TempoBpm(bpm) => bpm }
        .map(|bpm| bpm)
        .or(just(Token::Punctuation(Punctuation::Minus))
            .ignore_then(select! { Token::TempoBpm(bpm) => bpm })
            .map(|bpm| SourceBpm::from_value(-bpm.get())))
}

fn tracks_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, TracksBlock, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Tracks))
        .map_with(|_, extra| source_span(extra.span()))
        .then(
            just(left_brace())
                .ignore_then(track_declaration_parser().repeated().collect::<Vec<_>>())
                .then_ignore(just(right_brace())),
        )
        .map_with(|(keyword_span, tracks), extra| TracksBlock {
            tracks,
            keyword_span,
            span: source_span(extra.span()),
        })
}

fn track_declaration_parser<'tokens, I>()
-> impl Parser<'tokens, I, TrackDeclaration, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Track))
        .ignore_then(identifier_with_span())
        .then_ignore(just(arrow()))
        .then(field_path_parser())
        .then_ignore(just(colon()))
        .then(type_parser())
        .then(track_body_parser())
        .map_with(
            |((((name, name_span), target), value_type_source), (settings, segments)), extra| {
                let mut segments = segments;
                assign_track_generator_owners(
                    &mut segments.items,
                    &name,
                    &target,
                    SourceSpan::new(name_span.start, source_span(extra.span()).end),
                );
                TrackDeclaration {
                    name,
                    name_span,
                    target,
                    value_type: value_type_source.to_type(),
                    value_type_source,
                    settings,
                    segments,
                    span: source_span(extra.span()),
                }
            },
        )
}

fn track_body_parser<'tokens, I>()
-> impl Parser<'tokens, I, (Vec<TrackSetting>, SegmentsBlock), ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(left_brace())
        .ignore_then(track_setting_parser().repeated().collect::<Vec<_>>())
        .then(segments_block_parser())
        .then_ignore(just(right_brace()))
}

fn track_setting_parser<'tokens, I>()
-> impl Parser<'tokens, I, TrackSetting, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    identifier_with_span()
        .then_ignore(just(colon()))
        .then(expression_parser())
        .then_ignore(just(semicolon()))
        .map_with(|((name, name_span), value), extra| TrackSetting {
            name,
            name_span,
            value,
            span: source_span(extra.span()),
        })
}

fn segments_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, SegmentsBlock, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Segments))
        .map_with(|_, extra| source_span(extra.span()))
        .then(
            just(left_brace())
                .ignore_then(segment_item_parser().repeated().collect::<Vec<_>>())
                .then_ignore(just(right_brace())),
        )
        .map_with(|(keyword_span, items), extra| SegmentsBlock {
            items,
            keyword_span,
            span: source_span(extra.span()),
        })
}

fn segment_item_parser<'tokens, I>()
-> impl Parser<'tokens, I, TrackSegmentItem, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    recursive(|item| {
        let block = item
            .clone()
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(just(left_brace()), just(right_brace()));
        let conditional = just(Token::Keyword(Keyword::If))
            .ignore_then(expression_parser())
            .then(block.clone())
            .then(
                just(Token::Keyword(Keyword::Else))
                    .ignore_then(block)
                    .or_not(),
            )
            .map_with(|((condition, then_items), else_items), extra| {
                TrackSegmentItem::Conditional {
                    condition,
                    then_items,
                    else_items: else_items.unwrap_or_default(),
                    span: source_span(extra.span()),
                }
            });
        choice((
            generator_parser(track_generator_owner_placeholder()).map(TrackSegmentItem::Generator),
            conditional,
            direct_segment_parser().map(TrackSegmentItem::DirectSegment),
            direct_point_parser().map(TrackSegmentItem::DirectPoint),
        ))
    })
}

fn direct_segment_parser<'tokens, I>()
-> impl Parser<'tokens, I, DirectSegment, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    half_open_interval_parser()
        .then_ignore(just(colon()))
        .then(expression_parser())
        .then_ignore(just(arrow()))
        .then(expression_parser())
        .then_ignore(just(Token::Keyword(Keyword::Using)))
        .then(interpolation_parser())
        .then_ignore(just(semicolon()))
        .map_with(
            |(((interval, start_value), end_value), interpolation), extra| DirectSegment {
                interval,
                start_value,
                end_value,
                interpolation,
                span: source_span(extra.span()),
            },
        )
}

fn direct_point_parser<'tokens, I>()
-> impl Parser<'tokens, I, DirectPoint, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Point))
        .ignore_then(expression_parser())
        .then_ignore(just(colon()))
        .then(expression_parser())
        .then_ignore(just(semicolon()))
        .map_with(|(time, value), extra| DirectPoint {
            time,
            value,
            span: source_span(extra.span()),
        })
}

fn half_open_interval_parser<'tokens, I>()
-> impl Parser<'tokens, I, HalfOpenInterval, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(left_bracket())
        .ignore_then(expression_parser())
        .then_ignore(just(comma()))
        .then(expression_parser())
        .then_ignore(just(right_parenthesis()))
        .map_with(|(start, end), extra| HalfOpenInterval {
            start,
            end,
            span: source_span(extra.span()),
        })
}

fn interpolation_parser<'tokens, I>()
-> impl Parser<'tokens, I, Interpolation, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    cubic_bezier_parser()
        .map(|(values, span)| Interpolation::CubicBezier { values, span })
        .or(expression_parser().map(Interpolation::Expression))
}

fn cubic_bezier_parser<'tokens, I>()
-> impl Parser<'tokens, I, ([SourceExpression; 4], SourceSpan), ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::CubicBezier))
        .ignore_then(
            expression_parser()
                .then_ignore(just(comma()))
                .then(expression_parser())
                .then_ignore(just(comma()))
                .then(expression_parser())
                .then_ignore(just(comma()))
                .then(expression_parser())
                .delimited_by(just(left_parenthesis()), just(right_parenthesis())),
        )
        .map_with(|(((first, second), third), fourth), extra| {
            ([first, second, third, fourth], source_span(extra.span()))
        })
}

fn assign_track_generator_owners(
    items: &mut [TrackSegmentItem],
    track: &str,
    target: &FieldPath,
    span: SourceSpan,
) {
    for item in items {
        match item {
            TrackSegmentItem::Generator(generator) => {
                *generator.owner = GeneratorOwner::TrackSegments {
                    track: track.to_owned(),
                    target: target.clone(),
                    span,
                };
            }
            TrackSegmentItem::Conditional {
                then_items,
                else_items,
                ..
            } => {
                assign_track_generator_owners(then_items, track, target, span);
                assign_track_generator_owners(else_items, track, target, span);
            }
            TrackSegmentItem::DirectSegment(_) | TrackSegmentItem::DirectPoint(_) => {}
        }
    }
}

fn track_generator_owner_placeholder() -> GeneratorOwner {
    GeneratorOwner::TrackSegments {
        track: String::new(),
        target: FieldPath {
            segments: Vec::new(),
            span: SourceSpan::new(0, 0),
        },
        span: SourceSpan::new(0, 0),
    }
}

fn left_brace() -> Token {
    Token::Punctuation(Punctuation::LeftBrace)
}
fn right_brace() -> Token {
    Token::Punctuation(Punctuation::RightBrace)
}
fn left_bracket() -> Token {
    Token::Punctuation(Punctuation::LeftBracket)
}
fn right_parenthesis() -> Token {
    Token::Punctuation(Punctuation::RightParenthesis)
}
fn left_parenthesis() -> Token {
    Token::Punctuation(Punctuation::LeftParenthesis)
}
fn arrow() -> Token {
    Token::Punctuation(Punctuation::Arrow)
}
fn colon() -> Token {
    Token::Punctuation(Punctuation::Colon)
}
fn comma() -> Token {
    Token::Punctuation(Punctuation::Comma)
}
fn semicolon() -> Token {
    Token::Punctuation(Punctuation::Semicolon)
}
