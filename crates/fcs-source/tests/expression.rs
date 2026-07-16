use fcs_source::ast::{BinaryOperator, SourceExpression, SourceLiteral, SourceSpan, UnaryOperator};
use fcs_source::diagnostic::DiagnosticCode;
use fcs_source::parser::{ParseLimits, parse_expression, parse_expression_with_limits};

fn binary(
    expression: &SourceExpression,
    operator: BinaryOperator,
) -> (&SourceExpression, &SourceExpression) {
    match expression {
        SourceExpression::Binary {
            left,
            operator: actual,
            right,
            ..
        } if *actual == operator => (left, right),
        other => panic!("expected {operator:?} binary expression, got {other:?}"),
    }
}

#[test]
fn token_parser_preserves_frozen_precedence_and_spans() {
    let expression = parse_expression("-a ** b + c * d").into_result().unwrap();
    assert_eq!(expression.span(), SourceSpan::new(0, 15));
    let (left, right) = binary(&expression, BinaryOperator::Add);
    let (power_left, power_right) = binary(left, BinaryOperator::Power);
    assert!(
        matches!(power_left, SourceExpression::Unary { operator: UnaryOperator::Negate, span, .. } if *span == SourceSpan::new(0, 2))
    );
    assert!(
        matches!(power_right, SourceExpression::Name { name, span } if name == "b" && *span == SourceSpan::new(6, 7))
    );
    let (multiply_left, multiply_right) = binary(right, BinaryOperator::Multiply);
    assert!(matches!(multiply_left, SourceExpression::Name { name, .. } if name == "c"));
    assert!(matches!(multiply_right, SourceExpression::Name { name, .. } if name == "d"));
}

#[test]
fn comparison_chains_share_the_middle_operand() {
    let expression = parse_expression("a < b <= c").into_result().unwrap();
    let (first, second) = binary(&expression, BinaryOperator::And);
    let (_, middle_left) = binary(first, BinaryOperator::LessThan);
    let (middle_right, _) = binary(second, BinaryOperator::LessThanOrEqual);
    assert_eq!(middle_left, middle_right);
    assert_eq!(expression.span(), SourceSpan::new(0, 10));
}

#[test]
fn equality_remains_left_associative() {
    let expression = parse_expression("a == b == c").into_result().unwrap();
    let (left, right) = binary(&expression, BinaryOperator::Equal);
    binary(left, BinaryOperator::Equal);
    assert!(matches!(right, SourceExpression::Name { name, .. } if name == "c"));
}

#[test]
fn power_is_right_associative() {
    let expression = parse_expression("a ** b ** c").into_result().unwrap();
    let (left, right) = binary(&expression, BinaryOperator::Power);
    assert!(matches!(left, SourceExpression::Name { name, .. } if name == "a"));
    let (middle, last) = binary(right, BinaryOperator::Power);
    assert!(matches!(middle, SourceExpression::Name { name, .. } if name == "b"));
    assert!(matches!(last, SourceExpression::Name { name, .. } if name == "c"));
}

#[test]
fn token_parser_rejects_reserved_names_and_trailing_input() {
    for source in ["vec2", "a b", "a +"] {
        let diagnostics = parse_expression(source).into_result().expect_err(source);
        assert_eq!(diagnostics[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
    }
}

#[test]
fn parser_accepts_an_array_expression_with_a_trailing_comma() {
    let expression = parse_expression("[1, 2,]").into_result().unwrap();
    assert_eq!(expression.span(), SourceSpan::new(0, 7));
    let SourceExpression::Array { elements, .. } = expression else {
        panic!("expected array expression");
    };
    assert_eq!(elements.len(), 2);
    assert!(matches!(elements[0], SourceExpression::Literal { .. }));
}

#[test]
fn parser_preserves_ordered_object_entries_and_duplicate_keys() {
    let expression = parse_expression(r#"{"a": 1, "a": 2,}"#)
        .into_result()
        .unwrap();
    let SourceExpression::Object { entries, span } = expression else {
        panic!("expected object expression");
    };
    assert_eq!(span, SourceSpan::new(0, 17));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "a");
    assert_eq!(entries[1].key, "a");
    assert_eq!(entries[0].key_span, SourceSpan::new(1, 4));
    assert_eq!(entries[1].key_span, SourceSpan::new(9, 12));
}

#[test]
fn parser_supports_references_index_postfix_and_keyword_field_names() {
    let expression = parse_expression("@asset[0].length").into_result().unwrap();
    let SourceExpression::FieldAccess { base, field, span } = expression else {
        panic!("expected field access");
    };
    assert_eq!(field, "length");
    assert_eq!(span, SourceSpan::new(0, 16));
    let SourceExpression::Index { base, index, span } = *base else {
        panic!("expected index postfix");
    };
    assert_eq!(span, SourceSpan::new(0, 9));
    assert!(matches!(*index, SourceExpression::Literal { .. }));
    assert!(
        matches!(*base, SourceExpression::Reference { name, span } if name == "asset" && span == SourceSpan::new(0, 6))
    );
}

#[test]
fn parser_preserves_choose_arm_order_and_else_value() {
    let source = "choose { when a < b => 1; when b < c => 2; else => 3; }";
    let expression = parse_expression(source).into_result().unwrap();
    let SourceExpression::Choose {
        arms,
        else_value,
        span,
    } = expression
    else {
        panic!("expected choose expression");
    };
    assert_eq!(span, SourceSpan::new(0, source.len()));
    assert_eq!(arms.len(), 2);
    assert!(matches!(arms[0].value, SourceExpression::Literal { .. }));
    assert!(matches!(arms[1].value, SourceExpression::Literal { .. }));
    assert!(matches!(*else_value, SourceExpression::Literal { .. }));
}

#[test]
fn parser_retains_null_as_a_source_literal() {
    let expression = parse_expression("null").into_result().unwrap();
    assert!(matches!(
        expression,
        SourceExpression::Literal {
            literal: SourceLiteral::Null,
            span
        } if span == SourceSpan::new(0, 4)
    ));
}

#[test]
fn parser_requires_choose_when_arms_and_else() {
    for source in ["choose { else => 0; }", "choose { when true => 1; }"] {
        let diagnostics = parse_expression(source).into_result().expect_err(source);
        assert_eq!(diagnostics[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
    }
}

#[test]
fn parser_limits_array_object_and_choose_nesting_before_parsing() {
    let limits = ParseLimits {
        max_nesting_depth: 2,
        ..ParseLimits::default()
    };
    let diagnostics = parse_expression_with_limits("[[[1]]]", limits)
        .into_result()
        .expect_err("array nesting should exceed the shared parser limit");
    assert_eq!(
        diagnostics[0].code(),
        DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
    );
}

#[test]
fn parser_accepts_empty_array_source_nodes() {
    let expression = parse_expression("[]").into_result().unwrap();
    assert!(matches!(
        expression,
        SourceExpression::Array { elements, span }
            if elements.is_empty() && span == SourceSpan::new(0, 2)
    ));
}

#[test]
fn parser_accepts_empty_object_source_nodes() {
    let expression = parse_expression("{}").into_result().unwrap();
    assert!(matches!(
        expression,
        SourceExpression::Object { entries, span }
            if entries.is_empty() && span == SourceSpan::new(0, 2)
    ));
}

#[test]
fn parser_accepts_trailing_call_arguments() {
    let expression = parse_expression("factory(1,)").into_result().unwrap();
    let SourceExpression::Call {
        arguments, span, ..
    } = expression
    else {
        panic!("expected call expression");
    };
    assert_eq!(arguments.len(), 1);
    assert_eq!(span, SourceSpan::new(0, 11));
}

#[test]
fn parser_rejects_object_keys_that_are_not_string_literals() {
    let diagnostics = parse_expression("{key: 1}")
        .into_result()
        .expect_err("object keys must use string literals");
    assert_eq!(diagnostics[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
}

#[test]
fn parser_keeps_unresolved_schema_enum_words_as_name_references() {
    let expression = parse_expression("above").into_result().unwrap();
    assert!(matches!(
        expression,
        SourceExpression::Name { name, span }
            if name == "above" && span == SourceSpan::new(0, 5)
    ));
}
