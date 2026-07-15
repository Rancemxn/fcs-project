use chumsky::{
    error::Rich,
    input::{Input as _, ValueInput},
    prelude::*,
};

use crate::{
    ast::{BinaryOperator, SourceExpression, SourceSpan, Type, UnaryOperator},
    diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage, ParseOutput},
};

use super::{
    ParseError, ParseLimits,
    input::{ChumskySpan, SpannedToken, source_span},
    lexer::lex,
    token::{Keyword, Punctuation, Token},
};

type ParserExtra<'tokens> = extra::Err<Rich<'tokens, Token, ChumskySpan>>;

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
            if let Some(span) = expression_limit_span(&tokens, limits.max_nesting_depth) {
                return resource_limit_output(span);
            }
            parse_expression_tokens(input, &tokens)
        }
        Err(diagnostics) => ParseOutput::new(None, diagnostics),
    }
}

/// Parses one complete compile-time type.
pub fn parse_type(input: &str) -> ParseOutput<Type> {
    parse_type_with_limits(input, ParseLimits::default())
}

/// Parses one complete compile-time type with explicit resource limits.
pub fn parse_type_with_limits<L: Into<ParseLimits>>(input: &str, limits: L) -> ParseOutput<Type> {
    let limits = limits.into();
    match lex(input, limits) {
        Ok(tokens) => {
            if let Some(span) = type_limit_span(&tokens, limits.max_nesting_depth) {
                return resource_limit_output(span);
            }
            parse_type_tokens(input, &tokens)
        }
        Err(diagnostics) => ParseOutput::new(None, diagnostics),
    }
}

/// Compatibility boundary for the handwritten document parser. Task 8 will pass its original
/// token stream to [`parse_expression_tokens`] instead of lexing a substring here.
pub(super) fn parse_expression_at(
    input: &str,
    byte_offset: usize,
) -> Result<SourceExpression, ParseError> {
    parse_expression_inner(input, ParseLimits::default())
        .map(|expression| shift_expression(expression, byte_offset))
}

pub(super) fn parse_expression_inner(
    input: &str,
    limits: ParseLimits,
) -> Result<SourceExpression, ParseError> {
    into_parse_error(parse_expression_with_limits(input, limits))
}

pub(super) fn parse_type_inner(input: &str, limits: ParseLimits) -> Result<Type, ParseError> {
    into_parse_error(parse_type_with_limits(input, limits))
}

pub(super) fn parse_expression_tokens(
    source: &str,
    tokens: &[SpannedToken],
) -> ParseOutput<SourceExpression> {
    if expression_limit_span(tokens, 64).is_some() {
        parse_with_large_stack(source.len(), tokens, parse_expression_tokens_inline)
    } else {
        parse_expression_tokens_inline(source.len(), tokens)
    }
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

pub(super) fn parse_type_tokens(source: &str, tokens: &[SpannedToken]) -> ParseOutput<Type> {
    if type_limit_span(tokens, 64).is_some() {
        parse_with_large_stack(source.len(), tokens, parse_type_tokens_inline)
    } else {
        parse_type_tokens_inline(source.len(), tokens)
    }
}

fn parse_type_tokens_inline(source_len: usize, tokens: &[SpannedToken]) -> ParseOutput<Type> {
    let end_span = ChumskySpan::new((), source_len..source_len);
    let input = tokens.map(end_span, |(token, span)| (token, span));
    let (output, errors) = type_parser()
        .then_ignore(end())
        .parse(input)
        .into_output_errors();
    parse_output(output, errors)
}

fn parse_with_large_stack<T: Send + 'static>(
    source_len: usize,
    tokens: &[SpannedToken],
    parse: fn(usize, &[SpannedToken]) -> ParseOutput<T>,
) -> ParseOutput<T> {
    let tokens = tokens.to_vec();
    match std::thread::Builder::new()
        .name("fcs-source-parser".to_owned())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || parse(source_len, &tokens))
        .and_then(|thread| {
            thread
                .join()
                .map_err(|_| std::io::Error::other("parser panicked"))
        }) {
        Ok(output) => output,
        Err(_) => ParseOutput::new(
            None,
            vec![Diagnostic::new(
                DiagnosticCode::RESOURCE_LIMIT_EXCEEDED,
                DiagnosticStage::Parse,
                "parser exhausted its execution stack",
                SourceSpan::new(0, source_len),
            )],
        ),
    }
}

fn into_parse_error<T>(output: ParseOutput<T>) -> Result<T, ParseError> {
    output
        .into_result()
        .map_err(|mut diagnostics| ParseError::Diagnostic(diagnostics.remove(0)))
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

fn resource_limit_output<T>(span: SourceSpan) -> ParseOutput<T> {
    ParseOutput::new(
        None,
        vec![Diagnostic::new(
            DiagnosticCode::RESOURCE_LIMIT_EXCEEDED,
            DiagnosticStage::Parse,
            "parser resource limit exceeded",
            span,
        )],
    )
}

fn expression_parser<'tokens, I>()
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

        let primary = literal.or(vec2).or(identifier).or(grouped).boxed();

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
            .ignore_then(select! { Token::Identifier(identifier) => identifier })
            .map_with(|field, extra| Postfix::Field(field, source_span(extra.span())))
            .labelled("field access");
        let postfix =
            primary.foldl_with(call.or(field).repeated(), |base, suffix, _| match suffix {
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
            });

        let unary = recursive(|unary| {
            choice((
                just(Token::Punctuation(Punctuation::Minus)).to(UnaryOperator::Negate),
                just(Token::Punctuation(Punctuation::Bang)).to(UnaryOperator::Not),
            ))
            .then(unary)
            .map_with(|(operator, operand), extra| SourceExpression::Unary {
                operator,
                span: source_span(extra.span()),
                operand: Box::new(operand),
            })
            .or(postfix.clone())
        });

        let power = recursive(|power| {
            unary
                .clone()
                .then(
                    just(Token::Punctuation(Punctuation::Power))
                        .ignore_then(power)
                        .or_not(),
                )
                .map(|(left, right)| match right {
                    Some(right) => binary(left, BinaryOperator::Power, right),
                    None => left,
                })
        });

        let product = power.clone().foldl_with(
            choice((
                just(Token::Punctuation(Punctuation::Star)).to(BinaryOperator::Multiply),
                just(Token::Punctuation(Punctuation::Slash)).to(BinaryOperator::Divide),
                just(Token::Punctuation(Punctuation::Percent)).to(BinaryOperator::Remainder),
            ))
            .then(power)
            .repeated(),
            |left, (operator, right), _| binary(left, operator, right),
        );
        let sum = product.clone().foldl_with(
            choice((
                just(Token::Punctuation(Punctuation::Plus)).to(BinaryOperator::Add),
                just(Token::Punctuation(Punctuation::Minus)).to(BinaryOperator::Subtract),
            ))
            .then(product)
            .repeated(),
            |left, (operator, right), _| binary(left, operator, right),
        );

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
        );
        let equality = ordering.clone().foldl_with(
            choice((
                just(Token::Punctuation(Punctuation::EqualEqual)).to(BinaryOperator::Equal),
                just(Token::Punctuation(Punctuation::BangEqual)).to(BinaryOperator::NotEqual),
            ))
            .then(ordering)
            .repeated(),
            |left, (operator, right), _| binary(left, operator, right),
        );
        let logical_and = equality.clone().foldl_with(
            just(Token::Punctuation(Punctuation::AndAnd))
                .to(BinaryOperator::And)
                .then(equality)
                .repeated(),
            |left, (operator, right), _| binary(left, operator, right),
        );
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

fn type_parser<'tokens, I>() -> impl Parser<'tokens, I, Type, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    recursive(|ty| {
        let scalar = select! {
            Token::Keyword(Keyword::Bool) => Type::Bool,
            Token::Keyword(Keyword::Int) => Type::Int,
            Token::Keyword(Keyword::Float) => Type::Float,
            Token::Keyword(Keyword::String) => Type::String,
            Token::Keyword(Keyword::Time) => Type::Time,
            Token::Keyword(Keyword::Beat) => Type::Beat,
            Token::Keyword(Keyword::Length) => Type::Length,
            Token::Keyword(Keyword::Angle) => Type::Angle,
            Token::Keyword(Keyword::Color) => Type::Color,
            Token::Keyword(Keyword::Note) => Type::Note,
            Token::Keyword(Keyword::LineType) => Type::Line,
            Token::Keyword(Keyword::RenderNode) => Type::RenderNode,
        };
        let generic = choice((
            just(Token::Keyword(Keyword::Vec2)).to(TypeConstructor::Vec2),
            just(Token::Keyword(Keyword::TrackSegment)).to(TypeConstructor::TrackSegment),
            just(Token::Keyword(Keyword::KeyframeType)).to(TypeConstructor::Keyframe),
        ))
        .then_ignore(just(Token::Punctuation(Punctuation::LessThan)))
        .then(ty)
        .then_ignore(just(Token::Punctuation(Punctuation::GreaterThan)))
        .map(|(constructor, element)| match constructor {
            TypeConstructor::Vec2 => Type::Vec2(Box::new(element)),
            TypeConstructor::TrackSegment => Type::TrackSegment(Box::new(element)),
            TypeConstructor::Keyframe => Type::Keyframe(Box::new(element)),
        });
        scalar.or(generic).labelled("type").as_context()
    })
}

#[derive(Debug)]
enum Postfix {
    Call(Vec<SourceExpression>, SourceSpan),
    Field(String, SourceSpan),
}

#[derive(Debug, Clone, Copy)]
enum TypeConstructor {
    Vec2,
    TrackSegment,
    Keyframe,
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
        SourceExpression::Vec2 { x, y, .. } => SourceExpression::Vec2 { x, y, span },
    }
}

fn expression_limit_span(tokens: &[SpannedToken], maximum: usize) -> Option<SourceSpan> {
    let mut unary_run = 0usize;
    let mut group_depth = 0usize;
    let mut power_depth = 0usize;
    for (token, span) in tokens {
        match token {
            Token::Punctuation(Punctuation::Minus | Punctuation::Bang) => {
                unary_run += 1;
            }
            Token::Punctuation(Punctuation::LeftParenthesis) => {
                group_depth += 1;
            }
            Token::Punctuation(Punctuation::RightParenthesis) => {
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
        if group_depth
            .saturating_add(unary_run)
            .saturating_add(power_depth)
            > maximum
        {
            return Some(source_span(*span));
        }
    }
    None
}

fn type_limit_span(tokens: &[SpannedToken], maximum: usize) -> Option<SourceSpan> {
    let mut depth = 0usize;
    for (token, span) in tokens {
        match token {
            Token::Punctuation(Punctuation::LessThan) => {
                depth += 1;
                if depth > maximum {
                    return Some(source_span(*span));
                }
            }
            Token::Punctuation(Punctuation::GreaterThan) => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn shift_expression(expression: SourceExpression, offset: usize) -> SourceExpression {
    let shift = |span: SourceSpan| SourceSpan::new(span.start + offset, span.end + offset);
    match expression {
        SourceExpression::Literal { literal, span } => SourceExpression::Literal {
            literal,
            span: shift(span),
        },
        SourceExpression::Name { name, span } => SourceExpression::Name {
            name,
            span: shift(span),
        },
        SourceExpression::Unary {
            operator,
            operand,
            span,
        } => SourceExpression::Unary {
            operator,
            operand: Box::new(shift_expression(*operand, offset)),
            span: shift(span),
        },
        SourceExpression::Binary {
            left,
            operator,
            right,
            span,
        } => SourceExpression::Binary {
            left: Box::new(shift_expression(*left, offset)),
            operator,
            right: Box::new(shift_expression(*right, offset)),
            span: shift(span),
        },
        SourceExpression::Call {
            callee,
            arguments,
            span,
        } => SourceExpression::Call {
            callee: Box::new(shift_expression(*callee, offset)),
            arguments: arguments
                .into_iter()
                .map(|argument| shift_expression(argument, offset))
                .collect(),
            span: shift(span),
        },
        SourceExpression::FieldAccess { base, field, span } => SourceExpression::FieldAccess {
            base: Box::new(shift_expression(*base, offset)),
            field,
            span: shift(span),
        },
        SourceExpression::Vec2 { x, y, span } => SourceExpression::Vec2 {
            x: Box::new(shift_expression(*x, offset)),
            y: Box::new(shift_expression(*y, offset)),
            span: shift(span),
        },
    }
}
