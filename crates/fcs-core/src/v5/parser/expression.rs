use crate::v5::ast::{BinaryOperator, SourceExpression, SourceSpan, Type, UnaryOperator};

use super::ParseError;
use super::lexer::{Symbol, Token, TokenKind, lex};

pub fn parse_expression(input: &str) -> Result<SourceExpression, ParseError> {
    let tokens = lex(input).map_err(|()| ParseError::InvalidSyntax("expression"))?;
    let mut parser = Parser::new(tokens);
    let expression = parser
        .parse_or()
        .map_err(|()| ParseError::InvalidSyntax("expression"))?;
    if !parser.is_at_end() {
        return Err(ParseError::InvalidSyntax("expression"));
    }
    Ok(expression)
}

pub fn parse_type(input: &str) -> Result<Type, ParseError> {
    let tokens = lex(input).map_err(|()| ParseError::InvalidSyntax("type"))?;
    let mut parser = Parser::new(tokens);
    let ty = parser
        .parse_type_inner()
        .map_err(|()| ParseError::InvalidSyntax("type"))?;
    if !parser.is_at_end() {
        return Err(ParseError::InvalidSyntax("type"));
    }
    Ok(ty)
}

struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    fn parse_type_inner(&mut self) -> Result<Type, ()> {
        let name = self.consume_identifier().ok_or(())?;
        let scalar = match name.as_str() {
            "bool" => Some(Type::Bool),
            "int" => Some(Type::Int),
            "float" => Some(Type::Float),
            "string" => Some(Type::String),
            "time" => Some(Type::Time),
            "beat" => Some(Type::Beat),
            "length" => Some(Type::Length),
            "angle" => Some(Type::Angle),
            "color" => Some(Type::Color),
            "Note" => Some(Type::Note),
            "Line" => Some(Type::Line),
            "RenderNode" => Some(Type::RenderNode),
            _ => None,
        };
        if let Some(ty) = scalar {
            return Ok(ty);
        }

        self.expect_symbol(Symbol::LessThan)?;
        let element = Box::new(self.parse_type_inner()?);
        self.expect_symbol(Symbol::GreaterThan)?;
        match name.as_str() {
            "vec2" => Ok(Type::Vec2(element)),
            "TrackSegment" => Ok(Type::TrackSegment(element)),
            "Keyframe" => Ok(Type::Keyframe(element)),
            _ => Err(()),
        }
    }

    fn parse_or(&mut self) -> Result<SourceExpression, ()> {
        let mut expression = self.parse_and()?;
        while self.consume_symbol(Symbol::OrOr).is_some() {
            let right = self.parse_and()?;
            expression = binary(expression, BinaryOperator::Or, right);
        }
        Ok(expression)
    }

    fn parse_and(&mut self) -> Result<SourceExpression, ()> {
        let mut expression = self.parse_equality()?;
        while self.consume_symbol(Symbol::AndAnd).is_some() {
            let right = self.parse_equality()?;
            expression = binary(expression, BinaryOperator::And, right);
        }
        Ok(expression)
    }

    fn parse_equality(&mut self) -> Result<SourceExpression, ()> {
        let mut expression = self.parse_comparison()?;
        loop {
            let operator = if self.consume_symbol(Symbol::EqualEqual).is_some() {
                BinaryOperator::Equal
            } else if self.consume_symbol(Symbol::BangEqual).is_some() {
                BinaryOperator::NotEqual
            } else {
                break;
            };
            let right = self.parse_comparison()?;
            expression = binary(expression, operator, right);
        }
        Ok(expression)
    }

    fn parse_comparison(&mut self) -> Result<SourceExpression, ()> {
        let mut expression = self.parse_additive()?;
        loop {
            let operator = if self.consume_symbol(Symbol::LessThan).is_some() {
                BinaryOperator::LessThan
            } else if self.consume_symbol(Symbol::LessThanOrEqual).is_some() {
                BinaryOperator::LessThanOrEqual
            } else if self.consume_symbol(Symbol::GreaterThan).is_some() {
                BinaryOperator::GreaterThan
            } else if self.consume_symbol(Symbol::GreaterThanOrEqual).is_some() {
                BinaryOperator::GreaterThanOrEqual
            } else {
                break;
            };
            let right = self.parse_additive()?;
            expression = binary(expression, operator, right);
        }
        Ok(expression)
    }

    fn parse_additive(&mut self) -> Result<SourceExpression, ()> {
        let mut expression = self.parse_multiplicative()?;
        loop {
            let operator = if self.consume_symbol(Symbol::Plus).is_some() {
                BinaryOperator::Add
            } else if self.consume_symbol(Symbol::Minus).is_some() {
                BinaryOperator::Subtract
            } else {
                break;
            };
            let right = self.parse_multiplicative()?;
            expression = binary(expression, operator, right);
        }
        Ok(expression)
    }

    fn parse_multiplicative(&mut self) -> Result<SourceExpression, ()> {
        let mut expression = self.parse_unary()?;
        loop {
            let operator = if self.consume_symbol(Symbol::Star).is_some() {
                BinaryOperator::Multiply
            } else if self.consume_symbol(Symbol::Slash).is_some() {
                BinaryOperator::Divide
            } else if self.consume_symbol(Symbol::Percent).is_some() {
                BinaryOperator::Remainder
            } else {
                break;
            };
            let right = self.parse_unary()?;
            expression = binary(expression, operator, right);
        }
        Ok(expression)
    }

    fn parse_unary(&mut self) -> Result<SourceExpression, ()> {
        if let Some(operator) = self.consume_symbol(Symbol::Minus) {
            let operand = self.parse_unary()?;
            let span = SourceSpan::new(operator.span.start, operand.span().end);
            return Ok(SourceExpression::Unary {
                operator: UnaryOperator::Negate,
                operand: Box::new(operand),
                span,
            });
        }
        if let Some(operator) = self.consume_symbol(Symbol::Bang) {
            let operand = self.parse_unary()?;
            let span = SourceSpan::new(operator.span.start, operand.span().end);
            return Ok(SourceExpression::Unary {
                operator: UnaryOperator::Not,
                operand: Box::new(operand),
                span,
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<SourceExpression, ()> {
        let mut expression = self.parse_primary()?;
        loop {
            if self.consume_symbol(Symbol::LeftParenthesis).is_some() {
                expression = self.finish_call(expression)?;
            } else if self.consume_symbol(Symbol::Dot).is_some() {
                let (field, field_span) = self.consume_identifier_with_span().ok_or(())?;
                let span = SourceSpan::new(expression.span().start, field_span.end);
                expression = SourceExpression::FieldAccess {
                    base: Box::new(expression),
                    field,
                    span,
                };
            } else {
                break;
            }
        }
        Ok(expression)
    }

    fn finish_call(&mut self, callee: SourceExpression) -> Result<SourceExpression, ()> {
        let start = callee.span().start;
        let vec2_constructor = matches!(
            &callee,
            SourceExpression::Name { name, .. } if name == "vec2"
        );
        let mut arguments = Vec::new();
        if !self.check_symbol(Symbol::RightParenthesis) {
            loop {
                arguments.push(self.parse_or()?);
                if self.consume_symbol(Symbol::Comma).is_none() {
                    break;
                }
                if self.check_symbol(Symbol::RightParenthesis) {
                    break;
                }
            }
        }
        let close = self.expect_symbol(Symbol::RightParenthesis)?;
        let span = SourceSpan::new(start, close.span.end);

        if vec2_constructor {
            if arguments.len() != 2 {
                return Err(());
            }
            let mut arguments = arguments.into_iter();
            let x = arguments.next().ok_or(())?;
            let y = arguments.next().ok_or(())?;
            Ok(SourceExpression::Vec2 {
                x: Box::new(x),
                y: Box::new(y),
                span,
            })
        } else {
            Ok(SourceExpression::Call {
                callee: Box::new(callee),
                arguments,
                span,
            })
        }
    }

    fn parse_primary(&mut self) -> Result<SourceExpression, ()> {
        let token = self.advance().ok_or(())?.clone();
        match token.kind {
            TokenKind::Literal(literal) => Ok(SourceExpression::Literal {
                literal,
                span: token.span,
            }),
            TokenKind::Identifier(name) => Ok(SourceExpression::Name {
                name,
                span: token.span,
            }),
            TokenKind::Symbol(Symbol::LeftParenthesis) => {
                let expression = self.parse_or()?;
                let close = self.expect_symbol(Symbol::RightParenthesis)?;
                Ok(with_span(
                    expression,
                    SourceSpan::new(token.span.start, close.span.end),
                ))
            }
            TokenKind::Symbol(_) => Err(()),
        }
    }

    fn consume_identifier(&mut self) -> Option<String> {
        self.consume_identifier_with_span().map(|(name, _)| name)
    }

    fn consume_identifier_with_span(&mut self) -> Option<(String, SourceSpan)> {
        let Token {
            kind: TokenKind::Identifier(name),
            span,
        } = self.peek()?.clone()
        else {
            return None;
        };
        self.current += 1;
        Some((name, span))
    }

    fn expect_symbol(&mut self, symbol: Symbol) -> Result<Token, ()> {
        self.consume_symbol(symbol).ok_or(())
    }

    fn consume_symbol(&mut self, symbol: Symbol) -> Option<Token> {
        if !self.check_symbol(symbol) {
            return None;
        }
        let token = self.tokens[self.current].clone();
        self.current += 1;
        Some(token)
    }

    fn check_symbol(&self, symbol: Symbol) -> bool {
        matches!(
            self.peek(),
            Some(Token {
                kind: TokenKind::Symbol(actual),
                ..
            }) if *actual == symbol
        )
    }

    fn advance(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.current)?;
        self.current += 1;
        Some(token)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.current)
    }

    fn is_at_end(&self) -> bool {
        self.current == self.tokens.len()
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
