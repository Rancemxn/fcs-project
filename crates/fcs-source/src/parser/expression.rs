use chumsky::{
    error::Rich,
    input::{Input as _, ValueInput},
    prelude::*,
};

use crate::{
    ast::{
        BinaryOperator, SourceChooseArm, SourceExpression, SourceLiteral, SourceObjectEntry,
        SourceSpan, SourceType, SourceTypeKind, UnaryOperator,
    },
    diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage, ParseOutput},
};

use super::{
    ParseLimits,
    input::{ChumskySpan, ParserExtra, SpannedToken, source_span},
    lexer::lex,
    token::{Keyword, Punctuation, Token},
};

/// Parses one complete compile-time expression.
pub fn parse_expression(input: &str) -> ParseOutput<SourceExpression> {
    parse_expression_with_limits(input, ParseLimits::default())
}

/// Parses one complete compile-time expression with explicit resource limits.
pub fn parse_expression_with_limits<L: Into<ParseLimits>>(
    input: &str,
    limits: L,
) -> ParseOutput<SourceExpression> {
    let limits = limits.into();
    match lex(input, limits) {
        Ok(tokens) => {
            if let Some((span, observed)) = expression_limit_span(&tokens, limits.max_nesting_depth)
            {
                return resource_limit_output(
                    span,
                    "max_nesting_depth",
                    limits.max_nesting_depth,
                    observed,
                );
            }
            parse_expression_tokens(input, &tokens)
        }
        Err(diagnostics) => ParseOutput::new(None, diagnostics),
    }
}

/// Parses one complete compile-time type.
pub fn parse_type(input: &str) -> ParseOutput<SourceType> {
    parse_type_with_limits(input, ParseLimits::default())
}

/// Parses one complete compile-time type with explicit resource limits.
pub fn parse_type_with_limits<L: Into<ParseLimits>>(
    input: &str,
    limits: L,
) -> ParseOutput<SourceType> {
    let limits = limits.into();
    match lex(input, limits) {
        Ok(tokens) => {
            if let Some((span, observed)) = type_limit_span(&tokens, limits.max_nesting_depth) {
                return resource_limit_output(
                    span,
                    "max_nesting_depth",
                    limits.max_nesting_depth,
                    observed,
                );
            }
            parse_type_tokens(input, &tokens)
        }
        Err(diagnostics) => ParseOutput::new(None, diagnostics),
    }
}

pub(super) fn parse_expression_tokens(
    source: &str,
    tokens: &[SpannedToken],
) -> ParseOutput<SourceExpression> {
    parse_expression_tokens_inline(source.len(), tokens)
}

fn parse_expression_tokens_inline(
    source_len: usize,
    tokens: &[SpannedToken],
) -> ParseOutput<SourceExpression> {
    let end_span = ChumskySpan::new((), source_len..source_len);
    let input = tokens.map(end_span, |(token, span)| (token, span));
    let (output, errors) = expression_parser()
        .then_ignore(end())
        .parse(input)
        .into_output_errors();
    parse_output(output, errors)
}

pub(super) fn parse_type_tokens(source: &str, tokens: &[SpannedToken]) -> ParseOutput<SourceType> {
    parse_type_tokens_inline(source.len(), tokens)
}

fn parse_type_tokens_inline(source_len: usize, tokens: &[SpannedToken]) -> ParseOutput<SourceType> {
    let end_span = ChumskySpan::new((), source_len..source_len);
    let input = tokens.map(end_span, |(token, span)| (token, span));
    let (output, errors) = type_parser()
        .then_ignore(end())
        .parse(input)
        .into_output_errors();
    parse_output(output, errors)
}

fn parse_output<T>(output: Option<T>, errors: Vec<Rich<'_, Token, ChumskySpan>>) -> ParseOutput<T> {
    ParseOutput::new(output, errors.into_iter().map(parser_diagnostic).collect())
}

fn parser_diagnostic(error: Rich<'_, Token, ChumskySpan>) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::SYNTAX_INVALID_TOKEN,
        DiagnosticStage::Parse,
        "invalid expression or type syntax",
        source_span(*error.span()),
    )
}

fn resource_limit_output<T>(
    span: SourceSpan,
    kind: &str,
    limit: usize,
    observed: usize,
) -> ParseOutput<T> {
    ParseOutput::new(
        None,
        vec![
            Diagnostic::new(
                DiagnosticCode::RESOURCE_LIMIT_EXCEEDED,
                DiagnosticStage::Parse,
                "parser resource limit exceeded",
                span,
            )
            .with_budget(kind, limit, observed),
        ],
    )
}

pub(super) fn expression_parser<'tokens, I>()
-> impl Parser<'tokens, I, SourceExpression, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    recursive(|expression| {
        let literal = select! { Token::Literal(literal) => literal }
            .map_with(|literal, extra| SourceExpression::Literal {
                literal,
                span: source_span(extra.span()),
            })
            .labelled("literal");
        let null =
            just(Token::Keyword(Keyword::Null)).map_with(|_, extra| SourceExpression::Literal {
                literal: SourceLiteral::Null,
                span: source_span(extra.span()),
            });
        let identifier = select! { Token::Identifier(identifier) => identifier }
            .map_with(|name, extra| SourceExpression::Name {
                name,
                span: source_span(extra.span()),
            })
            .labelled("identifier");

        let grouped = expression
            .clone()
            .delimited_by(
                just(Token::Punctuation(Punctuation::LeftParenthesis)),
                just(Token::Punctuation(Punctuation::RightParenthesis)),
            )
            .map_with(|expression, extra| with_span(expression, source_span(extra.span())))
            .labelled("parenthesized expression");

        let vec2 = just(Token::Keyword(Keyword::Vec2))
            .ignore_then(
                expression
                    .clone()
                    .then_ignore(just(Token::Punctuation(Punctuation::Comma)))
                    .then(expression.clone())
                    .delimited_by(
                        just(Token::Punctuation(Punctuation::LeftParenthesis)),
                        just(Token::Punctuation(Punctuation::RightParenthesis)),
                    )
                    .labelled("vec2 arguments"),
            )
            .map_with(|(x, y), extra| SourceExpression::Vec2 {
                x: Box::new(x),
                y: Box::new(y),
                span: source_span(extra.span()),
            });

        let reference = just(Token::Punctuation(Punctuation::At))
            .ignore_then(select! { Token::Identifier(identifier) => identifier })
            .map_with(|name, extra| SourceExpression::Reference {
                name,
                span: source_span(extra.span()),
            })
            .labelled("reference");

        let array = expression
            .clone()
            .separated_by(just(Token::Punctuation(Punctuation::Comma)))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(
                just(Token::Punctuation(Punctuation::LeftBracket)),
                just(Token::Punctuation(Punctuation::RightBracket)),
            )
            .map_with(|elements, extra| SourceExpression::Array {
                elements,
                span: source_span(extra.span()),
            })
            .labelled("array");

        let object_entry = select! {
            Token::Literal(SourceLiteral::String(key)) => key,
        }
        .map_with(|key, extra| (key, source_span(extra.span())))
        .then_ignore(just(Token::Punctuation(Punctuation::Colon)))
        .then(expression.clone())
        .map_with(|((key, key_span), value), extra| SourceObjectEntry {
            key,
            key_span,
            value,
            span: source_span(extra.span()),
        });
        let object = object_entry
            .separated_by(just(Token::Punctuation(Punctuation::Comma)))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(
                just(Token::Punctuation(Punctuation::LeftBrace)),
                just(Token::Punctuation(Punctuation::RightBrace)),
            )
            .map_with(|entries, extra| SourceExpression::Object {
                entries,
                span: source_span(extra.span()),
            })
            .labelled("object");

        let choose_arm = just(Token::Keyword(Keyword::When))
            .ignore_then(expression.clone())
            .then_ignore(just(Token::Punctuation(Punctuation::FatArrow)))
            .then(expression.clone())
            .then_ignore(just(Token::Punctuation(Punctuation::Semicolon)))
            .map_with(|(condition, value), extra| SourceChooseArm {
                condition,
                value,
                span: source_span(extra.span()),
            });
        let choose_body = choose_arm
            .repeated()
            .at_least(1)
            .collect::<Vec<_>>()
            .then(
                just(Token::Keyword(Keyword::Else))
                    .ignore_then(just(Token::Punctuation(Punctuation::FatArrow)))
                    .ignore_then(expression.clone())
                    .then_ignore(just(Token::Punctuation(Punctuation::Semicolon))),
            )
            .delimited_by(
                just(Token::Punctuation(Punctuation::LeftBrace)),
                just(Token::Punctuation(Punctuation::RightBrace)),
            );
        let choose = just(Token::Keyword(Keyword::Choose))
            .then(choose_body)
            .map_with(|(_, (arms, else_value)), extra| SourceExpression::Choose {
                arms,
                else_value: Box::new(else_value),
                span: source_span(extra.span()),
            })
            .labelled("choose expression");

        let primary = choose
            .or(literal)
            .or(null)
            .or(reference)
            .or(array)
            .or(object)
            .or(vec2)
            .or(identifier)
            .or(grouped)
            .boxed();

        let arguments = expression
            .clone()
            .separated_by(just(Token::Punctuation(Punctuation::Comma)))
            .allow_trailing()
            .collect::<Vec<_>>();
        let call = arguments
            .delimited_by(
                just(Token::Punctuation(Punctuation::LeftParenthesis)),
                just(Token::Punctuation(Punctuation::RightParenthesis)),
            )
            .map_with(|arguments, extra| Postfix::Call(arguments, source_span(extra.span())))
            .labelled("argument list");
        let field = just(Token::Punctuation(Punctuation::Dot))
            .ignore_then(field_name_parser())
            .map_with(|field, extra| Postfix::Field(field, source_span(extra.span())))
            .labelled("field access");
        let index = expression
            .clone()
            .delimited_by(
                just(Token::Punctuation(Punctuation::LeftBracket)),
                just(Token::Punctuation(Punctuation::RightBracket)),
            )
            .map_with(|index, extra| Postfix::Index(index, source_span(extra.span())))
            .labelled("index expression");
        let postfix = primary
            .foldl_with(
                call.or(field).or(index).repeated(),
                |base, suffix, _| match suffix {
                    Postfix::Call(arguments, suffix_span) => SourceExpression::Call {
                        span: SourceSpan::new(base.span().start, suffix_span.end),
                        callee: Box::new(base),
                        arguments,
                    },
                    Postfix::Field(field, suffix_span) => SourceExpression::FieldAccess {
                        span: SourceSpan::new(base.span().start, suffix_span.end),
                        base: Box::new(base),
                        field,
                    },
                    Postfix::Index(index, suffix_span) => SourceExpression::Index {
                        span: SourceSpan::new(base.span().start, suffix_span.end),
                        base: Box::new(base),
                        index: Box::new(index),
                    },
                },
            )
            .boxed();

        let unary_operator = choice((
            just(Token::Punctuation(Punctuation::Minus)).to(UnaryOperator::Negate),
            just(Token::Punctuation(Punctuation::Bang)).to(UnaryOperator::Not),
        ))
        .map_with(|operator, extra| (operator, source_span(extra.span())));
        let unary = unary_operator
            .repeated()
            .collect::<Vec<_>>()
            .then(postfix)
            .map(|(operators, operand)| {
                operators
                    .into_iter()
                    .rev()
                    .fold(operand, |operand, (operator, operator_span)| {
                        let span = SourceSpan::new(operator_span.start, operand.span().end);
                        SourceExpression::Unary {
                            operator,
                            operand: Box::new(operand),
                            span,
                        }
                    })
            })
            .boxed();

        let power = unary
            .clone()
            .then(
                just(Token::Punctuation(Punctuation::Power))
                    .ignore_then(unary)
                    .repeated()
                    .collect::<Vec<_>>(),
            )
            .map(|(first, mut remaining)| {
                if remaining.is_empty() {
                    return first;
                }
                let mut right = remaining.pop().expect("checked non-empty");
                while let Some(left) = remaining.pop() {
                    right = binary(left, BinaryOperator::Power, right);
                }
                binary(first, BinaryOperator::Power, right)
            })
            .boxed();

        let product = power
            .clone()
            .foldl_with(
                choice((
                    just(Token::Punctuation(Punctuation::Star)).to(BinaryOperator::Multiply),
                    just(Token::Punctuation(Punctuation::Slash)).to(BinaryOperator::Divide),
                    just(Token::Punctuation(Punctuation::Percent)).to(BinaryOperator::Remainder),
                ))
                .then(power)
                .repeated(),
                |left, (operator, right), _| binary(left, operator, right),
            )
            .boxed();
        let sum = product
            .clone()
            .foldl_with(
                choice((
                    just(Token::Punctuation(Punctuation::Plus)).to(BinaryOperator::Add),
                    just(Token::Punctuation(Punctuation::Minus)).to(BinaryOperator::Subtract),
                ))
                .then(product)
                .repeated(),
                |left, (operator, right), _| binary(left, operator, right),
            )
            .boxed();

        let ordering = comparison_chain(
            sum.clone(),
            choice((
                just(Token::Punctuation(Punctuation::LessThan)).to(BinaryOperator::LessThan),
                just(Token::Punctuation(Punctuation::LessThanOrEqual))
                    .to(BinaryOperator::LessThanOrEqual),
                just(Token::Punctuation(Punctuation::GreaterThan)).to(BinaryOperator::GreaterThan),
                just(Token::Punctuation(Punctuation::GreaterThanOrEqual))
                    .to(BinaryOperator::GreaterThanOrEqual),
            )),
        )
        .boxed();
        let equality = ordering
            .clone()
            .foldl_with(
                choice((
                    just(Token::Punctuation(Punctuation::EqualEqual)).to(BinaryOperator::Equal),
                    just(Token::Punctuation(Punctuation::BangEqual)).to(BinaryOperator::NotEqual),
                ))
                .then(ordering)
                .repeated(),
                |left, (operator, right), _| binary(left, operator, right),
            )
            .boxed();
        let logical_and = equality
            .clone()
            .foldl_with(
                just(Token::Punctuation(Punctuation::AndAnd))
                    .to(BinaryOperator::And)
                    .then(equality)
                    .repeated(),
                |left, (operator, right), _| binary(left, operator, right),
            )
            .boxed();
        logical_and
            .clone()
            .foldl_with(
                just(Token::Punctuation(Punctuation::OrOr))
                    .to(BinaryOperator::Or)
                    .then(logical_and)
                    .repeated(),
                |left, (operator, right), _| binary(left, operator, right),
            )
            .labelled("expression")
            .as_context()
    })
}

fn comparison_chain<'tokens, I, P, O>(
    operand: P,
    operator: O,
) -> impl Parser<'tokens, I, SourceExpression, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
    P: Parser<'tokens, I, SourceExpression, ParserExtra<'tokens>> + Clone,
    O: Parser<'tokens, I, BinaryOperator, ParserExtra<'tokens>> + Clone,
{
    operand
        .clone()
        .then(operator.then(operand).repeated().collect::<Vec<_>>())
        .map(|(left, pairs)| {
            let mut expression = left;
            let mut previous = None;
            for (operator, right) in pairs {
                let comparison_left = previous.clone().unwrap_or_else(|| expression.clone());
                let comparison = binary(comparison_left, operator, right.clone());
                expression = if previous.is_some() {
                    binary(expression, BinaryOperator::And, comparison)
                } else {
                    comparison
                };
                previous = Some(right);
            }
            expression
        })
}

pub(super) fn type_parser<'tokens, I>()
-> impl Parser<'tokens, I, SourceType, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    recursive(|source_type| {
        let scalar = select! {
            Token::Keyword(Keyword::Bool) => SourceTypeKind::Bool,
            Token::Keyword(Keyword::Int) => SourceTypeKind::Int,
            Token::Keyword(Keyword::Float) => SourceTypeKind::Float,
            Token::Keyword(Keyword::String) => SourceTypeKind::String,
            Token::Keyword(Keyword::Time) => SourceTypeKind::Time,
            Token::Keyword(Keyword::Beat) => SourceTypeKind::Beat,
            Token::Keyword(Keyword::Length) => SourceTypeKind::Length,
            Token::Keyword(Keyword::Angle) => SourceTypeKind::Angle,
            Token::Keyword(Keyword::Color) => SourceTypeKind::Color,
        }
        .map_with(|kind, extra| SourceType::new(kind, source_span(extra.span())));

        let entity = select! {
            Token::Keyword(Keyword::Note) => SourceTypeKind::Note,
            Token::Keyword(Keyword::LineType) => SourceTypeKind::Line,
            Token::Keyword(Keyword::RenderNode) => SourceTypeKind::RenderNode,
        }
        .map_with(|kind, extra| SourceType::new(kind, source_span(extra.span())));

        let generic = |keyword, constructor: fn(Box<SourceType>) -> SourceTypeKind| {
            just(Token::Keyword(keyword))
                .ignore_then(source_type.clone().delimited_by(
                    just(Token::Punctuation(Punctuation::LessThan)),
                    just(Token::Punctuation(Punctuation::GreaterThan)),
                ))
                .map_with(move |element, extra| {
                    SourceType::new(constructor(Box::new(element)), source_span(extra.span()))
                })
        };

        let vector = just(Token::Keyword(Keyword::Vec2))
            .ignore_then(scalar.delimited_by(
                just(Token::Punctuation(Punctuation::LessThan)),
                just(Token::Punctuation(Punctuation::GreaterThan)),
            ))
            .map_with(|element, extra| {
                SourceType::new(
                    SourceTypeKind::Vec2(Box::new(element)),
                    source_span(extra.span()),
                )
            });

        choice((
            vector,
            generic(Keyword::Array, SourceTypeKind::Array),
            generic(Keyword::TrackType, SourceTypeKind::Track),
            generic(Keyword::TrackSegment, SourceTypeKind::TrackSegment),
            generic(Keyword::KeyframeType, SourceTypeKind::Keyframe),
            scalar,
            entity,
        ))
        .labelled("type")
        .as_context()
    })
}

#[derive(Debug)]
enum Postfix {
    Call(Vec<SourceExpression>, SourceSpan),
    Field(String, SourceSpan),
    Index(SourceExpression, SourceSpan),
}

fn field_name_parser<'tokens, I>() -> impl Parser<'tokens, I, String, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    select! {
        Token::Identifier(identifier) => identifier,
        Token::Keyword(keyword) => keyword.as_str().to_owned(),
    }
}

fn binary(
    left: SourceExpression,
    operator: BinaryOperator,
    right: SourceExpression,
) -> SourceExpression {
    let span = SourceSpan::new(left.span().start, right.span().end);
    SourceExpression::Binary {
        left: Box::new(left),
        operator,
        right: Box::new(right),
        span,
    }
}

fn with_span(expression: SourceExpression, span: SourceSpan) -> SourceExpression {
    match expression {
        SourceExpression::Literal { literal, .. } => SourceExpression::Literal { literal, span },
        SourceExpression::Array { elements, .. } => SourceExpression::Array { elements, span },
        SourceExpression::Object { entries, .. } => SourceExpression::Object { entries, span },
        SourceExpression::Reference { name, .. } => SourceExpression::Reference { name, span },
        SourceExpression::Name { name, .. } => SourceExpression::Name { name, span },
        SourceExpression::Unary {
            operator, operand, ..
        } => SourceExpression::Unary {
            operator,
            operand,
            span,
        },
        SourceExpression::Binary {
            left,
            operator,
            right,
            ..
        } => SourceExpression::Binary {
            left,
            operator,
            right,
            span,
        },
        SourceExpression::Call {
            callee, arguments, ..
        } => SourceExpression::Call {
            callee,
            arguments,
            span,
        },
        SourceExpression::FieldAccess { base, field, .. } => {
            SourceExpression::FieldAccess { base, field, span }
        }
        SourceExpression::Index { base, index, .. } => {
            SourceExpression::Index { base, index, span }
        }
        SourceExpression::Choose {
            arms, else_value, ..
        } => SourceExpression::Choose {
            arms,
            else_value,
            span,
        },
        SourceExpression::Vec2 { x, y, .. } => SourceExpression::Vec2 { x, y, span },
    }
}

fn expression_limit_span(tokens: &[SpannedToken], maximum: usize) -> Option<(SourceSpan, usize)> {
    let mut unary_run = 0usize;
    let mut group_depth = 0usize;
    let mut power_depth = 0usize;
    for (token, span) in tokens {
        match token {
            Token::Punctuation(Punctuation::Minus | Punctuation::Bang) => {
                unary_run += 1;
            }
            Token::Punctuation(
                Punctuation::LeftParenthesis | Punctuation::LeftBracket | Punctuation::LeftBrace,
            ) => {
                group_depth += 1;
            }
            Token::Punctuation(
                Punctuation::RightParenthesis | Punctuation::RightBracket | Punctuation::RightBrace,
            ) => {
                group_depth = group_depth.saturating_sub(1);
            }
            Token::Punctuation(Punctuation::Power) => {
                power_depth += 1;
                unary_run = 0;
            }
            Token::Literal(_) | Token::Identifier(_) | Token::Keyword(Keyword::Vec2) => {
                unary_run = 0;
            }
            Token::Punctuation(_) => {
                unary_run = 0;
                power_depth = 0;
            }
            _ => unary_run = 0,
        }
        let observed = group_depth
            .saturating_add(unary_run)
            .saturating_add(power_depth);
        if observed > maximum {
            return Some((source_span(*span), observed));
        }
    }
    None
}

fn type_limit_span(tokens: &[SpannedToken], maximum: usize) -> Option<(SourceSpan, usize)> {
    let mut depth = 0usize;
    for (token, span) in tokens {
        match token {
            Token::Punctuation(Punctuation::LessThan) => {
                depth += 1;
                if depth > maximum {
                    return Some((source_span(*span), depth));
                }
            }
            Token::Punctuation(Punctuation::GreaterThan) => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}
