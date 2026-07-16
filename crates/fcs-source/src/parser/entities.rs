use chumsky::{input::ValueInput, prelude::*};

use crate::ast::{
    CollectionBlock, CollectionItem, CollectionsBlock, EntityConstructor, EntityExpression,
    EntityField, FieldPath, Generator, GeneratorItem, GeneratorOwner, NoteVariant, SchemaField,
    SchemaValue, SourceEntityConstructor, SourceEntityConstructorKind, SourceExpression,
    SourceRange, SourceSpan, Type, WithExpression,
};

use super::{
    MISPLACED_GENERATOR_ERROR, NESTED_GENERATOR_ERROR,
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
        .map_with(|(collection_name, mut items), extra| {
            assign_collection_generator_owners(&mut items, &collection_name);
            CollectionBlock {
                collection_name,
                items,
                span: source_span(extra.span()),
            }
        })
}

fn assign_collection_generator_owners(items: &mut [CollectionItem], collection_name: &str) {
    for item in items {
        match item {
            CollectionItem::Generator(generator) => {
                *generator.owner = GeneratorOwner::Collection {
                    name: collection_name.to_owned(),
                };
            }
            CollectionItem::Conditional {
                then_items,
                else_items,
                ..
            } => {
                assign_collection_generator_owners(then_items, collection_name);
                assign_collection_generator_owners(else_items, collection_name);
            }
            CollectionItem::Constructor(_) | CollectionItem::Expression(_) => {}
        }
    }
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
            generator_parser(GeneratorOwner::Collection {
                name: String::new(),
            })
            .map(CollectionItem::Generator),
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

pub(super) fn generator_parser<'tokens, I>(
    owner: GeneratorOwner,
) -> impl Parser<'tokens, I, Generator, ParserExtra<'tokens>> + Clone
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
            move |(
                ((((((variable, variable_span), variable_type), start), inclusive_end), end), step),
                body,
            ),
                  extra| {
                let range_span = SourceSpan::new(start.span().start, step.span().end);
                Generator {
                    owner: Box::new(owner.clone()),
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
        let nested_generator = just(Token::Keyword(Keyword::Generate))
            .try_map(|_, span| Err(chumsky::error::Rich::custom(span, NESTED_GENERATOR_ERROR)));
        choice((
            nested_generator,
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
        .or(source_entity_constructor_parser().map(EntityExpression::SourceConstructor))
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

/// Parses the Appendix B `entityExpression` production without accepting a
/// value-only literal/name as a template or generator entity result.
pub(super) fn strict_entity_expression_parser<'tokens, I>()
-> impl Parser<'tokens, I, EntityExpression, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    let base = constructor_parser()
        .map(EntityExpression::Constructor)
        .or(source_entity_constructor_parser().map(EntityExpression::SourceConstructor))
        .or(entity_call_parser());
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

fn entity_call_parser<'tokens, I>()
-> impl Parser<'tokens, I, EntityExpression, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    identifier_with_span()
        .then(
            expression_parser()
                .separated_by(just(Token::Punctuation(Punctuation::Comma)))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(
                    just(Token::Punctuation(Punctuation::LeftParenthesis)),
                    just(Token::Punctuation(Punctuation::RightParenthesis)),
                ),
        )
        .map_with(|((name, name_span), arguments), extra| {
            EntityExpression::Source(SourceExpression::Call {
                callee: Box::new(SourceExpression::Name {
                    name,
                    span: name_span,
                }),
                arguments,
                span: source_span(extra.span()),
            })
        })
}

fn source_entity_constructor_parser<'tokens, I>()
-> impl Parser<'tokens, I, SourceEntityConstructor, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    select! {
        Token::Keyword(Keyword::RenderNode) => SourceEntityConstructorKind::RenderNode,
        Token::Keyword(Keyword::Segment) => SourceEntityConstructorKind::Segment,
        Token::Keyword(Keyword::Keyframe) => SourceEntityConstructorKind::Keyframe,
    }
    .then(schema_fields_parser())
    .map_with(|(kind, fields), extra| SourceEntityConstructor {
        kind,
        fields,
        span: source_span(extra.span()),
    })
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

pub(super) fn schema_fields_parser<'tokens, I>()
-> impl Parser<'tokens, I, Vec<SchemaField>, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    schema_field_parser()
        .repeated()
        .collect()
        .delimited_by(just(left_brace()), just(right_brace()))
}

fn schema_field_parser<'tokens, I>()
-> impl Parser<'tokens, I, SchemaField, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    field_path_parser()
        .then_ignore(just(colon()))
        .then(schema_value_parser())
        .then_ignore(just(semicolon()))
        .map_with(|(path, value), extra| SchemaField {
            path,
            value,
            span: source_span(extra.span()),
        })
}

fn schema_value_parser<'tokens, I>()
-> impl Parser<'tokens, I, SchemaValue, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    cubic_bezier_value_parser()
        .map(|(values, span)| SchemaValue::CubicBezier { values, span })
        .or(schema_interval_parser())
        .or(expression_parser().map(SchemaValue::Expression))
}

fn schema_interval_parser<'tokens, I>()
-> impl Parser<'tokens, I, SchemaValue, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(left_bracket())
        .ignore_then(expression_parser())
        .then_ignore(just(comma()))
        .then(expression_parser())
        .then_ignore(just(right_parenthesis()))
        .map_with(|(start, end), extra| SchemaValue::Interval {
            start,
            end,
            span: source_span(extra.span()),
        })
}

pub(super) fn cubic_bezier_value_parser<'tokens, I>()
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

pub(super) fn field_parser<'tokens, I>()
-> impl Parser<'tokens, I, EntityField, ParserExtra<'tokens>> + Clone
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
        .then(
            just(Token::Keyword(Keyword::Generate))
                .try_map(|_, span| {
                    Err(chumsky::error::Rich::custom(
                        span,
                        MISPLACED_GENERATOR_ERROR,
                    ))
                })
                .or(expression_parser()),
        )
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

pub(super) fn field_path_parser<'tokens, I>()
-> impl Parser<'tokens, I, FieldPath, ParserExtra<'tokens>> + Clone
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
}

pub(super) fn field_segment_parser<'tokens, I>()
-> impl Parser<'tokens, I, String, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    select! {
        Token::Identifier(segment) => segment,
        Token::Keyword(keyword) => keyword.as_str().to_owned(),
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
fn left_parenthesis() -> Token {
    Token::Punctuation(Punctuation::LeftParenthesis)
}
fn right_parenthesis() -> Token {
    Token::Punctuation(Punctuation::RightParenthesis)
}
fn left_bracket() -> Token {
    Token::Punctuation(Punctuation::LeftBracket)
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
