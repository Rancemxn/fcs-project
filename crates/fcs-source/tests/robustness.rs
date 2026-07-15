use fcs_source::{
    ast::SourceSpan,
    diagnostic::{DiagnosticCode, DiagnosticStage, ParseOutput},
    parser::{
        ParseLimits, parse_document, parse_document_bytes, parse_document_bytes_with_limits,
        parse_document_with_limits,
    },
};
use proptest::{
    prelude::*,
    test_runner::{RngAlgorithm, RngSeed},
};

const VALID_SOURCE: &str = "#fcs 5.0.0\nformat { profile: fragment; }";

fn deterministic_config() -> ProptestConfig {
    ProptestConfig {
        cases: 512,
        failure_persistence: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(0xF0C5_0001),
        ..ProptestConfig::default()
    }
}

fn assert_spans_are_bounded<T>(output: &ParseOutput<T>, source: &[u8], valid_utf8: bool) {
    for diagnostic in output.diagnostics() {
        for span in std::iter::once(diagnostic.primary_span())
            .chain(diagnostic.labels().iter().map(|label| label.span()))
            .chain(
                diagnostic
                    .expansion_trace()
                    .iter()
                    .filter_map(|frame| frame.span()),
            )
        {
            assert!(span.start <= span.end, "reversed diagnostic span: {span:?}");
            assert!(
                span.end <= source.len(),
                "out-of-bounds diagnostic span: {span:?}"
            );
            if valid_utf8 {
                let text = std::str::from_utf8(source).expect("caller checked UTF-8");
                assert!(text.is_char_boundary(span.start));
                assert!(text.is_char_boundary(span.end));
            }
        }
    }
    if !output.diagnostics().is_empty() {
        assert!(output.output().is_none());
    }
}

#[test]
fn byte_entry_decodes_once_and_preserves_utf8_error_spans() {
    assert_eq!(
        parse_document_bytes(VALID_SOURCE.as_bytes()),
        parse_document(VALID_SOURCE)
    );

    let invalid_at = VALID_SOURCE
        .find("fragment")
        .expect("fixture contains profile");
    let mut malformed = VALID_SOURCE.as_bytes().to_vec();
    malformed[invalid_at] = 0xFF;
    let output = parse_document_bytes(&malformed);
    assert_eq!(output.diagnostics().len(), 1);
    assert_eq!(
        output.diagnostics()[0].code(),
        DiagnosticCode::DECODE_INVALID_UTF8
    );
    assert_eq!(output.diagnostics()[0].stage(), DiagnosticStage::Decode);
    assert_eq!(
        output.diagnostics()[0].primary_span(),
        SourceSpan::new(invalid_at, invalid_at + 1)
    );

    let prefix = b"#fcs 5.0.0\n";
    let mut incomplete = prefix.to_vec();
    incomplete.extend_from_slice(&[0xE2, 0x82]);
    let output = parse_document_bytes(&incomplete);
    assert_eq!(output.diagnostics().len(), 1);
    assert_eq!(
        output.diagnostics()[0].code(),
        DiagnosticCode::DECODE_INVALID_UTF8
    );
    assert_eq!(output.diagnostics()[0].stage(), DiagnosticStage::Decode);
    assert_eq!(
        output.diagnostics()[0].primary_span(),
        SourceSpan::new(prefix.len(), incomplete.len())
    );
}

#[test]
fn every_parser_limit_has_a_bounded_failure() {
    let cases = [
        (
            VALID_SOURCE,
            ParseLimits {
                max_source_bytes: VALID_SOURCE.len() - 1,
                ..ParseLimits::default()
            },
        ),
        (
            VALID_SOURCE,
            ParseLimits {
                max_tokens: 1,
                ..ParseLimits::default()
            },
        ),
        (
            "#fcs 5.0.0\nformat { profile: fragment; }\ndefinitions { const X: int = (((1))); }",
            ParseLimits {
                max_nesting_depth: 1,
                ..ParseLimits::default()
            },
        ),
        (
            "#fcs 5.0.0\n/* /* nested */ */ format { profile: fragment; }",
            ParseLimits {
                max_comment_depth: 1,
                ..ParseLimits::default()
            },
        ),
        (
            "#fcs 5.0.0\nformat { profile: fragment; }\ndefinitions { const X: string = \"long\"; }",
            ParseLimits {
                max_literal_bytes: 3,
                ..ParseLimits::default()
            },
        ),
    ];

    for (source, limits) in cases {
        let output = parse_document_with_limits(source, limits);
        assert_eq!(
            output.diagnostics()[0].code(),
            DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
        );
        assert_spans_are_bounded(&output, source.as_bytes(), true);
    }
}

proptest! {
    #![proptest_config(deterministic_config())]

    #[test]
    fn arbitrary_bytes_never_escape_decode_or_parse_boundaries(source in prop::collection::vec(any::<u8>(), 0..512)) {
        let first = parse_document_bytes(&source);
        let second = parse_document_bytes(&source);
        prop_assert_eq!(&first, &second);
        assert_spans_are_bounded(&first, &source, std::str::from_utf8(&source).is_ok());

        if std::str::from_utf8(&source).is_err() {
            prop_assert_eq!(first.diagnostics().len(), 1);
            prop_assert_eq!(first.diagnostics()[0].code(), DiagnosticCode::DECODE_INVALID_UTF8);
            prop_assert_eq!(first.diagnostics()[0].stage(), DiagnosticStage::Decode);
        }
    }

    #[test]
    fn arbitrary_utf8_is_deterministic_and_has_character_boundary_spans(source in ".{0,256}") {
        let first = parse_document(&source);
        let second = parse_document(&source);
        prop_assert_eq!(&first, &second);
        assert_spans_are_bounded(&first, source.as_bytes(), true);
    }

    #[test]
    fn byte_limits_are_deterministic(source in prop::collection::vec(any::<u8>(), 0..512), max_source_bytes in 0usize..256) {
        let limits = ParseLimits {
            max_source_bytes,
            ..ParseLimits::default()
        };
        let first = parse_document_bytes_with_limits(&source, limits);
        let second = parse_document_bytes_with_limits(&source, limits);
        prop_assert_eq!(&first, &second);
        assert_spans_are_bounded(&first, &source, std::str::from_utf8(&source).is_ok());
    }
}
