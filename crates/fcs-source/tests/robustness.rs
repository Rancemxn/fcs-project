use fcs_source::{
    ast::SourceSpan,
    diagnostic::{DiagnosticCode, DiagnosticStage, ParseOutput},
    parser::{
        ParseLimits, parse_document, parse_document_bytes, parse_document_bytes_with_limits,
        parse_document_with_limits, parse_expression_with_limits,
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
            "max_source_bytes",
            VALID_SOURCE.len() - 1,
        ),
        (
            VALID_SOURCE,
            ParseLimits {
                max_tokens: 1,
                ..ParseLimits::default()
            },
            "max_tokens",
            1,
        ),
        (
            "#fcs 5.0.0\nformat { profile: fragment; }\ndefinitions { const X: int = (((1))); }",
            ParseLimits {
                max_nesting_depth: 1,
                ..ParseLimits::default()
            },
            "max_nesting_depth",
            1,
        ),
        (
            "#fcs 5.0.0\n/* /* nested */ */ format { profile: fragment; }",
            ParseLimits {
                max_comment_depth: 1,
                ..ParseLimits::default()
            },
            "max_comment_depth",
            1,
        ),
        (
            "#fcs 5.0.0\nformat { profile: fragment; }\ndefinitions { const X: string = \"long\"; }",
            ParseLimits {
                max_literal_bytes: 3,
                ..ParseLimits::default()
            },
            "max_literal_bytes",
            3,
        ),
    ];

    for (source, limits, kind, limit) in cases {
        let output = parse_document_with_limits(source, limits);
        assert_eq!(
            output.diagnostics()[0].code(),
            DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
        );
        let budget = output.diagnostics()[0]
            .budget()
            .expect("resource diagnostics carry structured budget details");
        assert_eq!(budget.kind(), kind);
        assert_eq!(budget.limit(), limit);
        assert!(budget.observed() > limit);
        assert_spans_are_bounded(&output, source.as_bytes(), true);
    }
}

#[test]
fn token_and_literal_limits_stop_at_the_first_excess_unit() {
    let token_output = parse_document_with_limits(
        VALID_SOURCE,
        ParseLimits {
            max_tokens: 1,
            ..ParseLimits::default()
        },
    );
    let token_budget = token_output.diagnostics()[0]
        .budget()
        .expect("token limit details");
    assert_eq!(token_budget.kind(), "max_tokens");
    assert_eq!(token_budget.observed(), 2);

    let literal_source =
        "#fcs 5.0.0\nformat { profile: fragment; }\ndefinitions { const X: string = \"long\"; }";
    let literal_output = parse_document_with_limits(
        literal_source,
        ParseLimits {
            max_literal_bytes: 3,
            ..ParseLimits::default()
        },
    );
    let literal_budget = literal_output.diagnostics()[0]
        .budget()
        .expect("literal limit details");
    assert_eq!(literal_budget.kind(), "max_literal_bytes");
    assert_eq!(literal_budget.observed(), 4);
}

#[test]
fn token_payload_limit_bounds_identifier_allocation() {
    let identifier = "identifier";
    let failure = parse_expression_with_limits(
        identifier,
        ParseLimits {
            max_token_bytes: identifier.len() - 1,
            ..ParseLimits::default()
        },
    );
    let budget = failure.diagnostics()[0]
        .budget()
        .expect("token payload limit details");
    assert_eq!(budget.kind(), "max_token_bytes");
    assert_eq!(budget.limit(), identifier.len() - 1);
    assert_eq!(budget.observed(), identifier.len());

    parse_expression_with_limits(
        identifier,
        ParseLimits {
            max_token_bytes: identifier.len(),
            ..ParseLimits::default()
        },
    )
    .into_result()
    .expect("a token exactly at the payload limit is valid");
}

#[test]
fn token_payload_limit_covers_header_and_literal_tokens() {
    let header = "#fcs 5.0.0\n";
    let document = format!("{header}format {{ profile: fragment; }}");
    for max_token_bytes in [header.len(), header.len() + 1] {
        parse_document_with_limits(
            &document,
            ParseLimits {
                max_token_bytes,
                ..ParseLimits::default()
            },
        )
        .into_result()
        .expect("a header at or below the token payload limit is valid");
    }
    let header_failure = parse_document_with_limits(
        &document,
        ParseLimits {
            max_token_bytes: header.len() - 1,
            ..ParseLimits::default()
        },
    );
    let header_budget = header_failure.diagnostics()[0]
        .budget()
        .expect("header token payload budget");
    assert_eq!(header_budget.kind(), "max_token_bytes");
    assert_eq!(header_budget.limit(), header.len() - 1);
    assert_eq!(header_budget.observed(), header.len());

    for source in ["12345", "#102030", r#""abc""#] {
        for max_token_bytes in [source.len(), source.len() + 1] {
            parse_expression_with_limits(
                source,
                ParseLimits {
                    max_token_bytes,
                    ..ParseLimits::default()
                },
            )
            .into_result()
            .expect("a literal at or below the token payload limit is valid");
        }

        let failure = parse_expression_with_limits(
            source,
            ParseLimits {
                max_token_bytes: source.len() - 1,
                ..ParseLimits::default()
            },
        );
        let budget = failure.diagnostics()[0]
            .budget()
            .expect("literal token payload budget");
        assert_eq!(budget.kind(), "max_token_bytes", "{source}");
        assert_eq!(budget.limit(), source.len() - 1, "{source}");
        assert_eq!(budget.observed(), source.len(), "{source}");
    }
}

#[test]
fn every_public_parser_limit_has_exact_boundary_evidence() {
    for max_source_bytes in [VALID_SOURCE.len(), VALID_SOURCE.len() + 1] {
        parse_document_with_limits(
            VALID_SOURCE,
            ParseLimits {
                max_source_bytes,
                ..ParseLimits::default()
            },
        )
        .into_result()
        .expect("source at or below the byte limit is valid");
    }
    assert_eq!(
        parse_document_with_limits(
            VALID_SOURCE,
            ParseLimits {
                max_source_bytes: VALID_SOURCE.len() - 1,
                ..ParseLimits::default()
            },
        )
        .diagnostics()[0]
            .budget()
            .expect("source byte budget")
            .observed(),
        VALID_SOURCE.len()
    );

    for max_tokens in [3, 4] {
        parse_expression_with_limits(
            "a+b",
            ParseLimits {
                max_tokens,
                ..ParseLimits::default()
            },
        )
        .into_result()
        .expect("token count at or below the limit is valid");
    }
    assert_eq!(
        parse_expression_with_limits(
            "a+b",
            ParseLimits {
                max_tokens: 2,
                ..ParseLimits::default()
            },
        )
        .diagnostics()[0]
            .budget()
            .expect("token count budget")
            .observed(),
        3
    );

    let identifier = "identifier";
    for max_token_bytes in [identifier.len(), identifier.len() + 1] {
        parse_expression_with_limits(
            identifier,
            ParseLimits {
                max_token_bytes,
                ..ParseLimits::default()
            },
        )
        .into_result()
        .expect("token payload at or below the limit is valid");
    }
    assert_eq!(
        parse_expression_with_limits(
            identifier,
            ParseLimits {
                max_token_bytes: identifier.len() - 1,
                ..ParseLimits::default()
            },
        )
        .diagnostics()[0]
            .budget()
            .expect("token payload budget")
            .observed(),
        identifier.len()
    );

    let string = r#""abc""#;
    for max_literal_bytes in [string.len(), string.len() + 1] {
        parse_expression_with_limits(
            string,
            ParseLimits {
                max_literal_bytes,
                ..ParseLimits::default()
            },
        )
        .into_result()
        .expect("literal at or below the limit is valid");
    }
    assert_eq!(
        parse_expression_with_limits(
            string,
            ParseLimits {
                max_literal_bytes: string.len() - 1,
                ..ParseLimits::default()
            },
        )
        .diagnostics()[0]
            .budget()
            .expect("literal budget")
            .observed(),
        string.len()
    );

    for max_nesting_depth in [2, 3] {
        parse_expression_with_limits(
            "((1))",
            ParseLimits {
                max_nesting_depth,
                ..ParseLimits::default()
            },
        )
        .into_result()
        .expect("nesting at or below the limit is valid");
    }
    assert_eq!(
        parse_expression_with_limits(
            "((1))",
            ParseLimits {
                max_nesting_depth: 1,
                ..ParseLimits::default()
            },
        )
        .diagnostics()[0]
            .budget()
            .expect("nesting budget")
            .observed(),
        2
    );

    let nested_comment = "#fcs 5.0.0\n/* outer /* inner */ */ format { profile: fragment; }";
    for max_comment_depth in [2, 3] {
        parse_document_with_limits(
            nested_comment,
            ParseLimits {
                max_comment_depth,
                ..ParseLimits::default()
            },
        )
        .into_result()
        .expect("comment nesting at or below the limit is valid");
    }
    assert_eq!(
        parse_document_with_limits(
            nested_comment,
            ParseLimits {
                max_comment_depth: 1,
                ..ParseLimits::default()
            },
        )
        .diagnostics()[0]
            .budget()
            .expect("comment depth budget")
            .observed(),
        2
    );
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
