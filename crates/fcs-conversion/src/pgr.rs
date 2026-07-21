use std::fmt;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};

use crate::{
    DecimalLimits, ExactDecimal, ExactNumberError, ExactRational, LosslessJsonMember,
    LosslessJsonValue, ParsedSourceDocument, SourceFormat,
};

pub const SOURCE_INVALID: &str = "conversion.source-invalid";
pub const UNSUPPORTED_FORMAT_VERSION: &str = "conversion.unsupported-format-version";
pub const PROFILE_PARAMETER_INVALID: &str = "conversion.profile-parameter-invalid";
pub const PROFILE_NOT_APPLICABLE: &str = "conversion.profile-not-applicable";
pub const DISTANCE_MISMATCH: &str = "conversion.distance-mismatch";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PgrLimits {
    pub decimal: DecimalLimits,
    pub max_lines: usize,
    pub max_events_per_line: usize,
    pub max_notes_per_line: usize,
}

impl Default for PgrLimits {
    fn default() -> Self {
        Self {
            decimal: DecimalLimits::default(),
            max_lines: 4096,
            max_events_per_line: 262_144,
            max_notes_per_line: 262_144,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PgrFormatVersion {
    V1,
    V3,
}

impl PgrFormatVersion {
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::V1 => 1,
            Self::V3 => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgrSourceDocument {
    format_version: PgrFormatVersion,
    offset: ExactDecimal,
    lines: Vec<PgrSourceLine>,
    unknown_fields: Vec<LosslessJsonMember>,
}

impl PgrSourceDocument {
    pub const fn format_version(&self) -> PgrFormatVersion {
        self.format_version
    }

    pub fn offset(&self) -> &ExactDecimal {
        &self.offset
    }

    pub fn lines(&self) -> &[PgrSourceLine] {
        &self.lines
    }

    pub fn unknown_fields(&self) -> &[LosslessJsonMember] {
        &self.unknown_fields
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgrSourceLine {
    bpm: ExactDecimal,
    move_events: Vec<PgrSourceMoveEvent>,
    rotate_events: Vec<PgrSourceScalarEvent>,
    disappear_events: Vec<PgrSourceScalarEvent>,
    speed_events: Vec<PgrSourceSpeedEvent>,
    notes_above: Vec<PgrSourceNote>,
    notes_below: Vec<PgrSourceNote>,
    unknown_fields: Vec<LosslessJsonMember>,
}

impl PgrSourceLine {
    pub fn bpm(&self) -> &ExactDecimal {
        &self.bpm
    }

    pub fn move_events(&self) -> &[PgrSourceMoveEvent] {
        &self.move_events
    }

    pub fn rotate_events(&self) -> &[PgrSourceScalarEvent] {
        &self.rotate_events
    }

    pub fn disappear_events(&self) -> &[PgrSourceScalarEvent] {
        &self.disappear_events
    }

    pub fn speed_events(&self) -> &[PgrSourceSpeedEvent] {
        &self.speed_events
    }

    pub fn notes_above(&self) -> &[PgrSourceNote] {
        &self.notes_above
    }

    pub fn notes_below(&self) -> &[PgrSourceNote] {
        &self.notes_below
    }

    pub fn unknown_fields(&self) -> &[LosslessJsonMember] {
        &self.unknown_fields
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgrSourceScalarEvent {
    start_time: ExactDecimal,
    end_time: ExactDecimal,
    start: ExactDecimal,
    end: ExactDecimal,
    unknown_fields: Vec<LosslessJsonMember>,
}

impl PgrSourceScalarEvent {
    pub fn start_time(&self) -> &ExactDecimal {
        &self.start_time
    }

    pub fn end_time(&self) -> &ExactDecimal {
        &self.end_time
    }

    pub fn start(&self) -> &ExactDecimal {
        &self.start
    }

    pub fn end(&self) -> &ExactDecimal {
        &self.end
    }

    pub fn unknown_fields(&self) -> &[LosslessJsonMember] {
        &self.unknown_fields
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgrSourceMoveEvent {
    start_time: ExactDecimal,
    end_time: ExactDecimal,
    start_x: ExactDecimal,
    end_x: ExactDecimal,
    start_y: Option<ExactDecimal>,
    end_y: Option<ExactDecimal>,
    unknown_fields: Vec<LosslessJsonMember>,
}

impl PgrSourceMoveEvent {
    pub fn start_time(&self) -> &ExactDecimal {
        &self.start_time
    }

    pub fn end_time(&self) -> &ExactDecimal {
        &self.end_time
    }

    pub fn start_x(&self) -> &ExactDecimal {
        &self.start_x
    }

    pub fn end_x(&self) -> &ExactDecimal {
        &self.end_x
    }

    pub fn start_y(&self) -> Option<&ExactDecimal> {
        self.start_y.as_ref()
    }

    pub fn end_y(&self) -> Option<&ExactDecimal> {
        self.end_y.as_ref()
    }

    pub fn unknown_fields(&self) -> &[LosslessJsonMember] {
        &self.unknown_fields
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgrSourceSpeedEvent {
    start_time: ExactDecimal,
    end_time: ExactDecimal,
    value: ExactDecimal,
    floor_position: Option<ExactDecimal>,
    unknown_fields: Vec<LosslessJsonMember>,
}

impl PgrSourceSpeedEvent {
    pub fn start_time(&self) -> &ExactDecimal {
        &self.start_time
    }

    pub fn end_time(&self) -> &ExactDecimal {
        &self.end_time
    }

    pub fn value(&self) -> &ExactDecimal {
        &self.value
    }

    pub fn floor_position(&self) -> Option<&ExactDecimal> {
        self.floor_position.as_ref()
    }

    pub fn unknown_fields(&self) -> &[LosslessJsonMember] {
        &self.unknown_fields
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PgrNoteKind {
    Tap,
    Drag,
    Hold,
    Flick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PgrNoteSide {
    Above,
    Below,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgrSourceNote {
    kind: PgrNoteKind,
    time: ExactDecimal,
    hold_time: ExactDecimal,
    position_x: ExactDecimal,
    speed: ExactDecimal,
    floor_position: ExactDecimal,
    unknown_fields: Vec<LosslessJsonMember>,
}

impl PgrSourceNote {
    pub const fn kind(&self) -> PgrNoteKind {
        self.kind
    }

    pub fn time(&self) -> &ExactDecimal {
        &self.time
    }

    pub fn hold_time(&self) -> &ExactDecimal {
        &self.hold_time
    }

    pub fn position_x(&self) -> &ExactDecimal {
        &self.position_x
    }

    pub fn speed(&self) -> &ExactDecimal {
        &self.speed
    }

    pub fn floor_position(&self) -> &ExactDecimal {
        &self.floor_position
    }

    pub fn unknown_fields(&self) -> &[LosslessJsonMember] {
        &self.unknown_fields
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PgrProfile {
    PhiraV1,
    PhiraV3,
    PhichainImportV1,
    PhichainImportV3,
}

impl PgrProfile {
    pub const fn id(self) -> &'static str {
        match self {
            Self::PhiraV1 => "pgr.phira.v1",
            Self::PhiraV3 => "pgr.phira.v3",
            Self::PhichainImportV1 => "pgr.phichain-import.v1",
            Self::PhichainImportV3 => "pgr.phichain-import.v3",
        }
    }

    pub const fn version(self) -> &'static str {
        "1.0.0"
    }

    pub const fn format_version(self) -> PgrFormatVersion {
        match self {
            Self::PhiraV1 | Self::PhichainImportV1 => PgrFormatVersion::V1,
            Self::PhiraV3 | Self::PhichainImportV3 => PgrFormatVersion::V3,
        }
    }

    pub const fn strict_eligible(self) -> bool {
        matches!(self, Self::PhiraV1 | Self::PhiraV3)
    }

    const fn is_phira(self) -> bool {
        matches!(self, Self::PhiraV1 | Self::PhiraV3)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgrProfileBinding {
    profile: PgrProfile,
    floor_scale_px: ExactRational,
}

impl PgrProfileBinding {
    pub fn new(profile: PgrProfile, floor_scale_px: ExactDecimal) -> Result<Self, PgrError> {
        if !floor_scale_px.exact().is_positive() || floor_scale_px.to_f64().is_err() {
            return Err(PgrError::new(
                PROFILE_PARAMETER_INVALID,
                "profile.floorScalePx",
                "floorScalePx must be finite and positive",
            ));
        }
        Ok(Self {
            profile,
            floor_scale_px: floor_scale_px.exact().clone(),
        })
    }

    pub const fn profile(&self) -> PgrProfile {
        self.profile
    }

    pub fn floor_scale_px(&self) -> &ExactRational {
        &self.floor_scale_px
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgrSemanticTime {
    source_line_beat: ExactRational,
    chart_time_seconds: ExactRational,
}

impl PgrSemanticTime {
    pub fn source_line_beat(&self) -> &ExactRational {
        &self.source_line_beat
    }

    pub fn chart_time_seconds(&self) -> &ExactRational {
        &self.chart_time_seconds
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgrSemanticScalarEvent {
    start_time: PgrSemanticTime,
    end_time: PgrSemanticTime,
    start_value: ExactRational,
    end_value: ExactRational,
}

impl PgrSemanticScalarEvent {
    pub fn start_time(&self) -> &PgrSemanticTime {
        &self.start_time
    }

    pub fn end_time(&self) -> &PgrSemanticTime {
        &self.end_time
    }

    pub fn start_value(&self) -> &ExactRational {
        &self.start_value
    }

    pub fn end_value(&self) -> &ExactRational {
        &self.end_value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgrSemanticMoveEvent {
    start_time: PgrSemanticTime,
    end_time: PgrSemanticTime,
    start_x_px: ExactRational,
    end_x_px: ExactRational,
    start_y_px: ExactRational,
    end_y_px: ExactRational,
}

impl PgrSemanticMoveEvent {
    pub fn start_time(&self) -> &PgrSemanticTime {
        &self.start_time
    }

    pub fn end_time(&self) -> &PgrSemanticTime {
        &self.end_time
    }

    pub fn start_x_px(&self) -> &ExactRational {
        &self.start_x_px
    }

    pub fn end_x_px(&self) -> &ExactRational {
        &self.end_x_px
    }

    pub fn start_y_px(&self) -> &ExactRational {
        &self.start_y_px
    }

    pub fn end_y_px(&self) -> &ExactRational {
        &self.end_y_px
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgrSemanticSpeedEvent {
    start_time: PgrSemanticTime,
    end_time: PgrSemanticTime,
    value: ExactRational,
    distance_start: ExactRational,
    distance_end: ExactRational,
    distance_start_px: ExactRational,
    distance_end_px: ExactRational,
    raw_floor_position: Option<ExactRational>,
}

impl PgrSemanticSpeedEvent {
    pub fn start_time(&self) -> &PgrSemanticTime {
        &self.start_time
    }

    pub fn end_time(&self) -> &PgrSemanticTime {
        &self.end_time
    }

    pub fn value(&self) -> &ExactRational {
        &self.value
    }

    pub fn distance_start(&self) -> &ExactRational {
        &self.distance_start
    }

    pub fn distance_end(&self) -> &ExactRational {
        &self.distance_end
    }

    pub fn distance_start_px(&self) -> &ExactRational {
        &self.distance_start_px
    }

    pub fn distance_end_px(&self) -> &ExactRational {
        &self.distance_end_px
    }

    pub fn raw_floor_position(&self) -> Option<&ExactRational> {
        self.raw_floor_position.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgrSemanticNote {
    kind: PgrNoteKind,
    side: PgrNoteSide,
    start_time: PgrSemanticTime,
    end_time: Option<PgrSemanticTime>,
    position_x_px: ExactRational,
    raw_speed: ExactRational,
    scroll_factor: ExactRational,
    raw_floor_position: ExactRational,
    reconstructed_floor_position: ExactRational,
    hold_tail_distance_px: Option<ExactRational>,
}

impl PgrSemanticNote {
    pub const fn kind(&self) -> PgrNoteKind {
        self.kind
    }

    pub const fn side(&self) -> PgrNoteSide {
        self.side
    }

    pub fn start_time(&self) -> &PgrSemanticTime {
        &self.start_time
    }

    pub fn end_time(&self) -> Option<&PgrSemanticTime> {
        self.end_time.as_ref()
    }

    pub fn position_x_px(&self) -> &ExactRational {
        &self.position_x_px
    }

    pub fn raw_speed(&self) -> &ExactRational {
        &self.raw_speed
    }

    pub fn scroll_factor(&self) -> &ExactRational {
        &self.scroll_factor
    }

    pub fn raw_floor_position(&self) -> &ExactRational {
        &self.raw_floor_position
    }

    pub fn reconstructed_floor_position(&self) -> &ExactRational {
        &self.reconstructed_floor_position
    }

    pub fn hold_tail_distance_px(&self) -> Option<&ExactRational> {
        self.hold_tail_distance_px.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgrSemanticLine {
    source_bpm: ExactRational,
    move_events: Vec<PgrSemanticMoveEvent>,
    rotate_events: Vec<PgrSemanticScalarEvent>,
    disappear_events: Vec<PgrSemanticScalarEvent>,
    speed_events: Vec<PgrSemanticSpeedEvent>,
    notes_above: Vec<PgrSemanticNote>,
    notes_below: Vec<PgrSemanticNote>,
}

impl PgrSemanticLine {
    pub fn source_bpm(&self) -> &ExactRational {
        &self.source_bpm
    }

    pub fn move_events(&self) -> &[PgrSemanticMoveEvent] {
        &self.move_events
    }

    pub fn rotate_events(&self) -> &[PgrSemanticScalarEvent] {
        &self.rotate_events
    }

    pub fn disappear_events(&self) -> &[PgrSemanticScalarEvent] {
        &self.disappear_events
    }

    pub fn speed_events(&self) -> &[PgrSemanticSpeedEvent] {
        &self.speed_events
    }

    pub fn notes_above(&self) -> &[PgrSemanticNote] {
        &self.notes_above
    }

    pub fn notes_below(&self) -> &[PgrSemanticNote] {
        &self.notes_below
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgrSemanticDocument {
    profile: PgrProfile,
    format_version: PgrFormatVersion,
    audio_offset_seconds: ExactRational,
    floor_scale_px: ExactRational,
    lines: Vec<PgrSemanticLine>,
}

impl PgrSemanticDocument {
    pub const fn profile(&self) -> PgrProfile {
        self.profile
    }

    pub const fn format_version(&self) -> PgrFormatVersion {
        self.format_version
    }

    pub fn audio_offset_seconds(&self) -> &ExactRational {
        &self.audio_offset_seconds
    }

    pub fn floor_scale_px(&self) -> &ExactRational {
        &self.floor_scale_px
    }

    pub fn lines(&self) -> &[PgrSemanticLine] {
        &self.lines
    }
}

pub fn parse_pgr_document(
    document: &ParsedSourceDocument,
    limits: PgrLimits,
) -> Result<PgrSourceDocument, PgrError> {
    if document.format() != SourceFormat::Pgr {
        return Err(PgrError::new(
            "conversion.unsupported-format",
            "$",
            "typed PGR parsing requires a PGR ParsedSourceDocument",
        ));
    }
    let root = object(document.body(), "$")?;
    let format_number = number(
        required(root, "formatVersion", "$")?,
        "$.formatVersion",
        limits,
    )?;
    let format_version = match format_number.exact().to_i64() {
        Some(1) => PgrFormatVersion::V1,
        Some(3) => PgrFormatVersion::V3,
        _ => {
            return Err(PgrError::new(
                UNSUPPORTED_FORMAT_VERSION,
                "$.formatVersion",
                "PGR formatVersion must be exact integer 1 or 3",
            ));
        }
    };
    let offset = number(required(root, "offset", "$")?, "$.offset", limits)?;
    let line_values = array(required(root, "judgeLineList", "$")?, "$.judgeLineList")?;
    enforce_count(
        "$.judgeLineList",
        "max_lines",
        line_values.len(),
        limits.max_lines,
    )?;
    let mut lines = Vec::with_capacity(line_values.len());
    for (index, value) in line_values.iter().enumerate() {
        lines.push(parse_line(value, index, format_version, limits)?);
    }
    Ok(PgrSourceDocument {
        format_version,
        offset,
        lines,
        unknown_fields: unknown(root, &["formatVersion", "offset", "judgeLineList"]),
    })
}

pub fn interpret_pgr(
    source: &PgrSourceDocument,
    binding: &PgrProfileBinding,
) -> Result<PgrSemanticDocument, PgrError> {
    let profile = binding.profile();
    if profile.format_version() != source.format_version {
        return Err(PgrError::new(
            PROFILE_NOT_APPLICABLE,
            "profile",
            format!(
                "profile {}@{} requires PGR formatVersion {}",
                profile.id(),
                profile.version(),
                profile.format_version().as_u8()
            ),
        ));
    }
    let first_line = source.lines.first().ok_or_else(|| {
        PgrError::new(
            SOURCE_INVALID,
            "$.judgeLineList",
            "PGR requires at least one judge Line",
        )
    })?;
    for (index, line) in source.lines.iter().enumerate() {
        validate_positive_finite(
            line.bpm.exact(),
            &format!("$.judgeLineList[{index}].bpm"),
            "Line BPM",
        )?;
    }

    let first_bpm = first_line.bpm.exact();
    let mut lines = Vec::with_capacity(source.lines.len());
    for (index, line) in source.lines.iter().enumerate() {
        let path = format!("$.judgeLineList[{index}]");
        let time_bpm = if profile.is_phira() {
            line.bpm.exact()
        } else {
            first_bpm
        };
        lines.push(interpret_line(
            line,
            &path,
            profile,
            time_bpm,
            binding.floor_scale_px(),
        )?);
    }

    Ok(PgrSemanticDocument {
        profile,
        format_version: source.format_version,
        audio_offset_seconds: source.offset.exact().clone(),
        floor_scale_px: binding.floor_scale_px().clone(),
        lines,
    })
}

fn interpret_line(
    line: &PgrSourceLine,
    path: &str,
    profile: PgrProfile,
    time_bpm: &ExactRational,
    floor_scale_px: &ExactRational,
) -> Result<PgrSemanticLine, PgrError> {
    validate_intervals(
        line.move_events
            .iter()
            .enumerate()
            .map(|(index, event)| (index, &event.start_time, &event.end_time)),
        &format!("{path}.judgeLineMoveEvents"),
    )?;
    validate_intervals(
        line.rotate_events
            .iter()
            .enumerate()
            .map(|(index, event)| (index, &event.start_time, &event.end_time)),
        &format!("{path}.judgeLineRotateEvents"),
    )?;
    validate_intervals(
        line.disappear_events
            .iter()
            .enumerate()
            .map(|(index, event)| (index, &event.start_time, &event.end_time)),
        &format!("{path}.judgeLineDisappearEvents"),
    )?;

    for (index, event) in line.disappear_events.iter().enumerate() {
        for (field, value) in [("start", &event.start), ("end", &event.end)] {
            validate_closed_unit(
                value.exact(),
                &format!("{path}.judgeLineDisappearEvents[{index}].{field}"),
            )?;
        }
    }

    let required_end = required_speed_end(line);
    let speed_model = build_speed_model(
        &line.speed_events,
        &format!("{path}.speedEvents"),
        time_bpm,
        &required_end,
    )?;

    let move_events = line
        .move_events
        .iter()
        .enumerate()
        .map(|(index, event)| {
            let event_path = format!("{path}.judgeLineMoveEvents[{index}]");
            let (start_x, start_y) = map_move_point(event, true, profile, &event_path)?;
            let (end_x, end_y) = map_move_point(event, false, profile, &event_path)?;
            Ok(PgrSemanticMoveEvent {
                start_time: semantic_time(&event.start_time, time_bpm),
                end_time: semantic_time(&event.end_time, time_bpm),
                start_x_px: start_x,
                end_x_px: end_x,
                start_y_px: start_y,
                end_y_px: end_y,
            })
        })
        .collect::<Result<Vec<_>, PgrError>>()?;

    let rotate_events = line
        .rotate_events
        .iter()
        .map(|event| PgrSemanticScalarEvent {
            start_time: semantic_time(&event.start_time, time_bpm),
            end_time: semantic_time(&event.end_time, time_bpm),
            start_value: rotation_pi(event.start.exact()),
            end_value: rotation_pi(event.end.exact()),
        })
        .collect();
    let disappear_events = line
        .disappear_events
        .iter()
        .map(|event| PgrSemanticScalarEvent {
            start_time: semantic_time(&event.start_time, time_bpm),
            end_time: semantic_time(&event.end_time, time_bpm),
            start_value: event.start.exact().clone(),
            end_value: event.end.exact().clone(),
        })
        .collect();

    let speed_events = speed_model
        .events
        .iter()
        .map(|event| PgrSemanticSpeedEvent {
            start_time: semantic_time_value(&event.start, time_bpm),
            end_time: semantic_time_value(&event.end, time_bpm),
            value: ExactRational(event.value.clone()),
            distance_start: ExactRational(event.distance_start.clone()),
            distance_end: ExactRational(event.distance_end.clone()),
            distance_start_px: ExactRational(&event.distance_start * floor_scale_px.value()),
            distance_end_px: ExactRational(&event.distance_end * floor_scale_px.value()),
            raw_floor_position: event.raw_floor_position.clone().map(ExactRational),
        })
        .collect();

    let notes_above = interpret_notes(
        &line.notes_above,
        PgrNoteSide::Above,
        &format!("{path}.notesAbove"),
        profile,
        time_bpm,
        floor_scale_px,
        &speed_model,
    )?;
    let notes_below = interpret_notes(
        &line.notes_below,
        PgrNoteSide::Below,
        &format!("{path}.notesBelow"),
        profile,
        time_bpm,
        floor_scale_px,
        &speed_model,
    )?;

    Ok(PgrSemanticLine {
        source_bpm: line.bpm.exact().clone(),
        move_events,
        rotate_events,
        disappear_events,
        speed_events,
        notes_above,
        notes_below,
    })
}

fn interpret_notes(
    notes: &[PgrSourceNote],
    side: PgrNoteSide,
    path: &str,
    profile: PgrProfile,
    time_bpm: &ExactRational,
    floor_scale_px: &ExactRational,
    speed_model: &SpeedModel,
) -> Result<Vec<PgrSemanticNote>, PgrError> {
    notes
        .iter()
        .enumerate()
        .map(|(index, note)| {
            let note_path = format!("{path}[{index}]");
            if !note.hold_time.exact().is_nonnegative() {
                return Err(PgrError::new(
                    SOURCE_INVALID,
                    format!("{note_path}.holdTime"),
                    "PGR holdTime must not be negative",
                ));
            }
            if note.kind == PgrNoteKind::Hold && !note.hold_time.exact().is_positive() {
                return Err(PgrError::new(
                    SOURCE_INVALID,
                    format!("{note_path}.holdTime"),
                    "PGR Hold end must be strictly later than its start",
                ));
            }
            if !note.speed.exact().is_nonnegative() {
                return Err(PgrError::new(
                    SOURCE_INVALID,
                    format!("{note_path}.speed"),
                    "PGR Note speed must not be negative",
                ));
            }

            let start = note.time.exact().value().clone();
            let reconstructed = speed_model.distance_at(&start, &format!("{note_path}.time"))?;
            if note.floor_position.exact().value() != &reconstructed {
                return Err(PgrError::new(
                    DISTANCE_MISMATCH,
                    format!("{note_path}.floorPosition"),
                    format!(
                        "raw floorPosition {} does not match reconstructed {}",
                        note.floor_position.exact(),
                        ExactRational(reconstructed)
                    ),
                ));
            }

            let end = if note.kind == PgrNoteKind::Hold {
                Some(&start + note.hold_time.exact().value())
            } else {
                None
            };
            let line_speed = speed_model.value_at(&start, &format!("{note_path}.time"))?;
            let raw_speed = note.speed.exact().value().clone();
            let scroll_factor = if note.kind != PgrNoteKind::Hold {
                raw_speed.clone()
            } else if profile.is_phira() {
                BigRational::one()
            } else if line_speed.is_zero() {
                BigRational::zero()
            } else {
                let phichain_line_speed = line_speed * integer(9) / integer(2);
                raw_speed.clone() * integer(9) / (phichain_line_speed * integer(2))
            };
            let hold_tail_distance_px = end
                .as_ref()
                .map(|end| {
                    speed_model
                        .distance_at(end, &format!("{note_path}.holdTime"))
                        .map(|distance| {
                            ExactRational(
                                (distance - reconstructed.clone()) * floor_scale_px.value(),
                            )
                        })
                })
                .transpose()?;

            Ok(PgrSemanticNote {
                kind: note.kind,
                side,
                start_time: semantic_time_value(&start, time_bpm),
                end_time: end
                    .as_ref()
                    .map(|value| semantic_time_value(value, time_bpm)),
                position_x_px: map_note_x(note.position_x.exact(), profile),
                raw_speed: ExactRational(raw_speed),
                scroll_factor: ExactRational(scroll_factor),
                raw_floor_position: note.floor_position.exact().clone(),
                reconstructed_floor_position: ExactRational(reconstructed),
                hold_tail_distance_px,
            })
        })
        .collect()
}

#[derive(Debug)]
struct SpeedModel {
    events: Vec<SpeedModelEvent>,
}

#[derive(Debug)]
struct SpeedModelEvent {
    start: BigRational,
    end: BigRational,
    value: BigRational,
    distance_start: BigRational,
    distance_end: BigRational,
    raw_floor_position: Option<BigRational>,
}

impl SpeedModel {
    fn event_at(&self, time: &BigRational, path: &str) -> Result<&SpeedModelEvent, PgrError> {
        self.events
            .iter()
            .enumerate()
            .find(|(index, event)| {
                &event.start <= time
                    && (time < &event.end
                        || (*index + 1 == self.events.len() && time == &event.end))
            })
            .map(|(_, event)| event)
            .ok_or_else(|| {
                PgrError::new(
                    SOURCE_INVALID,
                    path,
                    "PGR speed events do not cover this source time",
                )
            })
    }

    fn value_at(&self, time: &BigRational, path: &str) -> Result<BigRational, PgrError> {
        Ok(self.event_at(time, path)?.value.clone())
    }

    fn distance_at(&self, time: &BigRational, path: &str) -> Result<BigRational, PgrError> {
        let event = self.event_at(time, path)?;
        let duration_fraction = (time - &event.start) / (&event.end - &event.start);
        Ok(&event.distance_start
            + (&event.distance_end - &event.distance_start) * duration_fraction)
    }
}

fn build_speed_model(
    source: &[PgrSourceSpeedEvent],
    path: &str,
    time_bpm: &ExactRational,
    required_end: &BigRational,
) -> Result<SpeedModel, PgrError> {
    if source.is_empty() {
        return Err(PgrError::new(
            SOURCE_INVALID,
            path,
            "PGR requires a non-empty speed event list",
        ));
    }
    validate_intervals(
        source
            .iter()
            .enumerate()
            .map(|(index, event)| (index, &event.start_time, &event.end_time)),
        path,
    )?;
    if !source[0].start_time.exact().is_zero() {
        return Err(PgrError::new(
            SOURCE_INVALID,
            format!("{path}[0].startTime"),
            "the first PGR speed event must start at source time 0",
        ));
    }

    let seconds_per_t = integer(60) / (integer(32) * time_bpm.value());
    let mut distance = BigRational::zero();
    let mut events = Vec::with_capacity(source.len());
    for (index, event) in source.iter().enumerate() {
        if !event.value.exact().is_nonnegative() {
            return Err(PgrError::new(
                SOURCE_INVALID,
                format!("{path}[{index}].value"),
                "PGR speed value must not be negative",
            ));
        }
        if event.start_time.exact() == event.end_time.exact() {
            return Err(PgrError::new(
                SOURCE_INVALID,
                format!("{path}[{index}]"),
                "PGR speed events must have positive duration",
            ));
        }
        if index > 0 && source[index - 1].end_time.exact() != event.start_time.exact() {
            return Err(PgrError::new(
                SOURCE_INVALID,
                format!("{path}[{index}].startTime"),
                "PGR speed events must be source-ordered without gaps or overlaps",
            ));
        }
        let start = event.start_time.exact().value().clone();
        let end = event.end_time.exact().value().clone();
        let value = event.value.exact().value().clone();
        let distance_start = distance.clone();
        if let Some(raw) = &event.floor_position
            && raw.exact().value() != &distance_start
        {
            return Err(PgrError::new(
                DISTANCE_MISMATCH,
                format!("{path}[{index}].floorPosition"),
                format!(
                    "raw floorPosition {} does not match reconstructed {}",
                    raw.exact(),
                    ExactRational(distance_start)
                ),
            ));
        }
        distance += (&end - &start) * &value * &seconds_per_t;
        events.push(SpeedModelEvent {
            start,
            end,
            value,
            distance_start,
            distance_end: distance.clone(),
            raw_floor_position: event
                .floor_position
                .as_ref()
                .map(|value| value.exact().value().clone()),
        });
    }
    if events.last().is_none_or(|event| &event.end < required_end) {
        return Err(PgrError::new(
            SOURCE_INVALID,
            path,
            "PGR speed events do not cover all Note and Line event times",
        ));
    }
    Ok(SpeedModel { events })
}

fn required_speed_end(line: &PgrSourceLine) -> BigRational {
    let mut required = BigRational::zero();
    for value in line
        .move_events
        .iter()
        .map(|event| event.end_time.exact().value())
        .chain(
            line.rotate_events
                .iter()
                .map(|event| event.end_time.exact().value()),
        )
        .chain(
            line.disappear_events
                .iter()
                .map(|event| event.end_time.exact().value()),
        )
    {
        if value > &required {
            required = value.clone();
        }
    }
    for note in line.notes_above.iter().chain(&line.notes_below) {
        let end = if note.kind == PgrNoteKind::Hold {
            note.time.exact().value() + note.hold_time.exact().value()
        } else {
            note.time.exact().value().clone()
        };
        if end > required {
            required = end;
        }
    }
    required
}

fn validate_intervals<'a>(
    events: impl IntoIterator<Item = (usize, &'a ExactDecimal, &'a ExactDecimal)>,
    path: &str,
) -> Result<(), PgrError> {
    let mut previous_end: Option<&ExactRational> = None;
    for (index, start, end) in events {
        if start.exact() > end.exact() {
            return Err(PgrError::new(
                SOURCE_INVALID,
                format!("{path}[{index}]"),
                "PGR event startTime must not exceed endTime",
            ));
        }
        if previous_end.is_some_and(|previous| start.exact() < previous) {
            return Err(PgrError::new(
                SOURCE_INVALID,
                format!("{path}[{index}].startTime"),
                "PGR events must remain source-ordered and must not overlap",
            ));
        }
        previous_end = Some(end.exact());
    }
    Ok(())
}

fn validate_positive_finite(value: &ExactRational, path: &str, name: &str) -> Result<(), PgrError> {
    if value.is_positive() && value.to_f64().is_ok() {
        Ok(())
    } else {
        Err(PgrError::new(
            SOURCE_INVALID,
            path,
            format!("{name} must be finite and positive"),
        ))
    }
}

fn validate_closed_unit(value: &ExactRational, path: &str) -> Result<(), PgrError> {
    if value.is_nonnegative() && value.value() <= &BigRational::one() {
        Ok(())
    } else {
        Err(PgrError::new(
            SOURCE_INVALID,
            path,
            "PGR alpha must be in the closed interval 0..1",
        ))
    }
}

fn semantic_time(raw: &ExactDecimal, bpm: &ExactRational) -> PgrSemanticTime {
    semantic_time_value(raw.exact().value(), bpm)
}

fn semantic_time_value(raw: &BigRational, bpm: &ExactRational) -> PgrSemanticTime {
    PgrSemanticTime {
        source_line_beat: ExactRational(raw / integer(32)),
        chart_time_seconds: ExactRational(raw * integer(60) / (integer(32) * bpm.value())),
    }
}

fn rotation_pi(degrees: &ExactRational) -> ExactRational {
    ExactRational(-degrees.value() / integer(180))
}

fn map_note_x(value: &ExactRational, profile: PgrProfile) -> ExactRational {
    let scale = if profile.is_phira() {
        integer(108)
    } else {
        integer(320) / integer(3)
    };
    ExactRational(value.value() * scale)
}

fn map_move_point(
    event: &PgrSourceMoveEvent,
    start: bool,
    profile: PgrProfile,
    path: &str,
) -> Result<(ExactRational, ExactRational), PgrError> {
    let (x, y, suffix) = if start {
        (&event.start_x, event.start_y.as_ref(), "start")
    } else {
        (&event.end_x, event.end_y.as_ref(), "end")
    };
    if profile.format_version() == PgrFormatVersion::V1 {
        map_v1_point(x.exact(), profile, &format!("{path}.{suffix}"))
    } else {
        let Some(y) = y else {
            return Err(PgrError::new(
                SOURCE_INVALID,
                format!("{path}.{suffix}2"),
                "PGR v3 move event is missing its Y coordinate",
            ));
        };
        validate_normalized(x.exact(), &format!("{path}.{suffix}"))?;
        validate_normalized(y.exact(), &format!("{path}.{suffix}2"))?;
        Ok((
            ExactRational((x.exact().value() - half()) * integer(1920)),
            ExactRational((y.exact().value() - half()) * integer(1080)),
        ))
    }
}

fn map_v1_point(
    packed: &ExactRational,
    profile: PgrProfile,
    path: &str,
) -> Result<(ExactRational, ExactRational), PgrError> {
    if !packed.is_nonnegative() {
        return Err(PgrError::new(
            SOURCE_INVALID,
            path,
            "PGR v1 packed coordinate must not be negative",
        ));
    }
    let quotient = packed.value() / integer(1000);
    let truncated = quotient.to_integer();
    let remainder = packed.value() - BigRational::from_integer(truncated.clone()) * integer(1000);
    let x_units = if profile == PgrProfile::PhichainImportV1 {
        round_ties_away_nonnegative(&quotient)
    } else {
        truncated
    };
    let y_base = if profile == PgrProfile::PhichainImportV1 {
        530
    } else {
        520
    };
    if x_units > BigInt::from(880) || remainder > integer(y_base) {
        return Err(PgrError::new(
            SOURCE_INVALID,
            path,
            "PGR v1 packed coordinate is outside the declared canvas",
        ));
    }
    Ok((
        ExactRational((BigRational::from_integer(x_units) / integer(880) - half()) * integer(1920)),
        ExactRational((remainder / integer(y_base) - half()) * integer(1080)),
    ))
}

fn round_ties_away_nonnegative(value: &BigRational) -> BigInt {
    let integer_part = value.to_integer();
    let fraction = value - BigRational::from_integer(integer_part.clone());
    if fraction >= half() {
        integer_part + BigInt::one()
    } else {
        integer_part
    }
}

fn validate_normalized(value: &ExactRational, path: &str) -> Result<(), PgrError> {
    if value.is_nonnegative() && value.value() <= &BigRational::one() {
        Ok(())
    } else {
        Err(PgrError::new(
            SOURCE_INVALID,
            path,
            "PGR v3 normalized coordinate must be in the closed interval 0..1",
        ))
    }
}

fn integer(value: i64) -> BigRational {
    BigRational::from_integer(BigInt::from(value))
}

fn half() -> BigRational {
    BigRational::new(BigInt::one(), BigInt::from(2))
}

fn parse_line(
    value: &LosslessJsonValue,
    index: usize,
    format_version: PgrFormatVersion,
    limits: PgrLimits,
) -> Result<PgrSourceLine, PgrError> {
    let path = format!("$.judgeLineList[{index}]");
    let members = object(value, &path)?;
    let move_events = parse_array_field(
        members,
        "judgeLineMoveEvents",
        &path,
        limits.max_events_per_line,
        |value, index, path| parse_move_event(value, index, path, format_version, limits),
    )?;
    let rotate_events = parse_array_field(
        members,
        "judgeLineRotateEvents",
        &path,
        limits.max_events_per_line,
        |value, index, path| parse_scalar_event(value, index, path, limits),
    )?;
    let disappear_events = parse_array_field(
        members,
        "judgeLineDisappearEvents",
        &path,
        limits.max_events_per_line,
        |value, index, path| parse_scalar_event(value, index, path, limits),
    )?;
    let speed_events = parse_array_field(
        members,
        "speedEvents",
        &path,
        limits.max_events_per_line,
        |value, index, path| parse_speed_event(value, index, path, limits),
    )?;
    let notes_above = parse_array_field(
        members,
        "notesAbove",
        &path,
        limits.max_notes_per_line,
        |value, index, path| parse_note(value, index, path, limits),
    )?;
    let notes_below = parse_array_field(
        members,
        "notesBelow",
        &path,
        limits.max_notes_per_line,
        |value, index, path| parse_note(value, index, path, limits),
    )?;
    Ok(PgrSourceLine {
        bpm: number(
            required(members, "bpm", &path)?,
            &format!("{path}.bpm"),
            limits,
        )?,
        move_events,
        rotate_events,
        disappear_events,
        speed_events,
        notes_above,
        notes_below,
        unknown_fields: unknown(
            members,
            &[
                "bpm",
                "judgeLineMoveEvents",
                "judgeLineRotateEvents",
                "judgeLineDisappearEvents",
                "speedEvents",
                "notesAbove",
                "notesBelow",
            ],
        ),
    })
}

fn parse_scalar_event(
    value: &LosslessJsonValue,
    index: usize,
    parent: &str,
    limits: PgrLimits,
) -> Result<PgrSourceScalarEvent, PgrError> {
    let path = format!("{parent}[{index}]");
    let members = object(value, &path)?;
    Ok(PgrSourceScalarEvent {
        start_time: number(
            required(members, "startTime", &path)?,
            &format!("{path}.startTime"),
            limits,
        )?,
        end_time: number(
            required(members, "endTime", &path)?,
            &format!("{path}.endTime"),
            limits,
        )?,
        start: number(
            required(members, "start", &path)?,
            &format!("{path}.start"),
            limits,
        )?,
        end: number(
            required(members, "end", &path)?,
            &format!("{path}.end"),
            limits,
        )?,
        unknown_fields: unknown(members, &["startTime", "endTime", "start", "end"]),
    })
}

fn parse_move_event(
    value: &LosslessJsonValue,
    index: usize,
    parent: &str,
    format_version: PgrFormatVersion,
    limits: PgrLimits,
) -> Result<PgrSourceMoveEvent, PgrError> {
    let path = format!("{parent}[{index}]");
    let members = object(value, &path)?;
    let (start_y, end_y) = match format_version {
        PgrFormatVersion::V1 => (None, None),
        PgrFormatVersion::V3 => (
            Some(number(
                required(members, "start2", &path)?,
                &format!("{path}.start2"),
                limits,
            )?),
            Some(number(
                required(members, "end2", &path)?,
                &format!("{path}.end2"),
                limits,
            )?),
        ),
    };
    Ok(PgrSourceMoveEvent {
        start_time: number(
            required(members, "startTime", &path)?,
            &format!("{path}.startTime"),
            limits,
        )?,
        end_time: number(
            required(members, "endTime", &path)?,
            &format!("{path}.endTime"),
            limits,
        )?,
        start_x: number(
            required(members, "start", &path)?,
            &format!("{path}.start"),
            limits,
        )?,
        end_x: number(
            required(members, "end", &path)?,
            &format!("{path}.end"),
            limits,
        )?,
        start_y,
        end_y,
        unknown_fields: unknown(
            members,
            if format_version == PgrFormatVersion::V3 {
                &["startTime", "endTime", "start", "end", "start2", "end2"]
            } else {
                &["startTime", "endTime", "start", "end"]
            },
        ),
    })
}

fn parse_speed_event(
    value: &LosslessJsonValue,
    index: usize,
    parent: &str,
    limits: PgrLimits,
) -> Result<PgrSourceSpeedEvent, PgrError> {
    let path = format!("{parent}[{index}]");
    let members = object(value, &path)?;
    Ok(PgrSourceSpeedEvent {
        start_time: number(
            required(members, "startTime", &path)?,
            &format!("{path}.startTime"),
            limits,
        )?,
        end_time: number(
            required(members, "endTime", &path)?,
            &format!("{path}.endTime"),
            limits,
        )?,
        value: number(
            required(members, "value", &path)?,
            &format!("{path}.value"),
            limits,
        )?,
        floor_position: optional_number(members, "floorPosition", &path, limits)?,
        unknown_fields: unknown(members, &["startTime", "endTime", "value", "floorPosition"]),
    })
}

fn parse_note(
    value: &LosslessJsonValue,
    index: usize,
    parent: &str,
    limits: PgrLimits,
) -> Result<PgrSourceNote, PgrError> {
    let path = format!("{parent}[{index}]");
    let members = object(value, &path)?;
    let kind_number = number(
        required(members, "type", &path)?,
        &format!("{path}.type"),
        limits,
    )?;
    let kind = match kind_number.exact().to_i64() {
        Some(1) => PgrNoteKind::Tap,
        Some(2) => PgrNoteKind::Drag,
        Some(3) => PgrNoteKind::Hold,
        Some(4) => PgrNoteKind::Flick,
        _ => {
            return Err(PgrError::new(
                SOURCE_INVALID,
                format!("{path}.type"),
                "PGR Note type must be exact integer 1, 2, 3, or 4",
            ));
        }
    };
    Ok(PgrSourceNote {
        kind,
        time: number(
            required(members, "time", &path)?,
            &format!("{path}.time"),
            limits,
        )?,
        hold_time: number(
            required(members, "holdTime", &path)?,
            &format!("{path}.holdTime"),
            limits,
        )?,
        position_x: number(
            required(members, "positionX", &path)?,
            &format!("{path}.positionX"),
            limits,
        )?,
        speed: number(
            required(members, "speed", &path)?,
            &format!("{path}.speed"),
            limits,
        )?,
        floor_position: number(
            required(members, "floorPosition", &path)?,
            &format!("{path}.floorPosition"),
            limits,
        )?,
        unknown_fields: unknown(
            members,
            &[
                "type",
                "time",
                "holdTime",
                "positionX",
                "speed",
                "floorPosition",
            ],
        ),
    })
}

fn parse_array_field<T>(
    members: &[LosslessJsonMember],
    field: &str,
    parent: &str,
    limit: usize,
    mut parse: impl FnMut(&LosslessJsonValue, usize, &str) -> Result<T, PgrError>,
) -> Result<Vec<T>, PgrError> {
    let path = format!("{parent}.{field}");
    let values = array(required(members, field, parent)?, &path)?;
    enforce_count(&path, "max_items_per_line_field", values.len(), limit)?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| parse(value, index, &path))
        .collect()
}

fn object<'a>(
    value: &'a LosslessJsonValue,
    path: &str,
) -> Result<&'a [LosslessJsonMember], PgrError> {
    value
        .as_object()
        .ok_or_else(|| PgrError::new(SOURCE_INVALID, path, "expected a JSON object"))
}

fn array<'a>(
    value: &'a LosslessJsonValue,
    path: &str,
) -> Result<&'a [LosslessJsonValue], PgrError> {
    value
        .as_array()
        .ok_or_else(|| PgrError::new(SOURCE_INVALID, path, "expected a JSON array"))
}

fn required<'a>(
    members: &'a [LosslessJsonMember],
    key: &str,
    parent: &str,
) -> Result<&'a LosslessJsonValue, PgrError> {
    let mut matches = members.iter().filter(|member| member.key() == key);
    let value = matches.next().ok_or_else(|| {
        PgrError::new(
            SOURCE_INVALID,
            format!("{parent}.{key}"),
            "required PGR field is missing",
        )
    })?;
    if matches.next().is_some() {
        return Err(PgrError::new(
            SOURCE_INVALID,
            format!("{parent}.{key}"),
            "known PGR field is duplicated",
        ));
    }
    Ok(value.value())
}

fn optional<'a>(
    members: &'a [LosslessJsonMember],
    key: &str,
    parent: &str,
) -> Result<Option<&'a LosslessJsonValue>, PgrError> {
    let mut matches = members.iter().filter(|member| member.key() == key);
    let value = matches.next();
    if matches.next().is_some() {
        return Err(PgrError::new(
            SOURCE_INVALID,
            format!("{parent}.{key}"),
            "known PGR field is duplicated",
        ));
    }
    Ok(value.map(LosslessJsonMember::value))
}

fn number(
    value: &LosslessJsonValue,
    path: &str,
    limits: PgrLimits,
) -> Result<ExactDecimal, PgrError> {
    let raw = value
        .as_number_lexeme()
        .ok_or_else(|| PgrError::new(SOURCE_INVALID, path, "expected an exact JSON number"))?;
    ExactDecimal::parse(raw, limits.decimal).map_err(|error| PgrError::from_exact(path, error))
}

fn optional_number(
    members: &[LosslessJsonMember],
    key: &str,
    parent: &str,
    limits: PgrLimits,
) -> Result<Option<ExactDecimal>, PgrError> {
    optional(members, key, parent)?
        .map(|value| number(value, &format!("{parent}.{key}"), limits))
        .transpose()
}

fn unknown(members: &[LosslessJsonMember], known: &[&str]) -> Vec<LosslessJsonMember> {
    members
        .iter()
        .filter(|member| !known.contains(&member.key()))
        .cloned()
        .collect()
}

fn enforce_count(
    path: &str,
    kind: &'static str,
    observed: usize,
    limit: usize,
) -> Result<(), PgrError> {
    if observed > limit {
        Err(PgrError::new(
            SOURCE_INVALID,
            path,
            format!("PGR limit {kind} exceeded: limit {limit}, observed {observed}"),
        ))
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgrError {
    category: &'static str,
    path: String,
    message: String,
}

impl PgrError {
    fn new(category: &'static str, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            category,
            path: path.into(),
            message: message.into(),
        }
    }

    fn from_exact(path: &str, error: ExactNumberError) -> Self {
        Self::new(SOURCE_INVALID, path, error.to_string())
    }

    pub const fn category(&self) -> &'static str {
        self.category
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for PgrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at {}: {}",
            self.category, self.path, self.message
        )
    }
}

impl std::error::Error for PgrError {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde::Deserialize;

    use super::*;
    use crate::{ArtifactRole, SourceArtifact, parse_json_document};

    const V1_CHART: &str = r#"{
        "formatVersion": 1,
        "offset": 0.125,
        "rootUnknownA": 1,
        "rootUnknownB": 2,
        "judgeLineList": [
            {
                "bpm": 120,
                "judgeLineMoveEvents": [
                    {"startTime": 0, "endTime": 32, "start": 440260, "end": 440500, "moveUnknown": 1}
                ],
                "judgeLineRotateEvents": [
                    {"startTime": 0, "endTime": 32, "start": 0, "end": 90}
                ],
                "judgeLineDisappearEvents": [
                    {"startTime": 0, "endTime": 32, "start": 0, "end": 1}
                ],
                "speedEvents": [
                    {"startTime": 0, "endTime": 64, "value": 2, "floorPosition": 0}
                ],
                "notesAbove": [
                    {"type": 3, "time": 32, "holdTime": 32, "positionX": 1, "speed": 4, "floorPosition": 1, "noteUnknownA": 1, "noteUnknownB": 2}
                ],
                "notesBelow": [
                    {"type": 1, "time": 0, "holdTime": 0, "positionX": -1, "speed": 2, "floorPosition": 0}
                ],
                "lineUnknown": true
            },
            {
                "bpm": 60,
                "judgeLineMoveEvents": [
                    {"startTime": 0, "endTime": 32, "start": 440260, "end": 440260}
                ],
                "judgeLineRotateEvents": [],
                "judgeLineDisappearEvents": [],
                "speedEvents": [
                    {"startTime": 0, "endTime": 64, "value": 1, "floorPosition": 0}
                ],
                "notesAbove": [],
                "notesBelow": []
            }
        ]
    }"#;

    const V3_CHART: &str = r#"{
        "formatVersion": 3,
        "offset": 0,
        "judgeLineList": [{
            "bpm": 120,
            "judgeLineMoveEvents": [
                {"startTime": 0, "endTime": 32, "start": 0.75, "end": 0.5, "start2": 0.25, "end2": 0.5}
            ],
            "judgeLineRotateEvents": [],
            "judgeLineDisappearEvents": [],
            "speedEvents": [{"startTime": 0, "endTime": 32, "value": 1, "floorPosition": 0}],
            "notesAbove": [],
            "notesBelow": []
        }]
    }"#;

    fn decimal(raw: &str) -> ExactDecimal {
        ExactDecimal::parse(raw, DecimalLimits::default()).unwrap()
    }

    fn typed(json: &str, limits: PgrLimits) -> Result<PgrSourceDocument, PgrError> {
        let artifact =
            SourceArtifact::new("chart.json", ArtifactRole::Chart, json.as_bytes().to_vec())
                .unwrap();
        let document = parse_json_document(SourceFormat::Pgr, &artifact).unwrap();
        parse_pgr_document(&document, limits)
    }

    fn binding(profile: PgrProfile) -> PgrProfileBinding {
        PgrProfileBinding::new(profile, decimal("120")).unwrap()
    }

    fn exact(expected: &str) -> ExactRational {
        let (numerator, denominator) = expected.split_once('/').unwrap_or((expected, "1"));
        ExactRational(BigRational::new(
            BigInt::parse_bytes(numerator.as_bytes(), 10).unwrap(),
            BigInt::parse_bytes(denominator.as_bytes(), 10).unwrap(),
        ))
    }

    #[test]
    fn typed_parser_preserves_exact_values_and_unknown_order_and_rejects_known_duplicates() {
        let source = typed(V1_CHART, PgrLimits::default()).unwrap();
        assert_eq!(source.format_version(), PgrFormatVersion::V1);
        assert_eq!(source.offset().raw(), "0.125");
        assert_eq!(
            source
                .unknown_fields()
                .iter()
                .map(LosslessJsonMember::key)
                .collect::<Vec<_>>(),
            ["rootUnknownA", "rootUnknownB"]
        );
        assert_eq!(source.lines()[0].unknown_fields()[0].key(), "lineUnknown");
        assert_eq!(
            source.lines()[0].move_events()[0].unknown_fields()[0].key(),
            "moveUnknown"
        );
        assert_eq!(
            source.lines()[0].notes_above()[0]
                .unknown_fields()
                .iter()
                .map(LosslessJsonMember::key)
                .collect::<Vec<_>>(),
            ["noteUnknownA", "noteUnknownB"]
        );

        let duplicate =
            V1_CHART.replacen(r#""offset": 0.125"#, r#""offset": 0.125, "offset": 0"#, 1);
        let error = typed(&duplicate, PgrLimits::default()).unwrap_err();
        assert_eq!(error.category(), SOURCE_INVALID);
        assert_eq!(error.path(), "$.offset");

        let wrong_shape = r#"{"formatVersion":1,"offset":0,"judgeLineList":{}}"#;
        assert_eq!(
            typed(wrong_shape, PgrLimits::default())
                .unwrap_err()
                .category(),
            SOURCE_INVALID
        );
    }

    #[test]
    fn four_explicit_profiles_expose_timing_coordinate_note_and_hold_differences() {
        let source = typed(V1_CHART, PgrLimits::default()).unwrap();
        let phira = interpret_pgr(&source, &binding(PgrProfile::PhiraV1)).unwrap();
        let phichain = interpret_pgr(&source, &binding(PgrProfile::PhichainImportV1)).unwrap();

        assert_eq!(phira.audio_offset_seconds(), &exact("1/8"));
        assert_eq!(phira.lines()[0].move_events()[0].end_x_px(), &exact("0"));
        assert_eq!(
            phichain.lines()[0].move_events()[0].end_x_px(),
            &exact("24/11")
        );
        assert_eq!(phira.lines()[0].move_events()[0].start_y_px(), &exact("0"));
        assert_eq!(
            phichain.lines()[0].move_events()[0].start_y_px(),
            &exact("-540/53")
        );
        assert_eq!(
            phira.lines()[0].notes_above()[0].position_x_px(),
            &exact("108")
        );
        assert_eq!(
            phichain.lines()[0].notes_above()[0].position_x_px(),
            &exact("320/3")
        );
        assert_eq!(
            phira.lines()[0].notes_above()[0].scroll_factor(),
            &exact("1")
        );
        assert_eq!(
            phichain.lines()[0].notes_above()[0].scroll_factor(),
            &exact("2")
        );
        assert_eq!(
            phira.lines()[0].notes_above()[0].hold_tail_distance_px(),
            Some(&exact("120"))
        );
        assert_eq!(
            phira.lines()[0].rotate_events()[0].end_value(),
            &exact("-1/2")
        );
        assert_eq!(
            phira.lines()[0].speed_events()[0].distance_end_px(),
            &exact("240")
        );
        assert_eq!(
            phira.lines()[1].move_events()[0]
                .end_time()
                .chart_time_seconds(),
            &exact("1")
        );
        assert_eq!(
            phichain.lines()[1].move_events()[0]
                .end_time()
                .chart_time_seconds(),
            &exact("1/2")
        );
        assert!(PgrProfile::PhiraV1.strict_eligible());
        assert!(!PgrProfile::PhichainImportV1.strict_eligible());
    }

    #[test]
    fn v3_profiles_require_split_normalized_coordinates_and_matching_versions() {
        let source = typed(V3_CHART, PgrLimits::default()).unwrap();
        let semantic = interpret_pgr(&source, &binding(PgrProfile::PhiraV3)).unwrap();
        let phichain = interpret_pgr(&source, &binding(PgrProfile::PhichainImportV3)).unwrap();
        let movement = &semantic.lines()[0].move_events()[0];
        assert_eq!(movement.start_x_px(), &exact("480"));
        assert_eq!(movement.start_y_px(), &exact("-270"));
        assert_eq!(phichain.profile(), PgrProfile::PhichainImportV3);
        assert_eq!(
            interpret_pgr(&source, &binding(PgrProfile::PhiraV1))
                .unwrap_err()
                .category(),
            PROFILE_NOT_APPLICABLE
        );

        let missing_y = V3_CHART.replacen(r#", "start2": 0.25, "end2": 0.5"#, "", 1);
        assert_eq!(
            typed(&missing_y, PgrLimits::default())
                .unwrap_err()
                .category(),
            SOURCE_INVALID
        );
        let out_of_range = V3_CHART.replacen(r#""start": 0.75"#, r#""start": 1.1"#, 1);
        assert_eq!(
            interpret_pgr(
                &typed(&out_of_range, PgrLimits::default()).unwrap(),
                &binding(PgrProfile::PhiraV3)
            )
            .unwrap_err()
            .category(),
            SOURCE_INVALID
        );
    }

    #[test]
    fn semantic_validation_rejects_invalid_intervals_speed_hold_coordinates_alpha_and_caches() {
        let cases = [
            (V1_CHART.replacen(r#""bpm": 120"#, r#""bpm": 0"#, 1), SOURCE_INVALID),
            (
                V1_CHART.replacen(
                    r#""startTime": 0, "endTime": 32, "start": 440260"#,
                    r#""startTime": 33, "endTime": 32, "start": 440260"#,
                    1,
                ),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(
                    r#"{"startTime": 0, "endTime": 32, "start": 440260, "end": 440500, "moveUnknown": 1}"#,
                    r#"{"startTime": 0, "endTime": 32, "start": 440260, "end": 440500}, {"startTime": 16, "endTime": 48, "start": 440260, "end": 440260}"#,
                    1,
                ),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(r#""end": 1}"#, r#""end": 2}"#, 1),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(
                    r#"{"startTime": 0, "endTime": 64, "value": 2, "floorPosition": 0}"#,
                    "",
                    1,
                ),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(
                    r#"{"startTime": 0, "endTime": 64, "value": 2, "floorPosition": 0}"#,
                    r#"{"startTime": 0, "endTime": 16, "value": 2, "floorPosition": 0}, {"startTime": 32, "endTime": 64, "value": 2}"#,
                    1,
                ),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(r#""value": 2, "floorPosition": 0"#, r#""value": -1, "floorPosition": 0"#, 1),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(r#""holdTime": 32"#, r#""holdTime": 0"#, 1),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(r#""start": 440260"#, r#""start": -1"#, 1),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(r#""start": 440260"#, r#""start": 440999"#, 1),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(r#""speed": 4, "floorPosition": 1"#, r#""speed": 4, "floorPosition": 2"#, 1),
                DISTANCE_MISMATCH,
            ),
        ];
        for (json, category) in cases {
            let source = typed(&json, PgrLimits::default()).unwrap();
            let error = interpret_pgr(&source, &binding(PgrProfile::PhiraV1)).unwrap_err();
            assert_eq!(error.category(), category, "{error}");
        }
    }

    #[test]
    fn decimal_entity_and_profile_parameter_limits_cover_their_boundaries() {
        let source = typed(V1_CHART, PgrLimits::default()).unwrap();
        assert_eq!(source.lines().len(), 2);
        let line_limit = PgrLimits {
            max_lines: 1,
            ..PgrLimits::default()
        };
        assert_eq!(
            typed(V1_CHART, line_limit).unwrap_err().category(),
            SOURCE_INVALID
        );
        let event_limit = PgrLimits {
            max_events_per_line: 0,
            ..PgrLimits::default()
        };
        assert_eq!(
            typed(V1_CHART, event_limit).unwrap_err().category(),
            SOURCE_INVALID
        );
        let note_limit = PgrLimits {
            max_notes_per_line: 0,
            ..PgrLimits::default()
        };
        assert_eq!(
            typed(V1_CHART, note_limit).unwrap_err().category(),
            SOURCE_INVALID
        );

        for raw in ["0", "-1", "1e4096"] {
            assert_eq!(
                PgrProfileBinding::new(PgrProfile::PhiraV1, decimal(raw))
                    .unwrap_err()
                    .category(),
                PROFILE_PARAMETER_INVALID
            );
        }
    }

    #[derive(Deserialize)]
    struct MappingCorpus {
        vector: Vec<MappingVector>,
    }

    #[derive(Deserialize)]
    struct MappingVector {
        id: String,
        rule_id: String,
        source: BTreeMap<String, toml::Value>,
        expected: String,
    }

    fn source_decimal<'a>(vector: &'a MappingVector, key: &str) -> &'a str {
        vector.source[key].as_str().unwrap()
    }

    #[test]
    fn checked_in_pgr_source_semantic_mapping_vectors_execute_exactly() {
        let corpus: MappingCorpus = toml::from_str(include_str!(
            "../../../docs/conformance/conversion/mapping-vectors.toml"
        ))
        .unwrap();
        let mut executed = 0;
        for vector in &corpus.vector {
            let actual = match vector.rule_id.as_str() {
                "pgr.time.source-line-beat-t32" => ExactRational(
                    decimal(source_decimal(vector, "T")).exact().value() / integer(32),
                ),
                "pgr.time.per-line-bpm" => {
                    semantic_time_value(
                        decimal(source_decimal(vector, "T")).exact().value(),
                        decimal(source_decimal(vector, "current_line_bpm")).exact(),
                    )
                    .chart_time_seconds
                }
                "pgr.time.first-line-bpm" => {
                    semantic_time_value(
                        decimal(source_decimal(vector, "T")).exact().value(),
                        decimal(source_decimal(vector, "first_line_bpm")).exact(),
                    )
                    .chart_time_seconds
                }
                "pgr.note-x.unit108" => map_note_x(
                    decimal(source_decimal(vector, "x")).exact(),
                    PgrProfile::PhiraV1,
                ),
                "pgr.note-x.unit320_3" => map_note_x(
                    decimal(source_decimal(vector, "x")).exact(),
                    PgrProfile::PhichainImportV1,
                ),
                "pgr.line-x.normalized" => ExactRational(
                    (decimal(source_decimal(vector, "x")).exact().value() - half()) * integer(1920),
                ),
                "pgr.line-y.normalized" => ExactRational(
                    (decimal(source_decimal(vector, "y")).exact().value() - half()) * integer(1080),
                ),
                "pgr.v1-move-x.trunc1000-div880" => {
                    map_v1_point(
                        decimal(source_decimal(vector, "packed")).exact(),
                        PgrProfile::PhiraV1,
                        &vector.id,
                    )
                    .unwrap()
                    .0
                }
                "pgr.v1-move-x.round1000-div880" => {
                    map_v1_point(
                        decimal(source_decimal(vector, "packed")).exact(),
                        PgrProfile::PhichainImportV1,
                        &vector.id,
                    )
                    .unwrap()
                    .0
                }
                "pgr.v1-move-y.mod1000-div520" => {
                    map_v1_point(
                        decimal(source_decimal(vector, "packed")).exact(),
                        PgrProfile::PhiraV1,
                        &vector.id,
                    )
                    .unwrap()
                    .1
                }
                "pgr.v1-move-y.mod1000-div530" => {
                    map_v1_point(
                        decimal(source_decimal(vector, "packed")).exact(),
                        PgrProfile::PhichainImportV1,
                        &vector.id,
                    )
                    .unwrap()
                    .1
                }
                "pgr.offset.seconds" => decimal(source_decimal(vector, "offset_seconds"))
                    .exact()
                    .clone(),
                _ => continue,
            };
            assert_eq!(actual, exact(&vector.expected), "{}", vector.id);
            executed += 1;
        }
        assert_eq!(executed, 12);
    }
}
