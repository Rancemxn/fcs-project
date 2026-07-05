//! Expression parser — Pratt parser with chain comparisons, function calls,
//! ternaries, math functions, and easing functions.

use crate::ast::{BinaryOp, CompareOp, Expression, UnaryOp};
use crate::parser::literal::{parse_literal, ws};
use nom::Parser;
use nom::{
    IResult,
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, alphanumeric1, char},
    combinator::{map, opt, recognize, value},
    multi::{many0, separated_list0},
    sequence::{delimited, pair, preceded},
};

// ---------------------------------------------------------------------------
// Identifier
// ---------------------------------------------------------------------------

fn ident(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
    ))
    .parse(input)
}

// ---------------------------------------------------------------------------
// Primary expressions
// ---------------------------------------------------------------------------

fn primary(input: &str) -> IResult<&str, Expression> {
    alt((
        map(parse_literal, Expression::Literal),
        map(ident, |name: &str| Expression::Variable(name.to_string())),
        parse_grouped,
        parse_unary,
    ))
    .parse(input)
}

fn parse_grouped(input: &str) -> IResult<&str, Expression> {
    delimited(
        preceded(ws, char('(')),
        preceded(ws, parse_expression),
        preceded(ws, char(')')),
    )
    .parse(input)
}

fn parse_unary(input: &str) -> IResult<&str, Expression> {
    let (input, _) = preceded(ws, char('-')).parse(input)?;
    let (input, operand) = primary(input)?;
    Ok((
        input,
        Expression::UnaryOp {
            op: UnaryOp::Neg,
            operand: Box::new(operand),
        },
    ))
}

// ---------------------------------------------------------------------------
// Postfix: function calls
// ---------------------------------------------------------------------------

fn parse_postfix(input: &str) -> IResult<&str, Expression> {
    let (input, base) = primary(input)?;
    let (input, args_opt) = opt(preceded(
        preceded(ws, char('(')),
        preceded(
            ws,
            separated_list0(preceded(ws, char(',')), preceded(ws, parse_expression)),
        ),
    ))
    .parse(input)?;

    match args_opt {
        Some(args) => {
            let (input, _) = preceded(ws, char(')')).parse(input)?;
            match &base {
                Expression::Variable(name) => Ok((
                    input,
                    Expression::Call {
                        name: name.clone(),
                        args,
                    },
                )),
                _ => Ok((input, base)),
            }
        }
        None => Ok((input, base)),
    }
}

// ---------------------------------------------------------------------------
// Infix: operator precedence
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Prec {
    Compare = 1,
    Sum = 2,
    Product = 3,
    Power = 4,
}

fn op_prec(op: &BinaryOp) -> Prec {
    match op {
        BinaryOp::Add | BinaryOp::Sub => Prec::Sum,
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => Prec::Product,
        BinaryOp::Pow => Prec::Power,
    }
}

fn parse_binary_op(input: &str) -> IResult<&str, BinaryOp> {
    let (input, _) = ws(input)?;
    let (input, op) = alt((
        value(BinaryOp::Add, char('+')),
        value(BinaryOp::Sub, char('-')),
        value(BinaryOp::Mul, char('*')),
        value(BinaryOp::Div, char('/')),
        value(BinaryOp::Mod, char('%')),
        value(BinaryOp::Pow, char('^')),
    ))
    .parse(input)?;
    // Consume whitespace after operator
    let (input, _) = ws(input)?;
    Ok((input, op))
}

fn parse_compare_op(input: &str) -> IResult<&str, CompareOp> {
    let (input, _) = ws(input)?;
    let (input, op) = alt((
        value(CompareOp::Le, tag("<=")),
        value(CompareOp::Ge, tag(">=")),
        value(CompareOp::Eq, tag("==")),
        value(CompareOp::Ne, tag("!=")),
        value(CompareOp::Lt, char('<')),
        value(CompareOp::Gt, char('>')),
    ))
    .parse(input)?;
    let (input, _) = ws(input)?;
    Ok((input, op))
}

// ---------------------------------------------------------------------------
// Ternary: cond ? ifTrue : ifFalse
// ---------------------------------------------------------------------------

fn parse_ternary(input: &str) -> IResult<&str, Expression> {
    let (input, cond) = parse_comparison(input)?;
    let (input, has) = opt(preceded(ws, char('?'))).parse(input)?;
    match has {
        Some(_) => {
            let (input, if_true) = preceded(ws, parse_expression).parse(input)?;
            let (input, _) = preceded(ws, char(':')).parse(input)?;
            let (input, if_false) = preceded(ws, parse_expression).parse(input)?;
            Ok((
                input,
                Expression::Ternary {
                    cond: Box::new(cond),
                    if_true: Box::new(if_true),
                    if_false: Box::new(if_false),
                },
            ))
        }
        None => Ok((input, cond)),
    }
}

// ---------------------------------------------------------------------------
// Chain comparisons: a < b < c
// ---------------------------------------------------------------------------

fn parse_comparison(input: &str) -> IResult<&str, Expression> {
    let (input, left) = parse_binary(input, Prec::Compare)?;
    let (input, ops) = many0(pair(
        parse_compare_op,
        preceded(ws, |i| parse_binary(i, Prec::Compare)),
    ))
    .parse(input)?;

    if ops.is_empty() {
        Ok((input, left))
    } else {
        Ok((
            input,
            Expression::ChainCompare {
                left: Box::new(left),
                ops: ops.into_iter().map(|(op, e)| (op, Box::new(e))).collect(),
            },
        ))
    }
}

// ---------------------------------------------------------------------------
// Pratt parser core
// ---------------------------------------------------------------------------

fn parse_binary(input: &str, min_prec: Prec) -> IResult<&str, Expression> {
    let (mut input, mut left) = parse_postfix(input)?;
    loop {
        let saved = input;
        match parse_binary_op(input) {
            Ok((rest, op)) => {
                let prec = op_prec(&op);
                if prec < min_prec {
                    return Ok((saved, left));
                }
                input = rest;
                let (rest, right) = parse_binary(input, prec)?;
                left = Expression::BinaryOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                };
                input = rest;
            }
            Err(_) => break,
        }
    }
    Ok((input, left))
}

// ---------------------------------------------------------------------------
// Top-level entry point
// ---------------------------------------------------------------------------

pub fn parse_expression(input: &str) -> IResult<&str, Expression> {
    preceded(ws, parse_ternary).parse(input)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Literal;
    use crate::units::{LengthUnit, Unit};

    #[test]
    fn test_variable() {
        assert_eq!(
            parse_expression("b").unwrap().1,
            Expression::Variable("b".into())
        );
    }

    #[test]
    fn test_literal() {
        assert_eq!(
            parse_expression("42").unwrap().1,
            Expression::Literal(Literal::Integer(42))
        );
    }

    #[test]
    fn test_quantified() {
        assert_eq!(
            parse_expression("200px").unwrap().1,
            Expression::Literal(Literal::Quantified {
                value: 200.0,
                unit: Unit::Length(LengthUnit::Pixel)
            })
        );
    }

    #[test]
    fn test_binary() {
        let r = parse_expression("1 + 2").unwrap().1;
        assert!(matches!(
            r,
            Expression::BinaryOp {
                op: BinaryOp::Add,
                ..
            }
        ));
    }

    #[test]
    fn test_precedence() {
        let r = parse_expression("1 + 2 * 3").unwrap().1;
        match r {
            Expression::BinaryOp {
                op: BinaryOp::Add,
                right,
                ..
            } => {
                assert!(matches!(
                    *right,
                    Expression::BinaryOp {
                        op: BinaryOp::Mul,
                        ..
                    }
                ));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_call() {
        let r = parse_expression("sin(b)").unwrap().1;
        assert!(matches!(r, Expression::Call { name, .. } if name == "sin"));
    }

    #[test]
    fn test_chain_compare() {
        let r = parse_expression("a < b < c").unwrap().1;
        match r {
            Expression::ChainCompare { ops, .. } => assert_eq!(ops.len(), 2),
            _ => panic!("expected ChainCompare"),
        }
    }

    #[test]
    fn test_ternary() {
        let r = parse_expression("d > 200px ? 1.5 : 1.0").unwrap().1;
        assert!(matches!(r, Expression::Ternary { .. }));
    }
}
