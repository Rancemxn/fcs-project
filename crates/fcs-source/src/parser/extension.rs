use chumsky::{input::ValueInput, prelude::*};

use crate::{
    ast::{
        ExtensionDeclaration, ExtensionHeader, ExtensionRequirement, ExtensionsBlock,
        OrderedObject, PreserveBlock, PreserveItem, PreservePayload, PreserveSource, SourceLiteral,
        SourceObjectEntry, SourceSpan,
    },
    version::Version,
};

use super::{
    entities::schema_fields_parser,
    expression::expression_parser,
    input::{ChumskySpan, ParserExtra, source_span},
    token::{Keyword, Punctuation, Token},
};

pub(super) fn extensions_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, ExtensionsBlock, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Extensions))
        .map_with(|_, extra| source_span(extra.span()))
        .then(
            extension_declaration_parser()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(left_brace()), just(right_brace())),
        )
        .map_with(|(keyword_span, declarations), extra| ExtensionsBlock {
            declarations,
            keyword_span,
            span: source_span(extra.span()),
        })
}

fn extension_declaration_parser<'tokens, I>()
-> impl Parser<'tokens, I, ExtensionDeclaration, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    extension_header_parser()
        .then(extension_requirement_parser())
        .then(ordered_object_parser())
        .map_with(
            |((header, (requirement, requirement_span)), payload), extra| ExtensionDeclaration {
                header,
                requirement,
                requirement_span,
                payload,
                span: source_span(extra.span()),
            },
        )
}

pub(super) fn extension_header_parser<'tokens, I>()
-> impl Parser<'tokens, I, ExtensionHeader, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Extension))
        .map_with(|_, extra| source_span(extra.span()))
        .then(
            just(left_parenthesis())
                .ignore_then(namespace_parser())
                .then_ignore(just(comma()))
                .then(version_parser())
                .then_ignore(just(right_parenthesis())),
        )
        .map_with(
            |(extension_span, ((namespace, namespace_span), (version, version_span))), extra| {
                ExtensionHeader {
                    namespace,
                    namespace_span,
                    version,
                    version_span,
                    span: SourceSpan::new(extension_span.start, source_span(extra.span()).end),
                }
            },
        )
}

fn namespace_parser<'tokens, I>()
-> impl Parser<'tokens, I, (String, SourceSpan), ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    select! {
        Token::Literal(SourceLiteral::String(namespace)) => namespace,
    }
    .map_with(|namespace, extra| (namespace, source_span(extra.span())))
}

fn version_parser<'tokens, I>()
-> impl Parser<'tokens, I, (Version, SourceSpan), ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    select! { Token::Semver(version) => version }
        .map_with(|version, extra| (version, source_span(extra.span())))
}

fn extension_requirement_parser<'tokens, I>()
-> impl Parser<'tokens, I, (ExtensionRequirement, SourceSpan), ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    choice((
        just(Token::Keyword(Keyword::Required)).to(ExtensionRequirement::Required),
        just(Token::Keyword(Keyword::Optional)).to(ExtensionRequirement::Optional),
    ))
    .map_with(|requirement, extra| (requirement, source_span(extra.span())))
}

fn ordered_object_parser<'tokens, I>()
-> impl Parser<'tokens, I, OrderedObject, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    ordered_object_entry_parser()
        .separated_by(just(comma()))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(left_brace()), just(right_brace()))
        .map_with(|entries, extra| OrderedObject {
            entries,
            span: source_span(extra.span()),
        })
}

fn ordered_object_entry_parser<'tokens, I>()
-> impl Parser<'tokens, I, SourceObjectEntry, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    select! {
        Token::Literal(SourceLiteral::String(key)) => key,
    }
    .map_with(|key, extra| (key, source_span(extra.span())))
    .then_ignore(just(colon()))
    .then(expression_parser())
    .map_with(|((key, key_span), value), extra| SourceObjectEntry {
        key,
        key_span,
        value,
        span: source_span(extra.span()),
    })
}

pub(super) fn preserve_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, PreserveBlock, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Preserve))
        .map_with(|_, extra| source_span(extra.span()))
        .then(
            preserve_item_parser()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(left_brace()), just(right_brace())),
        )
        .map_with(|(keyword_span, items), extra| PreserveBlock {
            items,
            keyword_span,
            span: source_span(extra.span()),
        })
}

fn preserve_item_parser<'tokens, I>()
-> impl Parser<'tokens, I, PreserveItem, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    choice((
        preserve_source_parser().map(PreserveItem::Source),
        preserve_payload_parser().map(PreserveItem::Payload),
    ))
}

fn preserve_source_parser<'tokens, I>()
-> impl Parser<'tokens, I, PreserveSource, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Source))
        .then(schema_fields_parser())
        .map_with(|(_, fields), extra| PreserveSource {
            fields,
            span: source_span(extra.span()),
        })
}

fn preserve_payload_parser<'tokens, I>()
-> impl Parser<'tokens, I, PreservePayload, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Payload))
        .then_ignore(just(colon()))
        .then(extension_header_parser())
        .then(ordered_object_parser())
        .then_ignore(just(semicolon()))
        .map_with(|((_, header), payload), extra| PreservePayload {
            header,
            payload,
            span: source_span(extra.span()),
        })
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

fn colon() -> Token {
    Token::Punctuation(Punctuation::Colon)
}

fn comma() -> Token {
    Token::Punctuation(Punctuation::Comma)
}

fn semicolon() -> Token {
    Token::Punctuation(Punctuation::Semicolon)
}
