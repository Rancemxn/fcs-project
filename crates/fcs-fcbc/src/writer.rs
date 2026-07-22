use fcs_model::{
    CanonicalCompilation, CanonicalNoteKind, CanonicalNoteSide, CanonicalResourceKind,
    CanonicalTrack, CanonicalTrackBlend, CanonicalTrackFill, CanonicalTrackInterpolation,
    CanonicalTrackPiece, CanonicalTrackSegment, CanonicalTrackTarget, CanonicalTrackValue,
};
use fcs_runtime::EasingId;
use sha2::{Digest, Sha256};

use crate::error::{FcbcError, FcbcResult};

pub const EVALUABLE_DISTANCE_INDEX: u32 = 0;
pub const ANALYTIC_DISTANCE_INDEX: u32 = 1;

pub const SECONDS_ALPHA_DESCRIPTOR_INDEX: u32 = 0;
pub const CHOOSE_ALPHA_DESCRIPTOR_INDEX: u32 = 1;
pub const POSITION_DESCRIPTOR_INDEX: u32 = 2;
pub const ROTATION_DESCRIPTOR_INDEX: u32 = 3;
pub const SCALE_DESCRIPTOR_INDEX: u32 = 4;
pub const EVALUABLE_SPEED_DESCRIPTOR_INDEX: u32 = 5;
pub const ANALYTIC_SPEED_DESCRIPTOR_INDEX: u32 = 6;
pub const SCROLL_TEMPO_DESCRIPTOR_INDEX: u32 = 7;
pub const FLOAT_ONE_DESCRIPTOR_INDEX: u32 = 8;
pub const COLOR_DESCRIPTOR_INDEX: u32 = 9;
pub const NOTE_POSITION_X_DESCRIPTOR_INDEX: u32 = 10;
pub const PIECEWISE_ONE_DESCRIPTOR_INDEX: u32 = 11;
pub const VISIBILITY_DESCRIPTOR_INDEX: u32 = 12;
pub const LENGTH_ZERO_DESCRIPTOR_INDEX: u32 = 13;

const REQUIRED: u16 = 1;
const NULL_INDEX: u32 = u32::MAX;

const TY_BOOL: u8 = 1;
const TY_INT: u8 = 2;
const TY_FLOAT: u8 = 3;
const TY_TIME: u8 = 4;
const TY_LENGTH: u8 = 6;
const TY_ANGLE: u8 = 7;
const TY_COLOR: u8 = 8;
const TY_VEC2_FLOAT: u8 = 9;
const TY_VEC2_LENGTH: u8 = 10;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Constant {
    tag: u8,
    payload: Vec<u8>,
}

impl Constant {
    fn encoded(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        put_u8(&mut bytes, self.tag);
        put_u8(&mut bytes, 0);
        put_u16(&mut bytes, 0);
        put_u32(&mut bytes, self.payload.len() as u32);
        bytes.extend_from_slice(&self.payload);
        pad_to(&mut bytes, 8);
        bytes
    }
}

#[derive(Clone)]
struct LineFixture {
    id: u64,
    parent_id: u64,
    document_order: u32,
    z_order: i32,
    inherit_flags: u32,
    line_flags: u32,
    position: [f64; 2],
    rotation: f64,
    scale: [f64; 2],
    alpha: f64,
    transform_origin: [f64; 2],
    texture_anchor: [f64; 2],
    distance_index: u32,
    position_descriptor: u32,
    rotation_descriptor: u32,
    scale_descriptor: u32,
    alpha_descriptor: u32,
    scroll_tempo_descriptor: u32,
    speed_descriptor: u32,
    evaluable_speed: bool,
    floor_scale: f64,
    integration_origin: f64,
    initial_floor: f64,
}

#[derive(Clone)]
struct AlphaTrackFixture {
    line_id: u64,
    segments: Vec<TrackSegmentFixture>,
}

#[derive(Clone)]
struct TrackSegmentFixture {
    start: f64,
    end: f64,
    interpolation: u16,
    easing: u16,
    flags: u32,
    start_constant: Constant,
    end_constant: Constant,
    bezier: [f64; 4],
}

struct ConstantIndices {
    bool_false: u32,
    bool_true: u32,
    int_two: u32,
    float_zero: u32,
    float_one: u32,
    float_two: u32,
    float_ten: u32,
    float_sixty: u32,
    length_zero: u32,
    angle_zero: u32,
    color_white: u32,
    vec2_float_one: u32,
}

struct Section {
    kind: u32,
    payload: Vec<u8>,
    offset: u64,
}

#[derive(Clone, Copy)]
enum ExecutionGraph {
    Fixture,
    Native { has_notes: bool },
}

/// Builds the deterministic, non-empty FCBC 2 / Execution ABI 1 reference fixture.
///
/// This function intentionally derives the bytes from a fixed declarative chart model. It does
/// not read the checked-in golden, a manifest, or any product implementation.
pub fn write_nonempty_execution() -> Vec<u8> {
    let analytic_line_id = stable_id(b"fcs.line", b"fixture.analytic");
    let evaluable_line_id = stable_id(b"fcs.line", b"fixture.evaluable");
    let mut lines = vec![
        LineFixture {
            id: analytic_line_id,
            parent_id: 0,
            document_order: 0,
            z_order: 0,
            inherit_flags: 0,
            line_flags: 0,
            position: [0.0, 0.0],
            rotation: 0.0,
            scale: [1.0, 1.0],
            alpha: 1.0,
            transform_origin: [0.0, 0.0],
            texture_anchor: [0.0, 0.0],
            distance_index: ANALYTIC_DISTANCE_INDEX,
            position_descriptor: POSITION_DESCRIPTOR_INDEX,
            rotation_descriptor: ROTATION_DESCRIPTOR_INDEX,
            scale_descriptor: SCALE_DESCRIPTOR_INDEX,
            alpha_descriptor: CHOOSE_ALPHA_DESCRIPTOR_INDEX,
            scroll_tempo_descriptor: SCROLL_TEMPO_DESCRIPTOR_INDEX,
            speed_descriptor: ANALYTIC_SPEED_DESCRIPTOR_INDEX,
            evaluable_speed: false,
            floor_scale: 1.0,
            integration_origin: 0.0,
            initial_floor: 10.0,
        },
        LineFixture {
            id: evaluable_line_id,
            parent_id: 0,
            document_order: 0,
            z_order: 0,
            inherit_flags: 0,
            line_flags: 0,
            position: [0.0, 0.0],
            rotation: 0.0,
            scale: [1.0, 1.0],
            alpha: 1.0,
            transform_origin: [0.0, 0.0],
            texture_anchor: [0.0, 0.0],
            distance_index: EVALUABLE_DISTANCE_INDEX,
            position_descriptor: POSITION_DESCRIPTOR_INDEX,
            rotation_descriptor: ROTATION_DESCRIPTOR_INDEX,
            scale_descriptor: SCALE_DESCRIPTOR_INDEX,
            alpha_descriptor: SECONDS_ALPHA_DESCRIPTOR_INDEX,
            scroll_tempo_descriptor: SCROLL_TEMPO_DESCRIPTOR_INDEX,
            speed_descriptor: EVALUABLE_SPEED_DESCRIPTOR_INDEX,
            evaluable_speed: true,
            floor_scale: 1.0,
            integration_origin: 0.0,
            initial_floor: 20.0,
        },
    ];
    lines.sort_by_key(|line| line.id);
    let line_count = lines.len();
    for (index, line) in lines.iter_mut().enumerate() {
        line.document_order = index as u32;
        line.distance_index = index as u32;
        // Preserve the historical speed/alpha pairing used by the nonempty golden:
        // lower Line ID uses evaluable path; higher uses analytic path when two Lines exist.
        if line_count >= 2 {
            if index == 0 {
                line.alpha_descriptor = SECONDS_ALPHA_DESCRIPTOR_INDEX;
                line.speed_descriptor = EVALUABLE_SPEED_DESCRIPTOR_INDEX;
                line.evaluable_speed = true;
                line.initial_floor = 20.0;
            } else if index == 1 {
                line.alpha_descriptor = CHOOSE_ALPHA_DESCRIPTOR_INDEX;
                line.speed_descriptor = ANALYTIC_SPEED_DESCRIPTOR_INDEX;
                line.evaluable_speed = false;
                line.initial_floor = 10.0;
            }
        }
    }
    let notes = vec![
        NoteFixture {
            id: stable_id(b"fcs.note", b"fixture.analytic.note"),
            line_id: analytic_line_id,
            document_order: 0,
            kind: 1,
            side: 1,
            flags: 0b11,
            time: 0.5,
            end_time: 0.0,
            property_descriptors: fixture_note_descriptors(),
        },
        NoteFixture {
            id: stable_id(b"fcs.note", b"fixture.evaluable.note"),
            line_id: evaluable_line_id,
            document_order: 1,
            kind: 1,
            side: 1,
            flags: 0b11,
            time: 1.5,
            end_time: 0.0,
            property_descriptors: fixture_note_descriptors(),
        },
    ];
    assemble_package(
        &lines,
        &notes,
        &[(0, 1, 0.0, 60.0, 0)],
        0.0,
        &[],
        &[],
        ExecutionGraph::Fixture,
    )
}

/// Product CanonicalCompilation → FCBC runtime package writer.
///
/// Encodes chart Lines/Notes/tempo into Core sections and attaches only
/// descriptors owned by those records. Track/expression lowering is added by
/// the following native handoff slices.
pub fn write_from_compilation(compilation: &CanonicalCompilation) -> FcbcResult<Vec<u8>> {
    let chart = compilation.chart();
    let mut lines: Vec<LineFixture> = chart
        .lines()
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let base = line.base();
            let inherit = line.inherit();
            LineFixture {
                id: line.id().value(),
                parent_id: line.parent().map_or(0, |parent| parent.value()),
                document_order: line.document_order() as u32,
                z_order: base.z_order(),
                inherit_flags: u32::from(inherit.position())
                    | u32::from(inherit.rotation()) << 1
                    | u32::from(inherit.scale()) << 2
                    | u32::from(inherit.alpha()) << 3
                    | u32::from(inherit.scroll()) << 4,
                line_flags: u32::from(base.allow_reverse_scroll()),
                position: [base.position().x(), base.position().y()],
                rotation: base.rotation(),
                scale: [base.scale().x(), base.scale().y()],
                alpha: base.alpha(),
                transform_origin: [base.transform_origin().x(), base.transform_origin().y()],
                texture_anchor: [base.texture_anchor().x(), base.texture_anchor().y()],
                // distance_index is filled after sort so section order matches Line ID order.
                distance_index: index as u32,
                position_descriptor: 0,
                rotation_descriptor: 0,
                scale_descriptor: 0,
                alpha_descriptor: 0,
                scroll_tempo_descriptor: 0,
                speed_descriptor: 0,
                evaluable_speed: false,
                floor_scale: base.floor_scale(),
                integration_origin: base.integration_origin(),
                initial_floor: base.initial_floor_position(),
            }
        })
        .collect();
    if lines.is_empty() {
        // Native charts without Lines still need a self-contained Line so Core
        // section loaders can attach tempo/note graph ownership.
        lines.push(LineFixture {
            id: stable_id(b"fcs.line", b"generated/default"),
            parent_id: 0,
            document_order: 0,
            z_order: 0,
            inherit_flags: 0,
            line_flags: 0,
            position: [0.0, 0.0],
            rotation: 0.0,
            scale: [1.0, 1.0],
            alpha: 1.0,
            transform_origin: [0.0, 0.0],
            texture_anchor: [0.5, 0.5],
            distance_index: 0,
            position_descriptor: 0,
            rotation_descriptor: 0,
            scale_descriptor: 0,
            alpha_descriptor: 0,
            scroll_tempo_descriptor: 0,
            speed_descriptor: 0,
            evaluable_speed: false,
            floor_scale: 1.0,
            integration_origin: 0.0,
            initial_floor: 0.0,
        });
    }
    lines.sort_by_key(|line| line.id);
    for (index, line) in lines.iter_mut().enumerate() {
        line.distance_index = index as u32;
    }
    let line_ids: std::collections::BTreeSet<u64> = lines.iter().map(|line| line.id).collect();
    let alpha_tracks = native_alpha_tracks(chart.tracks().tracks(), &line_ids)?;

    let mut notes: Vec<NoteFixture> = chart
        .notes()
        .notes()
        .iter()
        .map(|note| {
            let line_id = note.gameplay().line().value();
            if !line_ids.contains(&line_id) {
                return Err(FcbcError::new(
                    "fcbc.dangling-reference",
                    format!(
                        "Note {} references missing Line {line_id}",
                        note.id().value()
                    ),
                ));
            }
            let (kind, flags, end_time) = match note.kind() {
                CanonicalNoteKind::Tap => (1u8, 0b11u16, 0.0),
                CanonicalNoteKind::Hold => {
                    let end = note
                        .gameplay()
                        .end_time()
                        .map(|time| time.chart_time_seconds())
                        .unwrap_or(note.gameplay().time().chart_time_seconds() + 0.5);
                    (2u8, 0b111u16, end)
                }
                CanonicalNoteKind::Drag => (3u8, 0b11u16, 0.0),
                CanonicalNoteKind::Flick => (4u8, 0b11u16, 0.0),
            };
            let side = match note.gameplay().side() {
                CanonicalNoteSide::Above => 1u8,
                CanonicalNoteSide::Below => 2u8,
            };
            let judgment = if note.gameplay().judgment_enabled() {
                0b11
            } else {
                0b10
            };
            Ok(NoteFixture {
                id: note.id().value(),
                line_id,
                document_order: note.document_order() as u32,
                kind,
                side,
                flags: (flags & !0b11) | judgment,
                time: note.gameplay().time().chart_time_seconds(),
                end_time,
                // Native descriptor indices are assigned after global interning.
                property_descriptors: [0; 10],
            })
        })
        .collect::<FcbcResult<Vec<_>>>()?;
    notes.sort_by(|left, right| {
        (
            left.time.to_bits(),
            left.line_id,
            left.document_order,
            left.id,
        )
            .cmp(&(
                right.time.to_bits(),
                right.line_id,
                right.document_order,
                right.id,
            ))
    });

    let tempo: Vec<(i64, i64, f64, f64, u32)> = chart
        .time_map()
        .segments()
        .enumerate()
        .map(|(order, (beat, chart_time, bpm))| {
            let whole = beat.as_f64().floor() as i64;
            (whole, 1i64, chart_time, bpm, order as u32)
        })
        .collect();
    if tempo.is_empty() {
        return Err(FcbcError::new(
            "fcbc.invalid-tempo",
            "CanonicalCompilation tempo map must be non-empty",
        ));
    }
    let audio_offset = chart
        .metadata()
        .sync()
        .map(|sync| sync.audio_offset().seconds())
        .unwrap_or(0.0);

    let resources = native_resources(compilation)?;

    Ok(assemble_package(
        &lines,
        &notes,
        &tempo,
        audio_offset,
        &resources,
        &alpha_tracks,
        ExecutionGraph::Native {
            has_notes: !notes.is_empty(),
        },
    ))
}

#[cfg(test)]
mod compilation_tests {
    use super::*;
    use std::fs;

    use fcs_source::ResourceLimits;
    use fcs_source::elaborator::CompileTimeLimits;
    use fcs_source::parser::parse_document;
    use tempfile::tempdir;

    #[test]
    fn write_from_compilation_round_trips_through_product_load() {
        let workspace = tempdir().unwrap();
        let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
"#;
        let document = parse_document(source).into_result().unwrap();
        let compilation = document
            .canonical_compilation(
                CompileTimeLimits::default(),
                workspace.path(),
                ResourceLimits::default(),
            )
            .unwrap();
        let bytes = write_from_compilation(&compilation).unwrap();
        let container = crate::load_container(&bytes).expect("compiled FCBC framing must load");
        assert_eq!(container.byte_length, bytes.len());
        assert!(container.sections.len() >= 14);
        assert_eq!(&bytes[..4], b"FCSB");
        let decoded = crate::load_chart(&bytes).expect("compiled FCBC Core chart must load");
        assert_eq!(
            decoded.lines.len(),
            compilation.chart().lines().lines().count()
        );
        assert_eq!(
            decoded.notes.len(),
            compilation.chart().notes().notes().len()
        );
    }

    #[test]
    fn write_from_compilation_embeds_exact_resource_data() {
        let workspace = tempdir().unwrap();
        let payload = b"opaque\0resource\xffbytes";
        fs::write(workspace.path().join("payload.bin"), payload).unwrap();
        let source = r#"#fcs 5.0.0
format { profile: chart; }
resources {
    binary blob { source: "payload.bin"; mediaType: "application/octet-stream"; }
}
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
"#;
        let document = parse_document(source).into_result().unwrap();
        let compilation = document
            .canonical_compilation(
                CompileTimeLimits::default(),
                workspace.path(),
                ResourceLimits::default(),
            )
            .unwrap();

        let bytes = write_from_compilation(&compilation).unwrap();
        let decoded = crate::load_chart(&bytes).expect("compiled resources must load");
        let resource = decoded.resources.first().expect("embedded resource");
        assert_eq!(resource.id, stable_id(b"fcs.resource", b"blob"));
        assert_eq!(resource.kind, 7);
        assert_eq!(resource.media_type, "application/octet-stream");
        assert_eq!(resource.data_offset, 0);
        assert_eq!(resource.data_length, payload.len() as u64);
        let expected_sha256: [u8; 32] = Sha256::digest(payload).into();
        assert_eq!(resource.content_sha256, expected_sha256);
        assert_eq!(resource.bytes.as_ref(), payload);
    }

    #[test]
    fn write_from_compilation_preserves_native_line_record_fields() {
        let workspace = tempdir().unwrap();
        let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line root {}
    line child {
        parent: @root;
        floorScale: 96px;
        integrationOrigin: -2s;
        initialFloorPosition: 4.5;
        allowReverseScroll: true;
        zOrder: -3;
        inherit.position: false;
        inherit.rotation: true;
        inherit.scale: false;
        inherit.alpha: true;
        inherit.scroll: true;
    }
}
"#;
        let document = parse_document(source).into_result().unwrap();
        let compilation = document
            .canonical_compilation(
                CompileTimeLimits::default(),
                workspace.path(),
                ResourceLimits::default(),
            )
            .unwrap();

        let bytes = write_from_compilation(&compilation).unwrap();
        let decoded = crate::load_chart(&bytes).expect("compiled Lines must load");
        let child_id = stable_id(b"fcs.line", b"child");
        let child = decoded
            .lines
            .iter()
            .find(|line| line.id == child_id)
            .expect("child Line");
        assert_eq!(child.parent_id, stable_id(b"fcs.line", b"root"));
        assert_eq!(child.document_order, 1);
        assert_eq!(child.z_order, -3);
        assert_eq!(child.inherit_flags, 0b1_1010);
        assert_eq!(child.line_flags, 1);
        assert_eq!(child.floor_scale, 96.0);
        assert_eq!(child.integration_origin, -2.0);
        assert_eq!(child.initial_floor_position, 4.5);
        assert_eq!(decoded.feature_flags & (1 << 8), 1 << 8);
    }

    #[test]
    fn write_from_compilation_evaluates_exact_line_base_descriptors() {
        let workspace = tempdir().unwrap();
        let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        position: vec2(3px, -4px);
        rotation: 90deg;
        scale: vec2(0.5, 2.0);
        alpha: 0.25;
        transformOrigin: vec2(1px, 2px);
        textureAnchor: vec2(0.25, 0.75);
    }
}
"#;
        let document = parse_document(source).into_result().unwrap();
        let compilation = document
            .canonical_compilation(
                CompileTimeLimits::default(),
                workspace.path(),
                ResourceLimits::default(),
            )
            .unwrap();

        let bytes = write_from_compilation(&compilation).unwrap();
        let decoded = crate::load_chart(&bytes).expect("compiled Line descriptors must load");
        let line = decoded.lines.first().expect("main Line");
        let evaluate = |descriptor| {
            crate::query_descriptor(
                &decoded,
                descriptor,
                7.0,
                crate::EvaluationEnvironment::at_time(7.0),
            )
            .expect("Line descriptor evaluation")
            .value
        };
        assert_eq!(
            evaluate(line.position_descriptor),
            crate::RuntimeValue::Vec2 {
                ty: crate::ValueType::Vec2Length,
                value: [3.0, -4.0],
            }
        );
        assert_eq!(
            evaluate(line.rotation_descriptor),
            crate::RuntimeValue::Scalar {
                ty: crate::ValueType::Angle,
                value: std::f64::consts::FRAC_PI_2,
            }
        );
        assert_eq!(
            evaluate(line.scale_descriptor),
            crate::RuntimeValue::Vec2 {
                ty: crate::ValueType::Vec2Float,
                value: [0.5, 2.0],
            }
        );
        assert_eq!(
            evaluate(line.alpha_descriptor),
            crate::RuntimeValue::Scalar {
                ty: crate::ValueType::Float,
                value: 0.25,
            }
        );
        assert_eq!(
            decoded.constants[line.transform_origin_constant as usize],
            crate::RuntimeValue::Vec2 {
                ty: crate::ValueType::Vec2Length,
                value: [1.0, 2.0],
            }
        );
        assert_eq!(
            decoded.constants[line.texture_anchor_constant as usize],
            crate::RuntimeValue::Vec2 {
                ty: crate::ValueType::Vec2Float,
                value: [0.25, 0.75],
            }
        );
    }

    #[test]
    fn write_from_compilation_evaluates_native_linear_alpha_track() {
        let workspace = tempdir().unwrap();
        let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        alpha: 0.25;
        tracks {
            track fade -> alpha: float {
                fill: "error";
                extrapolateBefore: "holdBefore";
                extrapolateAfter: "holdAfter";
                segments { [0s, 2s): 1.0 -> 0.0 using "linear"; }
            }
        }
    }
}
"#;
        let document = parse_document(source).into_result().unwrap();
        let compilation = document
            .canonical_compilation(
                CompileTimeLimits::default(),
                workspace.path(),
                ResourceLimits::default(),
            )
            .unwrap();

        let bytes = write_from_compilation(&compilation).unwrap();
        let decoded = crate::load_chart(&bytes).expect("compiled alpha Track must load");
        let descriptor = decoded.lines.first().expect("main Line").alpha_descriptor;
        let evaluate = |time| {
            crate::query_descriptor(
                &decoded,
                descriptor,
                time,
                crate::EvaluationEnvironment::at_time(time),
            )
            .expect("alpha Track evaluation")
            .value
        };
        let alpha = |value| crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Float,
            value,
        };
        assert_eq!(evaluate(-1.0), alpha(1.0));
        assert_eq!(evaluate(1.0), alpha(0.5));
        assert_eq!(evaluate(3.0), alpha(0.0));
    }

    #[test]
    fn write_from_compilation_evaluates_native_alpha_track_points() {
        let workspace = tempdir().unwrap();
        let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        tracks {
            track fade -> alpha: float {
                fill: "error";
                extrapolateBefore: "holdBefore";
                extrapolateAfter: "holdAfter";
                segments {
                    point 0s: 1.0;
                    [1s, 3s): 0.8 -> 0.0 using "linear";
                    point 3s: 0.0;
                }
            }
        }
    }
}
"#;
        let document = parse_document(source).into_result().unwrap();
        let compilation = document
            .canonical_compilation(
                CompileTimeLimits::default(),
                workspace.path(),
                ResourceLimits::default(),
            )
            .unwrap();

        let bytes = write_from_compilation(&compilation).unwrap();
        let decoded = crate::load_chart(&bytes).expect("compiled alpha Track points must load");
        let descriptor = decoded.lines.first().expect("main Line").alpha_descriptor;
        let evaluate = |time| {
            crate::query_descriptor(
                &decoded,
                descriptor,
                time,
                crate::EvaluationEnvironment::at_time(time),
            )
            .expect("alpha Track point evaluation")
            .value
        };
        let alpha = |value| crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Float,
            value,
        };
        assert_eq!(evaluate(-1.0), alpha(1.0));
        assert_eq!(evaluate(0.5), alpha(1.0));
        assert_eq!(evaluate(2.0), alpha(0.4));
        assert_eq!(evaluate(4.0), alpha(0.0));
    }

    #[test]
    fn write_from_compilation_evaluates_native_alpha_easing_and_bezier() {
        let workspace = tempdir().unwrap();
        let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        tracks {
            track fade -> alpha: float {
                fill: "error";
                extrapolateBefore: "holdBefore";
                extrapolateAfter: "holdAfter";
                segments {
                    [0s, 2s): 0.0 -> 1.0 using "easeInQuad";
                    [2s, 4s): 1.0 -> 0.0 using cubicBezier(0.0, 0.0, 1.0, 1.0);
                }
            }
        }
    }
}
"#;
        let document = parse_document(source).into_result().unwrap();
        let compilation = document
            .canonical_compilation(
                CompileTimeLimits::default(),
                workspace.path(),
                ResourceLimits::default(),
            )
            .unwrap();

        let bytes = write_from_compilation(&compilation).unwrap();
        let decoded = crate::load_chart(&bytes).expect("compiled alpha easing Track must load");
        let descriptor = decoded.lines.first().expect("main Line").alpha_descriptor;
        let evaluate = |time| {
            crate::query_descriptor(
                &decoded,
                descriptor,
                time,
                crate::EvaluationEnvironment::at_time(time),
            )
            .expect("alpha easing Track evaluation")
            .value
        };
        let alpha = |value| crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Float,
            value,
        };
        assert_eq!(evaluate(1.0), alpha(0.25));
        assert_eq!(evaluate(3.0), alpha(0.5));
    }

    #[test]
    fn write_from_compilation_evaluates_all_native_alpha_easings() {
        for easing in EasingId::ALL {
            let workspace = tempdir().unwrap();
            let source = format!(
                r#"#fcs 5.0.0
format {{ profile: chart; }}
tempoMap {{ 0beat -> 120bpm; }}
lines {{
    line main {{
        tracks {{
            track fade -> alpha: float {{
                fill: "error";
                extrapolateBefore: "holdBefore";
                extrapolateAfter: "holdAfter";
                segments {{ [0s, 2s): 0.0 -> 1.0 using "{}"; }}
            }}
        }}
    }}
}}
"#,
                easing.name()
            );
            let document = parse_document(&source).into_result().unwrap();
            let compilation = document
                .canonical_compilation(
                    CompileTimeLimits::default(),
                    workspace.path(),
                    ResourceLimits::default(),
                )
                .unwrap();

            let bytes = write_from_compilation(&compilation).unwrap();
            let decoded = crate::load_chart(&bytes).expect("compiled alpha easing Track must load");
            let descriptor = decoded.lines.first().expect("main Line").alpha_descriptor;
            let actual = crate::query_descriptor(
                &decoded,
                descriptor,
                1.0,
                crate::EvaluationEnvironment::at_time(1.0),
            )
            .expect("alpha easing Track evaluation")
            .value;
            assert_eq!(
                actual,
                crate::RuntimeValue::Scalar {
                    ty: crate::ValueType::Float,
                    value: easing.evaluate(0.5).unwrap(),
                },
                "{}",
                easing.name()
            );
        }
    }
}

#[derive(Clone)]
struct NoteFixture {
    id: u64,
    line_id: u64,
    document_order: u32,
    kind: u8,
    side: u8,
    flags: u16,
    time: f64,
    end_time: f64,
    property_descriptors: [u32; 10],
}

#[derive(Clone, Copy)]
struct ResourceFixture<'a> {
    id: u64,
    kind: u16,
    media_type: &'a str,
    content_sha256: [u8; 32],
    bytes: &'a [u8],
}

fn assemble_package(
    lines: &[LineFixture],
    notes: &[NoteFixture],
    tempo: &[(i64, i64, f64, f64, u32)],
    audio_offset: f64,
    resources: &[ResourceFixture<'_>],
    alpha_tracks: &[AlphaTrackFixture],
    execution_graph: ExecutionGraph,
) -> Vec<u8> {
    let mut lines = lines.to_vec();
    let mut notes = notes.to_vec();
    let mut constants = fixture_constants();
    if matches!(execution_graph, ExecutionGraph::Native { .. }) {
        for line in &lines {
            constants.extend([
                vec2_constant(7, line.position),
                scalar_constant(8, line.rotation),
                vec2_constant(3, line.scale),
                float_constant(line.alpha),
                vec2_constant(7, line.transform_origin),
                vec2_constant(3, line.texture_anchor),
            ]);
        }
        for track in alpha_tracks {
            for segment in &track.segments {
                constants.extend([segment.start_constant.clone(), segment.end_constant.clone()]);
            }
        }
    }
    constants.sort_by(|left, right| {
        (left.tag, left.payload.as_slice()).cmp(&(right.tag, right.payload.as_slice()))
    });
    constants.dedup();
    let indices = constant_indices(&constants);
    let (tracks, expressions) = match execution_graph {
        ExecutionGraph::Fixture => (tracks_section(&indices), expression_section(&indices)),
        ExecutionGraph::Native { has_notes } => (
            native_tracks_section(
                &constants,
                &indices,
                &mut lines,
                &mut notes,
                alpha_tracks,
                has_notes,
            ),
            count_zero_section(),
        ),
    };
    let distances = distance_section_for_lines(&lines);
    let feature_flags = if lines.iter().any(|line| line.line_flags & 1 != 0) {
        1 << 8
    } else {
        0
    };
    let strings = string_table_values(resources);
    let (resource_records, resource_data) = resource_sections(resources, &strings);

    let mut sections = vec![
        Section::new(1, string_table_section(&strings)),
        Section::new(2, constant_pool_section(&constants)),
        Section::new(3, meta_section()),
        Section::new(4, count_zero_section()),
        Section::new(5, count_zero_section()),
        Section::new(6, resource_records),
        Section::new(7, sync_section_with_offset(audio_offset)),
        Section::new(8, tempo_section_from(tempo)),
        Section::new(9, lines_section(&lines, &constants)),
        Section::new(10, notes_section_from(&notes, &strings)),
        Section::new(11, tracks),
        Section::new(12, expressions),
        Section::new(13, distances),
        Section::new(20, resource_data),
    ];
    let table_length = sections.len() * 40;
    let mut bytes = vec![0; 128 + table_length];
    let mut body_cursor = bytes.len();
    for section in &mut sections {
        let aligned = align_up(body_cursor, 8);
        bytes.resize(aligned, 0);
        section.offset = aligned as u64;
        bytes.extend_from_slice(&section.payload);
        body_cursor = bytes.len();
    }

    write_header(&mut bytes, sections.len() as u32, feature_flags);
    write_section_table(&mut bytes, &sections);
    bytes
}

impl Section {
    fn new(kind: u32, payload: Vec<u8>) -> Self {
        Self {
            kind,
            payload,
            offset: 0,
        }
    }
}

fn fixture_constants() -> Vec<Constant> {
    vec![
        bool_constant(false),
        bool_constant(true),
        int_constant(2),
        float_constant(0.0),
        float_constant(1.0),
        float_constant(2.0),
        float_constant(10.0),
        float_constant(60.0),
        scalar_constant(7, 0.0),
        scalar_constant(8, 0.0),
        color_constant([1.0, 1.0, 1.0, 1.0]),
        vec2_constant(3, [0.0, 0.0]),
        vec2_constant(3, [1.0, 1.0]),
        vec2_constant(7, [0.0, 0.0]),
    ]
}

fn constant_indices(constants: &[Constant]) -> ConstantIndices {
    ConstantIndices {
        bool_false: find_constant(constants, &bool_constant(false)),
        bool_true: find_constant(constants, &bool_constant(true)),
        int_two: find_constant(constants, &int_constant(2)),
        float_zero: find_constant(constants, &float_constant(0.0)),
        float_one: find_constant(constants, &float_constant(1.0)),
        float_two: find_constant(constants, &float_constant(2.0)),
        float_ten: find_constant(constants, &float_constant(10.0)),
        float_sixty: find_constant(constants, &float_constant(60.0)),
        length_zero: find_constant(constants, &scalar_constant(7, 0.0)),
        angle_zero: find_constant(constants, &scalar_constant(8, 0.0)),
        color_white: find_constant(constants, &color_constant([1.0, 1.0, 1.0, 1.0])),
        vec2_float_one: find_constant(constants, &vec2_constant(3, [1.0, 1.0])),
    }
}

fn find_constant(constants: &[Constant], wanted: &Constant) -> u32 {
    constants
        .iter()
        .position(|constant| constant == wanted)
        .expect("fixture constant must be present") as u32
}

fn bool_constant(value: bool) -> Constant {
    let mut payload = vec![u8::from(value)];
    payload.resize(8, 0);
    Constant { tag: 1, payload }
}

fn int_constant(value: i64) -> Constant {
    Constant {
        tag: 2,
        payload: value.to_le_bytes().to_vec(),
    }
}

fn float_constant(value: f64) -> Constant {
    scalar_constant(3, value)
}

fn scalar_constant(tag: u8, value: f64) -> Constant {
    Constant {
        tag,
        payload: value.to_bits().to_le_bytes().to_vec(),
    }
}

fn color_constant(value: [f64; 4]) -> Constant {
    let mut payload = Vec::with_capacity(32);
    for component in value {
        put_f64(&mut payload, component);
    }
    Constant { tag: 9, payload }
}

fn vec2_constant(element_tag: u8, value: [f64; 2]) -> Constant {
    let mut payload = vec![element_tag];
    payload.resize(8, 0);
    put_f64(&mut payload, value[0]);
    put_f64(&mut payload, value[1]);
    Constant { tag: 10, payload }
}

fn native_resources(compilation: &CanonicalCompilation) -> FcbcResult<Vec<ResourceFixture<'_>>> {
    let mut resources: Vec<_> = compilation
        .resources()
        .resources()
        .values()
        .map(|bundled| {
            let resource = bundled.resource();
            ResourceFixture {
                id: stable_id(b"fcs.resource", resource.id().as_bytes()),
                kind: match resource.kind() {
                    CanonicalResourceKind::Audio => 1,
                    CanonicalResourceKind::Image => 2,
                    CanonicalResourceKind::Font => 3,
                    CanonicalResourceKind::Texture => 4,
                    CanonicalResourceKind::Path => 5,
                    CanonicalResourceKind::Shader => 6,
                    CanonicalResourceKind::Binary => 7,
                },
                media_type: resource.media_type(),
                content_sha256: bundled.content_sha256().as_bytes(),
                bytes: bundled.bytes(),
            }
        })
        .collect();
    resources.sort_by_key(|resource| resource.id);
    if resources.iter().any(|resource| resource.id == 0)
        || resources.windows(2).any(|pair| pair[0].id == pair[1].id)
    {
        return Err(FcbcError::new(
            "fcbc.duplicate-id",
            "canonical resource IDs collide in FCBC stable-ID space",
        ));
    }
    Ok(resources)
}

fn native_alpha_tracks(
    tracks: &[CanonicalTrack],
    line_ids: &std::collections::BTreeSet<u64>,
) -> FcbcResult<Vec<AlphaTrackFixture>> {
    let mut lowered = Vec::new();
    for track in tracks {
        let line_id = track.owner().value();
        if !line_ids.contains(&line_id) {
            return Err(FcbcError::new(
                "fcbc.dangling-reference",
                format!("Track {} references missing Line {line_id}", track.name()),
            ));
        }
        if track.target() != CanonicalTrackTarget::Alpha {
            return Err(FcbcError::new(
                "fcbc.unsupported-track",
                format!(
                    "native FCBC lowering does not yet support Track {} target {:?}",
                    track.name(),
                    track.target()
                ),
            ));
        }
        if track.blend() != CanonicalTrackBlend::Replace
            || track.extrapolate_before() != CanonicalTrackFill::HoldBefore
            || track.extrapolate_after() != CanonicalTrackFill::HoldAfter
        {
            return Err(FcbcError::new(
                "fcbc.unsupported-track",
                format!(
                    "native alpha Track {} requires replace with holdBefore/holdAfter",
                    track.name()
                ),
            ));
        }
        if lowered
            .iter()
            .any(|candidate: &AlphaTrackFixture| candidate.line_id == line_id)
        {
            return Err(FcbcError::new(
                "fcbc.unsupported-track",
                format!("native alpha Track layering is not yet supported for Line {line_id}"),
            ));
        }
        let pieces = track.pieces();
        let first_time = track_piece_time(&pieces[0]);
        let mut segments = Vec::new();
        for piece in pieces {
            match piece {
                CanonicalTrackPiece::Segment(segment) => {
                    segments.push(native_alpha_segment(segment, track.name())?);
                }
                CanonicalTrackPiece::Point(point) => {
                    let value = native_alpha_constant(point.value(), track.name())?;
                    let time = point.time().chart_time_seconds();
                    segments.push(TrackSegmentFixture {
                        start: time,
                        end: time,
                        interpolation: 1,
                        easing: 0,
                        flags: 1,
                        start_constant: value.clone(),
                        end_constant: value,
                        bezier: [0.0; 4],
                    });
                }
            }
        }
        if !pieces.iter().any(|piece| {
            matches!(
                piece,
                CanonicalTrackPiece::Point(point)
                    if point.time().chart_time_seconds().to_bits() == first_time.to_bits()
            )
        }) {
            let CanonicalTrackPiece::Segment(segment) = &pieces[0] else {
                unreachable!("first point check covers non-segment Track pieces");
            };
            let value = native_alpha_constant(segment.start_value(), track.name())?;
            segments.push(TrackSegmentFixture {
                start: first_time,
                end: first_time,
                interpolation: 1,
                easing: 0,
                flags: 1,
                start_constant: value.clone(),
                end_constant: value,
                bezier: [0.0; 4],
            });
        }
        if let Some(CanonicalTrackPiece::Segment(segment)) = pieces.last()
            && !pieces.iter().any(|piece| {
                matches!(
                    piece,
                    CanonicalTrackPiece::Point(point)
                        if point.time().chart_time_seconds().to_bits()
                            == segment.end().chart_time_seconds().to_bits()
                )
            })
        {
            let value = native_alpha_constant(segment.end_value(), track.name())?;
            segments.push(TrackSegmentFixture {
                start: segment.end().chart_time_seconds(),
                end: segment.end().chart_time_seconds(),
                interpolation: 1,
                easing: 0,
                flags: 1,
                start_constant: value.clone(),
                end_constant: value,
                bezier: [0.0; 4],
            });
        }
        for pair in pieces.windows(2) {
            if let CanonicalTrackPiece::Segment(segment) = &pair[0]
                && segment.end().chart_time_seconds() < track_piece_time(&pair[1])
            {
                return Err(FcbcError::new(
                    "fcbc.unsupported-track",
                    format!(
                        "native alpha Track {} has an uncovered segment gap",
                        track.name()
                    ),
                ));
            }
        }
        segments.sort_by(|left, right| {
            left.start
                .total_cmp(&right.start)
                .then_with(|| right.flags.cmp(&left.flags))
        });
        lowered.push(AlphaTrackFixture { line_id, segments });
    }
    lowered.sort_by_key(|track| track.line_id);
    Ok(lowered)
}

fn native_alpha_constant(value: CanonicalTrackValue, track_name: &str) -> FcbcResult<Constant> {
    match value {
        CanonicalTrackValue::Float(value) => Ok(float_constant(value)),
        _ => Err(FcbcError::new(
            "fcbc.invalid-track",
            format!("native alpha Track {track_name} has a non-float value"),
        )),
    }
}

fn native_alpha_segment(
    segment: &CanonicalTrackSegment,
    track_name: &str,
) -> FcbcResult<TrackSegmentFixture> {
    let (interpolation, easing, bezier) = match segment.interpolation() {
        CanonicalTrackInterpolation::Step => (1, 0, [0.0; 4]),
        CanonicalTrackInterpolation::Linear => (2, 0, [0.0; 4]),
        CanonicalTrackInterpolation::Easing(name) => {
            let easing = EasingId::ALL
                .into_iter()
                .find(|easing| easing.name() == name.as_str())
                .map(EasingId::abi_id)
                .ok_or_else(|| {
                    FcbcError::new(
                        "fcbc.invalid-track",
                        format!("native alpha Track {track_name} has unknown easing {name}"),
                    )
                })?;
            (3, easing, [0.0; 4])
        }
        CanonicalTrackInterpolation::CubicBezier(bezier) => (4, 0, *bezier),
    };
    Ok(TrackSegmentFixture {
        start: segment.start().chart_time_seconds(),
        end: segment.end().chart_time_seconds(),
        interpolation,
        easing,
        flags: 0,
        start_constant: native_alpha_constant(segment.start_value(), track_name)?,
        end_constant: native_alpha_constant(segment.end_value(), track_name)?,
        bezier,
    })
}

fn track_piece_time(piece: &CanonicalTrackPiece) -> f64 {
    match piece {
        CanonicalTrackPiece::Segment(segment) => segment.start().chart_time_seconds(),
        CanonicalTrackPiece::Point(point) => point.time().chart_time_seconds(),
    }
}

fn string_table_values<'a>(resources: &[ResourceFixture<'a>]) -> Vec<&'a str> {
    let mut strings = vec!["kind", "lineDefault"];
    strings.extend(resources.iter().map(|resource| resource.media_type));
    strings.sort_unstable_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    strings.dedup();
    strings
}

fn string_index(strings: &[&str], value: &str) -> u32 {
    strings
        .binary_search_by(|candidate| candidate.as_bytes().cmp(value.as_bytes()))
        .expect("package string must be present") as u32
}

fn string_table_section(strings: &[&str]) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u32(&mut payload, strings.len() as u32);
    put_u32(&mut payload, 0);
    let mut offset = 0u32;
    for string in strings {
        offset += string.len() as u32;
        put_u32(&mut payload, offset);
    }
    for string in strings {
        payload.extend_from_slice(string.as_bytes());
    }
    pad_to(&mut payload, 8);
    payload
}

fn resource_sections(resources: &[ResourceFixture<'_>], strings: &[&str]) -> (Vec<u8>, Vec<u8>) {
    let mut records = Vec::new();
    let mut data = Vec::new();
    put_u32(&mut records, resources.len() as u32);
    for resource in resources {
        pad_to(&mut data, 8);
        let data_offset = data.len() as u64;
        data.extend_from_slice(resource.bytes);

        let mut payload = Vec::new();
        put_u64(&mut payload, resource.id);
        put_u16(&mut payload, resource.kind);
        put_u16(&mut payload, 0);
        put_u32(&mut payload, string_index(strings, resource.media_type));
        put_u16(&mut payload, 1);
        put_u16(&mut payload, 0);
        put_u64(&mut payload, data_offset);
        put_u64(&mut payload, resource.bytes.len() as u64);
        payload.extend_from_slice(&counted_bytes(&resource.content_sha256));
        payload.extend_from_slice(&empty_object());
        records.extend_from_slice(&record(payload));
    }
    (records, data)
}

fn constant_pool_section(constants: &[Constant]) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u32(&mut payload, constants.len() as u32);
    for constant in constants {
        payload.extend_from_slice(&constant.encoded());
    }
    payload
}

fn meta_section() -> Vec<u8> {
    let mut payload = vec![2, 0, 0, 0]; // documentProfile=chart
    put_u32(&mut payload, 0);
    payload.extend_from_slice(&empty_object());
    payload.extend_from_slice(&empty_object());
    payload
}

fn count_zero_section() -> Vec<u8> {
    0u32.to_le_bytes().to_vec()
}

fn sync_section_with_offset(audio_offset: f64) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u64(&mut payload, 0);
    put_f64(&mut payload, audio_offset);
    put_u8(&mut payload, 0);
    payload.resize(24, 0);
    put_f64(&mut payload, 0.0);
    put_f64(&mut payload, 0.0);
    record(payload)
}

fn tempo_section_from(points: &[(i64, i64, f64, f64, u32)]) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u32(&mut payload, points.len() as u32);
    for (whole, denom, chart_time, bpm, order) in points {
        put_i64(&mut payload, *whole);
        put_i64(&mut payload, *denom);
        put_f64(&mut payload, *chart_time);
        put_f64(&mut payload, *bpm);
        put_u32(&mut payload, *order);
        put_u32(&mut payload, 0);
    }
    payload
}

fn lines_section(lines: &[LineFixture], constants: &[Constant]) -> Vec<u8> {
    let mut section = Vec::new();
    put_u32(&mut section, lines.len() as u32);
    for line in lines {
        let mut payload = Vec::new();
        put_u64(&mut payload, line.id);
        put_u64(&mut payload, line.parent_id);
        put_u32(&mut payload, line.document_order);
        put_i32(&mut payload, line.z_order);
        put_u32(&mut payload, line.inherit_flags);
        put_u32(&mut payload, line.line_flags);
        put_u32(&mut payload, line.position_descriptor);
        put_u32(&mut payload, line.rotation_descriptor);
        put_u32(&mut payload, line.scale_descriptor);
        put_u32(&mut payload, line.alpha_descriptor);
        put_u32(
            &mut payload,
            find_constant(constants, &vec2_constant(7, line.transform_origin)),
        );
        put_u32(
            &mut payload,
            find_constant(constants, &vec2_constant(3, line.texture_anchor)),
        );
        put_u32(&mut payload, line.scroll_tempo_descriptor);
        put_u32(&mut payload, line.speed_descriptor);
        put_u32(&mut payload, line.distance_index);
        put_f64(&mut payload, line.floor_scale);
        put_f64(&mut payload, line.integration_origin);
        put_f64(&mut payload, line.initial_floor);
        payload.extend_from_slice(&empty_object());
        section.extend_from_slice(&record(payload));
    }
    section
}

fn notes_section_from(notes: &[NoteFixture], strings: &[&str]) -> Vec<u8> {
    let mut section = Vec::new();
    put_u32(&mut section, notes.len() as u32);
    for note in notes {
        let mut payload = Vec::new();
        put_u64(&mut payload, note.id);
        put_u64(&mut payload, note.line_id);
        put_u32(&mut payload, note.document_order);
        put_u8(&mut payload, note.kind);
        put_u8(&mut payload, note.side);
        put_u16(&mut payload, note.flags);
        put_f64(&mut payload, note.time);
        put_f64(&mut payload, note.end_time);
        payload.extend_from_slice(&line_default_judge_shape(strings));
        // judgment-enabled uses default sound/score; disabled uses none policies.
        let judgment_enabled = note.flags & 0b1 != 0;
        put_u16(&mut payload, if judgment_enabled { 1 } else { 2 }); // default/none sound
        put_u16(&mut payload, if judgment_enabled { 1 } else { 2 }); // default/none score
        put_u64(&mut payload, 0);
        put_u32(&mut payload, NULL_INDEX);
        put_u32(&mut payload, 0);
        for descriptor in note.property_descriptors {
            put_u32(&mut payload, descriptor);
        }
        put_u64(&mut payload, 0);
        payload.extend_from_slice(&empty_object());
        section.extend_from_slice(&record(payload));
    }
    section
}

fn tracks_section(constants: &ConstantIndices) -> Vec<u8> {
    let mut descriptors = vec![
        expression_descriptor(TY_FLOAT, 7),
        expression_descriptor(TY_FLOAT, 20),
        expression_descriptor(TY_VEC2_LENGTH, 22),
        expression_descriptor(TY_ANGLE, 23),
        constant_descriptor(TY_VEC2_FLOAT, constants.vec2_float_one),
        segment_descriptor(constants.float_zero, constants.float_two),
        constant_descriptor(TY_FLOAT, constants.float_two),
        constant_descriptor(TY_FLOAT, constants.float_sixty),
        constant_descriptor(TY_FLOAT, constants.float_one),
        constant_descriptor(TY_COLOR, constants.color_white),
        expression_descriptor(TY_LENGTH, 28),
        piecewise_descriptor(FLOAT_ONE_DESCRIPTOR_INDEX),
        expression_descriptor(TY_BOOL, 39),
        constant_descriptor(TY_LENGTH, constants.length_zero),
    ];
    debug_assert_eq!(descriptors.len(), 14);
    let mut section = Vec::new();
    put_u32(&mut section, descriptors.len() as u32);
    for descriptor in descriptors.drain(..) {
        section.extend_from_slice(&descriptor);
    }
    section
}

fn native_tracks_section(
    constants: &[Constant],
    indices: &ConstantIndices,
    lines: &mut [LineFixture],
    notes: &mut [NoteFixture],
    alpha_tracks: &[AlphaTrackFixture],
    has_notes: bool,
) -> Vec<u8> {
    let mut descriptors = Vec::new();

    // Descriptor order follows the canonical direct-root path order used by the loader:
    // all Line roots are grouped by path, then by stable Line ID.
    for line in lines.iter_mut() {
        line.alpha_descriptor =
            if let Some(track) = alpha_tracks.iter().find(|track| track.line_id == line.id) {
                let descriptor = segment_track_descriptor(TY_FLOAT, &track.segments, constants);
                intern_descriptor(&mut descriptors, descriptor)
            } else {
                intern_constant_descriptor(
                    &mut descriptors,
                    TY_FLOAT,
                    find_constant(constants, &float_constant(line.alpha)),
                )
            };
    }
    for line in lines.iter_mut() {
        line.position_descriptor = intern_constant_descriptor(
            &mut descriptors,
            TY_VEC2_LENGTH,
            find_constant(constants, &vec2_constant(7, line.position)),
        );
    }
    for line in lines.iter_mut() {
        line.rotation_descriptor = intern_constant_descriptor(
            &mut descriptors,
            TY_ANGLE,
            find_constant(constants, &scalar_constant(8, line.rotation)),
        );
    }
    for line in lines.iter_mut() {
        line.scale_descriptor = intern_constant_descriptor(
            &mut descriptors,
            TY_VEC2_FLOAT,
            find_constant(constants, &vec2_constant(3, line.scale)),
        );
    }
    for line in lines.iter_mut() {
        line.speed_descriptor =
            intern_constant_descriptor(&mut descriptors, TY_FLOAT, indices.float_one);
    }
    for line in lines.iter_mut() {
        line.scroll_tempo_descriptor =
            intern_constant_descriptor(&mut descriptors, TY_FLOAT, indices.float_sixty);
    }

    if has_notes {
        // Intern Note roots in canonical target-path order, then map them back to
        // the NoteRecord field order.
        let alpha = intern_constant_descriptor(&mut descriptors, TY_FLOAT, indices.float_one);
        let color = intern_constant_descriptor(&mut descriptors, TY_COLOR, indices.color_white);
        let position_x =
            intern_constant_descriptor(&mut descriptors, TY_LENGTH, indices.length_zero);
        let rotation = intern_constant_descriptor(&mut descriptors, TY_ANGLE, indices.angle_zero);
        let scale_x = intern_constant_descriptor(&mut descriptors, TY_FLOAT, indices.float_one);
        let scale_y = intern_constant_descriptor(&mut descriptors, TY_FLOAT, indices.float_one);
        let scroll_factor =
            intern_constant_descriptor(&mut descriptors, TY_FLOAT, indices.float_one);
        let visibility = intern_constant_descriptor(&mut descriptors, TY_BOOL, indices.bool_true);
        let x_offset = intern_constant_descriptor(&mut descriptors, TY_LENGTH, indices.length_zero);
        let y_offset = intern_constant_descriptor(&mut descriptors, TY_LENGTH, indices.length_zero);
        let note_descriptors = [
            position_x,
            scroll_factor,
            x_offset,
            y_offset,
            alpha,
            scale_x,
            scale_y,
            rotation,
            color,
            visibility,
        ];
        for note in notes {
            note.property_descriptors = note_descriptors;
        }
    }

    let mut section = Vec::new();
    put_u32(&mut section, descriptors.len() as u32);
    for descriptor in descriptors {
        section.extend_from_slice(&descriptor);
    }
    section
}

fn intern_constant_descriptor(
    descriptors: &mut Vec<Vec<u8>>,
    property_type: u8,
    constant_index: u32,
) -> u32 {
    intern_descriptor(
        descriptors,
        constant_descriptor(property_type, constant_index),
    )
}

fn intern_descriptor(descriptors: &mut Vec<Vec<u8>>, descriptor: Vec<u8>) -> u32 {
    if let Some(index) = descriptors
        .iter()
        .position(|candidate| candidate == &descriptor)
    {
        return index as u32;
    }
    let index = descriptors.len() as u32;
    descriptors.push(descriptor);
    index
}

const fn fixture_note_descriptors() -> [u32; 10] {
    [
        NOTE_POSITION_X_DESCRIPTOR_INDEX,
        FLOAT_ONE_DESCRIPTOR_INDEX,
        LENGTH_ZERO_DESCRIPTOR_INDEX,
        LENGTH_ZERO_DESCRIPTOR_INDEX,
        FLOAT_ONE_DESCRIPTOR_INDEX,
        PIECEWISE_ONE_DESCRIPTOR_INDEX,
        FLOAT_ONE_DESCRIPTOR_INDEX,
        ROTATION_DESCRIPTOR_INDEX,
        COLOR_DESCRIPTOR_INDEX,
        VISIBILITY_DESCRIPTOR_INDEX,
    ]
}

fn constant_descriptor(property_type: u8, constant_index: u32) -> Vec<u8> {
    let mut payload = descriptor_common(property_type, 1, 0b11, 0.0, 0.0);
    put_u32(&mut payload, constant_index);
    let descriptor = record(payload);
    debug_assert_eq!(descriptor.len(), 32);
    descriptor
}

fn segment_track_descriptor(
    property_type: u8,
    segments: &[TrackSegmentFixture],
    constants: &[Constant],
) -> Vec<u8> {
    let mut payload = descriptor_common(property_type, 2, 0b11, 0.0, 0.0);
    put_u32(&mut payload, segments.len() as u32);
    for segment in segments {
        put_f64(&mut payload, segment.start);
        put_f64(&mut payload, segment.end);
        put_u16(&mut payload, segment.interpolation);
        put_u16(&mut payload, segment.easing);
        put_u32(&mut payload, segment.flags);
        put_u32(
            &mut payload,
            find_constant(constants, &segment.start_constant),
        );
        put_u32(
            &mut payload,
            find_constant(constants, &segment.end_constant),
        );
        for value in segment.bezier {
            put_f64(&mut payload, value);
        }
    }
    let descriptor = record(payload);
    debug_assert_eq!(descriptor.len(), 32 + 64 * segments.len());
    descriptor
}

fn segment_descriptor(start_constant: u32, end_constant: u32) -> Vec<u8> {
    let mut payload = descriptor_common(TY_FLOAT, 2, 0b11, 0.0, 0.0);
    put_u32(&mut payload, 3);
    put_f64(&mut payload, 0.0);
    put_f64(&mut payload, 0.0);
    put_u16(&mut payload, 1);
    put_u16(&mut payload, 0);
    put_u32(&mut payload, 1);
    put_u32(&mut payload, start_constant);
    put_u32(&mut payload, start_constant);
    for _ in 0..4 {
        put_f64(&mut payload, 0.0);
    }
    put_f64(&mut payload, 0.0);
    put_f64(&mut payload, 2.0);
    put_u16(&mut payload, 2); // linear
    put_u16(&mut payload, 0);
    put_u32(&mut payload, 0);
    put_u32(&mut payload, start_constant);
    put_u32(&mut payload, end_constant);
    for _ in 0..4 {
        put_f64(&mut payload, 0.0);
    }
    put_f64(&mut payload, 2.0);
    put_f64(&mut payload, 2.0);
    put_u16(&mut payload, 1);
    put_u16(&mut payload, 0);
    put_u32(&mut payload, 1);
    put_u32(&mut payload, end_constant);
    put_u32(&mut payload, end_constant);
    for _ in 0..4 {
        put_f64(&mut payload, 0.0);
    }
    let descriptor = record(payload);
    debug_assert_eq!(descriptor.len(), 224);
    descriptor
}

fn piecewise_descriptor(inner_descriptor: u32) -> Vec<u8> {
    let mut payload = descriptor_common(TY_FLOAT, 3, 0b11, 0.0, 0.0);
    put_u32(&mut payload, 1);
    put_f64(&mut payload, 0.0);
    put_f64(&mut payload, 0.0);
    put_u32(&mut payload, inner_descriptor);
    put_u32(&mut payload, 0b110); // unbounded before + after
    let descriptor = record(payload);
    debug_assert_eq!(descriptor.len(), 56);
    descriptor
}

fn expression_descriptor(property_type: u8, root: u32) -> Vec<u8> {
    let mut payload = descriptor_common(property_type, 4, 0b11, 0.0, 0.0);
    put_u32(&mut payload, root);
    let descriptor = record(payload);
    debug_assert_eq!(descriptor.len(), 32);
    descriptor
}

fn descriptor_common(
    property_type: u8,
    kind: u8,
    flags: u16,
    domain_start: f64,
    domain_end: f64,
) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u8(&mut payload, property_type);
    put_u8(&mut payload, kind);
    put_u16(&mut payload, flags);
    put_f64(&mut payload, domain_start);
    put_f64(&mut payload, domain_end);
    payload
}

fn expression_section(constants: &ConstantIndices) -> Vec<u8> {
    let mut nodes = Vec::new();
    // D0: line.alpha for the lexicographically first (evaluable) line.
    expression_node(&mut nodes, 1, TY_FLOAT, &[], constants.float_ten);
    expression_node(&mut nodes, 2, TY_TIME, &[], 0);
    expression_node(&mut nodes, 80, 12, &[1, 1], 0); // vec2-time
    expression_node(&mut nodes, 81, TY_TIME, &[2], 0);
    expression_node(&mut nodes, 62, TY_FLOAT, &[3], 0); // Seconds
    expression_node(&mut nodes, 1, TY_FLOAT, &[], constants.float_two);
    expression_node(&mut nodes, 22, TY_FLOAT, &[4, 5], 0);
    expression_node(&mut nodes, 20, TY_FLOAT, &[0, 6], 0);

    // D1: the second line alpha. This executes int/angle conversions and vector X/Y.
    expression_node(&mut nodes, 1, TY_BOOL, &[], constants.bool_true);
    expression_node(&mut nodes, 1, TY_INT, &[], constants.int_two);
    expression_node(&mut nodes, 80, 11, &[9, 9], 0); // vec2-int
    expression_node(&mut nodes, 81, TY_INT, &[10], 0);
    expression_node(&mut nodes, 61, TY_FLOAT, &[11], 0); // ToFloat
    expression_node(&mut nodes, 1, TY_ANGLE, &[], constants.angle_zero);
    expression_node(&mut nodes, 80, 14, &[13, 13], 0); // vec2-angle
    expression_node(&mut nodes, 82, TY_ANGLE, &[14], 0);
    expression_node(&mut nodes, 63, TY_FLOAT, &[15], 0); // Radians
    expression_node(&mut nodes, 80, TY_VEC2_FLOAT, &[12, 16], 0);
    expression_node(&mut nodes, 81, TY_FLOAT, &[17], 0);
    expression_node(&mut nodes, 82, TY_FLOAT, &[17], 0);
    expression_node(&mut nodes, 70, TY_FLOAT, &[8, 18, 19], 0);

    // D2: line.position is independent of Note distance d.
    expression_node(&mut nodes, 1, TY_LENGTH, &[], constants.length_zero);
    expression_node(&mut nodes, 80, TY_VEC2_LENGTH, &[21, 21], 0);

    // D3: rotation shares the already emitted vec2-angle node.
    expression_node(&mut nodes, 81, TY_ANGLE, &[14], 0);

    // D10: Note presentation.positionX owns the EnvD-dependent vec2-length X/Y chain.
    expression_node(&mut nodes, 5, TY_LENGTH, &[], 0);
    expression_node(&mut nodes, 80, TY_VEC2_LENGTH, &[24, 21], 0);
    expression_node(&mut nodes, 81, TY_LENGTH, &[25], 0);
    expression_node(&mut nodes, 82, TY_LENGTH, &[25], 0);
    expression_node(&mut nodes, 20, TY_LENGTH, &[26, 27], 0);

    // D12: visibility demonstrates short-circuit And/Or/Choose and reaches every branch through
    // another selected path, including vec2-beat and ApproxEq.
    expression_node(&mut nodes, 3, 5, &[], 0);
    expression_node(&mut nodes, 80, 13, &[29, 29], 0); // vec2-beat
    expression_node(&mut nodes, 81, 5, &[30], 0);
    expression_node(&mut nodes, 30, TY_BOOL, &[31, 29], 0);
    expression_node(&mut nodes, 37, TY_BOOL, &[8, 32], 0); // short-circuit Or
    expression_node(&mut nodes, 1, TY_BOOL, &[], constants.bool_false);
    expression_node(&mut nodes, 36, TY_BOOL, &[34, 32], 0); // short-circuit And
    expression_node(&mut nodes, 38, TY_BOOL, &[18, 12, 19], 0); // ApproxEq
    expression_node(&mut nodes, 37, TY_BOOL, &[35, 36], 0);
    expression_node(&mut nodes, 36, TY_BOOL, &[33, 37], 0);
    expression_node(&mut nodes, 70, TY_BOOL, &[38, 32, 34], 0);

    let mut section = Vec::new();
    put_u32(&mut section, 40);
    section.extend_from_slice(&nodes);
    section
}

fn expression_node(
    nodes: &mut Vec<u8>,
    opcode: u16,
    result_type: u8,
    operands: &[u32],
    immediate: u32,
) {
    debug_assert!(operands.len() <= 3);
    put_u16(nodes, opcode);
    put_u8(nodes, result_type);
    put_u8(nodes, operands.len() as u8);
    for index in 0..3 {
        put_u32(nodes, operands.get(index).copied().unwrap_or(NULL_INDEX));
    }
    put_u32(nodes, immediate);
}

fn distance_section_for_lines(lines: &[LineFixture]) -> Vec<u8> {
    let mut section = Vec::new();
    put_u32(&mut section, lines.len() as u32);
    for line in lines {
        // Classification/boundary pairing must match the Line's scroll speed descriptor.
        let (classification, max_error, mut boundaries) = if line.evaluable_speed {
            (
                2u8,
                2.328_306_436_538_696_3e-10,
                vec![line.integration_origin, 2.0],
            )
        } else {
            (1u8, 0.0, vec![line.integration_origin])
        };
        boundaries.sort_by(f64::total_cmp);
        boundaries.dedup_by(|left, right| left.to_bits() == right.to_bits());
        section.extend_from_slice(&distance_record(
            line.id,
            line.speed_descriptor,
            line.integration_origin,
            line.initial_floor,
            classification,
            max_error,
            &boundaries,
        ));
    }
    section
}

fn distance_record(
    line_id: u64,
    scroll_speed_descriptor: u32,
    integration_origin: f64,
    initial_floor: f64,
    classification: u8,
    max_distance_error: f64,
    boundaries: &[f64],
) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u64(&mut payload, line_id);
    put_u32(&mut payload, scroll_speed_descriptor);
    put_u32(&mut payload, NULL_INDEX);
    put_f64(&mut payload, 0.0);
    put_f64(&mut payload, 0.0);
    put_f64(&mut payload, integration_origin);
    put_f64(&mut payload, initial_floor);
    put_f64(&mut payload, 0.0);
    put_f64(&mut payload, max_distance_error);
    put_u32(&mut payload, boundaries.len() as u32);
    put_u8(&mut payload, classification);
    put_u8(&mut payload, 0b11);
    put_u16(&mut payload, 0);
    for boundary in boundaries {
        put_f64(&mut payload, *boundary);
    }
    let result = record(payload);
    debug_assert_eq!(result.len(), 80 + boundaries.len() * 8);
    result
}

fn line_default_judge_shape(strings: &[&str]) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u32(&mut payload, 1);
    put_u32(&mut payload, string_index(strings, "kind"));
    put_u8(&mut payload, 4); // string Value
    put_u8(&mut payload, 0);
    put_u16(&mut payload, 0);
    put_u32(&mut payload, 8);
    put_u32(&mut payload, string_index(strings, "lineDefault"));
    put_u32(&mut payload, 0);
    value(14, payload)
}

fn empty_object() -> Vec<u8> {
    value(14, 0u32.to_le_bytes().to_vec())
}

fn value(tag: u8, payload: Vec<u8>) -> Vec<u8> {
    let mut bytes = Vec::new();
    put_u8(&mut bytes, tag);
    put_u8(&mut bytes, 0);
    put_u16(&mut bytes, 0);
    put_u32(&mut bytes, payload.len() as u32);
    bytes.extend_from_slice(&payload);
    pad_to(&mut bytes, 8);
    bytes
}

fn counted_bytes(payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    put_u32(&mut bytes, payload.len() as u32);
    bytes.extend_from_slice(payload);
    pad_to(&mut bytes, 4);
    bytes
}

fn record(mut payload: Vec<u8>) -> Vec<u8> {
    while !(payload.len() + 8).is_multiple_of(4) {
        payload.push(0);
    }
    let mut bytes = Vec::with_capacity(payload.len() + 8);
    put_u32(&mut bytes, (payload.len() + 8) as u32);
    put_u16(&mut bytes, 1);
    put_u16(&mut bytes, 0);
    bytes.extend_from_slice(&payload);
    bytes
}

fn write_header(bytes: &mut [u8], section_count: u32, feature_flags: u64) {
    bytes[0..4].copy_from_slice(b"FCSB");
    write_u16_at(bytes, 4, 128);
    write_u16_at(bytes, 6, 0);
    write_u16_at(bytes, 8, 5);
    write_u16_at(bytes, 10, 0);
    write_u16_at(bytes, 12, 0);
    write_u16_at(bytes, 14, 2);
    write_u16_at(bytes, 16, 0);
    write_u16_at(bytes, 18, 0);
    write_u16_at(bytes, 20, 1);
    write_u16_at(bytes, 22, 0);
    write_u16_at(bytes, 24, 0);
    bytes[26] = 3; // strict-runtime
    bytes[27] = 1; // binary64
    write_u64_at(bytes, 28, feature_flags);
    write_u32_at(bytes, 36, section_count);
    write_u64_at(bytes, 40, 128);
    write_u64_at(bytes, 48, bytes.len() as u64);
    write_u32_at(bytes, 88, NULL_INDEX);
    write_u32_at(bytes, 92, NULL_INDEX);
}

fn write_section_table(bytes: &mut [u8], sections: &[Section]) {
    for (index, section) in sections.iter().enumerate() {
        let start = 128 + index * 40;
        write_u32_at(bytes, start, section.kind);
        write_u16_at(bytes, start + 4, 1);
        write_u16_at(bytes, start + 6, 0);
        write_u16_at(bytes, start + 8, 0);
        write_u16_at(bytes, start + 10, REQUIRED);
        bytes[start + 12] = 3;
        write_u64_at(bytes, start + 16, section.offset);
        write_u64_at(bytes, start + 24, section.payload.len() as u64);
        write_u32_at(bytes, start + 32, crc32_iso_hdlc(&section.payload));
    }
}

fn stable_id(namespace: &[u8], textual_id: &[u8]) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(namespace);
    hasher.update([0]);
    hasher.update(textual_id);
    let digest = hasher.finalize();
    u64::from_le_bytes(digest[..8].try_into().expect("SHA-256 prefix width"))
}

fn crc32_iso_hdlc(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

fn pad_to(bytes: &mut Vec<u8>, alignment: usize) {
    bytes.resize(align_up(bytes.len(), alignment), 0);
}

fn put_u8(bytes: &mut Vec<u8>, value: u8) {
    bytes.push(value);
}

fn put_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_i64(bytes: &mut Vec<u8>, value: i64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_f64(bytes: &mut Vec<u8>, value: f64) {
    bytes.extend_from_slice(&value.to_bits().to_le_bytes());
}

fn write_u16_at(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32_at(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64_at(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
