use fcs_model::{
    CanonicalTrackBlend, CanonicalTrackFill, CanonicalTrackPiece, CanonicalTrackTarget,
};
use fcs_source::diagnostic::DiagnosticCode;
use fcs_source::elaborator::{CompileTimeLimits, elaborate};
use fcs_source::parser::parse_document;
use fcs_source::schema::phase2_schema;

const HEADER: &str = "#fcs 5.0.0\nformat { profile: chart; }\ntempoMap { 0beat -> 120bpm; }\n";

fn lower(source: &str) -> fcs_model::CanonicalTrackSet {
    let document = parse_document(source)
        .into_result()
        .expect("Track source should parse");
    let lines = document
        .canonical_line_graph()
        .expect("Line graph should lower");
    let expanded = elaborate(&document, phase2_schema(), CompileTimeLimits::default())
        .expect("Track source should elaborate");
    let time_map = expanded
        .canonical_time_map()
        .expect("tempo map should lower");
    expanded
        .canonical_tracks(&time_map, &lines)
        .unwrap_or_else(|diagnostics| panic!("Track lowering failed: {diagnostics:?}"))
}

fn diagnostics(source: &str) -> Vec<fcs_source::Diagnostic> {
    let document = parse_document(source)
        .into_result()
        .expect("Track source should parse");
    let lines = document
        .canonical_line_graph()
        .expect("Line graph should lower");
    let expanded = elaborate(&document, phase2_schema(), CompileTimeLimits::default())
        .expect("Track source should elaborate");
    let time_map = expanded
        .canonical_time_map()
        .expect("tempo map should lower");
    expanded
        .canonical_tracks(&time_map, &lines)
        .expect_err("Track lowering should fail")
}

#[test]
fn direct_track_normalizes_defaults_and_half_open_boundary() {
    let tracks = lower(&format!(
        "{HEADER}lines {{ line main {{ alpha: 1.0; tracks {{ track fade -> alpha: float {{ segments {{ [0s, 1s): 1.0 -> 0.5 using \"linear\"; point 1s: 0.5; }} }} }} }} }}"
    ));
    assert_eq!(tracks.tracks().len(), 1);
    let track = &tracks.tracks()[0];
    assert_eq!(track.target(), CanonicalTrackTarget::Alpha);
    assert_eq!(track.blend(), CanonicalTrackBlend::Replace);
    assert_eq!(track.fill(), CanonicalTrackFill::Base);
    assert_eq!(track.extrapolate_before(), CanonicalTrackFill::Base);
    assert_eq!(track.extrapolate_after(), CanonicalTrackFill::Base);
    assert!(matches!(track.pieces()[0], CanonicalTrackPiece::Segment(_)));
    assert!(matches!(track.pieces()[1], CanonicalTrackPiece::Point(_)));
}

#[test]
fn generator_and_conditional_track_items_share_the_compile_time_environment() {
    let tracks = lower(&format!(
        "{HEADER}definitions {{ const ENABLE: bool = true; }} lines {{ line main {{ tracks {{ track fade -> alpha: float {{ segments {{ if ENABLE {{ generate at: beat in 0beat..<2beat step 1beat {{ let value: float = 0.5; emit segment {{ start: at; end: at + 1beat; startValue: value; endValue: value; interpolation: \"step\"; }}; }} }} }} }} }} }} }}"
    ));
    assert_eq!(tracks.tracks()[0].pieces().len(), 2);
}

#[test]
fn overlap_and_point_conflicts_use_stable_track_diagnostics() {
    let overlap = diagnostics(&format!(
        "{HEADER}lines {{ line main {{ tracks {{ track fade -> alpha: float {{ segments {{ [0s, 2s): 1.0 -> 0.5 using \"linear\"; [1s, 3s): 0.5 -> 0.0 using \"linear\"; }} }} }} }} }}"
    ));
    assert_eq!(overlap[0].code(), DiagnosticCode::TRACK_OVERLAP);

    let point = diagnostics(&format!(
        "{HEADER}lines {{ line main {{ tracks {{ track fade -> alpha: float {{ segments {{ [0s, 1s): 1.0 -> 0.5 using \"linear\"; point 0s: 0.0; }} }} }} }} }}"
    ));
    assert_eq!(point[0].code(), DiagnosticCode::TRACK_REPLACE_CONFLICT);
}

#[test]
fn target_schema_and_easing_are_checked_at_the_canonical_boundary() {
    let target = diagnostics(&format!(
        "{HEADER}lines {{ line main {{ tracks {{ track fade -> alpha: angle {{ segments {{ [0s, 1s): 0deg -> 90deg using \"linear\"; }} }} }} }} }}"
    ));
    assert_eq!(target[0].code(), DiagnosticCode::TYPE_MISMATCH);

    let easing = diagnostics(&format!(
        "{HEADER}lines {{ line main {{ tracks {{ track fade -> alpha: float {{ segments {{ [0s, 1s): 1.0 -> 0.0 using \"not-an-easing\"; }} }} }} }} }}"
    ));
    assert_eq!(easing[0].code(), DiagnosticCode::TRACK_INVALID_EASING);
}

#[test]
fn line_targets_and_cubic_bezier_lower_to_typed_canonical_values() {
    let tracks = lower(&format!(
        "{HEADER}lines {{ line main {{ tracks {{
            track move -> position: vec2<length> {{ segments {{ [0s, 1s): vec2(0px, 0px) -> vec2(1px, 2px) using cubicBezier(0.42, 0.0, 0.58, 1.0); }} }}
            track turn -> rotation: angle {{ segments {{ [0s, 1s): 0deg -> 90deg using \"linear\"; }} }}
            track zoom -> scale: vec2<float> {{ segments {{ [0s, 1s): vec2(1.0, 1.0) -> vec2(2.0, 2.0) using \"linear\"; }} }}
            track fade -> alpha: float {{ segments {{ [0s, 1s): 1.0 -> 0.0 using \"step\"; }} }}
        }} }} }}"
    ));
    let targets = tracks
        .tracks()
        .iter()
        .map(|track| track.target())
        .collect::<Vec<_>>();
    assert_eq!(
        targets,
        vec![
            CanonicalTrackTarget::Position,
            CanonicalTrackTarget::Rotation,
            CanonicalTrackTarget::Scale,
            CanonicalTrackTarget::Alpha,
        ]
    );
}

#[test]
fn blend_defaults_and_owner_local_order_do_not_depend_on_declaration_order() {
    let source =
        |tracks: &str| format!("{HEADER}lines {{ line main {{ tracks {{ {tracks} }} }} }}");
    let first = lower(&source(
        "track z -> alpha: float { blend: \"add\"; segments { [0s, 1s): 0.0 -> 1.0 using \"linear\"; } }
         track a -> alpha: float { blend: \"add\"; segments { [0s, 1s): 0.0 -> 1.0 using \"linear\"; } }
         track scale -> scale: vec2<float> { blend: \"multiply\"; segments { [0s, 1s): vec2(1.0, 1.0) -> vec2(2.0, 2.0) using \"linear\"; } }",
    ));
    let second = lower(&source(
        "track scale -> scale: vec2<float> { blend: \"multiply\"; segments { [0s, 1s): vec2(1.0, 1.0) -> vec2(2.0, 2.0) using \"linear\"; } }
         track a -> alpha: float { blend: \"add\"; segments { [0s, 1s): 0.0 -> 1.0 using \"linear\"; } }
         track z -> alpha: float { blend: \"add\"; segments { [0s, 1s): 0.0 -> 1.0 using \"linear\"; } }",
    ));
    assert_eq!(first, second);
    assert_eq!(first.tracks()[0].fill(), CanonicalTrackFill::One);
    assert_eq!(first.tracks()[1].fill(), CanonicalTrackFill::Zero);
    assert_eq!(first.tracks()[2].fill(), CanonicalTrackFill::Zero);
}

#[test]
fn error_fill_is_preserved_and_equal_priority_replace_tracks_conflict() {
    let tracks = lower(&format!(
        "{HEADER}lines {{ line main {{ tracks {{ track sparse -> alpha: float {{ fill: \"error\"; segments {{ [0s, 1s): 1.0 -> 0.5 using \"linear\"; [2s, 3s): 0.5 -> 0.0 using \"linear\"; }} }} }} }} }}"
    ));
    assert_eq!(tracks.tracks()[0].fill(), CanonicalTrackFill::Error);

    let disjoint = lower(&format!(
        "{HEADER}lines {{ line main {{ tracks {{
            track first -> alpha: float {{ segments {{ [0s, 1s): 1.0 -> 0.5 using \"linear\"; }} }}
            track second -> alpha: float {{ segments {{ [1s, 2s): 0.5 -> 0.0 using \"linear\"; }} }}
        }} }} }}"
    ));
    assert_eq!(disjoint.tracks().len(), 2);

    let conflict = diagnostics(&format!(
        "{HEADER}lines {{ line main {{ tracks {{
            track first -> alpha: float {{ segments {{ [0s, 2s): 1.0 -> 0.5 using \"linear\"; }} }}
            track second -> alpha: float {{ segments {{ [1s, 3s): 0.5 -> 0.0 using \"linear\"; }} }}
        }} }} }}"
    ));
    assert_eq!(conflict[0].code(), DiagnosticCode::TRACK_REPLACE_CONFLICT);
}

#[test]
fn same_time_point_is_shadowed_by_segment_for_cross_track_conflicts() {
    let tracks = lower(&format!(
        "{HEADER}lines {{ line main {{ tracks {{
            track first -> alpha: float {{ segments {{ [0s, 1s): 0.0 -> 1.0 using \"linear\"; point 0s: 0.0; }} }}
            track second -> alpha: float {{ segments {{ [1s, 2s): 1.0 -> 0.0 using \"linear\"; }} }}
        }} }} }}"
    ));
    assert_eq!(tracks.tracks().len(), 2);
}

#[test]
fn higher_priority_replace_shadows_lower_priority_overlap() {
    let tracks = lower(&format!(
        "{HEADER}lines {{ line main {{ tracks {{
            track lowA -> alpha: float {{ priority: 0; segments {{ [0s, 2s): 0.0 -> 1.0 using \"linear\"; }} }}
            track lowB -> alpha: float {{ priority: 0; segments {{ [1s, 3s): 1.0 -> 0.0 using \"linear\"; }} }}
            track high -> alpha: float {{ priority: 1; segments {{ [0s, 3s): 0.0 -> 1.0 using \"linear\"; }} }}
        }} }} }}"
    ));
    assert_eq!(tracks.tracks().len(), 3);
}

#[test]
fn partially_uncovered_equal_priority_overlap_still_conflicts() {
    let conflict = diagnostics(&format!(
        "{HEADER}lines {{ line main {{ tracks {{
            track lowA -> alpha: float {{ priority: 0; segments {{ [0s, 2s): 0.0 -> 1.0 using \"linear\"; }} }}
            track lowB -> alpha: float {{ priority: 0; segments {{ [1s, 3s): 1.0 -> 0.0 using \"linear\"; }} }}
            track high -> alpha: float {{ priority: 1; segments {{ [0s, 1.5s): 0.0 -> 1.0 using \"linear\"; }} }}
        }} }} }}"
    ));
    assert_eq!(conflict[0].code(), DiagnosticCode::TRACK_REPLACE_CONFLICT);
}
