use std::fmt;

use crate::units::Color;

use super::Beat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

impl SourceSpan {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn len(self) -> usize {
        self.end - self.start
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Bool,
    Int,
    Float,
    String,
    Time,
    Beat,
    Length,
    Angle,
    Color,
    Vec2(Box<Type>),
    Note,
    Line,
    RenderNode,
    TrackSegment(Box<Type>),
    Keyframe(Box<Type>),
}

impl fmt::Display for Type {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool => formatter.write_str("bool"),
            Self::Int => formatter.write_str("int"),
            Self::Float => formatter.write_str("float"),
            Self::String => formatter.write_str("string"),
            Self::Time => formatter.write_str("time"),
            Self::Beat => formatter.write_str("beat"),
            Self::Length => formatter.write_str("length"),
            Self::Angle => formatter.write_str("angle"),
            Self::Color => formatter.write_str("color"),
            Self::Vec2(element) => write!(formatter, "vec2<{element}>"),
            Self::Note => formatter.write_str("Note"),
            Self::Line => formatter.write_str("Line"),
            Self::RenderNode => formatter.write_str("RenderNode"),
            Self::TrackSegment(element) => write!(formatter, "TrackSegment<{element}>"),
            Self::Keyframe(element) => write!(formatter, "Keyframe<{element}>"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SourceLiteral {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Time(f64),
    Beat(Beat),
    Length(f64),
    Angle(f64),
    Color(Color),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOperator {
    Negate,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SourceExpression {
    Literal {
        literal: SourceLiteral,
        span: SourceSpan,
    },
    Name {
        name: String,
        span: SourceSpan,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<SourceExpression>,
        span: SourceSpan,
    },
    Binary {
        left: Box<SourceExpression>,
        operator: BinaryOperator,
        right: Box<SourceExpression>,
        span: SourceSpan,
    },
    Call {
        callee: Box<SourceExpression>,
        arguments: Vec<SourceExpression>,
        span: SourceSpan,
    },
    FieldAccess {
        base: Box<SourceExpression>,
        field: String,
        span: SourceSpan,
    },
    Vec2 {
        x: Box<SourceExpression>,
        y: Box<SourceExpression>,
        span: SourceSpan,
    },
}

impl SourceExpression {
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Literal { span, .. }
            | Self::Name { span, .. }
            | Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::Call { span, .. }
            | Self::FieldAccess { span, .. }
            | Self::Vec2 { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Time(f64),
    Beat(Beat),
    Length(f64),
    Angle(f64),
    Color(Color),
    Vec2(Box<TypedValue>, Box<TypedValue>),
}

impl TypedValue {
    pub fn ty(&self) -> Type {
        match self {
            Self::Bool(_) => Type::Bool,
            Self::Int(_) => Type::Int,
            Self::Float(_) => Type::Float,
            Self::String(_) => Type::String,
            Self::Time(_) => Type::Time,
            Self::Beat(_) => Type::Beat,
            Self::Length(_) => Type::Length,
            Self::Angle(_) => Type::Angle,
            Self::Color(_) => Type::Color,
            Self::Vec2(x, y) => {
                let element_type = x.ty();
                debug_assert_eq!(element_type, y.ty());
                Type::Vec2(Box::new(element_type))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedExpressionKind {
    Literal(TypedValue),
    Name(String),
    Unary {
        operator: UnaryOperator,
        operand: Box<TypedExpression>,
    },
    Binary {
        left: Box<TypedExpression>,
        operator: BinaryOperator,
        right: Box<TypedExpression>,
    },
    Call {
        callee: Box<TypedExpression>,
        arguments: Vec<TypedExpression>,
    },
    FieldAccess {
        base: Box<TypedExpression>,
        field: String,
    },
    Vec2 {
        x: Box<TypedExpression>,
        y: Box<TypedExpression>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedExpression {
    pub expression: TypedExpressionKind,
    pub ty: Type,
    pub span: SourceSpan,
}
