use chumsky::{error::RichReason, input::Input as _, prelude::*};

use crate::ast::{
    Document, DocumentProfile, FeatureList, FormatBlock, FormatFeature, FormatField, FormatProfile,
    ProfileFeature, RenderBlock, SourceBlock, SourceElement, SourceGroup, SourceSpan,
    TempoMapBlock, TopLevelBlock,
};
use crate::diagnostic::{
    Diagnostic, DiagnosticCode, DiagnosticLabel, DiagnosticStage, ParseOutput,
};

use super::{
    MISPLACED_GENERATOR_ERROR, ParseLimits,
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

    let end_span = ChumskySpan::new((), source.len()..source.len());
    let input = tokens.map(end_span, |(token, span)| (token, span));
    let (parsed, errors) = document_parser()
        .then_ignore(end())
        .parse(input)
        .into_output_errors();
    if !errors.is_empty() {
        let diagnostics = errors
            .into_iter()
            .map(parse_error_diagnostic)
            .collect::<Vec<_>>();
        return ParseOutput::new(None, diagnostics);
    }

    let parsed = parsed.expect("document parser produces output when it has no errors");
    finish_document(source.len(), parsed)
}

fn parse_error_diagnostic(error: Rich<'_, Token, ChumskySpan>) -> Diagnostic {
    let span = source_span(*error.span());
    let (code, message) = match error.reason() {
        RichReason::Custom(kind) if kind == super::NESTED_GENERATOR_ERROR => (
            DiagnosticCode::COMPILE_TIME_NESTED_GENERATOR,
            "nested generator is not allowed in a generator body",
        ),
        RichReason::Custom(kind) if kind == MISPLACED_GENERATOR_ERROR => (
            DiagnosticCode::COMPILE_TIME_MISPLACED_GENERATOR,
            "generator is not allowed in this owner",
        ),
        _ => (
            DiagnosticCode::SYNTAX_INVALID_TOKEN,
            "invalid document syntax",
        ),
    };
    Diagnostic::new(code, DiagnosticStage::Parse, message, span)
}

fn finish_document(source_len: usize, parsed: ParsedDocument) -> ParseOutput<Document> {
    let format_index = parsed
        .items
        .iter()
        .position(|item| matches!(item, DocumentItem::Format(_)));

    let Some(format_index) = format_index else {
        let span = parsed
            .items
            .first()
            .map(DocumentItem::span)
            .unwrap_or_else(|| SourceSpan::new(source_len, source_len));
        return ParseOutput::new(
            None,
            vec![Diagnostic::new(
                DiagnosticCode::PROFILE_REQUIREMENT_MISSING,
                DiagnosticStage::Parse,
                "document requires a format block",
                span,
            )],
        );
    };

    if format_index > 0 {
        let first = &parsed.items[0];
        let diagnostic = match first {
            DocumentItem::Block(block) => Diagnostic::new(
                DiagnosticCode::SYNTAX_MISPLACED_BLOCK,
                DiagnosticStage::Parse,
                "format must immediately follow the source header",
                block.keyword_span(),
            ),
            DocumentItem::Unknown { kind, span } => diagnostic_for_unknown(*kind, *span),
            DocumentItem::Format(_) => unreachable!("format index is not zero"),
        };
        return ParseOutput::new(None, vec![diagnostic]);
    }

    let mut format: Option<FormatBlock> = None;
    let mut top_level_blocks = Vec::new();
    let mut diagnostics = Vec::new();
    let mut first_blocks = Vec::<(crate::ast::TopLevelBlockKind, SourceSpan)>::new();

    for item in parsed.items {
        match item {
            DocumentItem::Format(candidate) => {
                if let Some(first) = format.as_ref() {
                    diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::NAME_DUPLICATE,
                            DiagnosticStage::Parse,
                            "format block is declared more than once",
                            candidate.keyword_span,
                        )
                        .with_label(DiagnosticLabel::new(
                            first.keyword_span,
                            "first declaration",
                        )),
                    );
                } else {
                    format = Some(candidate);
                }
            }
            DocumentItem::Block(block) => {
                let kind = block.kind();
                if let Some((_, first_span)) = first_blocks.iter().find(|(seen, _)| *seen == kind) {
                    diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::NAME_DUPLICATE,
                            DiagnosticStage::Parse,
                            "top-level block is declared more than once",
                            block.keyword_span(),
                        )
                        .with_label(DiagnosticLabel::new(*first_span, "first declaration")),
                    );
                } else {
                    first_blocks.push((kind, block.span()));
                }
                top_level_blocks.push(block);
            }
            DocumentItem::Unknown { kind, span } => {
                diagnostics.push(diagnostic_for_unknown(kind, span));
            }
        }
    }

    let Some(format) = format else {
        unreachable!("format index was present");
    };
    validate_format(&format, &mut diagnostics);
    if !diagnostics.is_empty() {
        return ParseOutput::new(None, diagnostics);
    }

    ParseOutput::new(
        Some(Document::new(
            parsed.source_version,
            format,
            top_level_blocks,
        )),
        Vec::new(),
    )
}

fn validate_format(format: &FormatBlock, diagnostics: &mut Vec<Diagnostic>) {
    let mut profile = None;
    let mut features = None;
    for field in &format.fields {
        match field {
            FormatField::Profile(value) => {
                if let Some(first) = profile {
                    diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::NAME_DUPLICATE,
                            DiagnosticStage::Parse,
                            "format.profile is declared more than once",
                            value.name_span,
                        )
                        .with_label(DiagnosticLabel::new(first, "first declaration")),
                    );
                } else {
                    profile = Some(value.name_span);
                }
            }
            FormatField::Features(value) => {
                if let Some(first) = features {
                    diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::NAME_DUPLICATE,
                            DiagnosticStage::Parse,
                            "format.features is declared more than once",
                            value.name_span,
                        )
                        .with_label(DiagnosticLabel::new(first, "first declaration")),
                    );
                } else {
                    features = Some(value.name_span);
                }
            }
        }
    }
    if profile.is_none() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::PROFILE_REQUIREMENT_MISSING,
            DiagnosticStage::Parse,
            "format.profile is required",
            format.close_span,
        ));
    }
}

fn diagnostic_for_unknown(kind: UnknownKind, span: SourceSpan) -> Diagnostic {
    let (code, message) = match kind {
        UnknownKind::Unknown => (
            DiagnosticCode::SYNTAX_INVALID_TOKEN,
            "unknown top-level block",
        ),
        UnknownKind::Misplaced => (
            DiagnosticCode::SYNTAX_MISPLACED_BLOCK,
            "block is not valid in the current top-level context",
        ),
        UnknownKind::MisplacedGenerator => (
            DiagnosticCode::COMPILE_TIME_MISPLACED_GENERATOR,
            "generator is not allowed in this owner",
        ),
        UnknownKind::Trailing => (
            DiagnosticCode::SYNTAX_TRAILING_INPUT,
            "trailing non-trivia input",
        ),
    };
    Diagnostic::new(code, DiagnosticStage::Parse, message, span)
}

fn document_parser<'tokens, I>()
-> impl Parser<'tokens, I, ParsedDocument, ParserExtra<'tokens>> + Clone
where
    I: chumsky::input::ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    header_parser()
        .then(top_level_item_parser().repeated().collect::<Vec<_>>())
        .map(|(source_version, items)| ParsedDocument {
            source_version,
            items,
        })
}

fn top_level_item_parser<'tokens, I>()
-> impl Parser<'tokens, I, DocumentItem, ParserExtra<'tokens>> + Clone
where
    I: chumsky::input::ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    choice((
        format_parser().map(DocumentItem::Format),
        raw_block_parser(Keyword::Meta)
            .map(|block| DocumentItem::Block(TopLevelBlock::Meta(block))),
        raw_block_parser(Keyword::Contributors)
            .map(|block| DocumentItem::Block(TopLevelBlock::Contributors(block))),
        raw_block_parser(Keyword::Credits)
            .map(|block| DocumentItem::Block(TopLevelBlock::Credits(block))),
        raw_block_parser(Keyword::Resources)
            .map(|block| DocumentItem::Block(TopLevelBlock::Resources(block))),
        raw_block_parser(Keyword::Artwork)
            .map(|block| DocumentItem::Block(TopLevelBlock::Artwork(block))),
        raw_block_parser(Keyword::Sync)
            .map(|block| DocumentItem::Block(TopLevelBlock::Sync(block))),
        definitions_block_parser()
            .map(|block| DocumentItem::Block(TopLevelBlock::Definitions(block))),
        tempo_map_block_parser().map_with(|(map, span), _| {
            DocumentItem::Block(TopLevelBlock::TempoMap(TempoMapBlock {
                map,
                span,
                keyword_span: SourceSpan::new(span.start, span.start + "tempoMap".len()),
            }))
        }),
        raw_block_parser(Keyword::Lines)
            .map(|block| DocumentItem::Block(TopLevelBlock::Lines(block))),
        collections_block_parser()
            .map(|block| DocumentItem::Block(TopLevelBlock::Collections(block))),
        render_block_parser().map(|block| DocumentItem::Block(TopLevelBlock::Render(block))),
        raw_block_parser(Keyword::Extensions)
            .map(|block| DocumentItem::Block(TopLevelBlock::Extensions(block))),
        raw_block_parser(Keyword::Preserve)
            .map(|block| DocumentItem::Block(TopLevelBlock::Preserve(block))),
        misplaced_item_parser(),
        unknown_item_parser(),
        trailing_item_parser(),
    ))
}

fn format_parser<'tokens, I>() -> impl Parser<'tokens, I, FormatBlock, ParserExtra<'tokens>> + Clone
where
    I: chumsky::input::ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Format))
        .map_with(|_, extra| source_span(extra.span()))
        .then(
            just(left_brace())
                .ignore_then(format_field_parser().repeated().collect::<Vec<_>>())
                .then(just(right_brace()).map_with(|_, extra| source_span(extra.span()))),
        )
        .map_with(|(keyword_span, (fields, close_span)), extra| {
            let profile = fields
                .iter()
                .find_map(|field| match field {
                    FormatField::Profile(profile) => Some(*profile),
                    FormatField::Features(_) => None,
                })
                .unwrap_or(FormatProfile {
                    value: DocumentProfile::Fragment,
                    span: close_span,
                    name_span: close_span,
                });
            let features = fields.iter().find_map(|field| match field {
                FormatField::Features(features) => Some(features.clone()),
                FormatField::Profile(_) => None,
            });
            FormatBlock {
                profile,
                features,
                fields,
                span: source_span(extra.span()),
                close_span,
                keyword_span,
            }
        })
}

fn format_field_parser<'tokens, I>()
-> impl Parser<'tokens, I, FormatField, ParserExtra<'tokens>> + Clone
where
    I: chumsky::input::ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    let profile = just(Token::Keyword(Keyword::Profile))
        .map_with(|_, extra| source_span(extra.span()))
        .then_ignore(just(colon()))
        .then(
            select! {
                Token::Keyword(Keyword::Fragment) => DocumentProfile::Fragment,
                Token::Keyword(Keyword::Chart) => DocumentProfile::Chart,
                Token::Keyword(Keyword::Playable) => DocumentProfile::Playable,
                Token::Keyword(Keyword::Renderable) => DocumentProfile::Renderable,
                Token::Keyword(Keyword::Publishable) => DocumentProfile::Publishable,
            }
            .map_with(|value, extra| (value, source_span(extra.span()))),
        )
        .then_ignore(just(semicolon()))
        .map(|(name_span, (value, span))| {
            FormatField::Profile(FormatProfile {
                value,
                span,
                name_span,
            })
        });

    let feature = select! {
        Token::Keyword(Keyword::Playable) => ProfileFeature::Playable,
        Token::Keyword(Keyword::Renderable) => ProfileFeature::Renderable,
    }
    .map_with(|value, extra| FormatFeature {
        value,
        span: source_span(extra.span()),
    });
    let feature_array = feature
        .separated_by(just(comma()))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(left_bracket()), just(right_bracket()))
        .map_with(|features, extra| FeatureList {
            features,
            span: source_span(extra.span()),
            name_span: SourceSpan::new(0, 0),
        });
    let features = just(Token::Keyword(Keyword::Features))
        .map_with(|_, extra| source_span(extra.span()))
        .then_ignore(just(colon()))
        .then(feature_array)
        .then_ignore(just(semicolon()))
        .map(|(name_span, mut value)| {
            value.name_span = name_span;
            FormatField::Features(value)
        });
    choice((profile, features))
}

fn raw_block_parser<'tokens, I>(
    keyword: Keyword,
) -> impl Parser<'tokens, I, SourceBlock, ParserExtra<'tokens>> + Clone
where
    I: chumsky::input::ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(keyword))
        .map_with(|_, extra| source_span(extra.span()))
        .then(source_group_parser(true))
        .map_with(|(keyword_span, body), extra| SourceBlock {
            body,
            span: source_span(extra.span()),
            keyword_span,
        })
}

fn render_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, RenderBlock, ParserExtra<'tokens>> + Clone
where
    I: chumsky::input::ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Render))
        .map_with(|_, extra| source_span(extra.span()))
        .then_ignore(just(Token::Keyword(Keyword::Profile)))
        .then(select! { Token::Semver(version) => version })
        .then(source_group_parser(false))
        .map_with(|((keyword_span, version), payload), extra| RenderBlock {
            version,
            payload,
            span: source_span(extra.span()),
            keyword_span,
        })
}

fn source_group_parser<'tokens, I>(
    allow_half_open: bool,
) -> impl Parser<'tokens, I, SourceGroup, ParserExtra<'tokens>> + Clone
where
    I: chumsky::input::ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    let element = source_element_parser(allow_half_open);
    just(left_brace())
        .map_with(|_, extra| source_span(extra.span()))
        .then(element.repeated().collect::<Vec<_>>())
        .then(just(right_brace()).map_with(|_, extra| source_span(extra.span())))
        .map(|((open_span, elements), close_span)| SourceGroup {
            open_span,
            close_span,
            span: SourceSpan::new(open_span.start, close_span.end),
            elements,
        })
}

fn source_element_parser<'tokens, I>(
    allow_half_open: bool,
) -> impl Parser<'tokens, I, SourceElement, ParserExtra<'tokens>> + Clone
where
    I: chumsky::input::ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    recursive(|group| {
        let atom = any()
            .filter(|token: &Token| !is_delimiter(token))
            .map_with(|_, extra| SourceElement::Atom(source_span(extra.span())));
        let braces = just(left_brace())
            .map_with(|_, extra| source_span(extra.span()))
            .then(group.clone().repeated().collect::<Vec<_>>())
            .then(just(right_brace()).map_with(|_, extra| source_span(extra.span())))
            .map(|((open_span, elements), close_span)| {
                SourceElement::Group(SourceGroup {
                    open_span,
                    close_span,
                    span: SourceSpan::new(open_span.start, close_span.end),
                    elements,
                })
            });
        let parentheses = just(left_parenthesis())
            .map_with(|_, extra| source_span(extra.span()))
            .then(group.clone().repeated().collect::<Vec<_>>())
            .then(just(right_parenthesis()).map_with(|_, extra| source_span(extra.span())))
            .map(|((open_span, elements), close_span)| {
                SourceElement::Group(SourceGroup {
                    open_span,
                    close_span,
                    span: SourceSpan::new(open_span.start, close_span.end),
                    elements,
                })
            });
        let brackets = just(left_bracket())
            .map_with(|_, extra| source_span(extra.span()))
            .then(group.clone().repeated().collect::<Vec<_>>())
            .then(just(right_bracket()).map_with(|_, extra| source_span(extra.span())))
            .map(|((open_span, elements), close_span)| {
                SourceElement::Group(SourceGroup {
                    open_span,
                    close_span,
                    span: SourceSpan::new(open_span.start, close_span.end),
                    elements,
                })
            });
        let half_open = just(left_bracket())
            .map_with(|_, extra| source_span(extra.span()))
            .then(group.clone().repeated().collect::<Vec<_>>())
            .then(just(right_parenthesis()).map_with(|_, extra| source_span(extra.span())))
            .filter(move |_| allow_half_open)
            .map(|((open_span, elements), close_span)| {
                SourceElement::Group(SourceGroup {
                    open_span,
                    close_span,
                    span: SourceSpan::new(open_span.start, close_span.end),
                    elements,
                })
            });
        choice((half_open, braces, parentheses, brackets, atom)).boxed()
    })
}

fn misplaced_item_parser<'tokens, I>()
-> impl Parser<'tokens, I, DocumentItem, ParserExtra<'tokens>> + Clone
where
    I: chumsky::input::ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    any()
        .filter(is_misplaced_start)
        .map_with(|_, extra| source_span(extra.span()))
        .then(source_group_parser(true).or_not())
        .map(|(span, _)| DocumentItem::Unknown {
            kind: UnknownKind::Misplaced,
            span,
        })
}

fn unknown_item_parser<'tokens, I>()
-> impl Parser<'tokens, I, DocumentItem, ParserExtra<'tokens>> + Clone
where
    I: chumsky::input::ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    any()
        .filter(is_unknown_start)
        .map_with(|token, extra| (token, source_span(extra.span())))
        .then(source_group_parser(true).or_not())
        .map(|((token, span), body)| DocumentItem::Unknown {
            kind: if matches!(token, Token::Keyword(Keyword::Generate)) {
                UnknownKind::MisplacedGenerator
            } else if body.is_some() {
                UnknownKind::Unknown
            } else {
                UnknownKind::Trailing
            },
            span,
        })
}

fn trailing_item_parser<'tokens, I>()
-> impl Parser<'tokens, I, DocumentItem, ParserExtra<'tokens>> + Clone
where
    I: chumsky::input::ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    any()
        .filter(|token| {
            !is_top_level_keyword_token(token)
                && !is_misplaced_start(token)
                && !is_unknown_start(token)
        })
        .map_with(|_, extra| DocumentItem::Unknown {
            kind: UnknownKind::Trailing,
            span: source_span(extra.span()),
        })
}

fn is_delimiter(token: &Token) -> bool {
    matches!(
        token,
        Token::Punctuation(
            Punctuation::LeftBrace
                | Punctuation::RightBrace
                | Punctuation::LeftParenthesis
                | Punctuation::RightParenthesis
                | Punctuation::LeftBracket
                | Punctuation::RightBracket
        )
    )
}

fn is_misplaced_start(token: &Token) -> bool {
    matches!(
        token,
        Token::Identifier(name) if matches!(name.as_str(), "templates" | "metadata")
    ) || matches!(
        token,
        Token::Keyword(
            Keyword::Template
                | Keyword::Const
                | Keyword::Fn
                | Keyword::Person
                | Keyword::Credit
                | Keyword::Line
                | Keyword::Track
                | Keyword::Segment
                | Keyword::Keyframe
                | Keyword::Point
                | Keyword::Extension
                | Keyword::Source
                | Keyword::Payload
                | Keyword::Profile
                | Keyword::Features
                | Keyword::Notes
                | Keyword::Judgelines
        )
    )
}

fn is_unknown_start(token: &Token) -> bool {
    match token {
        Token::Identifier(_) => !is_misplaced_start(token),
        Token::Keyword(keyword) => !is_misplaced_start(token) && !is_top_level_keyword(*keyword),
        _ => false,
    }
}

fn is_top_level_keyword_token(token: &Token) -> bool {
    matches!(token, Token::Keyword(keyword) if is_top_level_keyword(*keyword))
}

const fn is_top_level_keyword(keyword: Keyword) -> bool {
    matches!(
        keyword,
        Keyword::Format
            | Keyword::Meta
            | Keyword::Contributors
            | Keyword::Credits
            | Keyword::Resources
            | Keyword::Artwork
            | Keyword::Sync
            | Keyword::Definitions
            | Keyword::TempoMap
            | Keyword::Lines
            | Keyword::Collections
            | Keyword::Render
            | Keyword::Extensions
            | Keyword::Preserve
    )
}

fn left_brace() -> Token {
    Token::Punctuation(Punctuation::LeftBrace)
}
fn right_brace() -> Token {
    Token::Punctuation(Punctuation::RightBrace)
}
fn left_parenthesis() -> Token {
    Token::Punctuation(Punctuation::LeftParenthesis)
}
fn right_parenthesis() -> Token {
    Token::Punctuation(Punctuation::RightParenthesis)
}
fn left_bracket() -> Token {
    Token::Punctuation(Punctuation::LeftBracket)
}
fn right_bracket() -> Token {
    Token::Punctuation(Punctuation::RightBracket)
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

#[derive(Debug)]
struct ParsedDocument {
    source_version: crate::version::Version,
    items: Vec<DocumentItem>,
}

#[derive(Debug)]
enum DocumentItem {
    Format(FormatBlock),
    Block(TopLevelBlock),
    Unknown { kind: UnknownKind, span: SourceSpan },
}

impl DocumentItem {
    const fn span(&self) -> SourceSpan {
        match self {
            Self::Format(format) => format.span,
            Self::Block(block) => block.span(),
            Self::Unknown { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum UnknownKind {
    Unknown,
    Misplaced,
    MisplacedGenerator,
    Trailing,
}

trait TopLevelBlockExt {
    fn keyword_span(&self) -> SourceSpan;
}

impl TopLevelBlockExt for TopLevelBlock {
    fn keyword_span(&self) -> SourceSpan {
        match self {
            TopLevelBlock::Meta(block)
            | TopLevelBlock::Contributors(block)
            | TopLevelBlock::Credits(block)
            | TopLevelBlock::Resources(block)
            | TopLevelBlock::Artwork(block)
            | TopLevelBlock::Sync(block)
            | TopLevelBlock::Lines(block)
            | TopLevelBlock::Extensions(block)
            | TopLevelBlock::Preserve(block) => block.keyword_span,
            TopLevelBlock::Definitions(block) => {
                SourceSpan::new(block.span.start, block.span.start + "definitions".len())
            }
            TopLevelBlock::TempoMap(block) => block.keyword_span,
            TopLevelBlock::Collections(block) => {
                SourceSpan::new(block.span.start, block.span.start + "collections".len())
            }
            TopLevelBlock::Render(block) => block.keyword_span,
        }
    }
}
