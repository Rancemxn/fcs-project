use fcs_source::ast::{Beat, Bpm, DocumentProfile, SourceSpan};
use fcs_source::diagnostic::DiagnosticCode;
use fcs_source::parser::{parse_document, parse_header};
use fcs_source::version::{
    EXECUTION_ABI_VERSION, FCBC_FORMAT_VERSION, FCS_SOURCE_VERSION, Version,
};
use std::{fs, path::PathBuf};

fn example(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/fcs")
        .join(name);
    fs::read_to_string(path).unwrap()
}

#[test]
fn parses_exact_fcs5_header() {
    let version = parse_header("#fcs 5.0.0\nformat { profile: fragment; }")
        .into_result()
        .expect("valid header");
    assert_eq!(version, FCS_SOURCE_VERSION);
}

#[test]
fn rejects_missing_or_wrong_major_header() {
    assert_eq!(
        parse_header("format { profile: fragment; }")
            .into_result()
            .expect_err("missing header")[0]
            .code(),
        DiagnosticCode::VERSION_MISSING_HEADER
    );
    for source in ["#fcs 4.1.0\n", "#fcs 5.1.0\n"] {
        assert_eq!(
            parse_header(source)
                .into_result()
                .expect_err("unsupported version")[0]
                .code(),
            DiagnosticCode::VERSION_UNSUPPORTED
        );
    }
}

#[test]
fn parses_fragment_profile() {
    let document = parse_document("#fcs 5.0.0\nformat { profile: fragment; }")
        .into_result()
        .expect("valid document");

    assert_eq!(document.profile, DocumentProfile::Fragment);
    assert_eq!(document.source_version, FCS_SOURCE_VERSION);
    assert_eq!(document.tempo_map, None);
}

#[test]
fn parses_chart_tempo_map_with_exact_beats() {
    let document = parse_document(
        "#fcs 5.0.0\nformat { profile: chart; }\ntempoMap {\n  0beat -> 180bpm;\n  4.5beat -> 200bpm;\n  [8,1,3]beat -> 220bpm;\n}",
    )
    .into_result()
    .expect("valid chart");

    let tempo_map = document.tempo_map.unwrap();
    assert_eq!(tempo_map.points[0].beat, Beat::new(0, 1).unwrap());
    assert_eq!(tempo_map.points[1].beat, Beat::new(9, 2).unwrap());
    assert_eq!(tempo_map.points[2].beat, Beat::new(25, 3).unwrap());
}

#[test]
fn rejects_tempo_maps_without_zero_start() {
    let negative_decimal =
        "#fcs 5.0.0\nformat { profile: fragment; }\ntempoMap { -0.5beat -> 180bpm; }";
    let negative_integer =
        "#fcs 5.0.0\nformat { profile: fragment; }\ntempoMap { -1beat -> 120bpm; }";
    let empty = "#fcs 5.0.0\nformat { profile: fragment; }\ntempoMap { }";

    for source in [negative_decimal, negative_integer, empty] {
        assert_eq!(
            parse_document(source)
                .into_result()
                .expect_err("invalid tempo start")[0]
                .code(),
            DiagnosticCode::TEMPO_INVALID
        );
    }
}

#[test]
fn parses_tempo_map_after_line_comment() {
    let document = parse_document(
        "#fcs 5.0.0\nformat { profile: chart; }\n// comment\ntempoMap { 0beat -> 120bpm; }",
    )
    .into_result()
    .expect("valid chart");

    assert_eq!(document.tempo_map.unwrap().points.len(), 1);
}

#[test]
fn parses_tempo_map_after_block_comment() {
    let document = parse_document(
        "#fcs 5.0.0\nformat { profile: chart; }\n/* comment */ tempoMap { 0beat -> 120bpm; }",
    )
    .into_result()
    .expect("valid chart");

    assert_eq!(document.tempo_map.unwrap().points.len(), 1);
}

#[test]
fn accepts_trailing_comments_without_tempo_map() {
    assert!(
        parse_document("#fcs 5.0.0\nformat { profile: fragment; }\n// comment")
            .into_result()
            .is_ok()
    );
    assert!(
        parse_document("#fcs 5.0.0\nformat { profile: fragment; }\n/* comment */")
            .into_result()
            .is_ok()
    );
}

#[test]
fn chart_profile_requires_tempo_starting_at_zero() {
    let missing = "#fcs 5.0.0\nformat { profile: chart; }";
    let non_zero = "#fcs 5.0.0\nformat { profile: chart; }\ntempoMap { 1beat -> 120bpm; }";
    assert_eq!(
        parse_document(missing)
            .into_result()
            .expect_err("tempoMap is required")[0]
            .code(),
        DiagnosticCode::PROFILE_REQUIREMENT_MISSING
    );
    assert_eq!(
        parse_document(non_zero)
            .into_result()
            .expect_err("tempoMap must start at zero")[0]
            .code(),
        DiagnosticCode::TEMPO_INVALID
    );
}

#[test]
fn tempo_points_must_be_non_decreasing() {
    let source = "#fcs 5.0.0\nformat { profile: chart; }\ntempoMap { 0beat -> 120bpm; 4beat -> 180bpm; 3beat -> 200bpm; }";
    assert_eq!(
        parse_document(source)
            .into_result()
            .expect_err("tempo points must be ordered")[0]
            .code(),
        DiagnosticCode::TEMPO_NON_MONOTONIC
    );
}

#[test]
fn rejects_unclosed_trailing_block_comment() {
    assert_eq!(
        parse_document("#fcs 5.0.0\nformat { profile: chart; }\n/* comment")
            .into_result()
            .expect_err("unclosed comment")[0]
            .code(),
        DiagnosticCode::SYNTAX_UNCLOSED_COMMENT
    );
}

#[test]
fn rejects_invalid_tempo_map_fraction_and_bpm() {
    let bad_fraction = parse_document(
        "#fcs 5.0.0\nformat { profile: chart; }\ntempoMap { [8,1,0]beat -> 220bpm; }",
    );
    let bad_bpm =
        parse_document("#fcs 5.0.0\nformat { profile: chart; }\ntempoMap { 0beat -> 0.0bpm; }");

    assert!(bad_fraction.into_result().is_err());
    assert!(bad_bpm.into_result().is_err());
}

#[test]
fn rejects_unknown_profile() {
    assert_eq!(
        parse_document("#fcs 5.0.0\nformat { profile: unknown; }")
            .into_result()
            .expect_err("unknown profile")[0]
            .code(),
        DiagnosticCode::SYNTAX_INVALID_TOKEN
    );
}

#[test]
fn rejects_profile_without_statement_terminator() {
    assert_eq!(
        parse_document("#fcs 5.0.0\nformat { profile: fragment }")
            .into_result()
            .expect_err("missing profile terminator")[0]
            .code(),
        DiagnosticCode::SYNTAX_INVALID_TOKEN
    );
}

#[test]
fn format_features_are_rejected_until_i1_adds_the_source_node() {
    let source = "#fcs 5.0.0\nformat { profile: fragment; features: []; }";
    assert_eq!(
        parse_document(source)
            .into_result()
            .expect_err("I0 format does not include features")[0]
            .code(),
        DiagnosticCode::SYNTAX_INVALID_TOKEN
    );
}

#[test]
fn document_rejects_misplaced_or_duplicate_top_level_blocks() {
    for source in [
        "#fcs 5.0.0\nformat { profile: fragment; }\ntemplates { }",
        "#fcs 5.0.0\nformat { profile: fragment; }\nmetadata { }",
    ] {
        assert_eq!(
            parse_document(source)
                .into_result()
                .expect_err("misplaced top-level block")[0]
                .code(),
            DiagnosticCode::SYNTAX_MISPLACED_BLOCK
        );
    }

    let duplicate = "#fcs 5.0.0\nformat { profile: fragment; }\nformat { profile: fragment; }";
    let error = parse_document(duplicate)
        .into_result()
        .expect_err("duplicate format block")
        .remove(0);
    assert_eq!(error.code(), DiagnosticCode::NAME_DUPLICATE);
    assert_eq!(error.labels().len(), 1);
}

#[test]
fn duplicate_optional_top_level_blocks_report_both_declarations() {
    let cases = [
        ("tempoMap { 0beat -> 120bpm; }", "tempoMap"),
        ("definitions { }", "definitions"),
        ("collections { }", "collections"),
    ];
    for (block, keyword) in cases {
        let source = format!("#fcs 5.0.0\nformat {{ profile: fragment; }}\n{block}\n{block}");
        let first_start = source.find(block).unwrap();
        let second_start = source.rfind(block).unwrap();
        let error = parse_document(&source)
            .into_result()
            .expect_err("duplicate top-level block")
            .remove(0);
        assert_eq!(error.code(), DiagnosticCode::NAME_DUPLICATE, "{keyword}");
        assert_eq!(
            error.primary_span(),
            SourceSpan::new(second_start, second_start + keyword.len()),
            "{keyword}"
        );
        assert_eq!(error.labels().len(), 1, "{keyword}");
        assert_eq!(
            error.labels()[0].span(),
            SourceSpan::new(first_start, first_start + block.len()),
            "{keyword}"
        );
    }
}

#[test]
fn parses_profile_with_line_and_block_comments() {
    let document = parse_document(
        "#fcs 5.0.0\nformat {\n // leading }\n /* block { } */\n profile: fragment; /* trailing } */\n}",
    )
    .into_result()
    .expect("valid commented format");

    assert_eq!(document.profile, DocumentProfile::Fragment);
}

#[test]
fn ignores_braces_in_unclosed_format_string() {
    assert_eq!(
        parse_document("#fcs 5.0.0\nformat { profile: fragment; \"}\"")
            .into_result()
            .expect_err("unclosed format string")[0]
            .code(),
        DiagnosticCode::SYNTAX_INVALID_TOKEN
    );
}

#[test]
fn exposes_independent_fcs_fcbc_and_abi_versions() {
    assert_eq!(FCS_SOURCE_VERSION, Version::new(5, 0, 0));
    assert_eq!(FCBC_FORMAT_VERSION, Version::new(2, 0, 0));
    assert_eq!(EXECUTION_ABI_VERSION, Version::new(1, 0, 0));
    assert_eq!(FCS_SOURCE_VERSION.to_string(), "5.0.0");
}

#[test]
fn beat_arithmetic_is_exact_and_normalized() {
    let one_third = Beat::new(1, 3).unwrap();
    let two_thirds = Beat::new(2, 3).unwrap();
    assert_eq!(
        one_third.checked_add(two_thirds).unwrap(),
        Beat::new(1, 1).unwrap()
    );
    assert_eq!(Beat::new(2, 6).unwrap(), one_third);
}

#[test]
fn accepts_minimum_i64_denominator_when_result_is_representable() {
    assert_eq!(
        Beat::new(i64::MIN, i64::MIN).unwrap(),
        Beat::new(1, 1).unwrap()
    );
    assert_eq!(Beat::new(0, i64::MIN).unwrap(), Beat::new(0, 1).unwrap());
    assert_eq!(
        Beat::new(2, i64::MIN).unwrap(),
        Beat::new(-1, 1_i64 << 62).unwrap()
    );
}

#[test]
fn checked_add_uses_wide_intermediates_for_exact_results() {
    let a = Beat::new(i64::MAX - 1, i64::MAX).unwrap();
    let b = Beat::new(-(i64::MAX - 1), i64::MAX).unwrap();
    assert_eq!(a.checked_add(b).unwrap(), Beat::new(0, 1).unwrap());
}

#[test]
fn rejects_zero_denominator_and_invalid_bpm() {
    assert!(Beat::new(1, 0).is_err());
    assert!(Bpm::new(0.0).is_err());
    assert!(Bpm::new(-1.0).is_err());
    assert!(Bpm::new(f64::NAN).is_err());
    assert!(Bpm::new(f64::INFINITY).is_err());
    assert!(Bpm::new(f64::NEG_INFINITY).is_err());
    assert_eq!(Bpm::new(180.0).unwrap().get(), 180.0);
}

#[test]
fn parses_public_fcs5_fixtures() {
    let fragment = parse_document(&example("fragment.fcs"))
        .into_result()
        .expect("fragment fixture");
    let chart = parse_document(&example("chart.fcs"))
        .into_result()
        .expect("chart fixture");
    assert_eq!(fragment.profile, DocumentProfile::Fragment);
    assert_eq!(chart.profile, DocumentProfile::Chart);
    assert_eq!(chart.tempo_map.unwrap().points.len(), 2);
}
