//! Source and typed expression data structures for the FCS 5 compile-time language.
//!
//! Source spans use half-open UTF-8 byte offsets (`[start, end)`). A valid span always
//! satisfies `start <= end`; both the constructor and length calculation enforce that
//! invariant in every build profile.

use std::fmt;

use super::Color;

use super::Beat;

/// A half-open range of UTF-8 byte offsets in an FCS source document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceSpan {
    /// Inclusive UTF-8 byte offset at which the source construct begins.
    pub start: usize,
    /// Exclusive UTF-8 byte offset at which the source construct ends.
    pub end: usize,
}

impl SourceSpan {
    /// Creates a source span, panicking if `end` precedes `start`.
    pub const fn new(start: usize, end: usize) -> Self {
        assert!(start <= end, "source span end precedes its start");
        Self { start, end }
    }

    /// Returns the number of UTF-8 source bytes covered by this span.
    ///
    /// This also validates spans created directly through the public fields and panics
    /// if `end` precedes `start`.
    pub const fn len(self) -> usize {
        match self.end.checked_sub(self.start) {
            Some(length) => length,
            None => panic!("source span end precedes its start"),
        }
    }

    /// Returns `true` when this span covers no source bytes.
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// A static type in the FCS 5 compile-time language.
///
/// The derived ordering traits exist only for host-side declaration and deterministic
/// map ordering. Variant order never defines FCS equality or ordering semantics.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

/// A literal as it appears in source before expression elaboration.
///
/// This is deliberately separate from [`TypedValue`]: parsing source does not by itself
/// produce trusted typed output.
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

/// A unary operator represented by the expression source AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOperator {
    Negate,
    Not,
}

/// A binary operator represented by the expression source AST.
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

/// An expression produced by the parser before name resolution or type checking.
///
/// Every variant owns the complete half-open span of that expression node.
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
    /// Returns this source expression node's complete source span.
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

/// A concrete compile-time value produced only after successful type checking.
///
/// The public variants are the representation layer needed by later FCS phases. Prefer
/// [`TypedValue::vec2`] over constructing [`TypedValue::Vec2`] directly so heterogeneous
/// component types are rejected before the value enters trusted typed output.
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
    /// A representation-level vector value whose components must have equal types.
    Vec2(Box<TypedValue>, Box<TypedValue>),
}

impl TypedValue {
    /// Constructs a vector when both components are valid values of the same type.
    ///
    /// Returns `None` for heterogeneous components or components containing an invalid
    /// raw vector representation.
    pub fn vec2(x: TypedValue, y: TypedValue) -> Option<Self> {
        let x_type = x.checked_type()?;
        let y_type = y.checked_type()?;
        (x_type == y_type).then(|| Self::Vec2(Box::new(x), Box::new(y)))
    }

    /// Returns this value's FCS language type.
    ///
    /// # Panics
    ///
    /// Panics if a [`TypedValue::Vec2`] was built directly with heterogeneous components
    /// or contains another invalid raw vector. Use [`TypedValue::vec2`] to reject those
    /// representations before construction.
    pub fn ty(&self) -> Type {
        self.checked_type()
            .expect("typed vec2 components must have the same valid type")
    }

    fn checked_type(&self) -> Option<Type> {
        match self {
            Self::Bool(_) => Some(Type::Bool),
            Self::Int(_) => Some(Type::Int),
            Self::Float(_) => Some(Type::Float),
            Self::String(_) => Some(Type::String),
            Self::Time(_) => Some(Type::Time),
            Self::Beat(_) => Some(Type::Beat),
            Self::Length(_) => Some(Type::Length),
            Self::Angle(_) => Some(Type::Angle),
            Self::Color(_) => Some(Type::Color),
            Self::Vec2(x, y) => {
                let x_type = x.checked_type()?;
                let y_type = y.checked_type()?;
                (x_type == y_type).then(|| Type::Vec2(Box::new(x_type)))
            }
        }
    }
}

/// The recursively typed expression node retained after successful elaboration.
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

/// The typed form of an expression node after successful elaboration.
///
/// Fields are read-only outside this module so callers cannot pair an expression with a
/// contradictory type. The future elaborator may use the crate-visible constructor only
/// after it has checked the complete expression.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedExpression {
    expression: TypedExpressionKind,
    ty: Type,
    span: SourceSpan,
}

impl TypedExpression {
    /// Creates a typed literal, inferring its type from the checked value.
    pub fn literal(value: TypedValue, span: SourceSpan) -> Self {
        let ty = value.ty();
        Self::new(TypedExpressionKind::Literal(value), ty, span)
    }

    /// Returns the typed expression node.
    pub fn expression(&self) -> &TypedExpressionKind {
        &self.expression
    }

    /// Returns the expression's unique elaborated type.
    pub fn ty(&self) -> &Type {
        &self.ty
    }

    /// Returns the source span retained during elaboration.
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    pub(crate) fn new(expression: TypedExpressionKind, ty: Type, span: SourceSpan) -> Self {
        Self {
            expression,
            ty,
            span,
        }
    }
}
