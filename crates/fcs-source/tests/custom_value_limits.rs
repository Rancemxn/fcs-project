//! I5.5 typed custom value limits and FCBC-compatible restrictions.

use fcs_source::CustomValueLimits;
use fcs_source::diagnostic::DiagnosticCode;
use fcs_source::parser::parse_document;

fn nested_object(depth: usize) -> String {
    let mut value = String::from("1");
    for _ in 0..depth {
        value = format!(r#"{{ "k": {value} }}"#);
    }
    value
}

#[test]
fn default_limits_accept_ordinary_custom_trees() {
    let source = r#"#fcs 5.0.0
format { profile: fragment; }
meta {
    custom: {
        "title": "ok",
        "list": [1, 2, 3],
        "nested": { "a": { "b": true } }
    };
}
"#;
    parse_document(source)
        .into_result()
        .unwrap()
        .canonical_metadata()
        .expect("ordinary custom data remains legal under default limits");
}

#[test]
fn depth_limit_is_budgeted_before_deeper_work() {
    let deep = nested_object(3);
    let source = format!(
        r#"#fcs 5.0.0
format {{ profile: fragment; }}
meta {{ custom: {deep}; }}
"#
    );
    let limits = CustomValueLimits::new(2, 4096, 64 * 1024, 1024 * 1024);
    let diagnostics = parse_document(&source)
        .into_result()
        .unwrap()
        .canonical_metadata_with_limits(limits)
        .expect_err("depth-3 object must exceed max_depth 2");
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code() == DiagnosticCode::RESOURCE_LIMIT_EXCEEDED)
        .expect("depth limit diagnostic");
    assert!(diagnostic.message().contains("custom-depth"));
    let budget = diagnostic.budget().expect("budget details");
    assert_eq!(budget.kind(), "custom-depth");
    assert_eq!(budget.limit(), 2);
    assert!(budget.observed() > 2);
}

#[test]
fn field_count_string_and_total_byte_limits_are_independent() {
    let many_fields = (0..5)
        .map(|index| format!(r#""f{index}": {index}"#))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        r#"#fcs 5.0.0
format {{ profile: fragment; }}
meta {{ custom: {{ {many_fields} }}; }}
"#
    );
    let field_limits = CustomValueLimits::new(32, 3, 64 * 1024, 1024 * 1024);
    let diagnostics = parse_document(&source)
        .into_result()
        .unwrap()
        .canonical_metadata_with_limits(field_limits)
        .expect_err("five fields exceed max_fields 3");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
            && diagnostic.message().contains("custom-fields")
    }));

    let long = "x".repeat(16);
    let source = format!(
        r#"#fcs 5.0.0
format {{ profile: fragment; }}
meta {{ custom: {{ "name": "{long}" }}; }}
"#
    );
    let string_limits = CustomValueLimits::new(32, 4096, 8, 1024 * 1024);
    let diagnostics = parse_document(&source)
        .into_result()
        .unwrap()
        .canonical_metadata_with_limits(string_limits)
        .expect_err("16-byte string exceeds max_string_bytes 8");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
            && diagnostic.message().contains("custom-string-bytes")
    }));

    let source = r#"#fcs 5.0.0
format { profile: fragment; }
meta {
    custom: {
        "a": "123456",
        "b": "123456"
    };
}
"#;
    let total_limits = CustomValueLimits::new(32, 4096, 64 * 1024, 20);
    let diagnostics = parse_document(source)
        .into_result()
        .unwrap()
        .canonical_metadata_with_limits(total_limits)
        .expect_err("two charged strings exceed tiny total-byte budget");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
            && diagnostic.message().contains("custom-total-bytes")
    }));
}

#[test]
fn existing_duplicate_key_fixture_still_fails_at_canonical_boundary() {
    let source =
        include_str!("../../../docs/conformance/fcs5/source/invalid/custom-duplicate-key.fcs");
    let diagnostics = parse_document(source)
        .into_result()
        .unwrap()
        .canonical_metadata()
        .expect_err("duplicate custom keys remain illegal");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code() == DiagnosticCode::SCHEMA_DUPLICATE_FIELD)
    );
}
