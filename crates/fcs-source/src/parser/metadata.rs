use chumsky::{input::ValueInput, prelude::*};

use crate::ast::{
    ArtworkBlock, ContributorDeclaration, ContributorsBlock, CreditEntry, CreditsBlock, MetaBlock,
    ResourceDeclaration, ResourceKind, ResourcesBlock, SchemaField, SourceSpan, SyncBlock,
};

use super::{
    definitions::identifier_with_span,
    entities::schema_fields_parser,
    input::{ChumskySpan, ParserExtra, source_span},
    token::{Keyword, Punctuation, Token},
};

pub(super) fn meta_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, MetaBlock, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    schema_top_level_block_parser(Token::Keyword(Keyword::Meta), MetaBlock::new)
}

pub(super) fn contributors_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, ContributorsBlock, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    top_level_keyword(Keyword::Contributors)
        .then(
            contributor_parser()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(left_brace()), just(right_brace())),
        )
        .map_with(|(keyword_span, people), extra| ContributorsBlock {
            people,
            keyword_span,
            span: source_span(extra.span()),
        })
}

fn contributor_parser<'tokens, I>()
-> impl Parser<'tokens, I, ContributorDeclaration, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Person))
        .ignore_then(identifier_with_span())
        .then(schema_fields_parser())
        .map_with(
            |((name, name_span), fields), extra| ContributorDeclaration {
                name,
                name_span,
                fields,
                span: source_span(extra.span()),
            },
        )
}

pub(super) fn credits_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, CreditsBlock, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    top_level_keyword(Keyword::Credits)
        .then(
            just(Token::Keyword(Keyword::Credit))
                .ignore_then(schema_fields_parser())
                .map_with(|fields, extra| CreditEntry {
                    fields,
                    span: source_span(extra.span()),
                })
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(left_brace()), just(right_brace())),
        )
        .map_with(|(keyword_span, entries), extra| CreditsBlock {
            entries,
            keyword_span,
            span: source_span(extra.span()),
        })
}

pub(super) fn resources_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, ResourcesBlock, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    top_level_keyword(Keyword::Resources)
        .then(
            resource_parser()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(left_brace()), just(right_brace())),
        )
        .map_with(|(keyword_span, resources), extra| ResourcesBlock {
            resources,
            keyword_span,
            span: source_span(extra.span()),
        })
}

fn resource_parser<'tokens, I>()
-> impl Parser<'tokens, I, ResourceDeclaration, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    resource_kind_parser()
        .then(identifier_with_span())
        .then(schema_fields_parser())
        .map_with(
            |(((kind, kind_span), (name, name_span)), fields), extra| ResourceDeclaration {
                kind,
                kind_span,
                name,
                name_span,
                fields,
                span: source_span(extra.span()),
            },
        )
}

fn resource_kind_parser<'tokens, I>()
-> impl Parser<'tokens, I, (ResourceKind, SourceSpan), ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    select! {
        Token::Keyword(Keyword::Audio) => ResourceKind::Audio,
        Token::Keyword(Keyword::Image) => ResourceKind::Image,
        Token::Keyword(Keyword::Font) => ResourceKind::Font,
        Token::Keyword(Keyword::Texture) => ResourceKind::Texture,
        Token::Keyword(Keyword::Path) => ResourceKind::Path,
        Token::Keyword(Keyword::Shader) => ResourceKind::Shader,
        Token::Keyword(Keyword::Binary) => ResourceKind::Binary,
    }
    .map_with(|kind, extra| (kind, source_span(extra.span())))
}

pub(super) fn artwork_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, ArtworkBlock, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    schema_top_level_block_parser(Token::Keyword(Keyword::Artwork), ArtworkBlock::new)
}

pub(super) fn sync_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, SyncBlock, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    schema_top_level_block_parser(Token::Keyword(Keyword::Sync), SyncBlock::new)
}

fn top_level_keyword<'tokens, I>(
    keyword: Keyword,
) -> impl Parser<'tokens, I, SourceSpan, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(keyword)).map_with(|_, extra| source_span(extra.span()))
}

fn schema_top_level_block_parser<'tokens, I, T, F>(
    keyword: Token,
    constructor: F,
) -> impl Parser<'tokens, I, T, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
    T: Clone,
    F: Fn(Vec<SchemaField>, SourceSpan, SourceSpan) -> T + Clone,
{
    just(keyword)
        .map_with(|_, extra| source_span(extra.span()))
        .then(schema_fields_parser())
        .map_with(move |(keyword_span, fields), extra| {
            constructor(fields, keyword_span, source_span(extra.span()))
        })
}

impl MetaBlock {
    fn new(fields: Vec<SchemaField>, keyword_span: SourceSpan, span: SourceSpan) -> Self {
        Self {
            fields,
            span,
            keyword_span,
        }
    }
}

impl ArtworkBlock {
    fn new(fields: Vec<SchemaField>, keyword_span: SourceSpan, span: SourceSpan) -> Self {
        Self {
            fields,
            span,
            keyword_span,
        }
    }
}

impl SyncBlock {
    fn new(fields: Vec<SchemaField>, keyword_span: SourceSpan, span: SourceSpan) -> Self {
        Self {
            fields,
            span,
            keyword_span,
        }
    }
}

fn left_brace() -> Token {
    Token::Punctuation(Punctuation::LeftBrace)
}

fn right_brace() -> Token {
    Token::Punctuation(Punctuation::RightBrace)
}
