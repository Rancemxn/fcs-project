use std::collections::HashMap;

use chumsky::{input::Input as _, prelude::*};

use crate::ast::{DefinitionsBlock, Document, DocumentProfile, SourceSpan, TempoMap};
use crate::diagnostic::{
    Diagnostic, DiagnosticCode, DiagnosticLabel, DiagnosticStage, ParseOutput,
};
use crate::validation::validate_profile;

use super::{
    ParseLimits,
    definitions::definitions_block_parser,
    entities::collections_block_parser,
    header::{header_parser, parse_header_tokens},
    input::{ChumskySpan, ParserExtra, SpannedToken, source_span},
    lexer::lex_document,
    tempo::tempo_map_block_parser,
    token::{Keyword, Punctuation, Token},
};

pub fn parse_document(source: &str) -> ParseOutput<Document> {
    parse_document_with_limits(source, ParseLimits::default())
}

pub fn parse_document_bytes(source: &[u8]) -> ParseOutput<Document> {
    parse_document_bytes_with_limits(source, ParseLimits::default())
}

pub fn parse_document_bytes_with_limits<L: Into<ParseLimits>>(
    source: &[u8],
    limits: L,
) -> ParseOutput<Document> {
    match std::str::from_utf8(source) {
        Ok(source) => parse_document_with_limits(source, limits),
        Err(error) => {
            let start = error.valid_up_to();
            let end = error
                .error_len()
                .map_or(source.len(), |length| start + length);
            ParseOutput::new(
                None,
                vec![Diagnostic::new(
                    DiagnosticCode::DECODE_INVALID_UTF8,
                    DiagnosticStage::Decode,
                    "source is not valid UTF-8",
                    SourceSpan::new(start, end),
                )],
            )
        }
    }
}

pub fn parse_document_with_limits<L: Into<ParseLimits>>(
    source: &str,
    limits: L,
) -> ParseOutput<Document> {
    match lex_document(source, limits.into()) {
        Ok(tokens) => parse_document_tokens(source, &tokens),
        Err(diagnostics) => ParseOutput::new(None, diagnostics),
    }
}

pub(crate) fn parse_document_tokens(
    source: &str,
    tokens: &[SpannedToken],
) -> ParseOutput<Document> {
    if !matches!(tokens.first(), Some((Token::Header(_), _))) {
        return ParseOutput::new(None, parse_header_tokens(tokens).into_result().unwrap_err());
    }
    if let Some(diagnostic) = validate_top_level_blocks(tokens) {
        return ParseOutput::new(None, vec![diagnostic]);
    }

    let end_span = ChumskySpan::new((), source.len()..source.len());
    let input = tokens.map(end_span, |(token, span)| (token, span));
    let (parsed, errors) = document_parser()
        .then_ignore(end())
        .parse(input)
        .into_output_errors();
    if !errors.is_empty() {
        let mut diagnostics = errors
            .into_iter()
            .map(|error| {
                Diagnostic::new(
                    DiagnosticCode::SYNTAX_INVALID_TOKEN,
                    DiagnosticStage::Parse,
                    "invalid document syntax",
                    source_span(*error.span()),
                )
            })
            .collect::<Vec<_>>();
        if let Some(span) = invalid_profile_span(tokens) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::SYNTAX_INVALID_TOKEN,
                DiagnosticStage::Parse,
                "invalid document profile",
                span,
            ));
        }
        if let Some(span) = extra_format_token_span(tokens) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::SYNTAX_INVALID_TOKEN,
                DiagnosticStage::Parse,
                "unexpected token in format block",
                span,
            ));
        }
        return ParseOutput::new(None, diagnostics);
    }

    let ParsedDocument {
        source_version,
        profile,
        format_span,
        blocks,
    } = parsed.expect("document parser produces output when it has no errors");
    let mut tempo_map = None;
    let mut tempo_span = None;
    let mut definitions = None;
    let mut collections = Vec::new();
    for block in blocks {
        match block {
            TopLevelBlock::Tempo(map, span) => {
                tempo_map = Some(map);
                tempo_span = Some(span);
            }
            TopLevelBlock::Definitions(block) => definitions = Some(block),
            TopLevelBlock::Collections(block) => collections = block.collections,
        }
    }
    if let Err(diagnostic) = validate_profile(
        profile,
        tempo_map.as_ref(),
        tempo_span.unwrap_or(format_span),
    ) {
        return ParseOutput::new(None, vec![diagnostic]);
    }
    ParseOutput::new(
        Some(Document {
            source_version,
            profile,
            tempo_map,
            definitions,
            collections,
        }),
        Vec::new(),
    )
}

fn document_parser<'tokens, I>()
-> impl Parser<'tokens, I, ParsedDocument, ParserExtra<'tokens>> + Clone
where
    I: chumsky::input::ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    header_parser()
        .then(format_parser())
        .then(top_level_parser().repeated().collect::<Vec<_>>())
        .map(
            |((source_version, (profile, format_span)), blocks)| ParsedDocument {
                source_version,
                profile,
                format_span,
                blocks,
            },
        )
}

fn format_parser<'tokens, I>()
-> impl Parser<'tokens, I, (DocumentProfile, SourceSpan), ParserExtra<'tokens>> + Clone
where
    I: chumsky::input::ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Format))
        .ignore_then(
            just(Token::Keyword(Keyword::Profile))
                .ignore_then(just(Token::Punctuation(Punctuation::Colon)))
                .ignore_then(select! {
                    Token::Keyword(Keyword::Fragment) => DocumentProfile::Fragment,
                    Token::Keyword(Keyword::Chart) => DocumentProfile::Chart,
                    Token::Keyword(Keyword::Playable) => DocumentProfile::Playable,
                    Token::Keyword(Keyword::Renderable) => DocumentProfile::Renderable,
                    Token::Keyword(Keyword::Publishable) => DocumentProfile::Publishable,
                })
                .then_ignore(just(Token::Punctuation(Punctuation::Semicolon)))
                .delimited_by(
                    just(Token::Punctuation(Punctuation::LeftBrace)),
                    just(Token::Punctuation(Punctuation::RightBrace)),
                ),
        )
        .map_with(|profile, extra| (profile, source_span(extra.span())))
}

fn top_level_parser<'tokens, I>()
-> impl Parser<'tokens, I, TopLevelBlock, ParserExtra<'tokens>> + Clone
where
    I: chumsky::input::ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    choice((
        tempo_map_block_parser().map(|(map, span)| TopLevelBlock::Tempo(map, span)),
        definitions_block_parser().map(TopLevelBlock::Definitions),
        collections_block_parser().map(TopLevelBlock::Collections),
    ))
}

#[derive(Debug)]
struct ParsedDocument {
    source_version: crate::version::Version,
    profile: DocumentProfile,
    format_span: SourceSpan,
    blocks: Vec<TopLevelBlock>,
}

#[derive(Debug)]
enum TopLevelBlock {
    Tempo(TempoMap, SourceSpan),
    Definitions(DefinitionsBlock),
    Collections(crate::ast::CollectionsBlock),
}

fn validate_top_level_blocks(tokens: &[SpannedToken]) -> Option<Diagnostic> {
    let mut seen = HashMap::<BlockKind, SourceSpan>::new();
    let mut index = 1;
    while index < tokens.len() {
        let (token, keyword_span) = &tokens[index];
        let Some(kind) = block_kind(token) else {
            return Some(Diagnostic::new(
                DiagnosticCode::SYNTAX_TRAILING_INPUT,
                DiagnosticStage::Parse,
                "trailing non-trivia input",
                source_span(*keyword_span),
            ));
        };
        let (block_span, next) = block_extent(tokens, index)?;
        if kind == BlockKind::Misplaced {
            return Some(Diagnostic::new(
                DiagnosticCode::SYNTAX_MISPLACED_BLOCK,
                DiagnosticStage::Parse,
                "top-level block is not available in the I0 source subset",
                source_span(*keyword_span),
            ));
        }
        if let Some(first) = seen.insert(kind, block_span) {
            return Some(
                Diagnostic::new(
                    DiagnosticCode::NAME_DUPLICATE,
                    DiagnosticStage::Parse,
                    "top-level block is declared more than once",
                    source_span(*keyword_span),
                )
                .with_label(DiagnosticLabel::new(first, "first declaration")),
            );
        }
        index = next;
    }
    None
}

fn block_extent(tokens: &[SpannedToken], start: usize) -> Option<(SourceSpan, usize)> {
    if !matches!(
        tokens.get(start + 1),
        Some((Token::Punctuation(Punctuation::LeftBrace), _))
    ) {
        return None;
    }
    let mut depth = 0usize;
    for (index, (token, span)) in tokens.iter().enumerate().skip(start + 1) {
        match token {
            Token::Punctuation(Punctuation::LeftBrace) => depth += 1,
            Token::Punctuation(Punctuation::RightBrace) => {
                depth -= 1;
                if depth == 0 {
                    return Some((SourceSpan::new(tokens[start].1.start, span.end), index + 1));
                }
            }
            _ => {}
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BlockKind {
    Format,
    TempoMap,
    Definitions,
    Collections,
    Misplaced,
}

fn block_kind(token: &Token) -> Option<BlockKind> {
    match token {
        Token::Keyword(Keyword::Format) => Some(BlockKind::Format),
        Token::Keyword(Keyword::TempoMap) => Some(BlockKind::TempoMap),
        Token::Keyword(Keyword::Definitions) => Some(BlockKind::Definitions),
        Token::Keyword(Keyword::Collections) => Some(BlockKind::Collections),
        Token::Identifier(name) if matches!(name.as_str(), "templates" | "metadata") => {
            Some(BlockKind::Misplaced)
        }
        Token::Keyword(
            Keyword::Meta
            | Keyword::Contributors
            | Keyword::Credits
            | Keyword::Resources
            | Keyword::Artwork
            | Keyword::Sync
            | Keyword::Lines
            | Keyword::Render
            | Keyword::Extensions
            | Keyword::Preserve
            | Keyword::Tracks,
        ) => Some(BlockKind::Misplaced),
        _ => None,
    }
}

fn invalid_profile_span(tokens: &[SpannedToken]) -> Option<SourceSpan> {
    let format = tokens
        .iter()
        .position(|(token, _)| *token == Token::Keyword(Keyword::Format))?;
    let profile = tokens[format..]
        .iter()
        .position(|(token, _)| *token == Token::Keyword(Keyword::Profile))?
        + format;
    let (_, span) = tokens.get(profile + 2)?;
    (!matches!(
        tokens[profile + 2].0,
        Token::Keyword(
            Keyword::Fragment
                | Keyword::Chart
                | Keyword::Playable
                | Keyword::Renderable
                | Keyword::Publishable
        )
    ))
    .then(|| source_span(*span))
}

fn extra_format_token_span(tokens: &[SpannedToken]) -> Option<SourceSpan> {
    let format = tokens
        .iter()
        .position(|(token, _)| *token == Token::Keyword(Keyword::Format))?;
    let (_, block_end) = block_extent(tokens, format)?;
    let semicolon = tokens[format..block_end]
        .iter()
        .position(|(token, _)| *token == Token::Punctuation(Punctuation::Semicolon))?
        + format;
    tokens[semicolon + 1..block_end - 1]
        .first()
        .map(|(_, span)| source_span(*span))
}
