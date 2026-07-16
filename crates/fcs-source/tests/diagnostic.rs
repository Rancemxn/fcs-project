use fcs_source::ast::SourceSpan;
use fcs_source::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage};
use fcs_source::elaborator::{CompileTimeLimits, elaborate};
use fcs_source::parser::{
    ParseLimits, parse_document, parse_document_with_limits, parse_expression,
    parse_expression_with_limits, parse_header, parse_type_with_limits,
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
fn bound_parse_error_fixtures_keep_stable_categories_and_spans() {
    let cases = [
        (
            include_str!("../../../conformance/fcs5/source/invalid/missing-header.fcs"),
            DiagnosticCode::VERSION_MISSING_HEADER,
        ),
        (
            include_str!("../../../conformance/fcs5/source/invalid/header-extra-space.fcs"),
            DiagnosticCode::VERSION_INVALID,
        ),
        (
            include_str!("../../../conformance/fcs5/source/invalid/header-leading-zero.fcs"),
            DiagnosticCode::VERSION_INVALID,
        ),
        (
            include_str!(
                "../../../conformance/fcs5/source/invalid/duplicate-top-level-block.fcs"
            ),
            DiagnosticCode::NAME_DUPLICATE,
        ),
        (
            include_str!("../../../conformance/fcs5/source/invalid/nested-generator.fcs"),
            DiagnosticCode::COMPILE_TIME_NESTED_GENERATOR,
        ),
        (
            include_str!("../../../conformance/fcs5/source/invalid/misplaced-generator.fcs"),
            DiagnosticCode::COMPILE_TIME_MISPLACED_GENERATOR,
        ),
        (
            include_str!(
                "../../../conformance/fcs5/source/invalid/unclosed-extension-payload.fcs"
            ),
            DiagnosticCode::SYNTAX_INVALID_TOKEN,
        ),
        (
            include_str!("../../../conformance/fcs5/source/invalid/mixed-beat-literal.fcs"),
            DiagnosticCode::SYNTAX_INVALID_TOKEN,
        ),
        (
            include_str!("../../../conformance/fcs5/source/invalid/bare-range.fcs"),
            DiagnosticCode::SYNTAX_INVALID_TOKEN,
        ),
    ];

    for (source, expected_code) in cases {
        let output = parse_document(source);
        assert!(output.output().is_none(), "parser must not expose partial AST");
        let diagnostics = output.diagnostics();
        assert!(!diagnostics.is_empty(), "fixture must produce a diagnostic");
        assert_eq!(diagnostics[0].code(), expected_code, "{source}");
        assert!(diagnostics.iter().all(|diagnostic| {
            let span = diagnostic.primary_span();
            diagnostic.stage() == DiagnosticStage::Parse
                && span.start <= span.end
                && span.end <= source.len()
                && source.is_char_boundary(span.start)
                && source.is_char_boundary(span.end)
        }));
    }
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
fn additional_bom_and_non_ascii_identifier_spans_are_exact() {
    let second_bom = "\u{feff}\u{feff}#fcs 5.0.0\nformat { profile: fragment; }";
    let errors = parse_document(second_bom)
        .into_result()
        .expect_err("only one leading BOM is allowed");
    assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
    assert_eq!(errors[0].primary_span(), SourceSpan::new(3, 6));

    let interior_bom = "1\u{feff}+2";
    let errors = parse_expression(interior_bom)
        .into_result()
        .expect_err("an interior BOM is not expression trivia");
    assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
    assert_eq!(errors[0].primary_span(), SourceSpan::new(1, 4));

    parse_expression("\"\u{feff}\"")
        .into_result()
        .expect("U+FEFF remains an ordinary string character away from the file start");
    parse_expression("/*\u{feff}*/ 1")
        .into_result()
        .expect("U+FEFF remains an ordinary comment character away from the file start");

    for (source, expected) in [
        ("\u{53d8}\u{91cf}", SourceSpan::new(0, 3)),
        ("ascii.\u{503c}", SourceSpan::new(6, 9)),
        ("\u{e9}clair", SourceSpan::new(0, 2)),
    ] {
        let errors = parse_expression(source)
            .into_result()
            .expect_err("identifiers are ASCII-only");
        assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
        assert_eq!(errors[0].primary_span(), expected, "{source:?}");
    }
}

#[test]
fn nul_and_unicode_noncharacters_obey_the_lexical_boundary() {
    parse_expression(r#""\0""#)
        .into_result()
        .expect("the explicit NUL string escape is valid");

    let raw_cases = [
        ("\0".to_owned(), SourceSpan::new(0, 1)),
        ("\"\0\"".to_owned(), SourceSpan::new(1, 2)),
        ("/*\0*/ 1".to_owned(), SourceSpan::new(2, 3)),
        (
            format!("\"{}\"", '\u{fdd0}'),
            SourceSpan::new(1, 1 + '\u{fdd0}'.len_utf8()),
        ),
        (
            format!("/*{}*/ 1", '\u{10ffff}'),
            SourceSpan::new(2, 2 + '\u{10ffff}'.len_utf8()),
        ),
    ];
    for (source, expected_span) in raw_cases {
        let errors = parse_expression(&source)
            .into_result()
            .expect_err("forbidden source scalar must fail");
        assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
        assert_eq!(errors[0].primary_span(), expected_span, "{source:?}");
    }

    for source in [r#""\u{FDD0}""#, r#""\u{10FFFF}""#] {
        let errors = parse_expression(source)
            .into_result()
            .expect_err("an escaped noncharacter must fail");
        assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
    }
}

#[test]
fn invalid_unit_adjacency_is_one_lexical_error() {
    for source in ["1foo", "1msx", "1beatExtra", "1_bogus"] {
        let errors = parse_expression(source)
            .into_result()
            .expect_err("an unknown adjacent suffix must fail");
        assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
        assert_eq!(
            errors[0].primary_span(),
            SourceSpan::new(0, source.len()),
            "{source}"
        );
    }
}

#[test]
fn malformed_numeric_candidates_are_one_lexical_error() {
    for source in ["01", "00.1", "01e2", "1e", "1e+", "1e-"] {
        let errors = parse_expression(source)
            .into_result()
            .expect_err("a malformed decimal candidate must fail");
        assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
        assert_eq!(
            errors[0].primary_span(),
            SourceSpan::new(0, source.len()),
            "{source}"
        );
    }
}

#[test]
fn malformed_color_string_and_comment_spans_are_stable() {
    for source in ["#12345", "#GGGGGG", "#123456789"] {
        let errors = parse_expression(source)
            .into_result()
            .expect_err("a malformed color literal must fail");
        assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
        assert_eq!(
            errors[0].primary_span(),
            SourceSpan::new(0, source.len()),
            "{source}"
        );
    }

    let raw_newline = "\"line\nnext\"";
    let errors = parse_expression(raw_newline)
        .into_result()
        .expect_err("a raw newline cannot continue a string");
    assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_UNCLOSED_STRING);
    assert_eq!(errors[0].primary_span(), SourceSpan::new(5, 6));

    for (source, code) in [
        ("\"unterminated", DiagnosticCode::SYNTAX_UNCLOSED_STRING),
        ("/* unterminated", DiagnosticCode::SYNTAX_UNCLOSED_COMMENT),
    ] {
        let errors = parse_expression(source)
            .into_result()
            .expect_err("an unclosed lexical construct must fail");
        assert_eq!(errors[0].code(), code);
        assert_eq!(
            errors[0].primary_span(),
            SourceSpan::new(source.len(), source.len()),
            "{source:?}"
        );
    }
}

#[test]
fn source_parser_retains_tempo_diagnostics_for_later_validation() {
    let source = "#fcs 5.0.0\nformat { profile: chart; }\n\
                  tempoMap { 4beat -> 180bpm; }";
    let document = parse_document(source)
        .into_result()
        .expect("tempo validity belongs to canonical validation");
    let tempo_start = source.find("tempoMap").unwrap();
    assert_eq!(
        document.tempo_map.as_ref().unwrap().points[0]
            .beat
            .numerator(),
        4
    );
    assert_eq!(tempo_start, document.top_level_blocks()[0].span().start);
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
fn expression_and_type_depth_limits_report_budget_details() {
    let limits = ParseLimits {
        max_nesting_depth: 1,
        ..ParseLimits::default()
    };
    let assert_budget = |diagnostic: &Diagnostic| {
        let budget = diagnostic.budget().expect("depth limit budget details");
        assert_eq!(budget.kind(), "max_nesting_depth");
        assert_eq!(budget.limit(), 1);
        assert_eq!(budget.observed(), 2);
    };

    let expression = parse_expression_with_limits("!!1", limits);
    assert_eq!(
        expression.diagnostics()[0].code(),
        DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
    );
    assert_budget(&expression.diagnostics()[0]);

    let ty = parse_type_with_limits("vec2<vec2<length>>", limits);
    assert_eq!(
        ty.diagnostics()[0].code(),
        DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
    );
    assert_budget(&ty.diagnostics()[0]);
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
        !diagnostics.is_empty(),
        "malformed format must be diagnosed"
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
