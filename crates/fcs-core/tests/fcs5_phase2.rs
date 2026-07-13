use fcs_core::v5::ast::{
    BinaryOperator, SourceExpression, SourceLiteral, SourceSpan, Type, TypedExpression,
    TypedExpressionKind, TypedValue, UnaryOperator,
};

#[test]
fn phase2_types_keep_units_distinct() {
    assert_ne!(Type::Beat, Type::Time);
    assert_eq!(
        Type::Vec2(Box::new(Type::Length)).to_string(),
        "vec2<length>"
    );
    assert_eq!(SourceSpan::new(3, 7).len(), 4);
    assert_eq!(TypedValue::Int(4).ty(), Type::Int);
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
        base: Box::new(name),
        field: "start".into(),
        span,
    };
    let vec2 = SourceExpression::Vec2 {
        x: Box::new(literal.clone()),
        y: Box::new(literal),
        span,
    };

    for expression in [unary, binary, call, field_access, vec2] {
        assert_eq!(expression.span(), span);
    }

    let typed = TypedExpression {
        expression: TypedExpressionKind::Literal(TypedValue::Int(1)),
        ty: Type::Int,
        span,
    };
    assert_eq!(typed.ty, Type::Int);
    assert_eq!(typed.span, span);
}
