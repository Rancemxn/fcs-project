use fcs_model::{
    CanonicalChart, CanonicalCompilation, CanonicalContributor, CanonicalCredit,
    CanonicalCreditRole, CanonicalDescriptorKind, CanonicalDescriptorTable, CanonicalExpressionDag,
    CanonicalExpressionOpcode, CanonicalExpressionType, CanonicalExpressionValue,
    CanonicalGradientSpread, CanonicalJudgeShape, CanonicalNoteKind, CanonicalNoteScorePolicy,
    CanonicalNoteSide, CanonicalNoteSoundPolicy, CanonicalObject, CanonicalPathCommand,
    CanonicalProfile, CanonicalProfileFeature, CanonicalRenderGeometryData,
    CanonicalRenderPaintData, CanonicalRenderScene, CanonicalRequiredExtension,
    CanonicalResourceKind, CanonicalTrack, CanonicalTrackBlend, CanonicalTrackFill,
    CanonicalTrackInterpolation, CanonicalTrackPiece, CanonicalTrackSegment, CanonicalTrackTarget,
    CanonicalTrackValue, CanonicalValue, CanonicalValueType, DistributionMetadata,
};
use fcs_runtime::EasingId;
use sha2::{Digest, Sha256};

use crate::container::ContainerProfile;
use crate::error::{FcbcError, FcbcResult};

/// The section 12 NoteRecord `kind` ordinals.
///
/// The loader only range-checks this byte and never maps it back to a kind, so
/// a transposition here survives any round trip and has to be pinned directly.
const fn note_kind_ordinal(kind: CanonicalNoteKind) -> u8 {
    match kind {
        CanonicalNoteKind::Tap => 1,
        CanonicalNoteKind::Hold => 2,
        CanonicalNoteKind::Flick => 3,
        CanonicalNoteKind::Drag => 4,
    }
}

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
const TY_BEAT: u8 = 5;
const TY_LENGTH: u8 = 6;
const TY_ANGLE: u8 = 7;
const TY_COLOR: u8 = 8;
const TY_VEC2_FLOAT: u8 = 9;
const TY_VEC2_LENGTH: u8 = 10;
const TY_VEC2_INT: u8 = 11;
const TY_VEC2_TIME: u8 = 12;
const TY_VEC2_BEAT: u8 = 13;
const TY_VEC2_ANGLE: u8 = 14;

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
    scroll_tempo: Vec<ScrollTempoPointFixture>,
    evaluable_speed: bool,
    floor_scale: f64,
    integration_origin: f64,
    initial_floor: f64,
}

#[derive(Clone, Copy)]
struct ScrollTempoPointFixture {
    time: f64,
    bpm: f64,
}

#[derive(Clone)]
enum JudgeShapeFixture {
    LineDefault,
    Rectangle {
        center: [f64; 2],
        half_extents: [f64; 2],
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
}

#[derive(Clone)]
struct ExtensionFixture<'a> {
    namespace: String,
    version: (u16, u16, u16),
    payload: &'a CanonicalObject,
}

#[derive(Clone, Copy)]
struct ContributorFixture<'a> {
    id: u64,
    contributor: &'a CanonicalContributor,
}

#[derive(Clone)]
struct CreditFixture<'a> {
    credit: &'a CanonicalCredit,
    contributor_ids: Vec<u64>,
}

#[derive(Clone, Copy)]
struct SyncFixture {
    primary_audio_id: u64,
    audio_offset: f64,
    preview: Option<(f64, f64)>,
}

#[derive(Clone)]
struct NativeTrackFixture {
    line_id: u64,
    target: CanonicalTrackTarget,
    first_time: f64,
    before_constant: Constant,
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

#[derive(Default)]
struct NativeExpressionPool {
    nodes: Vec<Vec<u8>>,
}

impl NativeExpressionPool {
    fn emit(
        &mut self,
        expression: &CanonicalExpressionDag,
        constants: &[Constant],
    ) -> FcbcResult<u32> {
        fn emit_node(
            pool: &mut NativeExpressionPool,
            expression: &CanonicalExpressionDag,
            index: usize,
            constants: &[Constant],
            mapped: &mut [Option<u32>],
        ) -> FcbcResult<u32> {
            if let Some(index) = mapped[index] {
                return Ok(index);
            }
            let node = &expression.nodes()[index];
            let mut operands = [NULL_INDEX; 3];
            let mut arity = 0;
            for (slot, operand) in node.operands().iter().enumerate() {
                if let Some(operand) = operand {
                    operands[slot] = emit_node(pool, expression, *operand, constants, mapped)?;
                    arity += 1;
                }
            }
            let immediate = match node.constant() {
                Some(value) => find_constant(constants, &canonical_expression_constant(value)?),
                None => node.immediate(),
            };
            let mut encoded = Vec::with_capacity(20);
            expression_node(
                &mut encoded,
                canonical_expression_opcode(node.opcode()),
                canonical_expression_type(node.result_type()),
                &operands[..arity],
                immediate,
            );
            let global = if let Some(index) = pool.nodes.iter().position(|item| item == &encoded) {
                index as u32
            } else {
                let index = pool.nodes.len() as u32;
                pool.nodes.push(encoded);
                index
            };
            mapped[index] = Some(global);
            Ok(global)
        }

        let mut mapped = vec![None; expression.nodes().len()];
        emit_node(self, expression, expression.root(), constants, &mut mapped)
    }

    fn section(&self) -> Vec<u8> {
        let mut section = Vec::with_capacity(4 + self.nodes.len() * 20);
        put_u32(&mut section, self.nodes.len() as u32);
        for node in &self.nodes {
            section.extend_from_slice(node);
        }
        section
    }
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
            scroll_tempo: vec![ScrollTempoPointFixture {
                time: 0.0,
                bpm: 60.0,
            }],
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
            scroll_tempo: vec![ScrollTempoPointFixture {
                time: 0.0,
                bpm: 60.0,
            }],
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
            property_constants: default_note_property_constants(),
            visible_from: None,
            visible_until: None,
            judge_shape: JudgeShapeFixture::LineDefault,
            sound_policy: 1,
            score_policy: 1,
            sound_resource_id: 0,
            score_extension: None,
            texture_resource_id: 0,
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
            property_constants: default_note_property_constants(),
            visible_from: None,
            visible_until: None,
            judge_shape: JudgeShapeFixture::LineDefault,
            sound_policy: 1,
            score_policy: 1,
            sound_resource_id: 0,
            score_extension: None,
            texture_resource_id: 0,
        },
    ];
    assemble_package(
        &lines,
        &notes,
        &[(0, 1, 0.0, 60.0, 0)],
        0.0,
        &[],
        &[],
        &[],
        ExecutionGraph::Fixture,
        None,
        None,
        ContainerProfile::StrictRuntime,
        None,
        None,
        &[],
        &[],
        None,
    )
    .expect("the fixed execution fixture is valid")
}

/// Product CanonicalCompilation → FCBC runtime package writer.
///
/// Encodes chart Lines/Notes/tempo into Core sections and attaches only
/// descriptors owned by those records. Track/expression lowering is added by
/// the following native handoff slices.
pub fn write_from_compilation(compilation: &CanonicalCompilation) -> FcbcResult<Vec<u8>> {
    write_from_compilation_with_profile(compilation, ContainerProfile::StrictRuntime)
}

/// Writes a canonical compilation using the requested FCBC container profile.
pub fn write_from_compilation_with_profile(
    compilation: &CanonicalCompilation,
    profile: ContainerProfile,
) -> FcbcResult<Vec<u8>> {
    let chart = compilation.chart();
    let mut lines: Vec<LineFixture> = chart
        .lines()
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let base = line.base();
            let inherit = line.inherit();
            let scroll = chart.scroll().line(line.id().value()).ok_or_else(|| {
                FcbcError::new(
                    "fcbc.invalid-scroll",
                    format!("Line {} has no canonical scroll tempo", line.id().value()),
                )
            })?;
            Ok(LineFixture {
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
                scroll_tempo: scroll
                    .coordinate()
                    .points()
                    .iter()
                    .map(|point| ScrollTempoPointFixture {
                        time: point.chart_time(),
                        bpm: point.bpm(),
                    })
                    .collect(),
                evaluable_speed: false,
                floor_scale: base.floor_scale(),
                integration_origin: base.integration_origin(),
                initial_floor: base.initial_floor_position(),
            })
        })
        .collect::<FcbcResult<Vec<_>>>()?;
    lines.sort_by_key(|line| line.id);
    for (index, line) in lines.iter_mut().enumerate() {
        line.distance_index = index as u32;
    }
    let line_ids: std::collections::BTreeSet<u64> = lines.iter().map(|line| line.id).collect();
    let tracks = native_tracks(chart.tracks().tracks(), &lines, &line_ids)?;

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
            let kind = note_kind_ordinal(note.kind());
            let (has_end, end_time) = match note.kind() {
                CanonicalNoteKind::Hold => {
                    let end = note
                        .gameplay()
                        .end_time()
                        .map(|time| time.chart_time_seconds())
                        .unwrap_or(note.gameplay().time().chart_time_seconds() + 0.5);
                    (0b100u16, end)
                }
                _ => (0u16, 0.0),
            };
            let side = match note.gameplay().side() {
                CanonicalNoteSide::Above => 1u8,
                CanonicalNoteSide::Below => 2u8,
            };
            let judge_shape = match note.gameplay().judge_shape() {
                CanonicalJudgeShape::LineDefault => JudgeShapeFixture::LineDefault,
                CanonicalJudgeShape::Rectangle {
                    center,
                    half_extents,
                } => JudgeShapeFixture::Rectangle {
                    center: [center.x(), center.y()],
                    half_extents: [half_extents.x(), half_extents.y()],
                },
                CanonicalJudgeShape::Circle { center, radius } => JudgeShapeFixture::Circle {
                    center: [center.x(), center.y()],
                    radius: *radius,
                },
            };
            let (sound_policy, sound_resource_id) = match note.gameplay().sound_policy() {
                CanonicalNoteSoundPolicy::Default => (1, 0),
                CanonicalNoteSoundPolicy::None => (2, 0),
                CanonicalNoteSoundPolicy::Resource(resource_id) => {
                    let resource = compilation.resources().get(resource_id).ok_or_else(|| {
                        FcbcError::new(
                            "fcbc.dangling-reference",
                            format!(
                                "Note {} references missing sound resource {resource_id}",
                                note.id().value()
                            ),
                        )
                    })?;
                    if resource.resource().kind() != CanonicalResourceKind::Audio {
                        return Err(FcbcError::new(
                            "fcbc.invalid-note",
                            format!("Note {} sound resource is not audio", note.id().value()),
                        ));
                    }
                    (3, stable_id(b"fcs.resource", resource_id.as_bytes()))
                }
            };
            let (score_policy, score_extension) = match note.gameplay().score_policy() {
                CanonicalNoteScorePolicy::Default => (1, None),
                CanonicalNoteScorePolicy::None => (2, None),
                CanonicalNoteScorePolicy::Custom(namespace) => {
                    if !chart
                        .required_extensions()
                        .iter()
                        .any(|extension| extension.namespace() == namespace)
                    {
                        return Err(FcbcError::new(
                            "fcbc.invalid-note",
                            format!(
                                "Note {} custom score extension {namespace} is not required",
                                note.id().value()
                            ),
                        ));
                    }
                    (3, Some(namespace.clone()))
                }
            };
            let presentation = note.presentation();
            let texture_resource_id = presentation
                .texture()
                .map(|resource_id| {
                    let resource = compilation.resources().get(resource_id).ok_or_else(|| {
                        FcbcError::new(
                            "fcbc.dangling-reference",
                            format!(
                                "Note {} references missing texture {resource_id}",
                                note.id().value()
                            ),
                        )
                    })?;
                    if !matches!(
                        resource.resource().kind(),
                        CanonicalResourceKind::Image | CanonicalResourceKind::Texture
                    ) {
                        return Err(FcbcError::new(
                            "fcbc.invalid-note",
                            format!(
                                "Note {} texture is not an image resource",
                                note.id().value()
                            ),
                        ));
                    }
                    Ok(stable_id(b"fcs.resource", resource_id.as_bytes()))
                })
                .transpose()?
                .unwrap_or(0);
            Ok(NoteFixture {
                id: note.id().value(),
                line_id,
                document_order: note.document_order() as u32,
                kind,
                side,
                flags: u16::from(note.gameplay().judgment_enabled())
                    | (u16::from(presentation.render_enabled()) << 1)
                    | has_end,
                time: note.gameplay().time().chart_time_seconds(),
                end_time,
                property_descriptors: [0; 10],
                property_constants: [
                    scalar_constant(7, presentation.position_x()),
                    float_constant(presentation.scroll_factor()),
                    scalar_constant(7, presentation.x_offset()),
                    scalar_constant(7, presentation.y_offset()),
                    float_constant(presentation.alpha()),
                    float_constant(presentation.scale_x()),
                    float_constant(presentation.scale_y()),
                    scalar_constant(8, presentation.rotation()),
                    color_constant([
                        presentation.color().red(),
                        presentation.color().green(),
                        presentation.color().blue(),
                        presentation.color().alpha(),
                    ]),
                ],
                visible_from: presentation
                    .visible_from()
                    .map(|time| time.chart_time_seconds()),
                visible_until: presentation
                    .visible_until()
                    .map(|time| time.chart_time_seconds()),
                judge_shape,
                sound_policy,
                score_policy,
                sound_resource_id,
                score_extension,
                texture_resource_id,
            })
        })
        .collect::<FcbcResult<Vec<_>>>()?;
    notes.sort_by(|left, right| {
        left.time
            .total_cmp(&right.time)
            .then_with(|| left.line_id.cmp(&right.line_id))
            .then_with(|| left.document_order.cmp(&right.document_order))
            .then_with(|| left.id.cmp(&right.id))
    });

    let tempo: Vec<(i64, i64, f64, f64, u32)> = chart
        .time_map()
        .segments()
        .enumerate()
        .map(|(order, (beat, chart_time, bpm))| {
            // Section 10 carries the exact rational beat. Flooring to an
            // integer would collapse every sub-beat tempo point onto the same
            // beat, and the canonical Beat is already reduced.
            (
                beat.numerator(),
                beat.denominator(),
                chart_time,
                bpm,
                order as u32,
            )
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
    let contributors = native_contributors(chart)?;
    let credits = native_credits(chart, &contributors)?;
    let sync = native_sync(chart, &resources)?;
    let extensions = native_extensions(chart.required_extensions())?;

    assemble_package(
        &lines,
        &notes,
        &tempo,
        audio_offset,
        &resources,
        &tracks,
        &extensions,
        ExecutionGraph::Native {
            has_notes: !notes.is_empty(),
        },
        chart.descriptors(),
        chart.render(),
        profile,
        (profile == ContainerProfile::Fidelity).then_some(compilation.distribution()),
        Some(chart),
        &contributors,
        &credits,
        sync,
    )
}

#[cfg(test)]
#[path = "writer_compilation_tests.rs"]
mod compilation_tests;

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
    property_constants: [Constant; 9],
    visible_from: Option<f64>,
    visible_until: Option<f64>,
    judge_shape: JudgeShapeFixture,
    sound_policy: u16,
    score_policy: u16,
    sound_resource_id: u64,
    score_extension: Option<String>,
    texture_resource_id: u64,
}

#[derive(Clone, Copy)]
struct ResourceFixture<'a> {
    id: u64,
    kind: u16,
    media_type: &'a str,
    metadata: &'a CanonicalObject,
    content_sha256: [u8; 32],
    bytes: &'a [u8],
}

#[allow(clippy::too_many_arguments)]
fn assemble_package(
    lines: &[LineFixture],
    notes: &[NoteFixture],
    tempo: &[(i64, i64, f64, f64, u32)],
    audio_offset: f64,
    resources: &[ResourceFixture<'_>],
    tracks: &[NativeTrackFixture],
    extensions: &[ExtensionFixture<'_>],
    execution_graph: ExecutionGraph,
    runtime_descriptors: Option<&CanonicalDescriptorTable>,
    render: Option<&CanonicalRenderScene>,
    profile: ContainerProfile,
    fidelity: Option<&DistributionMetadata>,
    chart: Option<&CanonicalChart>,
    contributors: &[ContributorFixture<'_>],
    credits: &[CreditFixture<'_>],
    sync: Option<SyncFixture>,
) -> FcbcResult<Vec<u8>> {
    let mut lines = lines.to_vec();
    let mut notes = notes.to_vec();
    let needs_visibility_constants = matches!(execution_graph, ExecutionGraph::Native { .. })
        && notes.iter().any(|note| {
            runtime_descriptors.is_none_or(|table| {
                !table.roots().iter().any(|root| {
                    root.target_path() == note_property_path(9) && root.owner() == note.id
                })
            })
        });
    let needs_default_scroll_speed = matches!(execution_graph, ExecutionGraph::Native { .. })
        && lines.iter().any(|line| {
            !tracks.iter().any(|track| {
                track.line_id == line.id && track.target == CanonicalTrackTarget::ScrollSpeed
            })
        });
    let mut constants = match execution_graph {
        ExecutionGraph::Fixture => fixture_constants(),
        ExecutionGraph::Native { .. } => {
            let mut constants = Vec::new();
            if needs_visibility_constants {
                constants.extend([bool_constant(false), bool_constant(true)]);
            }
            if needs_default_scroll_speed {
                constants.push(float_constant(1.0));
            }
            constants
        }
    };
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
            constants.extend(
                line.scroll_tempo
                    .iter()
                    .map(|point| float_constant(point.bpm)),
            );
        }
        for note in &notes {
            constants.extend(note.property_constants.iter().cloned());
        }
        for track in tracks {
            constants.push(track.before_constant.clone());
            for segment in &track.segments {
                constants.extend([segment.start_constant.clone(), segment.end_constant.clone()]);
            }
        }
        if let Some(runtime_descriptors) = runtime_descriptors {
            for descriptor in runtime_descriptors.descriptors() {
                match descriptor.kind() {
                    CanonicalDescriptorKind::Constant(value) => {
                        constants.push(canonical_expression_constant(value)?);
                    }
                    CanonicalDescriptorKind::Expression(expression) => {
                        for node in expression.nodes() {
                            if let Some(value) = node.constant() {
                                constants.push(canonical_expression_constant(value)?);
                            }
                        }
                    }
                    CanonicalDescriptorKind::Piecewise(_) => {}
                }
            }
        }
    }
    constants.sort_by(|left, right| {
        (left.tag, left.payload.as_slice()).cmp(&(right.tag, right.payload.as_slice()))
    });
    constants.dedup();
    let (track_section, expressions, descriptor_indices) = match execution_graph {
        ExecutionGraph::Fixture => {
            let indices = constant_indices(&constants);
            (
                tracks_section(&indices),
                expression_section(&indices),
                Vec::new(),
            )
        }
        ExecutionGraph::Native { has_notes } => native_tracks_section(
            &constants,
            needs_visibility_constants.then(|| {
                (
                    find_constant(&constants, &bool_constant(false)),
                    find_constant(&constants, &bool_constant(true)),
                )
            }),
            &mut lines,
            &mut notes,
            tracks,
            has_notes,
            runtime_descriptors,
        )?,
    };
    let distances = distance_section_for_lines(&lines, tracks);
    let mut feature_flags = if lines.iter().any(|line| line.line_flags & 1 != 0) {
        1 << 8
    } else {
        0
    };
    if !extensions.is_empty() {
        feature_flags |= 1 << 2;
    }
    if profile == ContainerProfile::Fidelity && fidelity.is_none() {
        return Err(FcbcError::new(
            "fcbc.profile-requirement-missing",
            "fidelity profile requires a structured Fidelity section",
        ));
    }
    let contributor_ids = contributors
        .iter()
        .map(|contributor| contributor.id)
        .collect();
    let mut strings = string_table_values(
        resources,
        &notes,
        extensions,
        fidelity,
        chart,
        contributors,
        credits,
    );
    if render.is_some() {
        strings.extend([
            "centerDescriptor",
            "destinationDescriptors",
            "originDescriptor",
            "pathRef",
            "pointDescriptors",
            "radiusDescriptor",
            "radiusXDescriptor",
            "radiusYDescriptor",
            "radiiDescriptors",
            "resourceId",
            "rotationDescriptor",
            "sampling",
            "sizeDescriptor",
            "sourceDescriptors",
            "startDescriptor",
            "endDescriptor",
            "glyphRunRefs",
        ]);
        strings.sort_unstable_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
        strings.dedup();
    }
    let (resource_records, resource_data) = resource_sections(resources, &strings)?;
    let fidelity_section = fidelity
        .map(|metadata| fidelity_section(metadata, &strings))
        .transpose()?;
    if fidelity_section.is_some() {
        feature_flags |= crate::FeatureFlags::HAS_FIDELITY;
    }
    let render_section = render
        .map(|scene| render_section(scene, &descriptor_indices, &strings, resources))
        .transpose()?;
    if render_section.is_some() {
        feature_flags |= crate::FeatureFlags::HAS_RENDER;
    }

    let mut sections = vec![
        Section::new(1, string_table_section(&strings)),
        Section::new(2, constant_pool_section(&constants)),
        Section::new(3, meta_section(chart, &strings, &contributor_ids)?),
        Section::new(4, contributors_section(contributors, &strings)?),
        Section::new(5, credits_section(credits, &strings)?),
        Section::new(6, resource_records),
        Section::new(
            7,
            sync_section(sync.unwrap_or(SyncFixture {
                primary_audio_id: 0,
                audio_offset,
                preview: None,
            })),
        ),
        Section::new(8, tempo_section_from(tempo)),
        Section::new(9, lines_section(&lines, &constants)),
        Section::new(10, notes_section_from(&notes, &strings)),
        Section::new(11, track_section),
        Section::new(12, expressions),
        Section::new(13, distances),
    ];
    if let Some(render_section) = render_section {
        sections.push(Section::new(14, render_section));
    }
    if !extensions.is_empty() {
        sections.push(Section::new(
            15,
            extensions_section(extensions, &strings, &contributor_ids)?,
        ));
    }
    if let Some(fidelity_section) = fidelity_section {
        sections.push(Section::new(16, fidelity_section));
    }
    sections.push(Section::new(20, resource_data));
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

    let source_version = chart
        .map(|chart| parse_source_version(chart.source_version().as_str()))
        .transpose()?
        .unwrap_or((5, 0, 0));
    write_header(
        &mut bytes,
        sections.len() as u32,
        feature_flags,
        profile,
        source_version,
    );
    write_section_table(&mut bytes, &sections);
    Ok(bytes)
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

fn append_render_subtree(scene: &CanonicalRenderScene, parent: usize, order: &mut Vec<usize>) {
    let mut children: Vec<usize> = scene
        .nodes()
        .iter()
        .enumerate()
        .filter_map(|(index, node)| (node.parent() == Some(parent)).then_some(index))
        .collect();
    children.sort_by_key(|index| {
        let node = &scene.nodes()[*index];
        (node.z_order(), node.document_order(), node.id().value())
    });
    for child in children {
        order.push(child);
        append_render_subtree(scene, child, order);
    }
}

fn render_section(
    scene: &CanonicalRenderScene,
    descriptor_indices: &[u32],
    strings: &[&str],
    resources: &[ResourceFixture<'_>],
) -> FcbcResult<Vec<u8>> {
    let descriptor = |index: usize| {
        descriptor_indices.get(index).copied().ok_or_else(|| {
            FcbcError::new(
                "fcbc.dangling-reference",
                format!("Render descriptor {index} is not encoded in Core"),
            )
        })
    };
    let layer_order = scene.layer_draw_order();
    let mut encoded_layer = vec![0u32; scene.layers().len()];
    for (index, layer) in layer_order.iter().enumerate() {
        encoded_layer[*layer] = index as u32;
    }
    let mut geometry_order: Vec<usize> = (0..scene.geometries().len()).collect();
    geometry_order.sort_by_key(|index| scene.geometries()[*index].id().value());
    let mut geometry_indices = vec![0u32; scene.geometries().len()];
    for (index, geometry) in geometry_order.iter().enumerate() {
        geometry_indices[*geometry] = index as u32;
    }
    let mut path_order: Vec<usize> = (0..scene.paths().len()).collect();
    path_order.sort_by_key(|index| scene.paths()[*index].id().value());
    let mut path_indices = vec![NULL_INDEX; scene.paths().len()];
    for (index, path) in path_order.iter().enumerate() {
        path_indices[*path] = index as u32;
    }
    let mut paint_order: Vec<usize> = (0..scene.paints().len()).collect();
    paint_order.sort_by_key(|index| scene.paints()[*index].id().value());
    let mut paint_indices = vec![0u32; scene.paints().len()];
    for (index, paint) in paint_order.iter().enumerate() {
        paint_indices[*paint] = index as u32;
    }
    let mut stroke_order: Vec<usize> = (0..scene.strokes().len()).collect();
    stroke_order.sort_by_key(|index| scene.strokes()[*index].id().value());
    let mut stroke_indices = vec![NULL_INDEX; scene.strokes().len()];
    for (index, stroke) in stroke_order.iter().enumerate() {
        stroke_indices[*stroke] = index as u32;
    }
    let mut clip_order: Vec<usize> = (0..scene.clips().len()).collect();
    clip_order.sort_by_key(|index| scene.clips()[*index].id().value());
    let mut clip_indices = vec![NULL_INDEX; scene.clips().len()];
    for (index, clip) in clip_order.iter().enumerate() {
        clip_indices[*clip] = index as u32;
    }
    let mut glyph_run_order: Vec<usize> = (0..scene.glyph_runs().len()).collect();
    glyph_run_order.sort_by_key(|index| scene.glyph_runs()[*index].id().value());
    let mut glyph_run_indices = vec![NULL_INDEX; scene.glyph_runs().len()];
    for (index, glyph_run) in glyph_run_order.iter().enumerate() {
        glyph_run_indices[*glyph_run] = index as u32;
    }

    let mut roots = Vec::<(usize, usize)>::new();
    let mut layer_root_ranges = vec![(NULL_INDEX, 0u32); layer_order.len()];
    for (encoded, source_layer) in layer_order.iter().enumerate() {
        let mut layer_roots = scene.layers()[*source_layer].roots().to_vec();
        layer_roots.sort_by(|left, right| {
            let left = &scene.nodes()[*left];
            let right = &scene.nodes()[*right];
            left.z_order()
                .cmp(&right.z_order())
                .then(left.document_order().cmp(&right.document_order()))
                .then(left.id().value().cmp(&right.id().value()))
        });
        let first = roots.len();
        roots.extend(layer_roots.into_iter().map(|node| (*source_layer, node)));
        layer_root_ranges[encoded] = if roots.len() == first {
            (NULL_INDEX, 0)
        } else {
            (first as u32, (roots.len() - first) as u32)
        };
    }

    // Roots are contiguous at the front of the table. Descendants then use a
    // preorder walk with the same sibling key the loader validates.
    let mut node_order: Vec<usize> = roots.iter().map(|(_, node)| *node).collect();
    for (_, root) in &roots {
        append_render_subtree(scene, *root, &mut node_order);
    }
    if node_order.len() != scene.nodes().len() {
        return Err(FcbcError::new(
            "fcbc.dangling-reference",
            "Render node order does not cover the canonical scene",
        ));
    }
    let mut encoded_nodes = vec![NULL_INDEX; scene.nodes().len()];
    for (encoded, source_node) in node_order.iter().enumerate() {
        encoded_nodes[*source_node] = encoded as u32;
    }

    let mut payload = Vec::new();
    put_u16(&mut payload, 1);
    put_u16(&mut payload, 0);
    put_u16(&mut payload, 0);
    put_u16(&mut payload, 0);
    put_f64(&mut payload, scene.viewport().width());
    put_f64(&mut payload, scene.viewport().height());
    put_u16(&mut payload, scene.viewport().color_space().ordinal());
    put_u16(&mut payload, 0);
    put_u32(&mut payload, scene.layers().len() as u32);
    put_u32(&mut payload, scene.nodes().len() as u32);
    put_u32(&mut payload, scene.geometries().len() as u32);
    put_u32(&mut payload, scene.paths().len() as u32);
    put_u32(&mut payload, scene.paints().len() as u32);
    put_u32(&mut payload, scene.strokes().len() as u32);
    put_u32(&mut payload, scene.clips().len() as u32);
    put_u32(&mut payload, scene.glyph_runs().len() as u32);
    for (encoded, source_layer) in layer_order.iter().enumerate() {
        let layer = &scene.layers()[*source_layer];
        let (first_root, root_count) = layer_root_ranges[encoded];
        let mut record_payload = Vec::new();
        put_u64(&mut record_payload, layer.id().value());
        put_u16(&mut record_payload, layer.pass().ordinal());
        put_u16(&mut record_payload, 0);
        put_i32(&mut record_payload, layer.z_order());
        put_u32(&mut record_payload, layer.document_order());
        put_u32(&mut record_payload, first_root);
        put_u32(&mut record_payload, root_count);
        payload.extend_from_slice(&record(record_payload));
    }
    for source_node in &node_order {
        let node = &scene.nodes()[*source_node];
        let encoded_layer_index = *encoded_layer.get(node.layer()).ok_or_else(|| {
            FcbcError::new(
                "fcbc.dangling-reference",
                "Render node references a missing layer",
            )
        })?;
        let parent = match node.parent() {
            Some(parent) => *encoded_nodes.get(parent).ok_or_else(|| {
                FcbcError::new(
                    "fcbc.dangling-reference",
                    "Render node references a missing parent",
                )
            })?,
            None => NULL_INDEX,
        };
        let geometry = node.geometry();
        let (paint, stroke) = match node.kind() {
            fcs_model::CanonicalRenderNodeKind::Rect
            | fcs_model::CanonicalRenderNodeKind::RoundedRect
            | fcs_model::CanonicalRenderNodeKind::Circle
            | fcs_model::CanonicalRenderNodeKind::Ellipse
            | fcs_model::CanonicalRenderNodeKind::Polyline
            | fcs_model::CanonicalRenderNodeKind::Polygon => {
                if node.stroke().is_some() {
                    return Err(FcbcError::new(
                        "fcbc.render-unsupported",
                        "product Render writer only supports strokes on Line nodes",
                    ));
                }
                (
                    Some(node.fill_paint().ok_or_else(|| {
                        FcbcError::new("fcbc.dangling-reference", "Render Rect has no fill paint")
                    })?),
                    None,
                )
            }
            fcs_model::CanonicalRenderNodeKind::Path => (
                Some(node.fill_paint().ok_or_else(|| {
                    FcbcError::new("fcbc.dangling-reference", "Render Path has no fill paint")
                })?),
                node.stroke(),
            ),
            fcs_model::CanonicalRenderNodeKind::Line => {
                if node.fill_paint().is_some() {
                    return Err(FcbcError::new(
                        "fcbc.render-unsupported",
                        "Render Line cannot carry a fill paint",
                    ));
                }
                (
                    None,
                    Some(node.stroke().ok_or_else(|| {
                        FcbcError::new("fcbc.dangling-reference", "Render Line has no stroke")
                    })?),
                )
            }
            fcs_model::CanonicalRenderNodeKind::Group => {
                if node.clip().is_some() {
                    return Err(FcbcError::new(
                        "fcbc.render-unsupported",
                        "Render Group cannot carry a clip",
                    ));
                }
                (None, None)
            }
            fcs_model::CanonicalRenderNodeKind::ClipGroup => (None, None),
            fcs_model::CanonicalRenderNodeKind::Image => {
                if node.fill_paint().is_some() || node.stroke().is_some() {
                    return Err(FcbcError::new(
                        "fcbc.render-unsupported",
                        "Render Image cannot carry fill or stroke",
                    ));
                }
                (None, None)
            }
            fcs_model::CanonicalRenderNodeKind::Text => {
                let fill = node.fill_paint();
                let stroke = node.stroke();
                if fill.is_none() && stroke.is_none() {
                    return Err(FcbcError::new(
                        "fcbc.dangling-reference",
                        "Render Text has no fill paint or stroke",
                    ));
                }
                (fill, stroke)
            }
        };
        let geometry = geometry
            .map(|geometry| {
                geometry_indices.get(geometry).copied().ok_or_else(|| {
                    FcbcError::new(
                        "fcbc.dangling-reference",
                        "Render node references a missing geometry",
                    )
                })
            })
            .transpose()?
            .unwrap_or(NULL_INDEX);
        let paint = paint
            .map(|paint| {
                paint_indices.get(paint).copied().ok_or_else(|| {
                    FcbcError::new(
                        "fcbc.dangling-reference",
                        "Render node references a missing paint",
                    )
                })
            })
            .transpose()?
            .unwrap_or(NULL_INDEX);
        let stroke = stroke
            .map(|stroke| {
                stroke_indices.get(stroke).copied().ok_or_else(|| {
                    FcbcError::new(
                        "fcbc.dangling-reference",
                        "Render node references a missing stroke",
                    )
                })
            })
            .transpose()?
            .unwrap_or(NULL_INDEX);
        let clip = node
            .clip()
            .map(|clip| {
                clip_indices.get(clip).copied().ok_or_else(|| {
                    FcbcError::new(
                        "fcbc.dangling-reference",
                        "Render node references a missing clip",
                    )
                })
            })
            .transpose()?
            .unwrap_or(NULL_INDEX);
        let attachment_id = node.attachment().target().map_or(0, |id| id.value());
        let active = node.active();
        let flags = u16::from(active.unbounded_before())
            | (u16::from(active.unbounded_after()) << 1)
            | (u16::from(node.isolate()) << 2)
            | (u16::from(node.follow_hidden_attachment()) << 3);
        let mut record_payload = Vec::new();
        put_u64(&mut record_payload, node.id().value());
        put_u16(&mut record_payload, node.kind().ordinal());
        put_u16(&mut record_payload, flags);
        put_u32(&mut record_payload, parent);
        put_u32(&mut record_payload, encoded_layer_index);
        put_u32(&mut record_payload, node.document_order());
        put_i32(&mut record_payload, node.z_order());
        put_u16(&mut record_payload, node.attachment().ordinal());
        put_u16(&mut record_payload, 0);
        put_u64(&mut record_payload, attachment_id);
        put_f64(&mut record_payload, node.active().start());
        put_f64(&mut record_payload, node.active().end());
        put_u32(&mut record_payload, descriptor(node.position())?);
        put_u32(&mut record_payload, descriptor(node.origin())?);
        put_u32(&mut record_payload, descriptor(node.rotation())?);
        put_u32(&mut record_payload, descriptor(node.scale())?);
        put_u32(&mut record_payload, descriptor(node.opacity())?);
        put_u32(&mut record_payload, descriptor(node.visibility())?);
        put_u32(&mut record_payload, geometry);
        put_u32(&mut record_payload, paint);
        put_u32(&mut record_payload, stroke);
        put_u32(&mut record_payload, clip);
        put_u16(&mut record_payload, node.composite().ordinal());
        put_u16(&mut record_payload, 0);
        record_payload.extend_from_slice(&empty_object());
        payload.extend_from_slice(&record(record_payload));
    }
    for geometry_index in &geometry_order {
        let geometry = &scene.geometries()[*geometry_index];
        let descriptor_array = |values: &[usize]| -> FcbcResult<Vec<u8>> {
            let values = values
                .iter()
                .map(|index| descriptor(*index).map(|value| value_int(i64::from(value))))
                .collect::<FcbcResult<Vec<_>>>()?;
            Ok(value_array(2, values))
        };
        let fields = match geometry.data() {
            CanonicalRenderGeometryData::Rect { origin, size } => value_object(
                &[
                    (
                        "originDescriptor",
                        value_int(i64::from(descriptor(*origin)?)),
                    ),
                    ("sizeDescriptor", value_int(i64::from(descriptor(*size)?))),
                ],
                strings,
            ),
            CanonicalRenderGeometryData::RoundedRect {
                origin,
                size,
                radii,
            } => value_object(
                &[
                    (
                        "originDescriptor",
                        value_int(i64::from(descriptor(*origin)?)),
                    ),
                    ("sizeDescriptor", value_int(i64::from(descriptor(*size)?))),
                    ("radiiDescriptors", descriptor_array(radii)?),
                ],
                strings,
            ),
            CanonicalRenderGeometryData::Circle { center, radius } => value_object(
                &[
                    (
                        "centerDescriptor",
                        value_int(i64::from(descriptor(*center)?)),
                    ),
                    (
                        "radiusDescriptor",
                        value_int(i64::from(descriptor(*radius)?)),
                    ),
                ],
                strings,
            ),
            CanonicalRenderGeometryData::Ellipse {
                center,
                radius_x,
                radius_y,
                rotation,
            } => value_object(
                &[
                    (
                        "centerDescriptor",
                        value_int(i64::from(descriptor(*center)?)),
                    ),
                    (
                        "radiusXDescriptor",
                        value_int(i64::from(descriptor(*radius_x)?)),
                    ),
                    (
                        "radiusYDescriptor",
                        value_int(i64::from(descriptor(*radius_y)?)),
                    ),
                    (
                        "rotationDescriptor",
                        value_int(i64::from(descriptor(*rotation)?)),
                    ),
                ],
                strings,
            ),
            CanonicalRenderGeometryData::Line { start, end } => value_object(
                &[
                    ("startDescriptor", value_int(i64::from(descriptor(*start)?))),
                    ("endDescriptor", value_int(i64::from(descriptor(*end)?))),
                ],
                strings,
            ),
            CanonicalRenderGeometryData::Polyline { points }
            | CanonicalRenderGeometryData::Polygon { points } => {
                value_object(&[("pointDescriptors", descriptor_array(points)?)], strings)
            }
            CanonicalRenderGeometryData::Path { path } => {
                let path_ref = path_indices.get(*path).copied().ok_or_else(|| {
                    FcbcError::new(
                        "fcbc.dangling-reference",
                        "Render geometry references a missing path",
                    )
                })?;
                if path_ref == NULL_INDEX {
                    return Err(FcbcError::new(
                        "fcbc.dangling-reference",
                        "Render geometry references a missing path",
                    ));
                }
                value_object(&[("pathRef", value_int(i64::from(path_ref)))], strings)
            }
            CanonicalRenderGeometryData::Image {
                resource,
                destination,
                source,
                sampling,
            } => {
                let resource_id = resource.value();
                let resource = resources
                    .iter()
                    .find(|candidate| candidate.id == resource_id)
                    .ok_or_else(|| {
                        FcbcError::new(
                            "fcbc.render-resource-not-found",
                            format!("Render Image references resource {resource_id}"),
                        )
                    })?;
                if !matches!(resource.kind, 2 | 4) {
                    return Err(FcbcError::new(
                        "fcbc.render-resource-type-mismatch",
                        "Render Image requires an image or texture resource",
                    ));
                }
                let mut fields = vec![
                    ("resourceId", value_resource(resource_id)),
                    ("destinationDescriptors", descriptor_array(destination)?),
                ];
                if let Some(source) = source {
                    fields.push(("sourceDescriptors", descriptor_array(source)?));
                }
                fields.push(("sampling", value_int(i64::from(sampling.ordinal()))));
                value_object(&fields, strings)
            }
            CanonicalRenderGeometryData::Text { glyph_runs, origin } => {
                let runs = glyph_runs
                    .iter()
                    .map(|glyph_run| {
                        glyph_run_indices
                            .get(*glyph_run)
                            .copied()
                            .filter(|index| *index != NULL_INDEX)
                            .map(|index| value_int(i64::from(index)))
                            .ok_or_else(|| {
                                FcbcError::new(
                                    "fcbc.dangling-reference",
                                    "Render Text geometry references a missing glyph run",
                                )
                            })
                    })
                    .collect::<FcbcResult<Vec<_>>>()?;
                value_object(
                    &[
                        ("glyphRunRefs", value_array(2, runs)),
                        (
                            "originDescriptor",
                            value_int(i64::from(descriptor(*origin)?)),
                        ),
                    ],
                    strings,
                )
            }
        };
        let mut record_payload = Vec::new();
        put_u64(&mut record_payload, geometry.id().value());
        put_u16(&mut record_payload, geometry.kind().ordinal());
        put_u16(&mut record_payload, 0);
        record_payload.extend_from_slice(&fields);
        payload.extend_from_slice(&record(record_payload));
    }
    for path_index in &path_order {
        let path = &scene.paths()[*path_index];
        let mut record_payload = Vec::new();
        put_u64(&mut record_payload, path.id().value());
        put_u16(&mut record_payload, 0);
        put_u16(&mut record_payload, path.fill_rule().ordinal());
        put_u32(&mut record_payload, path.commands().len() as u32);
        for command in path.commands() {
            let mut command_payload = Vec::new();
            match command {
                CanonicalPathCommand::MoveTo(point) => {
                    put_u16(&mut command_payload, 1);
                    put_u16(&mut command_payload, 0);
                    put_u32(&mut command_payload, descriptor(*point)?);
                }
                CanonicalPathCommand::LineTo(point) => {
                    put_u16(&mut command_payload, 2);
                    put_u16(&mut command_payload, 0);
                    put_u32(&mut command_payload, descriptor(*point)?);
                }
                CanonicalPathCommand::QuadraticTo(control, end) => {
                    put_u16(&mut command_payload, 3);
                    put_u16(&mut command_payload, 0);
                    put_u32(&mut command_payload, descriptor(*control)?);
                    put_u32(&mut command_payload, descriptor(*end)?);
                }
                CanonicalPathCommand::CubicTo(first, second, end) => {
                    put_u16(&mut command_payload, 4);
                    put_u16(&mut command_payload, 0);
                    put_u32(&mut command_payload, descriptor(*first)?);
                    put_u32(&mut command_payload, descriptor(*second)?);
                    put_u32(&mut command_payload, descriptor(*end)?);
                }
                CanonicalPathCommand::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    direction,
                } => {
                    put_u16(&mut command_payload, 5);
                    put_u16(&mut command_payload, 0);
                    put_u32(&mut command_payload, descriptor(*center)?);
                    put_u32(&mut command_payload, descriptor(*radius)?);
                    put_u32(&mut command_payload, descriptor(*start_angle)?);
                    put_u32(&mut command_payload, descriptor(*end_angle)?);
                    put_u16(&mut command_payload, direction.ordinal());
                    put_u16(&mut command_payload, 0);
                }
                CanonicalPathCommand::EllipseArc {
                    center,
                    radius_x,
                    radius_y,
                    rotation,
                    start_angle,
                    end_angle,
                    direction,
                } => {
                    put_u16(&mut command_payload, 6);
                    put_u16(&mut command_payload, 0);
                    put_u32(&mut command_payload, descriptor(*center)?);
                    put_u32(&mut command_payload, descriptor(*radius_x)?);
                    put_u32(&mut command_payload, descriptor(*radius_y)?);
                    put_u32(&mut command_payload, descriptor(*rotation)?);
                    put_u32(&mut command_payload, descriptor(*start_angle)?);
                    put_u32(&mut command_payload, descriptor(*end_angle)?);
                    put_u16(&mut command_payload, direction.ordinal());
                    put_u16(&mut command_payload, 0);
                }
                CanonicalPathCommand::Close => {
                    put_u16(&mut command_payload, 7);
                    put_u16(&mut command_payload, 0);
                }
            }
            record_payload.extend_from_slice(&record(command_payload));
        }
        payload.extend_from_slice(&record(record_payload));
    }
    for paint_index in &paint_order {
        let paint = &scene.paints()[*paint_index];
        let mut record_payload = Vec::new();
        put_u64(&mut record_payload, paint.id().value());
        match paint.data() {
            CanonicalRenderPaintData::Solid { color } => {
                put_u16(&mut record_payload, 1);
                put_u16(&mut record_payload, 0);
                put_u32(&mut record_payload, descriptor(*color)?);
            }
            CanonicalRenderPaintData::LinearGradient {
                start,
                end,
                spread,
                stops,
            } => {
                put_u16(&mut record_payload, 2);
                put_u16(&mut record_payload, 0);
                put_u32(&mut record_payload, descriptor(*start)?);
                put_u32(&mut record_payload, descriptor(*end)?);
                put_u16(
                    &mut record_payload,
                    match spread {
                        CanonicalGradientSpread::Pad => 1,
                        CanonicalGradientSpread::Repeat => 2,
                        CanonicalGradientSpread::Reflect => 3,
                    },
                );
                put_u16(&mut record_payload, 0);
                put_u32(&mut record_payload, stops.len() as u32);
                for stop in stops {
                    put_f64(&mut record_payload, stop.offset());
                    put_u32(&mut record_payload, descriptor(stop.color())?);
                    put_u32(&mut record_payload, 0);
                }
            }
            CanonicalRenderPaintData::RadialGradient {
                start_center,
                start_radius,
                end_center,
                end_radius,
                spread,
                stops,
            } => {
                put_u16(&mut record_payload, 3);
                put_u16(&mut record_payload, 0);
                put_u32(&mut record_payload, descriptor(*start_center)?);
                put_u32(&mut record_payload, descriptor(*start_radius)?);
                put_u32(&mut record_payload, descriptor(*end_center)?);
                put_u32(&mut record_payload, descriptor(*end_radius)?);
                put_u16(
                    &mut record_payload,
                    match spread {
                        CanonicalGradientSpread::Pad => 1,
                        CanonicalGradientSpread::Repeat => 2,
                        CanonicalGradientSpread::Reflect => 3,
                    },
                );
                put_u16(&mut record_payload, 0);
                put_u32(&mut record_payload, stops.len() as u32);
                for stop in stops {
                    put_f64(&mut record_payload, stop.offset());
                    put_u32(&mut record_payload, descriptor(stop.color())?);
                    put_u32(&mut record_payload, 0);
                }
            }
            CanonicalRenderPaintData::ImagePattern {
                resource,
                transform,
                repeat,
                sampling,
            } => {
                let resource_id = resource.value();
                let resource = resources
                    .iter()
                    .find(|candidate| candidate.id == resource_id)
                    .ok_or_else(|| {
                        FcbcError::new(
                            "fcbc.render-resource-not-found",
                            format!("Render ImagePattern references resource {resource_id}"),
                        )
                    })?;
                if !matches!(resource.kind, 2 | 4) {
                    return Err(FcbcError::new(
                        "fcbc.render-unsupported",
                        "Render ImagePattern requires an image or texture resource",
                    ));
                }
                put_u16(&mut record_payload, 4);
                put_u16(&mut record_payload, 0);
                put_u64(&mut record_payload, resource_id);
                put_u32(&mut record_payload, descriptor(transform.position)?);
                put_u32(&mut record_payload, descriptor(transform.origin)?);
                put_u32(&mut record_payload, descriptor(transform.rotation)?);
                put_u32(&mut record_payload, descriptor(transform.scale)?);
                put_u16(&mut record_payload, repeat.ordinal());
                put_u16(&mut record_payload, sampling.ordinal());
            }
        };
        payload.extend_from_slice(&record(record_payload));
    }
    for stroke_index in &stroke_order {
        let stroke = &scene.strokes()[*stroke_index];
        let mut record_payload = Vec::new();
        put_u64(&mut record_payload, stroke.id().value());
        put_u16(&mut record_payload, 0);
        put_u16(&mut record_payload, 0);
        put_u32(
            &mut record_payload,
            paint_indices.get(stroke.paint()).copied().ok_or_else(|| {
                FcbcError::new(
                    "fcbc.dangling-reference",
                    "Render stroke references a missing paint",
                )
            })?,
        );
        put_u32(&mut record_payload, descriptor(stroke.width())?);
        put_u16(&mut record_payload, stroke.cap().ordinal());
        put_u16(&mut record_payload, stroke.join().ordinal());
        put_f64(&mut record_payload, stroke.miter_limit());
        put_u32(&mut record_payload, descriptor(stroke.dash_offset())?);
        put_u32(&mut record_payload, stroke.dash().len() as u32);
        for dash in stroke.dash() {
            put_f64(&mut record_payload, *dash);
        }
        payload.extend_from_slice(&record(record_payload));
    }
    for clip_index in &clip_order {
        let clip = &scene.clips()[*clip_index];
        let geometry = geometry_indices
            .get(clip.geometry())
            .copied()
            .ok_or_else(|| {
                FcbcError::new(
                    "fcbc.dangling-reference",
                    "Render clip references a missing geometry",
                )
            })?;
        let mut record_payload = Vec::new();
        put_u64(&mut record_payload, clip.id().value());
        put_u16(&mut record_payload, 0);
        put_u16(&mut record_payload, clip.fill_rule().ordinal());
        put_u32(&mut record_payload, geometry);
        payload.extend_from_slice(&record(record_payload));
    }
    for glyph_run_index in &glyph_run_order {
        let glyph_run = &scene.glyph_runs()[*glyph_run_index];
        let resource_id = glyph_run.font().value();
        let resource = resources
            .iter()
            .find(|candidate| candidate.id == resource_id)
            .ok_or_else(|| {
                FcbcError::new(
                    "fcbc.render-resource-not-found",
                    format!("Render GlyphRun references resource {resource_id}"),
                )
            })?;
        if resource.kind != 3 || resource.media_type != "font/ttf" {
            return Err(FcbcError::new(
                "fcbc.render-resource-type-mismatch",
                "Render GlyphRun requires a font/ttf resource",
            ));
        }
        let mut record_payload = Vec::new();
        put_u64(&mut record_payload, glyph_run.id().value());
        put_u64(&mut record_payload, resource_id);
        put_u32(&mut record_payload, glyph_run.face_index());
        put_u16(&mut record_payload, 0);
        put_u16(&mut record_payload, 1);
        put_u32(&mut record_payload, descriptor(glyph_run.size())?);
        let [run_offset_x, run_offset_y] = glyph_run.run_offset();
        put_f64(&mut record_payload, run_offset_x);
        put_f64(&mut record_payload, run_offset_y);
        put_u32(&mut record_payload, glyph_run.glyphs().len() as u32);
        put_u32(&mut record_payload, 0);
        for glyph in glyph_run.glyphs() {
            put_u32(&mut record_payload, glyph.glyph_id);
            put_u32(&mut record_payload, 0);
            put_f64(&mut record_payload, glyph.x_advance);
            put_f64(&mut record_payload, glyph.y_advance);
            put_f64(&mut record_payload, glyph.x_offset);
            put_f64(&mut record_payload, glyph.y_offset);
        }
        payload.extend_from_slice(&record(record_payload));
    }
    Ok(record(payload))
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

fn canonical_expression_constant(value: &CanonicalExpressionValue) -> FcbcResult<Constant> {
    let constant = match value {
        CanonicalExpressionValue::Bool(value) => bool_constant(*value),
        CanonicalExpressionValue::Int(value) => int_constant(*value),
        CanonicalExpressionValue::Float(value) => float_constant(*value),
        CanonicalExpressionValue::Time(value) => scalar_constant(5, *value),
        CanonicalExpressionValue::Beat(value) => beat_constant(*value)?,
        CanonicalExpressionValue::ExactBeat(value) => exact_beat_constant(*value),
        CanonicalExpressionValue::Length(value) => scalar_constant(7, *value),
        CanonicalExpressionValue::Angle(value) => scalar_constant(8, *value),
        CanonicalExpressionValue::Color(value) => color_constant(*value),
        CanonicalExpressionValue::Vec2(x, y) => canonical_vec2_constant(x, y)?,
    };
    Ok(constant)
}

fn canonical_vec2_constant(
    x: &CanonicalExpressionValue,
    y: &CanonicalExpressionValue,
) -> FcbcResult<Constant> {
    let (tag, mut payload) = match (x, y) {
        (CanonicalExpressionValue::Int(x), CanonicalExpressionValue::Int(y)) => {
            let mut payload = Vec::with_capacity(24);
            put_i64(&mut payload, *x);
            put_i64(&mut payload, *y);
            (2, payload)
        }
        (CanonicalExpressionValue::Float(x), CanonicalExpressionValue::Float(y)) => {
            (3, scalar_pair(*x, *y))
        }
        (CanonicalExpressionValue::Time(x), CanonicalExpressionValue::Time(y)) => {
            (5, scalar_pair(*x, *y))
        }
        (x, y)
            if x.value_type() == CanonicalExpressionType::Beat
                && y.value_type() == CanonicalExpressionType::Beat =>
        {
            let mut payload = Vec::with_capacity(40);
            put_beat_value(&mut payload, x)?;
            put_beat_value(&mut payload, y)?;
            (6, payload)
        }
        (CanonicalExpressionValue::Length(x), CanonicalExpressionValue::Length(y)) => {
            (7, scalar_pair(*x, *y))
        }
        (CanonicalExpressionValue::Angle(x), CanonicalExpressionValue::Angle(y)) => {
            (8, scalar_pair(*x, *y))
        }
        _ => {
            return Err(FcbcError::new(
                "fcbc.invalid-expression",
                "canonical vec2 constant elements must have the same numeric type",
            ));
        }
    };
    let mut encoded = vec![tag];
    encoded.resize(8, 0);
    encoded.append(&mut payload);
    Ok(Constant {
        tag: 10,
        payload: encoded,
    })
}

fn scalar_pair(x: f64, y: f64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(16);
    put_f64(&mut payload, x);
    put_f64(&mut payload, y);
    payload
}

fn beat_constant(value: f64) -> FcbcResult<Constant> {
    let mut payload = Vec::with_capacity(16);
    put_beat(&mut payload, value)?;
    Ok(Constant { tag: 6, payload })
}

fn exact_beat_constant(value: fcs_model::Beat) -> Constant {
    let mut payload = Vec::with_capacity(16);
    put_i64(&mut payload, value.numerator());
    put_i64(&mut payload, value.denominator());
    Constant { tag: 6, payload }
}

fn put_beat_value(output: &mut Vec<u8>, value: &CanonicalExpressionValue) -> FcbcResult<()> {
    match value {
        CanonicalExpressionValue::Beat(value) => put_beat(output, *value),
        CanonicalExpressionValue::ExactBeat(value) => {
            put_i64(output, value.numerator());
            put_i64(output, value.denominator());
            Ok(())
        }
        _ => Err(FcbcError::new(
            "fcbc.invalid-expression",
            "canonical vec2 Beat constant has a non-Beat element",
        )),
    }
}

fn put_beat(output: &mut Vec<u8>, value: f64) -> FcbcResult<()> {
    let (numerator, denominator) = exact_f64_ratio(value).ok_or_else(|| {
        FcbcError::new(
            "fcbc.invalid-expression",
            "canonical Beat constant is not representable as FCBC i64/i64",
        )
    })?;
    put_i64(output, numerator);
    put_i64(output, denominator);
    Ok(())
}

fn exact_f64_ratio(value: f64) -> Option<(i64, i64)> {
    if !value.is_finite() {
        return None;
    }
    if value == 0.0 {
        return Some((0, 1));
    }
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let mut mantissa = bits & ((1_u64 << 52) - 1);
    let mut exponent = if exponent_bits == 0 {
        -1074
    } else {
        mantissa |= 1_u64 << 52;
        exponent_bits - 1023 - 52
    };
    while exponent < 0 && mantissa & 1 == 0 {
        mantissa >>= 1;
        exponent += 1;
    }
    let (numerator, denominator) = if exponent >= 0 {
        (i128::from(mantissa).checked_shl(exponent as u32)?, 1_i128)
    } else {
        (
            i128::from(mantissa),
            1_i128.checked_shl((-exponent) as u32)?,
        )
    };
    let numerator = if negative { -numerator } else { numerator };
    Some((
        i64::try_from(numerator).ok()?,
        i64::try_from(denominator).ok()?,
    ))
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
                metadata: resource.metadata(),
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

fn native_contributors(chart: &CanonicalChart) -> FcbcResult<Vec<ContributorFixture<'_>>> {
    let mut contributors: Vec<_> = chart
        .metadata()
        .contributors()
        .values()
        .map(|contributor| ContributorFixture {
            id: stable_id(b"fcs.contributor", contributor.id().as_bytes()),
            contributor,
        })
        .collect();
    contributors.sort_by_key(|contributor| contributor.id);
    if contributors.iter().any(|contributor| contributor.id == 0)
        || contributors.windows(2).any(|pair| pair[0].id == pair[1].id)
    {
        return Err(FcbcError::new(
            "fcbc.duplicate-id",
            "canonical contributor IDs collide in FCBC stable-ID space",
        ));
    }
    Ok(contributors)
}

fn native_credits<'a>(
    chart: &'a CanonicalChart,
    contributors: &[ContributorFixture<'a>],
) -> FcbcResult<Vec<CreditFixture<'a>>> {
    let contributor_ids: std::collections::BTreeMap<_, _> = contributors
        .iter()
        .map(|contributor| (contributor.contributor.id(), contributor.id))
        .collect();
    let mut stable_ids = std::collections::BTreeSet::new();
    let mut credits = Vec::with_capacity(chart.metadata().credits().len());
    for credit in chart.metadata().credits() {
        let id = credit.id().value();
        if id == 0 || !stable_ids.insert(id) {
            return Err(FcbcError::new(
                "fcbc.duplicate-id",
                "canonical credit IDs collide in FCBC stable-ID space",
            ));
        }
        let contributor_ids = credit
            .contributors()
            .iter()
            .map(|contributor| {
                contributor_ids
                    .get(contributor.as_str())
                    .copied()
                    .ok_or_else(|| {
                        FcbcError::new(
                            "fcbc.dangling-reference",
                            "canonical credit references a missing contributor",
                        )
                    })
            })
            .collect::<FcbcResult<Vec<_>>>()?;
        credits.push(CreditFixture {
            credit,
            contributor_ids,
        });
    }
    Ok(credits)
}

fn native_sync(
    chart: &CanonicalChart,
    resources: &[ResourceFixture<'_>],
) -> FcbcResult<Option<SyncFixture>> {
    let Some(sync) = chart.metadata().sync() else {
        return Ok(None);
    };
    let primary_audio_id = sync
        .primary_audio()
        .map(|id| {
            let resource = resources
                .iter()
                .find(|resource| resource.id == stable_id(b"fcs.resource", id.as_bytes()))
                .ok_or_else(|| {
                    FcbcError::new(
                        "fcbc.dangling-reference",
                        "canonical sync primary audio is missing",
                    )
                })?;
            if resource.kind != 1 {
                return Err(FcbcError::new(
                    "fcbc.invalid-sync",
                    "canonical sync primary audio is not an audio resource",
                ));
            }
            Ok(resource.id)
        })
        .transpose()?;
    Ok(Some(SyncFixture {
        primary_audio_id: primary_audio_id.unwrap_or(0),
        audio_offset: sync.audio_offset().seconds(),
        preview: sync
            .preview()
            .map(|preview| (preview.start_seconds(), preview.end_seconds())),
    }))
}

fn native_extensions<'a>(
    extensions: &'a [CanonicalRequiredExtension],
) -> FcbcResult<Vec<ExtensionFixture<'a>>> {
    let mut lowered = Vec::with_capacity(extensions.len());
    for extension in extensions {
        let mut components = extension.version().split('.');
        let version: (Option<u16>, Option<u16>, Option<u16>) = (
            components.next().and_then(|value| value.parse().ok()),
            components.next().and_then(|value| value.parse().ok()),
            components.next().and_then(|value| value.parse().ok()),
        );
        if components.next().is_some()
            || version.0.is_none()
            || version.1.is_none()
            || version.2.is_none()
        {
            return Err(FcbcError::new(
                "fcbc.invalid-extension",
                format!(
                    "required extension {} has invalid version",
                    extension.namespace()
                ),
            ));
        }
        lowered.push(ExtensionFixture {
            namespace: extension.namespace().to_owned(),
            version: (version.0.unwrap(), version.1.unwrap(), version.2.unwrap()),
            payload: extension.payload(),
        });
    }
    lowered.sort_by(|left, right| {
        left.namespace
            .cmp(&right.namespace)
            .then_with(|| left.version.cmp(&right.version))
    });
    if lowered
        .windows(2)
        .any(|pair| pair[0].namespace == pair[1].namespace && pair[0].version == pair[1].version)
    {
        return Err(FcbcError::new(
            "fcbc.duplicate-extension",
            "canonical required extensions contain a duplicate namespace/version",
        ));
    }
    Ok(lowered)
}

fn native_tracks(
    tracks: &[CanonicalTrack],
    lines: &[LineFixture],
    line_ids: &std::collections::BTreeSet<u64>,
) -> FcbcResult<Vec<NativeTrackFixture>> {
    let mut grouped: std::collections::BTreeMap<(u64, CanonicalTrackTarget), Vec<&CanonicalTrack>> =
        std::collections::BTreeMap::new();
    for track in tracks {
        let line_id = track.owner().value();
        if !line_ids.contains(&line_id) {
            return Err(FcbcError::new(
                "fcbc.dangling-reference",
                format!("Track {} references missing Line {line_id}", track.name()),
            ));
        }
        if track.blend() != CanonicalTrackBlend::Replace {
            return Err(FcbcError::new(
                "fcbc.unsupported-track",
                format!(
                    "native {:?} Track {} requires replace blend",
                    track.target(),
                    track.name()
                ),
            ));
        }
        grouped
            .entry((line_id, track.target()))
            .or_default()
            .push(track);
    }

    let mut lowered = Vec::new();
    for ((line_id, target), group) in grouped {
        if group.len() == 1 {
            lowered.push(native_single_track_fixture(group[0], lines)?);
        } else {
            lowered.push(native_disjoint_replace_fixture(
                &group, lines, line_id, target,
            )?);
        }
    }
    lowered.sort_by_key(|track| (track.line_id, track.target));
    Ok(lowered)
}

fn native_single_track_fixture(
    track: &CanonicalTrack,
    lines: &[LineFixture],
) -> FcbcResult<NativeTrackFixture> {
    let line_id = track.owner().value();
    let target = track.target();
    let pieces = track.pieces();
    let effective = pieces
        .iter()
        .filter(|piece| !track_point_is_shadowed(piece, pieces))
        .collect::<Vec<_>>();
    let first_time = track_piece_time(effective[0]);
    let base = native_line_base_constant(
        lines
            .iter()
            .find(|line| line.id == line_id)
            .expect("validated Line owner"),
        target,
    );
    let mut segments = Vec::new();
    for piece in pieces {
        match piece {
            CanonicalTrackPiece::Segment(segment) => {
                segments.push(native_track_segment(segment, target, track.name())?);
            }
            CanonicalTrackPiece::Point(point) => {
                let value = native_track_constant(point.value(), target, track.name())?;
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
        let value = native_track_constant(segment.start_value(), target, track.name())?;
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
    for (index, pair) in effective.windows(2).enumerate() {
        let CanonicalTrackPiece::Segment(segment) = pair[0] else {
            continue;
        };
        let left_end = segment.end().chart_time_seconds();
        let right_start = track_piece_time(pair[1]);
        if left_end >= right_start {
            continue;
        }
        let value = native_track_fill_constant(
            track,
            track.fill(),
            &base,
            &effective,
            NativeFillInterval::Gap {
                right_index: index + 1,
            },
        )?;
        segments.push(native_track_point(left_end, value));
    }
    if let Some(CanonicalTrackPiece::Segment(segment)) = effective.last()
        && !pieces.iter().any(|piece| {
            matches!(
                piece,
                CanonicalTrackPiece::Point(point)
                    if point.time().chart_time_seconds().to_bits()
                        == segment.end().chart_time_seconds().to_bits()
            )
        })
    {
        let value = native_track_fill_constant(
            track,
            track.extrapolate_after(),
            &base,
            &effective,
            NativeFillInterval::After,
        )?;
        segments.push(native_track_point(
            segment.end().chart_time_seconds(),
            value,
        ));
    }
    segments.sort_by(|left, right| {
        left.start
            .total_cmp(&right.start)
            .then_with(|| right.flags.cmp(&left.flags))
    });
    Ok(NativeTrackFixture {
        line_id,
        target,
        first_time,
        before_constant: native_track_fill_constant(
            track,
            track.extrapolate_before(),
            &base,
            &effective,
            NativeFillInterval::Before,
        )?,
        segments,
    })
}

fn native_disjoint_replace_fixture(
    tracks: &[&CanonicalTrack],
    lines: &[LineFixture],
    line_id: u64,
    target: CanonicalTrackTarget,
) -> FcbcResult<NativeTrackFixture> {
    let priority = tracks[0].priority();
    if tracks.iter().any(|track| {
        track.priority() != priority
            || track.fill() != CanonicalTrackFill::Base
            || track.extrapolate_before() != CanonicalTrackFill::Base
            || track.extrapolate_after() != CanonicalTrackFill::Base
    }) {
        return Err(FcbcError::new(
            "fcbc.unsupported-track",
            format!(
                "native {:?} Track layering for Line {line_id} requires equal-priority base-filled replace Tracks",
                target
            ),
        ));
    }
    let fixtures = tracks
        .iter()
        .map(|track| native_single_track_fixture(track, lines))
        .collect::<FcbcResult<Vec<_>>>()?;
    let base = native_line_base_constant(
        lines
            .iter()
            .find(|line| line.id == line_id)
            .expect("validated Line owner"),
        target,
    );
    let first_time = fixtures
        .iter()
        .map(|fixture| fixture.first_time)
        .min_by(f64::total_cmp)
        .expect("nonempty Track group");
    let mut segments = fixtures
        .into_iter()
        .flat_map(|fixture| fixture.segments)
        .collect::<Vec<_>>();
    normalize_merged_track_segments(&mut segments, &base);
    Ok(NativeTrackFixture {
        line_id,
        target,
        first_time,
        before_constant: base,
        segments,
    })
}

fn normalize_merged_track_segments(segments: &mut Vec<TrackSegmentFixture>, base: &Constant) {
    segments.sort_by(|left, right| {
        left.start
            .total_cmp(&right.start)
            .then_with(|| right.flags.cmp(&left.flags))
    });
    let mut normalized: Vec<TrackSegmentFixture> = Vec::with_capacity(segments.len());
    for segment in segments.drain(..) {
        if let Some(previous) = normalized.last_mut()
            && previous.start.to_bits() == segment.start.to_bits()
        {
            if segment.flags == 0 || previous.start_constant == *base {
                *previous = segment;
            }
            continue;
        }
        normalized.push(segment);
    }
    if !matches!(normalized.first(), Some(segment) if segment.flags != 0) {
        let first = normalized
            .first()
            .expect("merged Track has at least one segment");
        normalized.insert(
            0,
            native_track_point(first.start, first.start_constant.clone()),
        );
    }
    *segments = normalized;
}

#[derive(Clone, Copy)]
enum NativeFillInterval {
    Before,
    Gap { right_index: usize },
    After,
}

fn native_track_fill_constant(
    track: &CanonicalTrack,
    fill: CanonicalTrackFill,
    base: &Constant,
    pieces: &[&CanonicalTrackPiece],
    interval: NativeFillInterval,
) -> FcbcResult<Constant> {
    match fill {
        CanonicalTrackFill::Base => return Ok(base.clone()),
        CanonicalTrackFill::Zero => return Ok(native_track_identity(track.target(), false)),
        CanonicalTrackFill::One => return Ok(native_track_identity(track.target(), true)),
        _ => {}
    }
    let value = match fill {
        CanonicalTrackFill::HoldBefore => {
            let start = match interval {
                NativeFillInterval::Before => 0,
                NativeFillInterval::Gap { right_index } => right_index,
                NativeFillInterval::After => pieces.len(),
            };
            pieces[start..].iter().find_map(|piece| match piece {
                CanonicalTrackPiece::Segment(segment) => Some(segment.start_value()),
                CanonicalTrackPiece::Point(_) => None,
            })
        }
        CanonicalTrackFill::HoldAfter => {
            let end = match interval {
                NativeFillInterval::Before => 0,
                NativeFillInterval::Gap { right_index } => right_index,
                NativeFillInterval::After => pieces.len(),
            };
            pieces[..end].iter().rev().find_map(|piece| match piece {
                CanonicalTrackPiece::Segment(segment) => Some(segment.end_value()),
                CanonicalTrackPiece::Point(_) => None,
            })
        }
        CanonicalTrackFill::Error => None,
        CanonicalTrackFill::Base | CanonicalTrackFill::Zero | CanonicalTrackFill::One => {
            unreachable!("constant fill policies return above")
        }
    };
    let value = value.ok_or_else(|| {
        FcbcError::new(
            "fcbc.unsupported-track",
            format!(
                "native {:?} Track {} has an unresolved {:?} fill interval",
                track.target(),
                track.name(),
                fill
            ),
        )
    })?;
    native_track_constant(value, track.target(), track.name())
}

fn native_track_identity(target: CanonicalTrackTarget, one: bool) -> Constant {
    let value = if one { 1.0 } else { 0.0 };
    match target {
        CanonicalTrackTarget::Position => vec2_constant(7, [value, value]),
        CanonicalTrackTarget::Rotation => scalar_constant(8, value),
        CanonicalTrackTarget::Scale => vec2_constant(3, [value, value]),
        CanonicalTrackTarget::Alpha | CanonicalTrackTarget::ScrollSpeed => float_constant(value),
    }
}

fn native_line_base_constant(line: &LineFixture, target: CanonicalTrackTarget) -> Constant {
    match target {
        CanonicalTrackTarget::Position => vec2_constant(7, line.position),
        CanonicalTrackTarget::Rotation => scalar_constant(8, line.rotation),
        CanonicalTrackTarget::Scale => vec2_constant(3, line.scale),
        CanonicalTrackTarget::Alpha => float_constant(line.alpha),
        CanonicalTrackTarget::ScrollSpeed => float_constant(1.0),
    }
}

fn native_track_point(time: f64, value: Constant) -> TrackSegmentFixture {
    TrackSegmentFixture {
        start: time,
        end: time,
        interpolation: 1,
        easing: 0,
        flags: 1,
        start_constant: value.clone(),
        end_constant: value,
        bezier: [0.0; 4],
    }
}

fn track_point_is_shadowed(piece: &CanonicalTrackPiece, pieces: &[CanonicalTrackPiece]) -> bool {
    let CanonicalTrackPiece::Point(point) = piece else {
        return false;
    };
    pieces.iter().any(|other| {
        matches!(
            other,
            CanonicalTrackPiece::Segment(segment)
                if segment.start().chart_time_seconds() == point.time().chart_time_seconds()
        )
    })
}

fn native_track_constant(
    value: CanonicalTrackValue,
    target: CanonicalTrackTarget,
    track_name: &str,
) -> FcbcResult<Constant> {
    let constant = match (target, value) {
        (CanonicalTrackTarget::Position, CanonicalTrackValue::Vec2Length(value)) => {
            vec2_constant(7, [value.x(), value.y()])
        }
        (CanonicalTrackTarget::Rotation, CanonicalTrackValue::Angle(value)) => {
            scalar_constant(8, value)
        }
        (CanonicalTrackTarget::Scale, CanonicalTrackValue::Vec2Float(value)) => {
            vec2_constant(3, [value.x(), value.y()])
        }
        (
            CanonicalTrackTarget::Alpha | CanonicalTrackTarget::ScrollSpeed,
            CanonicalTrackValue::Float(value),
        ) => float_constant(value),
        _ => {
            return Err(FcbcError::new(
                "fcbc.invalid-track",
                format!(
                    "native {:?} Track {track_name} has an incompatible value",
                    target
                ),
            ));
        }
    };
    Ok(constant)
}

fn native_track_segment(
    segment: &CanonicalTrackSegment,
    target: CanonicalTrackTarget,
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
                        format!(
                            "native {:?} Track {track_name} has unknown easing {name}",
                            target
                        ),
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
        start_constant: native_track_constant(segment.start_value(), target, track_name)?,
        end_constant: native_track_constant(segment.end_value(), target, track_name)?,
        bezier,
    })
}

fn track_piece_time(piece: &CanonicalTrackPiece) -> f64 {
    match piece {
        CanonicalTrackPiece::Segment(segment) => segment.start().chart_time_seconds(),
        CanonicalTrackPiece::Point(point) => point.time().chart_time_seconds(),
    }
}

fn string_table_values<'a>(
    resources: &[ResourceFixture<'a>],
    notes: &'a [NoteFixture],
    extensions: &'a [ExtensionFixture],
    fidelity: Option<&'a DistributionMetadata>,
    chart: Option<&'a CanonicalChart>,
    contributors: &[ContributorFixture<'a>],
    credits: &[CreditFixture<'a>],
) -> Vec<&'a str> {
    let mut strings = vec!["kind", "lineDefault"];
    for note in notes {
        match &note.judge_shape {
            JudgeShapeFixture::LineDefault => {}
            JudgeShapeFixture::Rectangle { .. } => {
                strings.extend(["rectangle", "center", "halfExtents"]);
            }
            JudgeShapeFixture::Circle { .. } => {
                strings.extend(["circle", "center", "radius"]);
            }
        }
    }
    for resource in resources {
        strings.push(resource.media_type);
        collect_canonical_object_strings(resource.metadata, &mut strings);
    }
    for extension in extensions {
        collect_canonical_object_strings(extension.payload, &mut strings);
    }
    for contributor in contributors {
        strings.push(contributor.contributor.name());
        strings.extend(contributor.contributor.aliases().iter().map(String::as_str));
        collect_canonical_object_strings(contributor.contributor.identifiers(), &mut strings);
    }
    for credit in credits {
        if credit.credit.label().is_none() {
            strings.push("");
        }
        if let Some(label) = credit.credit.label() {
            strings.push(label);
        }
        if let Some(role) = credit.credit.role_kind().custom() {
            strings.push(role);
        }
    }
    strings.extend(
        notes
            .iter()
            .filter_map(|note| note.score_extension.as_deref()),
    );
    strings.extend(
        extensions
            .iter()
            .map(|extension| extension.namespace.as_str()),
    );
    if let Some(chart) = chart {
        strings.extend([
            "title",
            "subtitle",
            "alternativeTitles",
            "chartVersion",
            "difficulty",
            "level",
            "description",
            "language",
            "tags",
            "license",
            "documentId",
            "revision",
            "custom",
            "primary",
        ]);
        if let Some(meta) = chart.metadata().meta() {
            for value in meta.values() {
                collect_canonical_value_strings(value, &mut strings);
            }
        }
    }
    if let Some(fidelity) = fidelity {
        strings.extend([
            "specificationVersion",
            "1.0.0",
            "sources",
            "profileBindings",
            "entityMappings",
            "fieldFacts",
            "mappingRules",
            "semanticLosses",
            "custom",
            "logicalSource",
            "hashAlgorithm",
            "digest",
            "id",
            "sourceArtifactId",
            "sourceLocator",
            "sourceOrder",
            "mappingRuleRef",
            "originState",
            "semanticStatus",
            "dependencies",
            "stale",
            "domain",
            "status",
            "category",
            "entityId",
            "sha256",
        ]);
        for hash in fidelity.input_hashes() {
            strings.push(hash.algorithm());
            strings.push(hash.digest_lower_hex());
            if let Some(locator) = hash.logical_source() {
                strings.push(locator.as_str());
            }
        }
        for fact in fidelity.provenance().facts().values() {
            strings.push(fact.id());
            if let Some(artifact) = fact.source_artifact_id() {
                strings.push(artifact);
            }
            if let Some(locator) = fact.source_locator() {
                strings.push(locator.as_str());
            }
            if let Some(rule) = fact.mapping_rule_ref() {
                strings.push(rule.as_str());
            }
            strings.push(fact.origin_state().as_str());
            if let Some(status) = fact.semantic_status() {
                strings.push(status.as_str());
            }
            strings.extend(fact.dependencies().iter().map(String::as_str));
        }
        for loss in fidelity.semantic_losses() {
            strings.extend([
                loss.domain().as_str(),
                loss.status().as_str(),
                loss.category(),
            ]);
            if let Some(entity_id) = loss.entity_id() {
                strings.push(entity_id);
            }
        }
        collect_canonical_object_strings(fidelity.custom(), &mut strings);
    }
    strings.sort_unstable_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    strings.dedup();
    strings
}

fn collect_canonical_object_strings<'a>(object: &'a CanonicalObject, strings: &mut Vec<&'a str>) {
    for entry in object.entries() {
        strings.push(entry.key());
        collect_canonical_value_strings(entry.value(), strings);
    }
}

fn collect_canonical_value_strings<'a>(value: &'a CanonicalValue, strings: &mut Vec<&'a str>) {
    match value {
        CanonicalValue::String(value)
        | CanonicalValue::ResourceReference(value)
        | CanonicalValue::ContributorReference(value) => strings.push(value),
        CanonicalValue::Array { values, .. } => {
            for value in values {
                collect_canonical_value_strings(value, strings);
            }
        }
        CanonicalValue::Object(object) => collect_canonical_object_strings(object, strings),
        CanonicalValue::Null
        | CanonicalValue::Bool(_)
        | CanonicalValue::Int(_)
        | CanonicalValue::Float(_)
        | CanonicalValue::Time(_)
        | CanonicalValue::Beat(_)
        | CanonicalValue::Color(_) => {}
    }
}

fn fidelity_section(metadata: &DistributionMetadata, strings: &[&str]) -> FcbcResult<Vec<u8>> {
    let sources = metadata
        .input_hashes()
        .iter()
        .map(|hash| fidelity_source_value(hash, strings))
        .collect::<FcbcResult<Vec<_>>>()?;
    let facts = metadata
        .provenance()
        .facts()
        .values()
        .map(|fact| fidelity_fact_value(fact, strings))
        .collect::<FcbcResult<Vec<_>>>()?;
    let mapping_rules = metadata
        .provenance()
        .facts()
        .values()
        .filter_map(|fact| fact.mapping_rule_ref())
        .map(|rule| rule.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|rule| value_string(string_index(strings, rule)))
        .collect();
    let semantic_losses = metadata
        .semantic_losses()
        .iter()
        .map(|loss| fidelity_semantic_loss_value(loss, strings))
        .collect::<FcbcResult<Vec<_>>>()?;
    let fields = vec![
        (
            "specificationVersion",
            value_string(string_index(strings, "1.0.0")),
        ),
        ("sources", value_array(14, sources)),
        ("profileBindings", value_array(14, Vec::new())),
        ("entityMappings", value_array(14, Vec::new())),
        ("fieldFacts", value_array(14, facts)),
        ("mappingRules", value_array(4, mapping_rules)),
        ("semanticLosses", value_array(14, semantic_losses)),
        (
            "custom",
            canonical_object_value(metadata.custom(), strings)?,
        ),
    ];
    Ok(value_object(&fields, strings))
}

fn fidelity_semantic_loss_value(
    loss: &fcs_model::SemanticLoss,
    strings: &[&str],
) -> FcbcResult<Vec<u8>> {
    let mut fields = vec![
        (
            "domain",
            value_string(string_index(strings, loss.domain().as_str())),
        ),
        (
            "status",
            value_string(string_index(strings, loss.status().as_str())),
        ),
        (
            "category",
            value_string(string_index(strings, loss.category())),
        ),
    ];
    if let Some(entity_id) = loss.entity_id() {
        fields.push(("entityId", value_string(string_index(strings, entity_id))));
    }
    Ok(value_object(&fields, strings))
}

fn fidelity_source_value(
    hash: &fcs_model::InputContentHash,
    strings: &[&str],
) -> FcbcResult<Vec<u8>> {
    if hash.algorithm() != "sha256" || !is_sha256_digest(hash.digest_lower_hex()) {
        return Err(FcbcError::new(
            "fcbc.invalid-fidelity",
            "Fidelity source hashes must be lowercase SHA-256 digests",
        ));
    }
    let mut fields = Vec::new();
    if let Some(locator) = hash.logical_source() {
        fields.push((
            "logicalSource",
            value_string(string_index(strings, locator.as_str())),
        ));
    }
    fields.extend([
        (
            "hashAlgorithm",
            value_string(string_index(strings, hash.algorithm())),
        ),
        (
            "digest",
            value_string(string_index(strings, hash.digest_lower_hex())),
        ),
    ]);
    Ok(value_object(&fields, strings))
}

fn fidelity_fact_value(
    fact: &fcs_model::RestrictedProvenanceFact,
    strings: &[&str],
) -> FcbcResult<Vec<u8>> {
    let mut fields = vec![("id", value_string(string_index(strings, fact.id())))];
    if let Some(artifact) = fact.source_artifact_id() {
        fields.push((
            "sourceArtifactId",
            value_string(string_index(strings, artifact)),
        ));
    }
    if let Some(locator) = fact.source_locator() {
        fields.push((
            "sourceLocator",
            value_string(string_index(strings, locator.as_str())),
        ));
    }
    if let Some(order) = fact.source_order() {
        let order = i64::try_from(order).map_err(|_| {
            FcbcError::new(
                "fcbc.invalid-fidelity",
                "Fidelity sourceOrder exceeds the FCBC signed integer range",
            )
        })?;
        fields.push(("sourceOrder", value_int(order)));
    }
    if let Some(rule) = fact.mapping_rule_ref() {
        fields.push((
            "mappingRuleRef",
            value_string(string_index(strings, rule.as_str())),
        ));
    }
    fields.push((
        "originState",
        value_string(string_index(strings, fact.origin_state().as_str())),
    ));
    if let Some(status) = fact.semantic_status() {
        fields.push((
            "semanticStatus",
            value_string(string_index(strings, status.as_str())),
        ));
    }
    let dependencies = fact
        .dependencies()
        .iter()
        .map(|dependency| value_string(string_index(strings, dependency)))
        .collect();
    fields.push(("dependencies", value_array(4, dependencies)));
    fields.push(("stale", value_bool(fact.is_stale())));
    // `source_value` remains an untyped String in the model. It is intentionally
    // not projected: the writer cannot prove it is one numeric/enum fact rather
    // than source text, which Fidelity is forbidden to retain.
    Ok(value_object(&fields, strings))
}

fn canonical_object_value(object: &CanonicalObject, strings: &[&str]) -> FcbcResult<Vec<u8>> {
    canonical_object_value_with_references(
        object,
        strings,
        false,
        &std::collections::BTreeSet::new(),
    )
}

fn canonical_extension_object_value(
    object: &CanonicalObject,
    strings: &[&str],
    contributor_ids: &std::collections::BTreeSet<u64>,
) -> FcbcResult<Vec<u8>> {
    canonical_object_value_with_references(object, strings, true, contributor_ids)
}

fn canonical_object_value_with_references(
    object: &CanonicalObject,
    strings: &[&str],
    allow_references: bool,
    contributor_ids: &std::collections::BTreeSet<u64>,
) -> FcbcResult<Vec<u8>> {
    let fields = object
        .entries()
        .iter()
        .map(|entry| {
            Ok((
                entry.key(),
                canonical_value(entry.value(), strings, 0, allow_references, contributor_ids)?,
            ))
        })
        .collect::<FcbcResult<Vec<_>>>()?;
    Ok(value_object(&fields, strings))
}

fn canonical_value(
    value_: &CanonicalValue,
    strings: &[&str],
    depth: usize,
    allow_references: bool,
    contributor_ids: &std::collections::BTreeSet<u64>,
) -> FcbcResult<Vec<u8>> {
    if depth > 32 {
        return Err(FcbcError::new(
            "fcbc.limit-exceeded",
            "Fidelity custom value nesting exceeds the FCBC limit",
        ));
    }
    match value_ {
        CanonicalValue::Null => Ok(value(0, Vec::new())),
        CanonicalValue::Bool(value_) => Ok(value_bool(*value_)),
        CanonicalValue::Int(value_) => Ok(value_int(*value_)),
        CanonicalValue::Float(value_) => value_finite_scalar(3, *value_),
        CanonicalValue::String(value_) => Ok(value_string(string_index(strings, value_))),
        CanonicalValue::Time(value_) => value_finite_scalar(5, *value_),
        CanonicalValue::Beat(value_) => {
            let mut payload = Vec::new();
            put_i64(&mut payload, value_.numerator());
            put_i64(&mut payload, value_.denominator());
            Ok(value(6, payload))
        }
        CanonicalValue::Color(value_) => {
            let components = [value_.red(), value_.green(), value_.blue(), value_.alpha()];
            if components.iter().any(|component| !component.is_finite()) {
                return Err(FcbcError::new(
                    "fcbc.invalid-fidelity",
                    "Fidelity colors must have finite components",
                ));
            }
            let mut payload = Vec::new();
            for component in components {
                put_f64(&mut payload, component);
            }
            Ok(value(9, payload))
        }
        CanonicalValue::ResourceReference(value_) if allow_references => Ok(value_resource(
            stable_id(b"fcs.resource", value_.as_bytes()),
        )),
        CanonicalValue::ContributorReference(value_) if allow_references => {
            let id = stable_id(b"fcs.contributor", value_.as_bytes());
            if contributor_ids.contains(&id) {
                Ok(value_contributor(id))
            } else {
                Err(FcbcError::new(
                    "fcbc.dangling-reference",
                    "canonical value references a missing contributor",
                ))
            }
        }
        CanonicalValue::ResourceReference(_) | CanonicalValue::ContributorReference(_) => {
            Err(FcbcError::new(
                "fcbc.invalid-fidelity",
                "Fidelity custom cannot encode unresolved entity references",
            ))
        }
        CanonicalValue::Array {
            element_type,
            values,
        } => {
            let tag = canonical_value_type_tag(element_type)?;
            let values = values
                .iter()
                .map(|value_| {
                    canonical_value(
                        value_,
                        strings,
                        depth + 1,
                        allow_references,
                        contributor_ids,
                    )
                })
                .collect::<FcbcResult<Vec<_>>>()?;
            Ok(value_array(tag, values))
        }
        CanonicalValue::Object(object) => {
            let fields = object
                .entries()
                .iter()
                .map(|entry| {
                    Ok((
                        entry.key(),
                        canonical_value(
                            entry.value(),
                            strings,
                            depth + 1,
                            allow_references,
                            contributor_ids,
                        )?,
                    ))
                })
                .collect::<FcbcResult<Vec<_>>>()?;
            Ok(value_object(&fields, strings))
        }
    }
}

fn canonical_value_type_tag(value_type: &CanonicalValueType) -> FcbcResult<u8> {
    match value_type {
        CanonicalValueType::Null => Err(FcbcError::new(
            "fcbc.invalid-fidelity",
            "Fidelity arrays cannot have null elements",
        )),
        CanonicalValueType::Bool => Ok(1),
        CanonicalValueType::Int => Ok(2),
        CanonicalValueType::Float => Ok(3),
        CanonicalValueType::String => Ok(4),
        CanonicalValueType::Time => Ok(5),
        CanonicalValueType::Beat => Ok(6),
        CanonicalValueType::Color => Ok(9),
        CanonicalValueType::ResourceReference => Ok(11),
        CanonicalValueType::ContributorReference => Ok(12),
        CanonicalValueType::Array(_) => Ok(13),
        CanonicalValueType::Object => Ok(14),
    }
}

fn value_bool(value_: bool) -> Vec<u8> {
    let mut payload = vec![u8::from(value_)];
    payload.resize(8, 0);
    value(1, payload)
}

fn value_int(value_: i64) -> Vec<u8> {
    value(2, value_.to_le_bytes().to_vec())
}

fn value_resource(id: u64) -> Vec<u8> {
    value(11, id.to_le_bytes().to_vec())
}

fn value_contributor(id: u64) -> Vec<u8> {
    value(12, id.to_le_bytes().to_vec())
}

fn value_finite_scalar(tag: u8, value_: f64) -> FcbcResult<Vec<u8>> {
    if !value_.is_finite() {
        return Err(FcbcError::new(
            "fcbc.invalid-fidelity",
            "Fidelity scalar values must be finite",
        ));
    }
    Ok(value_scalar(tag, value_))
}

fn value_array(element_tag: u8, values: Vec<Vec<u8>>) -> Vec<u8> {
    let mut payload = vec![element_tag, 0, 0, 0];
    put_u32(&mut payload, values.len() as u32);
    for value_ in values {
        payload.extend_from_slice(&value_);
    }
    value(13, payload)
}

fn is_sha256_digest(value_: &str) -> bool {
    value_.len() == 64
        && value_
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
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

fn resource_sections(
    resources: &[ResourceFixture<'_>],
    strings: &[&str],
) -> FcbcResult<(Vec<u8>, Vec<u8>)> {
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
        payload.extend_from_slice(&canonical_object_value(resource.metadata, strings)?);
        records.extend_from_slice(&record(payload));
    }
    Ok((records, data))
}

fn contributors_section(
    contributors: &[ContributorFixture<'_>],
    strings: &[&str],
) -> FcbcResult<Vec<u8>> {
    let mut section = Vec::new();
    put_u32(&mut section, contributors.len() as u32);
    for contributor in contributors {
        let mut payload = Vec::new();
        put_u64(&mut payload, contributor.id);
        put_u32(
            &mut payload,
            string_index(strings, contributor.contributor.name()),
        );
        put_u32(&mut payload, contributor.contributor.aliases().len() as u32);
        for alias in contributor.contributor.aliases() {
            put_u32(&mut payload, string_index(strings, alias));
        }
        payload.extend_from_slice(&canonical_object_value(
            contributor.contributor.identifiers(),
            strings,
        )?);
        payload.extend_from_slice(&empty_object());
        section.extend_from_slice(&record(payload));
    }
    Ok(section)
}

fn credits_section(credits: &[CreditFixture<'_>], strings: &[&str]) -> FcbcResult<Vec<u8>> {
    let mut section = Vec::new();
    put_u32(&mut section, credits.len() as u32);
    for credit in credits {
        let mut payload = Vec::new();
        put_u64(&mut payload, credit.credit.id().value());
        let (role_kind, custom_role) = match credit.credit.role_kind() {
            CanonicalCreditRole::Standard(role) => (credit_role_ordinal(*role), NULL_INDEX),
            CanonicalCreditRole::Custom(role) => (0, string_index(strings, role)),
        };
        put_u16(&mut payload, role_kind);
        put_u16(&mut payload, 0);
        put_u32(&mut payload, custom_role);
        put_u32(
            &mut payload,
            credit.credit.label().map_or_else(
                || string_index(strings, ""),
                |label| string_index(strings, label),
            ),
        );
        put_u32(&mut payload, credit.contributor_ids.len() as u32);
        for contributor_id in &credit.contributor_ids {
            put_u64(&mut payload, *contributor_id);
        }
        payload.extend_from_slice(&empty_object());
        section.extend_from_slice(&record(payload));
    }
    Ok(section)
}

const fn credit_role_ordinal(role: fcs_model::CanonicalStandardCreditRole) -> u16 {
    match role {
        fcs_model::CanonicalStandardCreditRole::Composer => 1,
        fcs_model::CanonicalStandardCreditRole::Arranger => 2,
        fcs_model::CanonicalStandardCreditRole::Lyricist => 3,
        fcs_model::CanonicalStandardCreditRole::Vocalist => 4,
        fcs_model::CanonicalStandardCreditRole::Instrumentalist => 5,
        fcs_model::CanonicalStandardCreditRole::Mixer => 6,
        fcs_model::CanonicalStandardCreditRole::Mastering => 7,
        fcs_model::CanonicalStandardCreditRole::Charter => 8,
        fcs_model::CanonicalStandardCreditRole::Illustrator => 9,
        fcs_model::CanonicalStandardCreditRole::Designer => 10,
        fcs_model::CanonicalStandardCreditRole::Programmer => 11,
        fcs_model::CanonicalStandardCreditRole::Publisher => 12,
    }
}

fn constant_pool_section(constants: &[Constant]) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u32(&mut payload, constants.len() as u32);
    for constant in constants {
        payload.extend_from_slice(&constant.encoded());
    }
    payload
}

fn meta_section(
    chart: Option<&CanonicalChart>,
    strings: &[&str],
    contributor_ids: &std::collections::BTreeSet<u64>,
) -> FcbcResult<Vec<u8>> {
    let (document_profile, document_features, meta, artwork) = match chart {
        Some(chart) => (
            document_profile(chart.profile()),
            document_feature_bits(chart.profile(), chart.features()),
            canonical_meta_value(chart, strings, contributor_ids)?,
            canonical_artwork_value(chart, strings),
        ),
        None => (2, 0, empty_object(), empty_object()),
    };
    let mut payload = vec![document_profile, 0, 0, 0];
    put_u32(&mut payload, document_features);
    payload.extend_from_slice(&meta);
    payload.extend_from_slice(&artwork);
    Ok(payload)
}

const META_KEYS: [&str; 13] = [
    "title",
    "subtitle",
    "alternativeTitles",
    "chartVersion",
    "difficulty",
    "level",
    "description",
    "language",
    "tags",
    "license",
    "documentId",
    "revision",
    "custom",
];

fn canonical_meta_value(
    chart: &CanonicalChart,
    strings: &[&str],
    contributor_ids: &std::collections::BTreeSet<u64>,
) -> FcbcResult<Vec<u8>> {
    let Some(meta) = chart.metadata().meta() else {
        return Ok(empty_object());
    };
    if let Some(unknown) = meta.keys().find(|key| !META_KEYS.contains(&key.as_str())) {
        return Err(FcbcError::new(
            "fcbc.invalid-record",
            format!("unsupported canonical meta field {unknown}"),
        ));
    }
    let fields = META_KEYS
        .iter()
        .filter_map(|key| meta.get(*key).map(|value| (*key, value)))
        .map(|(key, value)| {
            Ok((
                key,
                canonical_value(value, strings, 0, true, contributor_ids)?,
            ))
        })
        .collect::<FcbcResult<Vec<_>>>()?;
    Ok(value_object(&fields, strings))
}

fn canonical_artwork_value(chart: &CanonicalChart, _strings: &[&str]) -> Vec<u8> {
    let fields = chart
        .metadata()
        .artwork()
        .and_then(|artwork| artwork.primary())
        .map(|primary| {
            vec![(
                "primary",
                value_resource(stable_id(b"fcs.resource", primary.as_bytes())),
            )]
        })
        .unwrap_or_default();
    value_object(&fields, _strings)
}

fn document_profile(profile: CanonicalProfile) -> u8 {
    match profile {
        CanonicalProfile::Fragment => 1,
        CanonicalProfile::Chart => 2,
        CanonicalProfile::Playable => 3,
        CanonicalProfile::Renderable => 4,
        CanonicalProfile::Publishable => 5,
    }
}

fn document_feature_bits(
    profile: CanonicalProfile,
    features: &std::collections::BTreeSet<CanonicalProfileFeature>,
) -> u32 {
    let mut bits = 0;
    if matches!(profile, CanonicalProfile::Playable) {
        bits |= 1;
    }
    if matches!(profile, CanonicalProfile::Renderable) {
        bits |= 1 << 1;
    }
    if features.contains(&CanonicalProfileFeature::Playable) {
        bits |= 1;
    }
    if features.contains(&CanonicalProfileFeature::Renderable) {
        bits |= 1 << 1;
    }
    bits
}

fn parse_source_version(value: &str) -> FcbcResult<(u16, u16, u16)> {
    let mut parts = value.split('.');
    let components = [parts.next(), parts.next(), parts.next()];
    if parts.next().is_some() {
        return Err(FcbcError::new(
            "fcbc.unsupported-source-version",
            format!("source FCS version {value} is not three numeric components"),
        ));
    }
    let mut parsed = [0u16; 3];
    for (index, component) in components.into_iter().enumerate() {
        parsed[index] = component
            .and_then(|component| component.parse().ok())
            .ok_or_else(|| {
                FcbcError::new(
                    "fcbc.unsupported-source-version",
                    format!("source FCS version {value} exceeds the FCBC header width"),
                )
            })?;
    }
    if parsed[0] != 5 {
        return Err(FcbcError::new(
            "fcbc.unsupported-source-version",
            format!("source FCS major version {} is not supported", parsed[0]),
        ));
    }
    Ok((parsed[0], parsed[1], parsed[2]))
}

fn sync_section(sync: SyncFixture) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u64(&mut payload, sync.primary_audio_id);
    put_f64(&mut payload, sync.audio_offset);
    put_u8(&mut payload, u8::from(sync.preview.is_some()));
    payload.resize(24, 0);
    let (preview_start, preview_end) = sync.preview.unwrap_or((0.0, 0.0));
    put_f64(&mut payload, preview_start);
    put_f64(&mut payload, preview_end);
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
        payload.extend_from_slice(&judge_shape_value(&note.judge_shape, strings));
        put_u16(&mut payload, note.sound_policy);
        put_u16(&mut payload, note.score_policy);
        put_u64(&mut payload, note.sound_resource_id);
        put_u32(
            &mut payload,
            note.score_extension
                .as_deref()
                .map_or(NULL_INDEX, |namespace| string_index(strings, namespace)),
        );
        put_u32(&mut payload, 0);
        for descriptor in note.property_descriptors {
            put_u32(&mut payload, descriptor);
        }
        put_u64(&mut payload, note.texture_resource_id);
        payload.extend_from_slice(&empty_object());
        section.extend_from_slice(&record(payload));
    }
    section
}

fn judge_shape_value(shape: &JudgeShapeFixture, strings: &[&str]) -> Vec<u8> {
    let mut fields = Vec::new();
    match shape {
        JudgeShapeFixture::LineDefault => {
            fields.push(("kind", value_string(string_index(strings, "lineDefault"))));
        }
        JudgeShapeFixture::Rectangle {
            center,
            half_extents,
        } => {
            fields.push(("kind", value_string(string_index(strings, "rectangle"))));
            fields.push(("center", value_vec2_length(*center)));
            fields.push(("halfExtents", value_vec2_length(*half_extents)));
        }
        JudgeShapeFixture::Circle { center, radius } => {
            fields.push(("kind", value_string(string_index(strings, "circle"))));
            fields.push(("center", value_vec2_length(*center)));
            fields.push(("radius", value_scalar(7, *radius)));
        }
    }
    value_object(&fields, strings)
}

fn value_object(fields: &[(&str, Vec<u8>)], strings: &[&str]) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u32(&mut payload, fields.len() as u32);
    for (key, encoded_value) in fields {
        put_u32(&mut payload, string_index(strings, key));
        payload.extend_from_slice(encoded_value);
    }
    value(14, payload)
}

fn value_string(string_ref: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u32(&mut payload, string_ref);
    put_u32(&mut payload, 0);
    value(4, payload)
}

fn value_scalar(tag: u8, scalar: f64) -> Vec<u8> {
    let mut payload = Vec::new();
    put_f64(&mut payload, scalar);
    value(tag, payload)
}

fn value_vec2_length(value_: [f64; 2]) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u8(&mut payload, 7); // Value tag: length
    payload.resize(8, 0);
    put_f64(&mut payload, value_[0]);
    put_f64(&mut payload, value_[1]);
    value(10, payload)
}

fn extensions_section(
    extensions: &[ExtensionFixture<'_>],
    strings: &[&str],
    contributor_ids: &std::collections::BTreeSet<u64>,
) -> FcbcResult<Vec<u8>> {
    let mut section = Vec::new();
    put_u32(&mut section, extensions.len() as u32);
    for extension in extensions {
        let mut payload = Vec::new();
        put_u32(&mut payload, string_index(strings, &extension.namespace));
        put_u16(&mut payload, extension.version.0);
        put_u16(&mut payload, extension.version.1);
        put_u16(&mut payload, extension.version.2);
        put_u16(&mut payload, 1); // required
        payload.extend_from_slice(&canonical_extension_object_value(
            extension.payload,
            strings,
            contributor_ids,
        )?);
        section.extend_from_slice(&record(payload));
    }
    Ok(section)
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
    visibility_constants: Option<(u32, u32)>,
    lines: &mut [LineFixture],
    notes: &mut [NoteFixture],
    tracks: &[NativeTrackFixture],
    has_notes: bool,
    runtime_descriptors: Option<&CanonicalDescriptorTable>,
) -> FcbcResult<(Vec<u8>, Vec<u8>, Vec<u32>)> {
    let mut descriptors = Vec::new();
    let mut expressions = NativeExpressionPool::default();
    let mut runtime_descriptor_indices = runtime_descriptors
        .map(|table| vec![None; table.descriptors().len()])
        .unwrap_or_default();

    // Descriptor order follows the canonical direct-root path order used by the loader:
    // all Line roots are grouped by path, then by stable Line ID.
    for line in lines.iter_mut() {
        line.alpha_descriptor = native_line_descriptor(
            &mut descriptors,
            constants,
            tracks,
            line.id,
            CanonicalTrackTarget::Alpha,
            TY_FLOAT,
            &float_constant(line.alpha),
        );
    }
    for line in lines.iter_mut() {
        line.position_descriptor = native_line_descriptor(
            &mut descriptors,
            constants,
            tracks,
            line.id,
            CanonicalTrackTarget::Position,
            TY_VEC2_LENGTH,
            &vec2_constant(7, line.position),
        );
    }
    for line in lines.iter_mut() {
        line.rotation_descriptor = native_line_descriptor(
            &mut descriptors,
            constants,
            tracks,
            line.id,
            CanonicalTrackTarget::Rotation,
            TY_ANGLE,
            &scalar_constant(8, line.rotation),
        );
    }
    for line in lines.iter_mut() {
        line.scale_descriptor = native_line_descriptor(
            &mut descriptors,
            constants,
            tracks,
            line.id,
            CanonicalTrackTarget::Scale,
            TY_VEC2_FLOAT,
            &vec2_constant(3, line.scale),
        );
    }
    for line in lines.iter_mut() {
        line.speed_descriptor = native_line_descriptor(
            &mut descriptors,
            constants,
            tracks,
            line.id,
            CanonicalTrackTarget::ScrollSpeed,
            TY_FLOAT,
            &float_constant(1.0),
        );
        line.evaluable_speed = tracks.iter().any(|track| {
            track.line_id == line.id && track.target == CanonicalTrackTarget::ScrollSpeed
        });
    }
    for line in lines.iter_mut() {
        line.scroll_tempo_descriptor =
            native_scroll_tempo_descriptor(&mut descriptors, constants, &line.scroll_tempo);
    }

    if has_notes {
        let mut note_order: Vec<_> = (0..notes.len()).collect();
        note_order.sort_by_key(|index| notes[*index].id);
        // Direct roots are allocated by canonical target path, then stable Note ID.
        for property in [4usize, 8, 0, 7, 5, 6, 1, 9, 2, 3] {
            for &index in &note_order {
                let runtime_descriptor = runtime_descriptors.and_then(|table| {
                    table
                        .roots()
                        .iter()
                        .find(|root| {
                            root.target_path() == note_property_path(property)
                                && root.owner() == notes[index].id
                        })
                        .map(|root| (table, root.descriptor()))
                });
                let descriptor = if let Some((table, descriptor)) = runtime_descriptor {
                    native_canonical_descriptor(
                        table,
                        descriptor,
                        constants,
                        &mut descriptors,
                        &mut expressions,
                        &mut runtime_descriptor_indices,
                    )?
                } else if property == 9 {
                    let (false_constant, true_constant) = visibility_constants
                        .expect("Native Note visibility fallback constants must be seeded");
                    native_note_visibility_descriptor(
                        &mut descriptors,
                        false_constant,
                        true_constant,
                        notes[index].visible_from,
                        notes[index].visible_until,
                    )
                } else {
                    let property_type = match property {
                        0 | 2 | 3 => TY_LENGTH,
                        7 => TY_ANGLE,
                        8 => TY_COLOR,
                        _ => TY_FLOAT,
                    };
                    intern_constant_descriptor(
                        &mut descriptors,
                        property_type,
                        find_constant(constants, &notes[index].property_constants[property]),
                    )
                };
                notes[index].property_descriptors[property] = descriptor;
            }
        }
    }

    if let Some(table) = runtime_descriptors {
        for canonical_index in 0..table.descriptors().len() {
            native_canonical_descriptor(
                table,
                canonical_index,
                constants,
                &mut descriptors,
                &mut expressions,
                &mut runtime_descriptor_indices,
            )?;
        }
    }

    let mut section = Vec::new();
    put_u32(&mut section, descriptors.len() as u32);
    for descriptor in descriptors {
        section.extend_from_slice(&descriptor);
    }
    Ok((
        section,
        expressions.section(),
        runtime_descriptor_indices
            .into_iter()
            .map(|index| index.expect("canonical descriptor must be reachable"))
            .collect(),
    ))
}

fn note_property_path(property: usize) -> &'static str {
    match property {
        0 => "note.presentation.positionX",
        1 => "note.presentation.scrollFactor",
        2 => "note.presentation.xOffset",
        3 => "note.presentation.yOffset",
        4 => "note.presentation.alpha",
        5 => "note.presentation.scaleX",
        6 => "note.presentation.scaleY",
        7 => "note.presentation.rotation",
        8 => "note.presentation.color",
        9 => "note.presentation.visibility",
        _ => unreachable!("Note property descriptor index is fixed by FCBC 2"),
    }
}

fn native_canonical_descriptor(
    table: &CanonicalDescriptorTable,
    canonical_index: usize,
    constants: &[Constant],
    descriptors: &mut Vec<Vec<u8>>,
    expressions: &mut NativeExpressionPool,
    mapped: &mut [Option<u32>],
) -> FcbcResult<u32> {
    if let Some(index) = mapped[canonical_index] {
        return Ok(index);
    }
    let descriptor = table.descriptor(canonical_index).ok_or_else(|| {
        FcbcError::new(
            "fcbc.dangling-reference",
            format!("canonical descriptor {canonical_index} is missing"),
        )
    })?;
    let property_type = canonical_expression_type(descriptor.property_type());
    let encoded = match descriptor.kind() {
        CanonicalDescriptorKind::Constant(value) => constant_descriptor(
            property_type,
            find_constant(constants, &canonical_expression_constant(value)?),
        ),
        CanonicalDescriptorKind::Expression(expression) => {
            expression_descriptor(property_type, expressions.emit(expression, constants)?)
        }
        CanonicalDescriptorKind::Piecewise(pieces) => {
            let domain = descriptor.domain();
            let mut flags = 0;
            if domain.start().is_none() {
                flags |= 1;
            }
            if domain.end().is_none() {
                flags |= 2;
            }
            let mut payload = descriptor_common(
                property_type,
                3,
                flags,
                domain.start().unwrap_or(0.0),
                domain.end().unwrap_or(0.0),
            );
            put_u32(&mut payload, pieces.len() as u32);
            for piece in pieces {
                let child = native_canonical_descriptor(
                    table,
                    piece.descriptor(),
                    constants,
                    descriptors,
                    expressions,
                    mapped,
                )?;
                put_f64(&mut payload, piece.start().unwrap_or(0.0));
                put_f64(&mut payload, piece.end().unwrap_or(0.0));
                put_u32(&mut payload, child);
                put_u32(
                    &mut payload,
                    u32::from(piece.end_inclusive())
                        | (u32::from(piece.start().is_none()) << 1)
                        | (u32::from(piece.end().is_none()) << 2),
                );
            }
            record(payload)
        }
    };
    let index = intern_descriptor(descriptors, encoded);
    mapped[canonical_index] = Some(index);
    Ok(index)
}

fn canonical_expression_type(value: &CanonicalExpressionType) -> u8 {
    match value {
        CanonicalExpressionType::Bool => TY_BOOL,
        CanonicalExpressionType::Int => TY_INT,
        CanonicalExpressionType::Float => TY_FLOAT,
        CanonicalExpressionType::Time => TY_TIME,
        CanonicalExpressionType::Beat => TY_BEAT,
        CanonicalExpressionType::Length => TY_LENGTH,
        CanonicalExpressionType::Angle => TY_ANGLE,
        CanonicalExpressionType::Color => TY_COLOR,
        CanonicalExpressionType::Vec2(element) => match element.as_ref() {
            CanonicalExpressionType::Int => TY_VEC2_INT,
            CanonicalExpressionType::Float => TY_VEC2_FLOAT,
            CanonicalExpressionType::Time => TY_VEC2_TIME,
            CanonicalExpressionType::Beat => TY_VEC2_BEAT,
            CanonicalExpressionType::Length => TY_VEC2_LENGTH,
            CanonicalExpressionType::Angle => TY_VEC2_ANGLE,
            _ => unreachable!("canonical expression vectors have numeric elements"),
        },
    }
}

fn canonical_expression_opcode(value: CanonicalExpressionOpcode) -> u16 {
    match value {
        CanonicalExpressionOpcode::Constant => 1,
        CanonicalExpressionOpcode::EnvS => 2,
        CanonicalExpressionOpcode::EnvB => 3,
        CanonicalExpressionOpcode::EnvQ => 4,
        CanonicalExpressionOpcode::EnvD => 5,
        CanonicalExpressionOpcode::EnvP => 6,
        CanonicalExpressionOpcode::Neg => 10,
        CanonicalExpressionOpcode::Not => 11,
        CanonicalExpressionOpcode::Add => 20,
        CanonicalExpressionOpcode::Sub => 21,
        CanonicalExpressionOpcode::Mul => 22,
        CanonicalExpressionOpcode::Div => 23,
        CanonicalExpressionOpcode::Mod => 24,
        CanonicalExpressionOpcode::Pow => 25,
        CanonicalExpressionOpcode::Eq => 30,
        CanonicalExpressionOpcode::Ne => 31,
        CanonicalExpressionOpcode::Lt => 32,
        CanonicalExpressionOpcode::Le => 33,
        CanonicalExpressionOpcode::Gt => 34,
        CanonicalExpressionOpcode::Ge => 35,
        CanonicalExpressionOpcode::And => 36,
        CanonicalExpressionOpcode::Or => 37,
        CanonicalExpressionOpcode::ApproxEq => 38,
        CanonicalExpressionOpcode::Abs => 40,
        CanonicalExpressionOpcode::Min => 41,
        CanonicalExpressionOpcode::Max => 42,
        CanonicalExpressionOpcode::Clamp => 43,
        CanonicalExpressionOpcode::Floor => 44,
        CanonicalExpressionOpcode::Ceil => 45,
        CanonicalExpressionOpcode::Round => 46,
        CanonicalExpressionOpcode::Sqrt => 47,
        CanonicalExpressionOpcode::Exp => 48,
        CanonicalExpressionOpcode::Ln => 49,
        CanonicalExpressionOpcode::Sin => 50,
        CanonicalExpressionOpcode::Cos => 51,
        CanonicalExpressionOpcode::Tan => 52,
        CanonicalExpressionOpcode::Asin => 53,
        CanonicalExpressionOpcode::Acos => 54,
        CanonicalExpressionOpcode::Atan => 55,
        CanonicalExpressionOpcode::Atan2 => 56,
        CanonicalExpressionOpcode::Easing => 60,
        CanonicalExpressionOpcode::ToFloat => 61,
        CanonicalExpressionOpcode::Seconds => 62,
        CanonicalExpressionOpcode::Radians => 63,
        CanonicalExpressionOpcode::Choose => 70,
        CanonicalExpressionOpcode::Vec2 => 80,
        CanonicalExpressionOpcode::Vec2X => 81,
        CanonicalExpressionOpcode::Vec2Y => 82,
    }
}

fn native_scroll_tempo_descriptor(
    descriptors: &mut Vec<Vec<u8>>,
    constants: &[Constant],
    points: &[ScrollTempoPointFixture],
) -> u32 {
    debug_assert!(!points.is_empty());
    if points.len() == 1 {
        return intern_constant_descriptor(
            descriptors,
            TY_FLOAT,
            find_constant(constants, &float_constant(points[0].bpm)),
        );
    }
    let segments: Vec<_> = points
        .iter()
        .map(|point| {
            let value = float_constant(point.bpm);
            TrackSegmentFixture {
                start: point.time,
                end: point.time,
                interpolation: 1,
                easing: 0,
                flags: 1,
                start_constant: value.clone(),
                end_constant: value,
                bezier: [0.0; 4],
            }
        })
        .collect();
    intern_descriptor(
        descriptors,
        segment_track_descriptor(TY_FLOAT, &segments, constants),
    )
}

fn native_note_visibility_descriptor(
    descriptors: &mut Vec<Vec<u8>>,
    false_constant: u32,
    true_constant: u32,
    visible_from: Option<f64>,
    visible_until: Option<f64>,
) -> u32 {
    if visible_from.is_none() && visible_until.is_none() {
        return intern_constant_descriptor(descriptors, TY_BOOL, true_constant);
    }
    let false_descriptor = intern_constant_descriptor(descriptors, TY_BOOL, false_constant);
    let true_descriptor = intern_constant_descriptor(descriptors, TY_BOOL, true_constant);
    let mut payload = descriptor_common(TY_BOOL, 3, 0b11, 0.0, 0.0);
    let piece_count =
        1 + usize::from(visible_from.is_some()) + usize::from(visible_until.is_some());
    put_u32(&mut payload, piece_count as u32);
    if let Some(end) = visible_from {
        put_f64(&mut payload, 0.0);
        put_f64(&mut payload, end);
        put_u32(&mut payload, false_descriptor);
        put_u32(&mut payload, 0b010);
    }
    if let Some(start) = visible_from {
        put_f64(&mut payload, start);
        put_f64(&mut payload, visible_until.unwrap_or(0.0));
        put_u32(&mut payload, true_descriptor);
        put_u32(
            &mut payload,
            if visible_until.is_some() { 0 } else { 0b100 },
        );
    } else if let Some(end) = visible_until {
        put_f64(&mut payload, 0.0);
        put_f64(&mut payload, end);
        put_u32(&mut payload, true_descriptor);
        put_u32(&mut payload, 0b010);
    }
    if let Some(start) = visible_until {
        put_f64(&mut payload, start);
        put_f64(&mut payload, 0.0);
        put_u32(&mut payload, false_descriptor);
        put_u32(&mut payload, 0b100);
    }
    intern_descriptor(descriptors, record(payload))
}

fn native_line_descriptor(
    descriptors: &mut Vec<Vec<u8>>,
    constants: &[Constant],
    tracks: &[NativeTrackFixture],
    line_id: u64,
    target: CanonicalTrackTarget,
    property_type: u8,
    base_constant: &Constant,
) -> u32 {
    if let Some(track) = tracks
        .iter()
        .find(|track| track.line_id == line_id && track.target == target)
    {
        let before_descriptor = intern_constant_descriptor(
            descriptors,
            property_type,
            find_constant(constants, &track.before_constant),
        );
        let track_descriptor = intern_descriptor(
            descriptors,
            segment_track_descriptor(property_type, &track.segments, constants),
        );
        intern_descriptor(
            descriptors,
            piecewise_track_descriptor(
                property_type,
                track.first_time,
                before_descriptor,
                track_descriptor,
            ),
        )
    } else {
        intern_constant_descriptor(
            descriptors,
            property_type,
            find_constant(constants, base_constant),
        )
    }
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

fn piecewise_track_descriptor(
    property_type: u8,
    first_time: f64,
    before_descriptor: u32,
    track_descriptor: u32,
) -> Vec<u8> {
    let mut payload = descriptor_common(property_type, 3, 0b11, 0.0, 0.0);
    put_u32(&mut payload, 2);
    put_f64(&mut payload, 0.0);
    put_f64(&mut payload, first_time);
    put_u32(&mut payload, before_descriptor);
    put_u32(&mut payload, 0b010);
    put_f64(&mut payload, first_time);
    put_f64(&mut payload, 0.0);
    put_u32(&mut payload, track_descriptor);
    put_u32(&mut payload, 0b100);
    let descriptor = record(payload);
    debug_assert_eq!(descriptor.len(), 80);
    descriptor
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

fn default_note_property_constants() -> [Constant; 9] {
    [
        scalar_constant(7, 0.0),
        float_constant(1.0),
        scalar_constant(7, 0.0),
        scalar_constant(7, 0.0),
        float_constant(1.0),
        float_constant(1.0),
        float_constant(1.0),
        scalar_constant(8, 0.0),
        color_constant([1.0, 1.0, 1.0, 1.0]),
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

fn distance_section_for_lines(lines: &[LineFixture], tracks: &[NativeTrackFixture]) -> Vec<u8> {
    let mut section = Vec::new();
    put_u32(&mut section, lines.len() as u32);
    for line in lines {
        // Classification/boundary pairing must match both Line scroll roots.
        let evaluable_distance = line.evaluable_speed || line.scroll_tempo.len() > 1;
        let (classification, max_error, mut boundaries) = if evaluable_distance {
            let mut boundaries = vec![line.integration_origin];
            if line.evaluable_speed {
                if let Some(track) = tracks.iter().find(|track| {
                    track.line_id == line.id && track.target == CanonicalTrackTarget::ScrollSpeed
                }) {
                    for segment in &track.segments {
                        boundaries.push(segment.start);
                        boundaries.push(segment.end);
                    }
                } else {
                    // The declarative non-empty fixture has no native Track graph.
                    boundaries.push(2.0);
                }
            }
            if line.scroll_tempo.len() > 1 {
                boundaries.extend(line.scroll_tempo.iter().map(|point| point.time));
            }
            (2u8, 2.328_306_436_538_696_3e-10, boundaries)
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

fn write_header(
    bytes: &mut [u8],
    section_count: u32,
    feature_flags: u64,
    profile: ContainerProfile,
    source_version: (u16, u16, u16),
) {
    bytes[0..4].copy_from_slice(b"FCSB");
    write_u16_at(bytes, 4, 128);
    write_u16_at(bytes, 6, 0);
    write_u16_at(bytes, 8, source_version.0);
    write_u16_at(bytes, 10, source_version.1);
    write_u16_at(bytes, 12, source_version.2);
    write_u16_at(bytes, 14, 2);
    write_u16_at(bytes, 16, 0);
    write_u16_at(bytes, 18, 0);
    write_u16_at(bytes, 20, 1);
    write_u16_at(bytes, 22, 0);
    write_u16_at(bytes, 24, 0);
    bytes[26] = profile as u8;
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

#[cfg(test)]
#[path = "writer_tests.rs"]
mod note_kind_tests;
