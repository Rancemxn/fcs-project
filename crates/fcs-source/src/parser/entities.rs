use chumsky::{input::ValueInput, prelude::*};

use crate::ast::{
    CollectionBlock, CollectionItem, CollectionsBlock, EntityConstructor, EntityExpression,
    EntityField, FieldPath, Generator, GeneratorItem, NoteVariant, SourceRange, SourceSpan, Type,
    WithExpression,
};

use super::{
    definitions::{identifier_with_span, let_statement_parser},
    expression::expression_parser,
    input::{ChumskySpan, ParserExtra, source_span},
    token::{Keyword, Punctuation, Token},
};

pub(super) fn collections_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, CollectionsBlock, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Collections))
        .ignore_then(
            collection_parser()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(left_brace()), just(right_brace())),
        )
        .map_with(|collections, extra| CollectionsBlock {
            collections,
            span: source_span(extra.span()),
        })
}

fn collection_parser<'tokens, I>()
-> impl Parser<'tokens, I, CollectionBlock, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    collection_name_parser()
        .then(
            collection_item_parser()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(left_brace()), just(right_brace())),
        )
        .map_with(|(collection_name, items), extra| CollectionBlock {
            collection_name,
            items,
            span: source_span(extra.span()),
        })
}

fn collection_item_parser<'tokens, I>()
-> impl Parser<'tokens, I, CollectionItem, ParserExtra<'tokens>> + Clone
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
            .map_with(
                |((condition, then_items), else_items), extra| CollectionItem::Conditional {
                    condition,
                    then_items,
                    else_items: else_items.unwrap_or_default(),
                    span: source_span(extra.span()),
                },
            );
        choice((
            generator_parser().map(CollectionItem::Generator),
            conditional,
            entity_expression_parser()
                .then_ignore(just(semicolon()))
                .map(|expression| match expression {
                    EntityExpression::Constructor(constructor) => {
                        CollectionItem::Constructor(constructor)
                    }
                    expression => CollectionItem::Expression(expression),
                }),
        ))
    })
}

fn generator_parser<'tokens, I>() -> impl Parser<'tokens, I, Generator, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Generate))
        .ignore_then(identifier_with_span())
        .then_ignore(just(colon()))
        .then(choice((
            just(Token::Keyword(Keyword::Int)).to(Type::Int),
            just(Token::Keyword(Keyword::Beat)).to(Type::Beat),
        )))
        .then_ignore(just(Token::Keyword(Keyword::In)))
        .then(expression_parser())
        .then(choice((
            just(Token::Punctuation(Punctuation::RangeExclusive)).to(false),
            just(Token::Punctuation(Punctuation::RangeInclusive)).to(true),
        )))
        .then(expression_parser())
        .then_ignore(just(Token::Keyword(Keyword::Step)))
        .then(expression_parser())
        .then(
            generator_item_parser()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(left_brace()), just(right_brace())),
        )
        .map_with(
            |(
                ((((((variable, variable_span), variable_type), start), inclusive_end), end), step),
                body,
            ),
             extra| {
                let range_span = SourceSpan::new(start.span().start, step.span().end);
                Generator {
                    variable,
                    variable_span,
                    variable_type,
                    range: SourceRange {
                        start,
                        end,
                        step,
                        inclusive_end,
                        span: range_span,
                    },
                    body,
                    span: source_span(extra.span()),
                }
            },
        )
}

fn generator_item_parser<'tokens, I>()
-> impl Parser<'tokens, I, GeneratorItem, ParserExtra<'tokens>> + Clone
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
            .map_with(
                |((condition, then_items), else_items), extra| GeneratorItem::Conditional {
                    condition,
                    then_items,
                    else_items: else_items.unwrap_or_default(),
                    span: source_span(extra.span()),
                },
            );
        choice((
            let_statement_parser().map(GeneratorItem::Let),
            just(Token::Keyword(Keyword::Emit))
                .ignore_then(entity_expression_parser())
                .then_ignore(just(semicolon()))
                .map(GeneratorItem::Emit),
            conditional,
        ))
    })
}

pub(super) fn entity_expression_parser<'tokens, I>()
-> impl Parser<'tokens, I, EntityExpression, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    let base = constructor_parser()
        .map(EntityExpression::Constructor)
        .or(expression_parser().map(EntityExpression::Source));
    base.foldl_with(
        just(Token::Keyword(Keyword::With))
            .ignore_then(fields_parser())
            .repeated(),
        |base, fields, extra| {
            EntityExpression::With(WithExpression {
                span: SourceSpan::new(base.span().start, source_span(extra.span()).end),
                base: Box::new(base),
                fields,
            })
        },
    )
}

fn constructor_parser<'tokens, I>()
-> impl Parser<'tokens, I, EntityConstructor, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    constructor_kind_parser()
        .then(fields_parser())
        .map_with(|(kind, fields), extra| EntityConstructor {
            entity_type: kind.entity_type(),
            note_variant: kind.note_variant(),
            fields,
            span: source_span(extra.span()),
        })
}

fn constructor_kind_parser<'tokens, I>()
-> impl Parser<'tokens, I, ConstructorKind, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    select! {
        Token::Keyword(Keyword::Tap) => ConstructorKind::Note(NoteVariant::Tap),
        Token::Keyword(Keyword::Hold) => ConstructorKind::Note(NoteVariant::Hold),
        Token::Keyword(Keyword::Flick) => ConstructorKind::Note(NoteVariant::Flick),
        Token::Keyword(Keyword::Drag) => ConstructorKind::Note(NoteVariant::Drag),
        Token::Keyword(Keyword::LineType) => ConstructorKind::Line,
    }
}

fn fields_parser<'tokens, I>()
-> impl Parser<'tokens, I, Vec<EntityField>, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    field_parser()
        .repeated()
        .collect()
        .delimited_by(just(left_brace()), just(right_brace()))
}

fn field_parser<'tokens, I>() -> impl Parser<'tokens, I, EntityField, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    field_segment_parser()
        .separated_by(just(Token::Punctuation(Punctuation::Dot)))
        .at_least(1)
        .collect::<Vec<_>>()
        .map_with(|segments, extra| FieldPath {
            segments,
            span: source_span(extra.span()),
        })
        .then_ignore(just(colon()))
        .then(expression_parser())
        .then_ignore(just(semicolon()))
        .map_with(|(path, value), extra| EntityField {
            path,
            value,
            span: source_span(extra.span()),
        })
}

fn collection_name_parser<'tokens, I>()
-> impl Parser<'tokens, I, String, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    select! {
        Token::Identifier(name) => name,
        Token::Keyword(Keyword::Notes) => "notes".to_owned(),
        Token::Keyword(Keyword::Judgelines) => "judgelines".to_owned(),
    }
}

fn field_segment_parser<'tokens, I>()
-> impl Parser<'tokens, I, String, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    select! {
        Token::Identifier(segment) => segment,
        Token::Keyword(Keyword::Time) => "time".to_owned(),
        Token::Keyword(Keyword::Color) => "color".to_owned(),
        Token::Keyword(Keyword::Render) => "render".to_owned(),
    }
}

#[derive(Clone, Copy)]
enum ConstructorKind {
    Note(NoteVariant),
    Line,
}

impl ConstructorKind {
    const fn entity_type(self) -> Type {
        match self {
            Self::Note(_) => Type::Note,
            Self::Line => Type::Line,
        }
    }

    const fn note_variant(self) -> Option<NoteVariant> {
        match self {
            Self::Note(variant) => Some(variant),
            Self::Line => None,
        }
    }
}

fn left_brace() -> Token {
    Token::Punctuation(Punctuation::LeftBrace)
}
fn right_brace() -> Token {
    Token::Punctuation(Punctuation::RightBrace)
}
fn colon() -> Token {
    Token::Punctuation(Punctuation::Colon)
}
fn semicolon() -> Token {
    Token::Punctuation(Punctuation::Semicolon)
}
