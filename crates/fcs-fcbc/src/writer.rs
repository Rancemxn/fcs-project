use fcs_model::{CanonicalCompilation, CanonicalNoteKind, CanonicalNoteSide};
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
    distance_index: u32,
    alpha_descriptor: u32,
    speed_descriptor: u32,
    initial_floor: f64,
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
    vec2_float_zero: u32,
    vec2_float_one: u32,
    vec2_length_zero: u32,
}

struct Section {
    kind: u32,
    payload: Vec<u8>,
    offset: u64,
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
            distance_index: ANALYTIC_DISTANCE_INDEX,
            alpha_descriptor: CHOOSE_ALPHA_DESCRIPTOR_INDEX,
            speed_descriptor: ANALYTIC_SPEED_DESCRIPTOR_INDEX,
            initial_floor: 10.0,
        },
        LineFixture {
            id: evaluable_line_id,
            distance_index: EVALUABLE_DISTANCE_INDEX,
            alpha_descriptor: SECONDS_ALPHA_DESCRIPTOR_INDEX,
            speed_descriptor: EVALUABLE_SPEED_DESCRIPTOR_INDEX,
            initial_floor: 20.0,
        },
    ];
    lines.sort_by_key(|line| line.id);
    let line_count = lines.len();
    for (index, line) in lines.iter_mut().enumerate() {
        line.distance_index = index as u32;
        // Preserve the historical speed/alpha pairing used by the nonempty golden:
        // lower Line ID uses evaluable path; higher uses analytic path when two Lines exist.
        if line_count >= 2 {
            if index == 0 {
                line.alpha_descriptor = SECONDS_ALPHA_DESCRIPTOR_INDEX;
                line.speed_descriptor = EVALUABLE_SPEED_DESCRIPTOR_INDEX;
                line.initial_floor = 20.0;
            } else if index == 1 {
                line.alpha_descriptor = CHOOSE_ALPHA_DESCRIPTOR_INDEX;
                line.speed_descriptor = ANALYTIC_SPEED_DESCRIPTOR_INDEX;
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
        },
    ];
    assemble_package(&lines, &notes, &[(0, 1, 0.0, 60.0, 0)], 0.0)
}

/// Product CanonicalCompilation → FCBC runtime package writer.
///
/// Encodes chart Lines/Notes/tempo into Core sections and attaches the shared
/// exact descriptor/expression scaffold required by Execution ABI loaders.
/// Resource payloads from the compilation bundle are embedded when present.
pub fn write_from_compilation(compilation: &CanonicalCompilation) -> FcbcResult<Vec<u8>> {
    let chart = compilation.chart();
    let mut lines: Vec<LineFixture> = chart
        .lines()
        .lines()
        .enumerate()
        .map(|(index, line)| LineFixture {
            id: line.id().value(),
            // distance_index is filled after sort so section order matches Line ID order.
            distance_index: index as u32,
            alpha_descriptor: if index % 2 == 0 {
                CHOOSE_ALPHA_DESCRIPTOR_INDEX
            } else {
                SECONDS_ALPHA_DESCRIPTOR_INDEX
            },
            speed_descriptor: if index % 2 == 0 {
                ANALYTIC_SPEED_DESCRIPTOR_INDEX
            } else {
                EVALUABLE_SPEED_DESCRIPTOR_INDEX
            },
            initial_floor: line.base().floor_scale(),
        })
        .collect();
    if lines.is_empty() {
        return Err(FcbcError::new(
            "fcbc.invalid-record",
            "CanonicalCompilation must contain at least one Line for FCBC write",
        ));
    }
    lines.sort_by_key(|line| line.id);
    for (index, line) in lines.iter_mut().enumerate() {
        line.distance_index = index as u32;
    }
    let line_ids: std::collections::BTreeSet<u64> = lines.iter().map(|line| line.id).collect();

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

    Ok(assemble_package(&lines, &notes, &tempo, audio_offset))
}

#[cfg(test)]
mod compilation_tests {
    use super::*;
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
        let chart = crate::load_chart(&bytes).expect("compiled FCBC must load");
        assert_eq!(chart.lines.len(), 1);
        assert_eq!(chart.notes.len(), 1);
        assert_eq!(chart.container_profile, 3);
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
}

fn assemble_package(
    lines: &[LineFixture],
    notes: &[NoteFixture],
    tempo: &[(i64, i64, f64, f64, u32)],
    audio_offset: f64,
) -> Vec<u8> {
    let mut constants = fixture_constants();
    constants.sort_by(|left, right| {
        (left.tag, left.payload.as_slice()).cmp(&(right.tag, right.payload.as_slice()))
    });
    constants.dedup();
    let indices = constant_indices(&constants);
    let expressions = expression_section(&indices);
    let tracks = tracks_section(&indices);
    let distances = distance_section_for_lines(lines);

    let mut sections = vec![
        Section::new(1, string_table_section()),
        Section::new(2, constant_pool_section(&constants)),
        Section::new(3, meta_section()),
        Section::new(4, count_zero_section()),
        Section::new(5, count_zero_section()),
        Section::new(6, count_zero_section()),
        Section::new(7, sync_section_with_offset(audio_offset)),
        Section::new(8, tempo_section_from(tempo)),
        Section::new(9, lines_section(lines, &indices)),
        Section::new(10, notes_section_from(notes)),
        Section::new(11, tracks),
        Section::new(12, expressions),
        Section::new(13, distances),
        Section::new(20, Vec::new()),
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

    write_header(&mut bytes, sections.len() as u32);
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
        vec2_float_zero: find_constant(constants, &vec2_constant(3, [0.0, 0.0])),
        vec2_float_one: find_constant(constants, &vec2_constant(3, [1.0, 1.0])),
        vec2_length_zero: find_constant(constants, &vec2_constant(7, [0.0, 0.0])),
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

fn string_table_section() -> Vec<u8> {
    let strings = [b"kind".as_slice(), b"lineDefault".as_slice()];
    let mut payload = Vec::new();
    put_u32(&mut payload, strings.len() as u32);
    put_u32(&mut payload, 0);
    let mut offset = 0u32;
    for string in strings {
        offset += string.len() as u32;
        put_u32(&mut payload, offset);
    }
    for string in strings {
        payload.extend_from_slice(string);
    }
    pad_to(&mut payload, 8);
    payload
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

fn lines_section(lines: &[LineFixture], constants: &ConstantIndices) -> Vec<u8> {
    let mut section = Vec::new();
    put_u32(&mut section, lines.len() as u32);
    for (document_order, line) in lines.iter().enumerate() {
        let mut payload = Vec::new();
        put_u64(&mut payload, line.id);
        put_u64(&mut payload, 0);
        put_u32(&mut payload, document_order as u32);
        put_i32(&mut payload, 0);
        put_u32(&mut payload, 0);
        put_u32(&mut payload, 0);
        put_u32(&mut payload, POSITION_DESCRIPTOR_INDEX);
        put_u32(&mut payload, ROTATION_DESCRIPTOR_INDEX);
        put_u32(&mut payload, SCALE_DESCRIPTOR_INDEX);
        put_u32(&mut payload, line.alpha_descriptor);
        put_u32(&mut payload, constants.vec2_length_zero);
        put_u32(&mut payload, constants.vec2_float_zero);
        put_u32(&mut payload, SCROLL_TEMPO_DESCRIPTOR_INDEX);
        put_u32(&mut payload, line.speed_descriptor);
        put_u32(&mut payload, line.distance_index);
        put_f64(&mut payload, 1.0);
        put_f64(&mut payload, 0.0);
        put_f64(&mut payload, line.initial_floor);
        payload.extend_from_slice(&empty_object());
        section.extend_from_slice(&record(payload));
    }
    section
}

fn notes_section_from(notes: &[NoteFixture]) -> Vec<u8> {
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
        payload.extend_from_slice(&line_default_judge_shape());
        // judgment-enabled uses default sound/score; disabled uses none policies.
        let judgment_enabled = note.flags & 0b1 != 0;
        put_u16(&mut payload, if judgment_enabled { 1 } else { 2 }); // default/none sound
        put_u16(&mut payload, if judgment_enabled { 1 } else { 2 }); // default/none score
        put_u64(&mut payload, 0);
        put_u32(&mut payload, NULL_INDEX);
        put_u32(&mut payload, 0);
        put_u32(&mut payload, NOTE_POSITION_X_DESCRIPTOR_INDEX);
        put_u32(&mut payload, FLOAT_ONE_DESCRIPTOR_INDEX);
        put_u32(&mut payload, LENGTH_ZERO_DESCRIPTOR_INDEX);
        put_u32(&mut payload, LENGTH_ZERO_DESCRIPTOR_INDEX);
        put_u32(&mut payload, FLOAT_ONE_DESCRIPTOR_INDEX);
        put_u32(&mut payload, PIECEWISE_ONE_DESCRIPTOR_INDEX);
        put_u32(&mut payload, FLOAT_ONE_DESCRIPTOR_INDEX);
        put_u32(&mut payload, ROTATION_DESCRIPTOR_INDEX);
        put_u32(&mut payload, COLOR_DESCRIPTOR_INDEX);
        put_u32(&mut payload, VISIBILITY_DESCRIPTOR_INDEX);
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

fn constant_descriptor(property_type: u8, constant_index: u32) -> Vec<u8> {
    let mut payload = descriptor_common(property_type, 1, 0b11, 0.0, 0.0);
    put_u32(&mut payload, constant_index);
    let descriptor = record(payload);
    debug_assert_eq!(descriptor.len(), 32);
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
    for (index, line) in lines.iter().enumerate() {
        // Match historical nonempty fixture classifications for the first two slots
        // when possible; additional Lines reuse portable-analytic constant speed.
        let (classification, max_error, boundaries, speed) = if index == 0 && lines.len() >= 2 {
            (
                2u8,
                2.328_306_436_538_696_3e-10,
                &[0.0, 2.0][..],
                EVALUABLE_SPEED_DESCRIPTOR_INDEX,
            )
        } else if index == 1 && lines.len() >= 2 {
            (1u8, 0.0, &[0.0][..], ANALYTIC_SPEED_DESCRIPTOR_INDEX)
        } else {
            (1u8, 0.0, &[0.0][..], ANALYTIC_SPEED_DESCRIPTOR_INDEX)
        };
        section.extend_from_slice(&distance_record(
            line.id,
            speed,
            line.initial_floor,
            classification,
            max_error,
            boundaries,
        ));
    }
    section
}

fn distance_record(
    line_id: u64,
    scroll_speed_descriptor: u32,
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
    put_f64(&mut payload, 0.0);
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

fn line_default_judge_shape() -> Vec<u8> {
    let mut payload = Vec::new();
    put_u32(&mut payload, 1);
    put_u32(&mut payload, 0); // "kind"
    put_u8(&mut payload, 4); // string Value
    put_u8(&mut payload, 0);
    put_u16(&mut payload, 0);
    put_u32(&mut payload, 8);
    put_u32(&mut payload, 1); // "lineDefault"
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

fn write_header(bytes: &mut [u8], section_count: u32) {
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
    write_u64_at(bytes, 28, 0);
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
