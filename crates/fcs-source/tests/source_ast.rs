use fcs_source::diagnostic::DiagnosticCode;
use fcs_source::{
    ast::{DocumentProfile, SourceSpan, SourceTypeKind, TopLevelBlockKind, Type},
    parser::parse_document,
    parser::parse_type,
};

#[test]
fn document_retains_format_features_and_top_level_source_order() {
    let source = "#fcs 5.0.0\n\
format { features: [playable, renderable,]; profile: chart; }\n\
meta { title: \"source\"; }\n\
tempoMap { 0beat -> 120bpm; }\n\
render profile 1.0.0 { }";

    let document = parse_document(source)
        .into_result()
        .expect("the complete document envelope is syntactically valid");

    assert_eq!(document.format.profile.value, DocumentProfile::Chart);
    assert_eq!(
        document
            .format
            .features
            .as_ref()
            .expect("features")
            .features
            .iter()
            .map(|feature| feature.value)
            .collect::<Vec<_>>(),
        [
            fcs_source::ast::ProfileFeature::Playable,
            fcs_source::ast::ProfileFeature::Renderable
        ]
    );
    assert_eq!(
        document
            .top_level_blocks()
            .iter()
            .map(|block| block.kind())
            .collect::<Vec<_>>(),
        [
            TopLevelBlockKind::Meta,
            TopLevelBlockKind::TempoMap,
            TopLevelBlockKind::Render,
        ]
    );
    let format_start = source.find("format").expect("format span");
    let format_end = source.find("meta").expect("next block");
    assert_eq!(
        document.format.span,
        SourceSpan::new(format_start, format_end - 1)
    );
}

#[test]
fn complete_source_grammar_fixture_parses_with_all_top_level_kinds() {
    let source = include_str!("../../../conformance/fcs5/source/valid/complete-source-grammar.fcs");
    let document = parse_document(source)
        .into_result()
        .expect("all Appendix B top-level envelopes are syntactically valid");

    assert_eq!(
        document
            .top_level_blocks()
            .iter()
            .map(|block| block.kind())
            .collect::<Vec<_>>(),
        [
            TopLevelBlockKind::Meta,
            TopLevelBlockKind::Contributors,
            TopLevelBlockKind::Credits,
            TopLevelBlockKind::Resources,
            TopLevelBlockKind::Artwork,
            TopLevelBlockKind::Sync,
            TopLevelBlockKind::Definitions,
            TopLevelBlockKind::TempoMap,
            TopLevelBlockKind::Lines,
            TopLevelBlockKind::Collections,
            TopLevelBlockKind::Render,
            TopLevelBlockKind::Extensions,
            TopLevelBlockKind::Preserve,
        ]
    );
}

#[test]
fn later_phase_conformance_sources_are_not_rejected_by_the_parser() {
    let sources = [
        include_str!("../../../conformance/fcs5/source/valid/minimal-chart.fcs"),
        include_str!("../../../conformance/fcs5/source/valid/compile-time-generator.fcs"),
        include_str!("../../../conformance/fcs5/source/valid/track-boundaries.fcs"),
        include_str!("../../../conformance/fcs5/source/valid/time-scroll-note.fcs"),
        include_str!("../../../conformance/fcs5/source/valid/runtime-choose.fcs"),
        include_str!("../../../conformance/fcs5/source/valid/canonical-equivalent-direct.fcs"),
        include_str!("../../../conformance/fcs5/source/valid/canonical-equivalent-template.fcs"),
        include_str!("../../../conformance/fcs5/source/valid/exact-expression-dag.fcs"),
        include_str!("../../../conformance/fcs5/source/valid/note-policies.fcs"),
        include_str!("../../../conformance/fcs5/source/valid/template-if-with.fcs"),
        include_str!("../../../conformance/fcs5/source/valid/int-range-descending.fcs"),
        include_str!("../../../conformance/fcs5/source/valid/parent-transform.fcs"),
        include_str!("../../../conformance/fcs5/source/valid/metadata-credits-resources-sync.fcs"),
        include_str!("../../../conformance/fcs5/source/invalid/unresolved-schema-enum.fcs"),
        include_str!("../../../conformance/fcs5/source/invalid/generator-zero-step.fcs"),
        include_str!("../../../conformance/fcs5/source/invalid/shadowing.fcs"),
        include_str!("../../../conformance/fcs5/source/invalid/template-missing-line.fcs"),
        include_str!("../../../conformance/fcs5/source/invalid/hold-end.fcs"),
        include_str!("../../../conformance/fcs5/source/invalid/track-overlap.fcs"),
        include_str!("../../../conformance/fcs5/source/invalid/parent-cycle.fcs"),
        include_str!("../../../conformance/fcs5/source/invalid/runtime-gameplay.fcs"),
        include_str!("../../../conformance/fcs5/source/invalid/note-policy-disabled-sound.fcs"),
        include_str!("../../../conformance/fcs5/source/invalid/unknown-resource.fcs"),
        include_str!("../../../conformance/fcs5/source/invalid/resource-path-escape.fcs"),
        include_str!("../../../conformance/fcs5/source/invalid/resource-hash-mismatch.fcs"),
        include_str!("../../../conformance/fcs5/source/invalid/custom-duplicate-key.fcs"),
    ];

    for source in sources {
        parse_document(source)
            .into_result()
            .expect("later-phase semantic behavior must not be a parser rejection");
    }
}

#[test]
fn document_boundary_diagnostics_are_stable_and_spanned() {
    let missing = "#fcs 5.0.0\n";
    let missing_error = parse_document(missing)
        .into_result()
        .expect_err("format is mandatory")[0]
        .clone();
    assert_eq!(
        missing_error.code(),
        DiagnosticCode::PROFILE_REQUIREMENT_MISSING
    );
    assert_eq!(
        missing_error.primary_span(),
        SourceSpan::new(missing.len(), missing.len())
    );

    let later = "#fcs 5.0.0\ntempoMap {}\nformat { profile: fragment; }";
    let later_error = parse_document(later)
        .into_result()
        .expect_err("format must be immediate")[0]
        .clone();
    assert_eq!(later_error.code(), DiagnosticCode::SYNTAX_MISPLACED_BLOCK);
    assert_eq!(later_error.primary_span(), SourceSpan::new(11, 19));

    let unknown = "#fcs 5.0.0\nformat { profile: fragment; }\nmystery {}";
    let unknown_error = parse_document(unknown)
        .into_result()
        .expect_err("unknown blocks are not skipped")[0]
        .clone();
    assert_eq!(unknown_error.code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
    assert_eq!(unknown_error.primary_span(), SourceSpan::new(41, 48));

    let trailing = "#fcs 5.0.0\nformat { profile: fragment; } 123";
    let trailing_error = parse_document(trailing)
        .into_result()
        .expect_err("trailing input is rejected")[0]
        .clone();
    assert_eq!(trailing_error.code(), DiagnosticCode::SYNTAX_TRAILING_INPUT);
    assert_eq!(trailing_error.primary_span(), SourceSpan::new(41, 44));
}

#[test]
fn format_field_duplicates_keep_first_and_second_spans() {
    let source = "#fcs 5.0.0\nformat { profile: fragment; profile: chart; features: [playable]; features: [renderable]; }";
    let errors = parse_document(source)
        .into_result()
        .expect_err("duplicate format fields");
    assert_eq!(errors.len(), 2);
    assert!(
        errors
            .iter()
            .all(|error| error.code() == DiagnosticCode::NAME_DUPLICATE)
    );
    assert!(errors.iter().all(|error| error.labels().len() == 1));
}

#[test]
fn source_types_preserve_nested_generic_spans_and_static_shape() {
    let source = "Track<array<vec2<bool>>>";
    let source_type = parse_type(source).into_result().unwrap();

    assert_eq!(source_type.span(), SourceSpan::new(0, 24));
    assert_eq!(
        source_type.to_type(),
        Type::Track(Box::new(Type::Array(Box::new(Type::Vec2(Box::new(
            Type::Bool
        )),))))
    );
    assert!(!source_type.is_constructible());

    let SourceTypeKind::Track(array) = source_type.kind() else {
        panic!("expected Track source type");
    };
    assert_eq!(array.span(), SourceSpan::new(6, 23));
    let SourceTypeKind::Array(vector) = array.kind() else {
        panic!("expected array source type");
    };
    assert_eq!(vector.span(), SourceSpan::new(12, 22));
    let SourceTypeKind::Vec2(element) = vector.kind() else {
        panic!("expected vec2 source type");
    };
    assert_eq!(element.span(), SourceSpan::new(17, 21));
    assert!(matches!(element.kind(), SourceTypeKind::Bool));

    let constructible = parse_type("TrackSegment<array<int>>")
        .into_result()
        .unwrap();
    assert!(constructible.is_constructible());

    let statically_invalid = parse_type("array<Note>").into_result().unwrap();
    assert_eq!(
        statically_invalid.to_type(),
        Type::Array(Box::new(Type::Note))
    );
    assert!(!statically_invalid.is_constructible());
}
