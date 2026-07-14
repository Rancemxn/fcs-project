use fcs_source::ast::SourceSpan;
use fcs_source::diagnostic::{DiagnosticCode, DiagnosticStage};
use fcs_source::elaborator::{CompileTimeLimits, elaborate};
use fcs_source::parser::{
    ParseLimits, parse_document, parse_document_with_limits, parse_expression_with_limits,
    parse_header, parse_type_with_limits,
};
use fcs_source::schema::phase2_schema;

#[test]
fn missing_header_has_the_frozen_code_and_byte_span() {
    let result = parse_document("format { profile: fragment; }");
    let errors = result.into_result().expect_err("missing header must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code(), DiagnosticCode::VERSION_MISSING_HEADER);
    assert_eq!(errors[0].stage(), DiagnosticStage::Parse);
    assert_eq!(
        errors[0].severity(),
        fcs_source::diagnostic::DiagnosticSeverity::Error
    );
    assert_eq!(errors[0].primary_span(), SourceSpan::new(0, 0));
}

#[test]
fn parser_resource_limits_use_the_stable_resource_code() {
    let result = parse_expression_with_limits(
        "1",
        ParseLimits {
            max_source_bytes: 0,
            ..ParseLimits::default()
        },
    );
    let errors = result
        .into_result()
        .expect_err("the input exceeds the byte limit");
    assert_eq!(errors[0].code(), DiagnosticCode::RESOURCE_LIMIT_EXCEEDED);
}

#[test]
fn bom_diagnostics_keep_original_utf8_byte_offsets() {
    let source = "\u{feff}#fcs invalid";
    let errors = parse_header(source)
        .into_result()
        .expect_err("invalid version must fail");
    assert_eq!(errors[0].code(), DiagnosticCode::VERSION_INVALID);
    assert_eq!(errors[0].primary_span(), SourceSpan::new(0, source.len()));
}

#[test]
fn tempo_diagnostics_point_at_the_tempo_block() {
    let source = "#fcs 5.0.0\nformat { profile: chart; }\n\
                  tempoMap { 4beat -> 180bpm; }";
    let errors = parse_document(source)
        .into_result()
        .expect_err("tempoMap must start at zero");
    assert_eq!(errors[0].code(), DiagnosticCode::TEMPO_INVALID);
    let tempo_start = source.find("tempoMap").unwrap();
    assert_eq!(errors[0].primary_span().start, tempo_start);
}

#[test]
fn type_parser_exposes_the_same_bounded_diagnostic_boundary() {
    let result = parse_type_with_limits(
        "vec2<vec2<length>>",
        ParseLimits {
            max_nesting_depth: 1,
            ..ParseLimits::default()
        },
    );
    let errors = result
        .into_result()
        .expect_err("type nesting exceeds the limit");
    assert_eq!(errors[0].code(), DiagnosticCode::RESOURCE_LIMIT_EXCEEDED);
}

#[test]
fn document_parser_enforces_its_public_token_and_nesting_limits() {
    let source = "#fcs 5.0.0\nformat { profile: fragment; }";
    let token_errors = parse_document_with_limits(
        source,
        ParseLimits {
            max_tokens: 0,
            ..ParseLimits::default()
        },
    )
    .into_result()
    .expect_err("document tokens exceed the configured limit");
    assert_eq!(
        token_errors[0].code(),
        DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
    );

    let nesting_errors = parse_document_with_limits(
        source,
        ParseLimits {
            max_nesting_depth: 0,
            ..ParseLimits::default()
        },
    )
    .into_result()
    .expect_err("document nesting exceeds the configured limit");
    assert_eq!(
        nesting_errors[0].code(),
        DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
    );
}

#[test]
fn quotes_inside_comments_do_not_create_unclosed_string_diagnostics() {
    let source = "#fcs 5.0.0\n// an unrelated quote: \"\nformat { profile fragment; }";
    let errors = parse_document(source)
        .into_result()
        .expect_err("the format declaration is malformed");
    assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
}

#[test]
fn profile_recovery_ignores_comments_around_a_valid_profile_value() {
    for source in [
        "#fcs 5.0.0\nformat { profile: /* before */ fragment; } extra",
        "#fcs 5.0.0\nformat { profile: fragment /* after */; } extra",
    ] {
        let result = parse_document(source);
        assert_eq!(result.diagnostics().len(), 1);
        assert_eq!(
            result.diagnostics()[0].code(),
            DiagnosticCode::SYNTAX_TRAILING_INPUT
        );
    }
}

#[test]
fn diagnostics_are_sorted_by_span_then_code() {
    let result = parse_document("#fcs 5.0.0\nformat { profile: nope; extra }");
    let diagnostics = result.diagnostics();
    assert!(
        diagnostics.len() >= 2,
        "recovery must retain both independent errors"
    );
    assert!(diagnostics.windows(2).all(|pair| {
        let left = {
            let span = pair[0].primary_span();
            (span.start, span.end)
        };
        let right = {
            let span = pair[1].primary_span();
            (span.start, span.end)
        };
        left < right || (left == right && pair[0].code() <= pair[1].code())
    }));
}

#[test]
fn same_scope_duplicate_binding_is_distinct_from_shadowing() {
    let source = "#fcs 5.0.0\nformat { profile: fragment; }\n\
                  definitions { const A: int = 1; const A: int = 2; }";
    let document = parse_document(source)
        .into_result()
        .expect("duplicate binding is an elaboration error");
    let errors = elaborate(&document, phase2_schema(), CompileTimeLimits::default())
        .expect_err("duplicate definitions must fail");
    assert_eq!(errors[0].code(), DiagnosticCode::NAME_DUPLICATE);
    assert!(!errors[0].labels().is_empty());
}
