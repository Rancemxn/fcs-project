use fcs_source::ast::{BinaryOperator, SourceExpression, SourceSpan, UnaryOperator};
use fcs_source::diagnostic::DiagnosticCode;
use fcs_source::parser::parse_expression;

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
    for source in ["null", "vec2", "a b", "a +"] {
        let diagnostics = parse_expression(source).into_result().expect_err(source);
        assert_eq!(diagnostics[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
    }
}
