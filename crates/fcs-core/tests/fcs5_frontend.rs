use fcs_core::v5::ast::{Beat, Bpm, DocumentProfile};
use fcs_core::v5::parser::{ParseError, parse_document, parse_header};
use fcs_core::v5::version::{
    EXECUTION_ABI_VERSION, FCBC_FORMAT_VERSION, FCS_SOURCE_VERSION, Version,
};

#[test]
fn parses_exact_fcs5_header() {
    let (rest, version) = parse_header("#fcs 5.0.0\nformat { profile: fragment; }").unwrap();
    assert_eq!(version, FCS_SOURCE_VERSION);
    assert_eq!(rest, "format { profile: fragment; }");
}

#[test]
fn rejects_missing_or_wrong_major_header() {
    assert_eq!(
        parse_header("format { profile: fragment; }"),
        Err(ParseError::MissingHeader)
    );
    assert_eq!(
        parse_header("#fcs 4.1.0\n"),
        Err(ParseError::UnsupportedSourceVersion(Version::new(4, 1, 0)))
    );
    assert_eq!(
        parse_header("#fcs 5.1.0\n"),
        Err(ParseError::UnsupportedSourceVersion(Version::new(5, 1, 0)))
    );
}

#[test]
fn parses_fragment_profile() {
    let document = parse_document("#fcs 5.0.0\nformat { profile: fragment; }").unwrap();

    assert_eq!(document.profile, DocumentProfile::Fragment);
    assert_eq!(document.source_version, FCS_SOURCE_VERSION);
    assert_eq!(document.tempo_map, None);
}

#[test]
fn parses_chart_tempo_map_with_exact_beats() {
    let document = parse_document(
        "#fcs 5.0.0\nformat { profile: chart; }\ntempoMap {\n  0beat -> 180bpm;\n  4.5beat -> 200bpm;\n  [8,1,3]beat -> 220bpm;\n}",
    )
    .unwrap();

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

    assert_eq!(
        parse_document(negative_decimal),
        Err(ParseError::InvalidTempoMap("first beat must be zero"))
    );
    assert_eq!(
        parse_document(negative_integer),
        Err(ParseError::InvalidTempoMap("first beat must be zero"))
    );
    assert_eq!(
        parse_document(empty),
        Err(ParseError::InvalidTempoMap("first beat must be zero"))
    );
}

#[test]
fn parses_tempo_map_after_line_comment() {
    let document = parse_document(
        "#fcs 5.0.0\nformat { profile: chart; }\n// comment\ntempoMap { 0beat -> 120bpm; }",
    )
    .unwrap();

    assert_eq!(document.tempo_map.unwrap().points.len(), 1);
}

#[test]
fn parses_tempo_map_after_block_comment() {
    let document = parse_document(
        "#fcs 5.0.0\nformat { profile: chart; }\n/* comment */ tempoMap { 0beat -> 120bpm; }",
    )
    .unwrap();

    assert_eq!(document.tempo_map.unwrap().points.len(), 1);
}

#[test]
fn accepts_trailing_comments_without_tempo_map() {
    assert!(parse_document("#fcs 5.0.0\nformat { profile: fragment; }\n// comment").is_ok());
    assert!(parse_document("#fcs 5.0.0\nformat { profile: fragment; }\n/* comment */").is_ok());
}

#[test]
fn chart_profile_requires_tempo_starting_at_zero() {
    let missing = "#fcs 5.0.0\nformat { profile: chart; }";
    let non_zero = "#fcs 5.0.0\nformat { profile: chart; }\ntempoMap { 1beat -> 120bpm; }";
    assert!(matches!(
        parse_document(missing),
        Err(ParseError::MissingRequiredBlock("tempoMap"))
    ));
    assert!(matches!(
        parse_document(non_zero),
        Err(ParseError::InvalidTempoMap("first beat must be zero"))
    ));
}

#[test]
fn tempo_points_must_be_non_decreasing() {
    let source = "#fcs 5.0.0\nformat { profile: chart; }\ntempoMap { 0beat -> 120bpm; 4beat -> 180bpm; 3beat -> 200bpm; }";
    assert!(matches!(
        parse_document(source),
        Err(ParseError::InvalidTempoMap("beats must be non-decreasing"))
    ));
}

#[test]
fn rejects_unclosed_trailing_block_comment() {
    assert_eq!(
        parse_document("#fcs 5.0.0\nformat { profile: chart; }\n/* comment"),
        Err(ParseError::InvalidSyntax("trailing document input"))
    );
}

#[test]
fn rejects_invalid_tempo_map_fraction_and_bpm() {
    let bad_fraction = parse_document(
        "#fcs 5.0.0\nformat { profile: chart; }\ntempoMap { [8,1,0]beat -> 220bpm; }",
    );
    let bad_bpm =
        parse_document("#fcs 5.0.0\nformat { profile: chart; }\ntempoMap { 0beat -> 0.0bpm; }");

    assert!(bad_fraction.is_err());
    assert!(bad_bpm.is_err());
}

#[test]
fn rejects_unknown_profile() {
    assert_eq!(
        parse_document("#fcs 5.0.0\nformat { profile: unknown; }").unwrap_err(),
        ParseError::InvalidSyntax("document profile")
    );
}

#[test]
fn rejects_profile_without_statement_terminator() {
    assert_eq!(
        parse_document("#fcs 5.0.0\nformat { profile: fragment }").unwrap_err(),
        ParseError::InvalidSyntax("document profile")
    );
}

#[test]
fn parses_profile_with_line_and_block_comments() {
    let document = parse_document(
        "#fcs 5.0.0\nformat {\n // leading }\n /* block { } */\n profile: fragment; /* trailing } */\n}",
    )
    .unwrap();

    assert_eq!(document.profile, DocumentProfile::Fragment);
}

#[test]
fn ignores_braces_in_unclosed_format_string() {
    assert_eq!(
        parse_document("#fcs 5.0.0\nformat { profile: fragment; \"}\"").unwrap_err(),
        ParseError::InvalidSyntax("format block")
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
