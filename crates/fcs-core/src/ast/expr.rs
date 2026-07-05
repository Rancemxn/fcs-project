//! Expression AST nodes — literals, variables, ops, calls, chain comparisons.

use crate::units::{Color, Unit};

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(Literal),
    Variable(String),
    BinaryOp {
        op: BinaryOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expression>,
    },
    Call {
        name: String,
        args: Vec<Expression>,
    },
    Ternary {
        cond: Box<Expression>,
        if_true: Box<Expression>,
        if_false: Box<Expression>,
    },
    ChainCompare {
        left: Box<Expression>,
        ops: Vec<(CompareOp, Box<Expression>)>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    Quantified { value: f64, unit: Unit },
    Boolean(bool),
    Color(Color),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueType {
    Note,
    Line,
    Float,
    Int,
    Bool,
    Time,
    Length,
    Angle,
}

impl ValueType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ValueType::Note => "Note",
            ValueType::Line => "Line",
            ValueType::Float => "float",
            ValueType::Int => "int",
            ValueType::Bool => "bool",
            ValueType::Time => "time",
            ValueType::Length => "length",
            ValueType::Angle => "angle",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

impl CompareOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompareOp::Lt => "<",
            CompareOp::Le => "<=",
            CompareOp::Gt => ">",
            CompareOp::Ge => ">=",
            CompareOp::Eq => "==",
            CompareOp::Ne => "!=",
        }
    }
}
