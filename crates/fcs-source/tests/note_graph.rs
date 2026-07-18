use fcs_model::{
    CanonicalJudgeShape, CanonicalNoteKind, CanonicalNoteScorePolicy, CanonicalNoteSide,
    CanonicalNoteSoundPolicy, EntityKind, derive_stable_id,
};
use fcs_source::elaborator::{CompileTimeLimits, elaborate};
use fcs_source::parser::parse_document;
use fcs_source::schema::phase2_schema;

const HEADER: &str = "#fcs 5.0.0\nformat { profile: chart; }\ntempoMap { 0beat -> 120bpm; }\n";
const POLICY_DECLARATIONS: &str = "resources { audio hit { source: \"assets/hit.ogg\"; hash: \"sha256:0000000000000000000000000000000000000000000000000000000000000000\"; mediaType: \"audio/ogg\"; } } extensions { extension(\"score.ext\", 1.0.0) required { \"mode\": \"test\", } } ";

fn canonical(source: &str) -> fcs_model::CanonicalNoteSet {
    let document = parse_document(source)
        .into_result()
        .expect("source should parse");
    let lines = document
        .canonical_line_graph()
        .expect("Line graph should lower");
    let expanded = elaborate(&document, phase2_schema(), CompileTimeLimits::default())
        .expect("source should elaborate");
    let time_map = expanded
        .canonical_time_map()
        .expect("tempo map should lower");
    expanded
        .canonical_notes(&time_map, &lines)
        .unwrap_or_else(|diagnostics| panic!("canonical Note lowering failed: {diagnostics:?}"))
}

fn canonical_diagnostics(source: &str) -> Vec<fcs_source::Diagnostic> {
    let document = parse_document(source)
        .into_result()
        .expect("source should parse");
    let lines = document
        .canonical_line_graph()
        .expect("Line graph should lower");
    let expanded = elaborate(&document, phase2_schema(), CompileTimeLimits::default())
        .expect("source should elaborate");
    let time_map = expanded
        .canonical_time_map()
        .expect("tempo map should lower");
    expanded
        .canonical_notes(&time_map, &lines)
        .expect_err("source should fail canonical Note lowering")
}

#[test]
fn four_note_kinds_lower_defaults_and_sort_by_canonical_key() {
    let notes = canonical(&format!(
        "{HEADER}lines {{ line main {{}} line other {{}} }} collections {{ notes {{\
            drag {{ line: @main; gameplay.time: 4beat; }};\
            tap {{ line: @main; gameplay.time: 2beat; }};\
            flick {{ line: @other; gameplay.time: 3beat; }};\
            hold {{ line: @main; gameplay.time: 1beat; gameplay.endTime: 2beat; }};\
        }} }}"
    ));

    assert_eq!(
        notes
            .notes()
            .iter()
            .map(|note| note.kind())
            .collect::<Vec<_>>(),
        vec![
            CanonicalNoteKind::Hold,
            CanonicalNoteKind::Tap,
            CanonicalNoteKind::Flick,
            CanonicalNoteKind::Drag,
        ]
    );
    let first = notes.notes().first().unwrap();
    assert_eq!(first.gameplay().side(), CanonicalNoteSide::Above);
    assert!(first.gameplay().judgment_enabled());
    assert_eq!(
        first.gameplay().judge_shape(),
        &CanonicalJudgeShape::LineDefault
    );
    assert_eq!(
        first.gameplay().sound_policy(),
        &CanonicalNoteSoundPolicy::Default
    );
    assert_eq!(
        first.gameplay().score_policy(),
        &CanonicalNoteScorePolicy::Default
    );
    assert_eq!(first.presentation().position_x(), 0.0);
    assert_eq!(first.presentation().scroll_factor(), 1.0);
    assert_eq!(first.presentation().alpha(), 1.0);
    assert_eq!(first.presentation().scale_x(), 1.0);
    assert_eq!(first.presentation().scale_y(), 1.0);
    assert!(first.presentation().render_enabled());
    assert!(first.presentation().texture().is_none());
}

#[test]
fn canonical_notes_validate_shapes_policies_and_hold_boundaries() {
    let notes = canonical(&format!(
        "{HEADER}{POLICY_DECLARATIONS}lines {{ line main {{}} line other {{}} }} collections {{ notes {{\
            tap {{ id: \"rectangle\"; line: @main; gameplay.time: 1beat;\
                gameplay.judgeShape.kind: \"rectangle\";\
                gameplay.judgeShape.center: vec2(2px, 3px);\
                gameplay.judgeShape.halfExtents: vec2(4px, 5px);\
                gameplay.soundPolicy: \"resource\";\
                gameplay.soundResource: \"hit\";\
                gameplay.scorePolicy: \"custom\";\
                gameplay.scoreExtension: \"score.ext\";\
                gameplay.side: \"below\"; gameplay.judgment.enabled: true; }};\
        }} }}"
    ));
    let note = notes.note_by_textual_id("rectangle").unwrap();
    assert_eq!(note.gameplay().side(), CanonicalNoteSide::Below);
    assert!(matches!(
        note.gameplay().judge_shape(),
        CanonicalJudgeShape::Rectangle { .. }
    ));
    assert_eq!(
        note.gameplay().sound_policy(),
        &CanonicalNoteSoundPolicy::Resource("hit".into())
    );
    assert_eq!(
        note.gameplay().score_policy(),
        &CanonicalNoteScorePolicy::Custom("score.ext".into())
    );

    let bad_hold = parse_document(&format!(
        "{HEADER}lines {{ line main {{}} }} collections {{ notes {{\
            hold {{ line: @main; gameplay.time: 2beat; gameplay.endTime: 1beat; }};\
        }} }}"
    ))
    .into_result()
    .unwrap();
    let lines = bad_hold.canonical_line_graph().unwrap();
    let expanded = elaborate(&bad_hold, phase2_schema(), CompileTimeLimits::default()).unwrap();
    let time_map = expanded.canonical_time_map().unwrap();
    let diagnostics = expanded
        .canonical_notes(&time_map, &lines)
        .expect_err("an inverted Hold must fail canonical validation");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.code()
            == fcs_source::diagnostic::DiagnosticCode::NOTE_INVALID_HOLD)
    );
}

#[test]
fn note_policy_fixture_binds_audio_resource_and_required_extension() {
    let source = include_str!("../../../docs/conformance/fcs5/source/valid/note-policies.fcs");
    let notes = canonical(source);
    let resource_note = notes.note_by_textual_id("resource-sound").unwrap();
    assert_eq!(
        resource_note.gameplay().sound_policy(),
        &CanonicalNoteSoundPolicy::Resource("click".into())
    );
    let custom_score = notes.note_by_textual_id("custom-score").unwrap();
    assert_eq!(
        custom_score.gameplay().score_policy(),
        &CanonicalNoteScorePolicy::Custom("org.fcs.custom-score".into())
    );
}

#[test]
fn canonical_notes_preserve_generated_template_and_generator_identity_paths() {
    let notes = canonical(include_str!(
        "../../../docs/conformance/fcs5/source/valid/compile-time-generator.fcs"
    ));
    let ids: Vec<_> = notes
        .notes()
        .iter()
        .map(|note| note.id().textual().as_str().to_owned())
        .collect();
    assert_eq!(
        ids,
        vec![
            "generated/note/collection/notes/item/0/template/generatedTap/call/0/generate/0/order/0",
            "generated/note/collection/notes/item/0/template/generatedTap/call/1/generate/1/order/1",
            "generated/note/collection/notes/item/0/template/generatedTap/call/2/generate/2/order/2",
            "generated/note/collection/notes/item/0/template/generatedTap/call/3/generate/3/order/3",
        ]
    );
}

#[test]
fn canonical_notes_eliminate_authoring_structure() {
    let direct = canonical(include_str!(
        "../../../docs/conformance/fcs5/source/valid/canonical-equivalent-direct.fcs"
    ));
    let template = canonical(include_str!(
        "../../../docs/conformance/fcs5/source/valid/canonical-equivalent-template.fcs"
    ));
    assert_eq!(direct, template);
}

#[test]
fn canonical_notes_reject_unknown_or_wrong_policy_bindings() {
    let cases = [
        (
            "resources { image cover { source: \"assets/cover.png\"; hash: \"sha256:0000000000000000000000000000000000000000000000000000000000000000\"; mediaType: \"image/png\"; } }",
            "gameplay.soundPolicy: \"resource\"; gameplay.soundResource: @cover;",
            fcs_source::diagnostic::DiagnosticCode::RESOURCE_TYPE_MISMATCH,
        ),
        (
            "",
            "gameplay.soundPolicy: \"resource\"; gameplay.soundResource: @missing;",
            fcs_source::diagnostic::DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE,
        ),
        (
            "extensions { extension(\"score.ext\", 1.0.0) optional {} }",
            "gameplay.scorePolicy: \"custom\"; gameplay.scoreExtension: \"score.ext\";",
            fcs_source::diagnostic::DiagnosticCode::EXTENSION_UNSUPPORTED_REQUIRED,
        ),
    ];

    for (declarations, policy, expected) in cases {
        let source = format!(
            "{HEADER}{declarations} lines {{ line main {{}} }} collections {{ notes {{ tap {{ line: @main; gameplay.time: 1beat; {policy} }}; }} }}"
        );
        let diagnostics = canonical_diagnostics(&source);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code() == expected),
            "expected {expected}, got {diagnostics:?}"
        );
    }
}

#[test]
fn canonical_notes_require_existing_lines_and_reject_non_hold_end_time() {
    let source = format!(
        "{HEADER}lines {{ line main {{}} line other {{}} }} collections {{ notes {{\
            tap {{ line: @main; gameplay.time: 1beat; gameplay.endTime: 2beat; }};\
            flick {{ line: @other; gameplay.time: 3beat; }};\
        }} }}"
    );
    let document = parse_document(&source).into_result().unwrap();
    let expanded = elaborate(&document, phase2_schema(), CompileTimeLimits::default()).unwrap();
    let time_map = expanded.canonical_time_map().unwrap();
    let graph_without_other = parse_document(&format!(
        "{HEADER}lines {{ line main {{}} }} collections {{ notes {{}} }}"
    ))
    .into_result()
    .unwrap()
    .canonical_line_graph()
    .unwrap();
    let diagnostics = expanded
        .canonical_notes(&time_map, &graph_without_other)
        .expect_err("unknown line and non-Hold endTime must fail");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.code()
            == fcs_source::diagnostic::DiagnosticCode::GRAPH_UNKNOWN_PARENT),
        "diagnostics: {diagnostics:?}"
    );
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.code()
            == fcs_source::diagnostic::DiagnosticCode::NOTE_INVALID_HOLD),
        "diagnostics: {diagnostics:?}"
    );
}

#[test]
fn canonical_notes_enforce_policy_and_shape_boundaries() {
    let cases = [
        (
            "tap { line: @main; gameplay.time: 1beat; gameplay.judgment.enabled: false; gameplay.soundPolicy: \"default\"; }",
            fcs_source::diagnostic::DiagnosticCode::SCHEMA_NON_CONSTRUCTIBLE,
        ),
        (
            "tap { line: @main; gameplay.time: 1beat; gameplay.judgment.enabled: false; gameplay.soundResource: \"hit\"; }",
            fcs_source::diagnostic::DiagnosticCode::SCHEMA_NON_CONSTRUCTIBLE,
        ),
        (
            "tap { line: @main; gameplay.time: 1beat; gameplay.scorePolicy: \"custom\"; }",
            fcs_source::diagnostic::DiagnosticCode::SCHEMA_MISSING_REQUIRED_FIELD,
        ),
        (
            "tap { line: @main; gameplay.time: 1beat; gameplay.judgeShape.kind: \"rectangle\"; gameplay.judgeShape.halfExtents: vec2(1px, 1px); gameplay.judgeShape.radius: 1px; }",
            fcs_source::diagnostic::DiagnosticCode::SCHEMA_NON_CONSTRUCTIBLE,
        ),
        (
            "tap { line: @main; gameplay.time: 1beat; gameplay.judgeShape.kind: \"circle\"; }",
            fcs_source::diagnostic::DiagnosticCode::SCHEMA_MISSING_REQUIRED_FIELD,
        ),
        (
            "hold { line: @main; gameplay.time: 1beat; }",
            fcs_source::diagnostic::DiagnosticCode::NOTE_INVALID_HOLD,
        ),
    ];

    for (note, expected) in cases {
        let source =
            format!("{HEADER}lines {{ line main {{}} }} collections {{ notes {{ {note}; }} }}");
        let diagnostics = canonical_diagnostics(&source);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code() == expected),
            "expected {expected}, got {diagnostics:?}"
        );
    }
}

#[test]
fn disabled_judgment_defaults_to_no_sound_and_score() {
    let notes = canonical(&format!(
        "{HEADER}lines {{ line main {{}} }} collections {{ notes {{ tap {{ line: @main; gameplay.time: 1beat; gameplay.judgment.enabled: false; }}; }} }}"
    ));
    let gameplay = notes.notes()[0].gameplay();
    assert!(!gameplay.judgment_enabled());
    assert_eq!(gameplay.sound_policy(), &CanonicalNoteSoundPolicy::None);
    assert_eq!(gameplay.score_policy(), &CanonicalNoteScorePolicy::None);
}

#[test]
fn canonical_note_presentation_retains_rotation_and_visibility_boundaries() {
    let notes = canonical(&format!(
        "{HEADER}lines {{ line main {{}} }} collections {{ notes {{ tap {{\
            id: \"visual\"; line: @main; gameplay.time: 1beat;\
            presentation.rotation: 90deg; presentation.visibleFrom: 1beat;\
            presentation.visibleUntil: 3beat; }}; }} }}"
    ));
    let presentation = notes.note_by_textual_id("visual").unwrap().presentation();
    assert_eq!(presentation.rotation(), std::f64::consts::FRAC_PI_2);
    assert_eq!(
        presentation
            .visible_from()
            .expect("visibleFrom is retained")
            .source_beat()
            .as_f64(),
        1.0
    );
    assert_eq!(
        presentation
            .visible_until()
            .expect("visibleUntil is retained")
            .source_beat()
            .as_f64(),
        3.0
    );
}

#[test]
fn canonical_notes_bind_explicit_and_generated_ids_and_sort_ties() {
    let notes = canonical(&format!(
        "{HEADER}lines {{ line a {{}} line b {{}} }} collections {{ notes {{\
            tap {{ id: \"explicit\"; line: @a; gameplay.time: 1beat; }};\
            tap {{ line: @b; gameplay.time: 1beat; }};\
            tap {{ id: \"later\"; line: @a; gameplay.time: 1beat; }};\
        }} }}"
    ));

    assert!(notes.note_by_textual_id("explicit").is_some());
    assert!(
        notes.notes().iter().any(|note| note
            .id()
            .textual()
            .as_str()
            .starts_with("generated/note/"))
    );

    let mut expected = vec![("a", 0_u64), ("b", 1), ("a", 2)];
    expected.sort_by(|left, right| {
        derive_stable_id(EntityKind::Line, left.0)
            .cmp(&derive_stable_id(EntityKind::Line, right.0))
            .then_with(|| left.1.cmp(&right.1))
    });
    let actual: Vec<_> = notes
        .notes()
        .iter()
        .map(|note| {
            (
                note.gameplay().line().textual().as_str(),
                note.document_order(),
            )
        })
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn duplicate_canonical_note_ids_report_the_second_id_field_span() {
    let source = format!(
        "{HEADER}lines {{ line main {{}} }} collections {{ notes {{\
            tap {{ id: \"duplicate\"; line: @main; gameplay.time: 1beat; }};\
            tap {{ id: \"duplicate\"; line: @main; gameplay.time: 2beat; }};\
        }} }}"
    );
    let diagnostics = canonical_diagnostics(&source);
    let duplicate = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.code() == fcs_source::diagnostic::DiagnosticCode::NAME_DUPLICATE
        })
        .expect("duplicate ID diagnostic");
    let second_id = source.rfind("id: \"duplicate\"").unwrap();
    assert_eq!(
        duplicate.primary_span().start,
        second_id,
        "diagnostics: {diagnostics:?}"
    );
    assert!(duplicate.primary_span().end > duplicate.primary_span().start);
}
