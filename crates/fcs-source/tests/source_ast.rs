use fcs_source::diagnostic::{DiagnosticCode, DiagnosticStage};
use fcs_source::{
    ast::{
        CollectionItem, Definition, DocumentProfile, ExtensionRequirement, FunctionStatement,
        GeneratorOwner, LineBodyItem, PreserveItem, ResourceKind, SchemaValue,
        SourceEntityConstructorKind, SourceExpression, SourceSpan, SourceTypeKind, TopLevelBlock,
        TopLevelBlockKind, TrackSegmentItem, Type,
    },
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

#[test]
fn definitions_retain_else_if_as_a_nested_typed_statement() {
    let source = "#fcs 5.0.0\nformat { profile: fragment; }\ndefinitions {\n\
        fn choose(flag: bool) -> int {\n\
            if flag { return 1; } else if false { return 2; } else { return 3; }\n\
        }\n\
    }";
    let document = parse_document(source)
        .into_result()
        .expect("else-if is valid definition syntax");
    let definitions = document.definitions.as_ref().expect("definitions");
    let Definition::Function(function) = &definitions.declarations[0] else {
        panic!("expected function");
    };
    let FunctionStatement::If(root) = &function.body[0] else {
        panic!("expected outer if");
    };
    assert!(matches!(
        root.else_branch.as_slice(),
        [FunctionStatement::If(_)]
    ));
}

#[test]
fn template_declarations_retain_nonconstructible_types_for_later_validation() {
    let source = "#fcs 5.0.0\nformat { profile: fragment; }\ndefinitions {\n\
        template int invalid() { return tap { gameplay.time: 0beat; }; }\n\
    }";
    let document = parse_document(source)
        .into_result()
        .expect("constructibility is an I2 semantic check");
    let definitions = document.definitions.as_ref().expect("definitions");
    let Definition::Template(template) = &definitions.declarations[0] else {
        panic!("expected template");
    };
    assert_eq!(template.return_type, Type::Int);
}

#[test]
fn template_returns_reject_value_only_expression_forms() {
    let source = "#fcs 5.0.0\nformat { profile: fragment; }\ndefinitions {\n\
        template Note invalid() { return 1; }\n\
    }";
    let errors = parse_document(source)
        .into_result()
        .expect_err("template return must use entityExpression syntax");
    assert_eq!(
        errors[0].code(),
        fcs_source::diagnostic::DiagnosticCode::SYNTAX_INVALID_TOKEN
    );
}

#[test]
fn definition_bodies_reject_owner_invalid_generator_and_entity_statements() {
    for (body, expected_code) in [
        (
            "emit tap { gameplay.time: 0beat; };",
            DiagnosticCode::SYNTAX_INVALID_TOKEN,
        ),
        (
            "generate i: int in 0..<1 step 1 { emit tap { gameplay.time: 0beat; }; }",
            DiagnosticCode::COMPILE_TIME_MISPLACED_GENERATOR,
        ),
        (
            "return tap { gameplay.time: 0beat; };",
            DiagnosticCode::SYNTAX_INVALID_TOKEN,
        ),
    ] {
        let source = format!(
            "#fcs 5.0.0\nformat {{ profile: fragment; }}\ndefinitions {{ fn invalid() -> int {{ {body} }} }}"
        );
        let errors = parse_document(&source)
            .into_result()
            .expect_err("function bodies only accept let/if/value-return statements");
        assert_eq!(errors[0].code(), expected_code, "{body}");
    }
}

#[test]
fn malformed_definition_body_does_not_swallow_following_declaration() {
    let source = "#fcs 5.0.0\nformat { profile: fragment; }\ndefinitions {\n\
        fn broken() -> int { generate i: int in 0..<1 step 1 { } }\n\
        const after: int = 1;\n\
    }";
    let errors = parse_document(source)
        .into_result()
        .expect_err("malformed statement must reject the definitions block");
    let malformed_start = source.find("generate").expect("malformed statement");
    let following_start = source.find("const after").expect("following declaration");
    assert_eq!(
        errors[0].primary_span(),
        SourceSpan::new(malformed_start, malformed_start + "generate".len())
    );
    assert!(
        errors
            .iter()
            .all(|error| error.primary_span().end <= following_start)
    );
}

#[test]
fn generator_placement_errors_use_stable_categories_and_keyword_spans() {
    let cases = [
        (
            include_str!("../../../conformance/fcs5/source/invalid/nested-generator.fcs"),
            DiagnosticCode::COMPILE_TIME_NESTED_GENERATOR,
        ),
        (
            include_str!("../../../conformance/fcs5/source/invalid/misplaced-generator.fcs"),
            DiagnosticCode::COMPILE_TIME_MISPLACED_GENERATOR,
        ),
        (
            "#fcs 5.0.0\nformat { profile: fragment; }\ndefinitions {\n\
                fn bad() -> int { generate i: int in 0..<1 step 1 { } }\n\
            }",
            DiagnosticCode::COMPILE_TIME_MISPLACED_GENERATOR,
        ),
        (
            "#fcs 5.0.0\nformat { profile: fragment; }\ndefinitions {\n\
                template Note bad() { generate i: int in 0..<1 step 1 { } }\n\
            }",
            DiagnosticCode::COMPILE_TIME_MISPLACED_GENERATOR,
        ),
        (
            "#fcs 5.0.0\nformat { profile: fragment; }\ncollections {\n\
                notes { tap { gameplay.time: generate; }; }\n\
            }",
            DiagnosticCode::COMPILE_TIME_MISPLACED_GENERATOR,
        ),
    ];

    for (source, expected_code) in cases {
        let errors = parse_document(source)
            .into_result()
            .expect_err("generator placement must be rejected");
        let start = if expected_code == DiagnosticCode::COMPILE_TIME_NESTED_GENERATOR {
            source.rfind("generate").expect("nested generator keyword")
        } else {
            source.find("generate").expect("generator keyword")
        };
        assert_eq!(errors[0].code(), expected_code, "{source}");
        assert_eq!(errors[0].stage(), DiagnosticStage::Parse);
        assert_eq!(
            errors[0].primary_span(),
            SourceSpan::new(start, start + "generate".len()),
            "{source}"
        );
    }
}

#[test]
fn collection_generators_retain_their_owner_context() {
    let source = "#fcs 5.0.0\nformat { profile: fragment; }\ncollections {\n\
        notes { if true { generate i: int in 0..<1 step 1 { } } }\n\
    }";
    let document = parse_document(source)
        .into_result()
        .expect("collection generator is valid source syntax");
    let CollectionItem::Conditional { then_items, .. } = &document.collections[0].items[0] else {
        panic!("expected collection conditional");
    };
    let CollectionItem::Generator(generator) = &then_items[0] else {
        panic!("expected nested collection generator");
    };
    assert_eq!(
        generator.owner.as_ref(),
        &GeneratorOwner::Collection {
            name: "notes".to_owned()
        }
    );
}

#[test]
fn track_ast_retains_settings_direct_segments_points_and_spans() {
    let source = include_str!("../../../conformance/fcs5/source/valid/track-boundaries.fcs");
    let document = parse_document(source)
        .into_result()
        .expect("Track source grammar is valid");
    assert_eq!(document.lines.len(), 1);
    let LineBodyItem::Tracks(tracks) = &document.lines[0].items[1] else {
        panic!("expected nested tracks block");
    };
    let track = &tracks.tracks[0];
    assert_eq!(track.name, "fade");
    assert_eq!(track.target.segments, ["alpha"]);
    assert_eq!(track.value_type, Type::Float);
    assert_eq!(track.settings.len(), 5);
    assert_eq!(track.segments.items.len(), 2);
    assert!(matches!(
        track.segments.items[0],
        TrackSegmentItem::DirectSegment(_)
    ));
    assert!(matches!(
        track.segments.items[1],
        TrackSegmentItem::DirectPoint(_)
    ));
    assert_eq!(track.segments.span.start, source.find("segments").unwrap());
    assert!(track.segments.span.end <= source.len());
}

#[test]
fn track_generators_retain_track_owner_and_schema_cubic_values() {
    let source = include_str!("../../../conformance/fcs5/source/valid/complete-source-grammar.fcs");
    let document = parse_document(source)
        .into_result()
        .expect("complete Track source grammar is valid");
    let LineBodyItem::Tracks(tracks) = &document.lines[0].items[1] else {
        panic!("expected nested tracks block");
    };
    let track = &tracks.tracks[0];
    let TrackSegmentItem::Generator(generator) = &track.segments.items[1] else {
        panic!("expected Track segment generator");
    };
    let GeneratorOwner::TrackSegments {
        track: owner_track,
        target: owner_target,
        span: owner_span,
    } = generator.owner.as_ref()
    else {
        panic!("expected Track segment generator owner");
    };
    assert_eq!(owner_track, "fade");
    assert_eq!(owner_target, &track.target);
    assert_eq!(
        *owner_span,
        SourceSpan::new(track.name_span.start, track.span.end,)
    );
    let TrackSegmentItem::Generator(generator) = &track.segments.items[1] else {
        unreachable!();
    };
    let fcs_source::ast::GeneratorItem::Emit(expression) = &generator.body[0] else {
        panic!("expected generated segment emit");
    };
    let fcs_source::ast::EntityExpression::SourceConstructor(constructor) = expression else {
        panic!("expected source segment constructor");
    };
    assert_eq!(constructor.kind, SourceEntityConstructorKind::Segment);
    assert!(
        constructor
            .fields
            .iter()
            .any(|field| matches!(&field.value, SchemaValue::CubicBezier { .. }))
    );
}

#[test]
fn metadata_schema_ast_retains_ordered_declarations_and_spans() {
    let source =
        include_str!("../../../conformance/fcs5/source/valid/metadata-credits-resources-sync.fcs");
    let document = parse_document(source)
        .into_result()
        .expect("metadata/resource/sync source grammar is valid");

    let TopLevelBlock::Meta(meta) = document
        .top_level(TopLevelBlockKind::Meta)
        .expect("meta block")
    else {
        panic!("expected typed meta block");
    };
    assert_eq!(
        meta.fields
            .iter()
            .map(|field| field.path.segments.as_slice())
            .collect::<Vec<_>>(),
        [
            ["title"].as_slice(),
            ["chartVersion"].as_slice(),
            ["documentId"].as_slice(),
            ["revision"].as_slice(),
            ["custom"].as_slice(),
        ]
    );
    let SchemaValue::Expression(SourceExpression::Object { entries, .. }) = &meta.fields[4].value
    else {
        panic!("expected ordered custom object");
    };
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.key.as_str())
            .collect::<Vec<_>>(),
        ["suite", "stable"]
    );

    let TopLevelBlock::Contributors(contributors) = document
        .top_level(TopLevelBlockKind::Contributors)
        .expect("contributors block")
    else {
        panic!("expected typed contributors block");
    };
    assert_eq!(contributors.people[0].name, "alice");
    assert!(contributors.people[0].span.start < contributors.people[0].span.end);

    let TopLevelBlock::Credits(credits) = document
        .top_level(TopLevelBlockKind::Credits)
        .expect("credits block")
    else {
        panic!("expected typed credits block");
    };
    assert_eq!(credits.entries.len(), 1);

    let TopLevelBlock::Resources(resources) = document
        .top_level(TopLevelBlockKind::Resources)
        .expect("resources block")
    else {
        panic!("expected typed resources block");
    };
    assert_eq!(resources.resources[0].kind, ResourceKind::Binary);
    assert_eq!(resources.resources[0].name, "empty");

    let complete_source =
        include_str!("../../../conformance/fcs5/source/valid/complete-source-grammar.fcs");
    let complete_document = parse_document(complete_source)
        .into_result()
        .expect("complete schema source is valid");
    let TopLevelBlock::Sync(sync) = complete_document
        .top_level(TopLevelBlockKind::Sync)
        .expect("sync block")
    else {
        panic!("expected typed sync block");
    };
    let preview = sync
        .fields
        .iter()
        .find(|field| field.path.segments == ["preview"])
        .expect("preview field");
    assert!(matches!(&preview.value, SchemaValue::Interval { .. }));
    assert!(sync.span.end <= complete_source.len());
}

#[test]
fn metadata_custom_object_duplicate_keys_remain_source_ordered() {
    let source = include_str!("../../../conformance/fcs5/source/invalid/custom-duplicate-key.fcs");
    let document = parse_document(source)
        .into_result()
        .expect("duplicate custom keys are a later semantic error");
    let TopLevelBlock::Meta(meta) = document
        .top_level(TopLevelBlockKind::Meta)
        .expect("meta block")
    else {
        panic!("expected typed meta block");
    };
    let SchemaValue::Expression(SourceExpression::Object { entries, .. }) = &meta.fields[0].value
    else {
        panic!("expected custom object");
    };
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "same");
    assert_eq!(entries[1].key, "same");
    assert!(entries[0].span.start < entries[1].span.start);
}

#[test]
fn every_core_resource_kind_has_a_typed_source_node() {
    let source = "#fcs 5.0.0\nformat { profile: fragment; }\nresources {\n\
        audio a {} image i {} font f {} texture t {} path p {} shader s {} binary b {}\n\
    }";
    let document = parse_document(source)
        .into_result()
        .expect("all Core resource kind productions are syntactically valid");
    let TopLevelBlock::Resources(resources) = document
        .top_level(TopLevelBlockKind::Resources)
        .expect("resources block")
    else {
        panic!("expected typed resources block");
    };
    assert_eq!(
        resources
            .resources
            .iter()
            .map(|resource| resource.kind)
            .collect::<Vec<_>>(),
        [
            ResourceKind::Audio,
            ResourceKind::Image,
            ResourceKind::Font,
            ResourceKind::Texture,
            ResourceKind::Path,
            ResourceKind::Shader,
            ResourceKind::Binary,
        ]
    );
}

#[test]
fn extension_preserve_and_render_envelopes_retain_order_and_balanced_spans() {
    let source = include_str!("../../../conformance/fcs5/source/valid/complete-source-grammar.fcs");
    let document = parse_document(source)
        .into_result()
        .expect("extension, preserve, and Render envelopes are valid Core source");

    let TopLevelBlock::Extensions(extensions) = document
        .top_level(TopLevelBlockKind::Extensions)
        .expect("extensions block")
    else {
        panic!("expected typed extensions block");
    };
    let extension = &extensions.declarations[0];
    assert_eq!(extension.header.namespace, "org.fcs.example");
    assert_eq!(extension.header.version.to_string(), "1.0.0");
    assert_eq!(extension.requirement, ExtensionRequirement::Optional);
    assert_eq!(
        extension
            .payload
            .entries
            .iter()
            .map(|entry| entry.key.as_str())
            .collect::<Vec<_>>(),
        ["enabled", "strength"]
    );

    let TopLevelBlock::Preserve(preserve) = document
        .top_level(TopLevelBlockKind::Preserve)
        .expect("preserve block")
    else {
        panic!("expected typed preserve block");
    };
    assert_eq!(preserve.items.len(), 2);
    let PreserveItem::Source(source_item) = &preserve.items[0] else {
        panic!("expected preserve source item first");
    };
    assert_eq!(source_item.fields[0].path.segments, ["format"]);
    let PreserveItem::Payload(payload) = &preserve.items[1] else {
        panic!("expected preserve payload item second");
    };
    assert_eq!(payload.header.namespace, "org.phigros.rpe");
    assert_eq!(payload.payload.entries[0].key, "encoding");
    assert!(payload.span.start < payload.span.end);

    let TopLevelBlock::Render(render) = document
        .top_level(TopLevelBlockKind::Render)
        .expect("render block")
    else {
        panic!("expected Render block");
    };
    assert_eq!(render.version.to_string(), "1.0.0");
    assert!(!render.payload.elements.is_empty());
    assert!(render.payload.span.start < render.payload.span.end);
}

#[test]
fn extension_payload_duplicate_keys_remain_ordered_and_unbalanced_envelopes_fail() {
    let source = "#fcs 5.0.0\nformat { profile: fragment; }\nextensions {\n\
        extension(\"not-normalized\", 9.9.9) required { \"same\": 1, \"same\": 2, }\n\
    }";
    let document = parse_document(source)
        .into_result()
        .expect("namespace semantics and duplicate keys belong to later phases");
    let TopLevelBlock::Extensions(extensions) = document
        .top_level(TopLevelBlockKind::Extensions)
        .expect("extensions block")
    else {
        panic!("expected typed extensions block");
    };
    let entries = &extensions.declarations[0].payload.entries;
    assert_eq!(entries[0].key, "same");
    assert_eq!(entries[1].key, "same");
    assert!(entries[0].span.start < entries[1].span.start);

    for malformed in [
        "#fcs 5.0.0\nformat { profile: fragment; }\nextensions { extension(\"x\", 1.0.0) optional { \"x\": 1, }",
        "#fcs 5.0.0\nformat { profile: fragment; }\npreserve { source { format: \"rpe\"; }",
        "#fcs 5.0.0\nformat { profile: fragment; }\nrender profile 1.0.0 { layer x {",
        "#fcs 5.0.0\nformat { profile: fragment; }\nrender profile 1.0.0 { active: [0s, 1s); }",
    ] {
        parse_document(malformed)
            .into_result()
            .expect_err("truncated envelope must fail at the parser boundary");
    }
}
