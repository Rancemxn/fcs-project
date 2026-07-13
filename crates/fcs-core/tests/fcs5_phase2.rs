use fcs_core::units::Color;
use fcs_core::v5::ast::{
    Beat, BinaryOperator, SourceExpression, SourceLiteral, SourceSpan, Type, TypedExpression,
    TypedExpressionKind, TypedValue, UnaryOperator,
};

#[test]
fn phase2_types_keep_units_distinct() {
    assert_ne!(Type::Beat, Type::Time);
    assert_eq!(SourceSpan::new(3, 7).len(), 4);
    assert_eq!(TypedValue::Int(4).ty(), Type::Int);
}

#[test]
fn source_spans_are_half_open_and_allow_empty_ranges() {
    const SPAN: SourceSpan = SourceSpan::new(3, 7);
    let span = SPAN;
    assert_eq!(span.start, 3);
    assert_eq!(span.end, 7);
    assert_eq!(span.len(), 4);
    assert!(!span.is_empty());

    let empty = SourceSpan::new(5, 5);
    assert_eq!(empty.len(), 0);
    assert!(empty.is_empty());
}

#[test]
fn phase2_type_display_uses_canonical_spellings() {
    let cases = [
        (Type::Bool, "bool"),
        (Type::Int, "int"),
        (Type::Float, "float"),
        (Type::String, "string"),
        (Type::Time, "time"),
        (Type::Beat, "beat"),
        (Type::Length, "length"),
        (Type::Angle, "angle"),
        (Type::Color, "color"),
        (Type::Vec2(Box::new(Type::Length)), "vec2<length>"),
        (Type::Note, "Note"),
        (Type::Line, "Line"),
        (Type::RenderNode, "RenderNode"),
        (
            Type::TrackSegment(Box::new(Type::Beat)),
            "TrackSegment<beat>",
        ),
        (Type::Keyframe(Box::new(Type::Angle)), "Keyframe<angle>"),
    ];

    for (ty, expected) in cases {
        assert_eq!(ty.to_string(), expected);
    }
}

#[test]
fn scalar_typed_values_report_their_distinct_types() {
    let cases = [
        (TypedValue::Bool(true), Type::Bool),
        (TypedValue::Int(1), Type::Int),
        (TypedValue::Float(1.0), Type::Float),
        (TypedValue::String("value".into()), Type::String),
        (TypedValue::Time(1.0), Type::Time),
        (
            TypedValue::Beat(Beat::new(1, 2).expect("valid beat")),
            Type::Beat,
        ),
        (TypedValue::Length(1.0), Type::Length),
        (TypedValue::Angle(1.0), Type::Angle),
        (TypedValue::Color(Color::WHITE), Type::Color),
    ];

    for (value, expected) in cases {
        assert_eq!(value.ty(), expected);
    }
}

#[test]
fn source_spans_reject_reversed_bounds() {
    assert!(std::panic::catch_unwind(|| SourceSpan::new(7, 3)).is_err());

    let reversed = SourceSpan { start: 7, end: 3 };
    assert!(std::panic::catch_unwind(|| reversed.len()).is_err());
}

#[test]
fn typed_value_vec2_constructor_accepts_homogeneous_components() {
    let value = TypedValue::vec2(TypedValue::Length(10.0), TypedValue::Length(20.0))
        .expect("homogeneous length components should form a vec2");

    assert_eq!(value.ty(), Type::Vec2(Box::new(Type::Length)));
}

#[test]
fn typed_value_vec2_rejects_heterogeneous_components() {
    assert!(TypedValue::vec2(TypedValue::Length(10.0), TypedValue::Time(20.0)).is_none());

    let raw = TypedValue::Vec2(
        Box::new(TypedValue::Length(10.0)),
        Box::new(TypedValue::Time(20.0)),
    );
    assert!(std::panic::catch_unwind(|| raw.ty()).is_err());
}

#[test]
fn typed_literal_type_is_inferred_from_its_value() {
    let span = SourceSpan::new(8, 9);
    let typed = TypedExpression::literal(TypedValue::Int(1), span);

    assert_eq!(
        typed.expression(),
        &TypedExpressionKind::Literal(TypedValue::Int(1))
    );
    assert_eq!(typed.ty(), &Type::Int);
    assert_eq!(typed.span(), span);
}

#[test]
fn phase2_expression_nodes_keep_source_spans() {
    let span = SourceSpan::new(2, 5);
    let literal = SourceExpression::Literal {
        literal: SourceLiteral::Int(1),
        span,
    };
    let name = SourceExpression::Name {
        name: "value".into(),
        span,
    };
    let unary = SourceExpression::Unary {
        operator: UnaryOperator::Negate,
        operand: Box::new(literal.clone()),
        span,
    };
    let binary = SourceExpression::Binary {
        left: Box::new(literal.clone()),
        operator: BinaryOperator::Add,
        right: Box::new(literal.clone()),
        span,
    };
    let call = SourceExpression::Call {
        callee: Box::new(name.clone()),
        arguments: vec![literal.clone()],
        span,
    };
    let field_access = SourceExpression::FieldAccess {
        base: Box::new(name.clone()),
        field: "start".into(),
        span,
    };
    let vec2 = SourceExpression::Vec2 {
        x: Box::new(literal.clone()),
        y: Box::new(literal.clone()),
        span,
    };

    for expression in [literal, name, unary, binary, call, field_access, vec2] {
        assert_eq!(expression.span(), span);
    }

    let typed = TypedExpression::literal(TypedValue::Int(1), span);
    assert_eq!(typed.ty(), &Type::Int);
    assert_eq!(typed.span(), span);
}
