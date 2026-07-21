//! PEC line-command import: parse, profile-bound semantics, and mapping helpers (I6.4).
//!
//! Primary dialect is Phira `pec.line-command` (one command per physical line after the
//! first-line offset). Token-stream and global-suffix-zip dialects remain profile-tagged
//! compatibility paths that fail strict parse when their shape is required but not used.

use std::fmt;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};

use crate::{
    DecimalLimits, ExactDecimal, ExactNumberError, ExactRational, LogicalSourceLocator,
    SourceArtifact, SourceFormat,
};

pub const SOURCE_INVALID: &str = "conversion.source-invalid";
pub const PROFILE_PARAMETER_INVALID: &str = "conversion.profile-parameter-invalid";
#[allow(dead_code)]
pub const PROFILE_NOT_APPLICABLE: &str = "conversion.profile-not-applicable";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PecLimits {
    pub decimal: DecimalLimits,
    pub max_commands: usize,
    pub max_lines: usize,
    pub max_notes: usize,
}

impl Default for PecLimits {
    fn default() -> Self {
        Self {
            decimal: DecimalLimits::default(),
            max_commands: 1_048_576,
            max_lines: 4096,
            max_notes: 1_048_576,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PecProfile {
    Phira,
    Extends,
    Phispler,
}

impl PecProfile {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Phira => "pec.phira",
            Self::Extends => "pec.extends",
            Self::Phispler => "pec.phispler",
        }
    }

    pub const fn version(self) -> &'static str {
        "1.0.0"
    }

    pub const fn offset_bias_ms(self) -> i64 {
        match self {
            Self::Phira | Self::Phispler => 150,
            Self::Extends => 175,
        }
    }

    pub const fn strict_eligible(self) -> bool {
        matches!(self, Self::Phira)
    }

    pub const fn cv_scale(self) -> PecCvScale {
        match self {
            Self::Phira => PecCvScale::Div585,
            Self::Extends => PecCvScale::Div7,
            Self::Phispler => PecCvScale::RpeHeight900,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PecCvScale {
    Div585,
    Div7,
    RpeHeight900,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PecProfileBinding {
    profile: PecProfile,
    floor_scale_px: ExactRational,
}

impl PecProfileBinding {
    pub fn new(profile: PecProfile, floor_scale_px: ExactDecimal) -> Result<Self, PecError> {
        if !floor_scale_px.exact().is_positive() || floor_scale_px.to_f64().is_err() {
            return Err(PecError::new(
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

    pub const fn profile(&self) -> PecProfile {
        self.profile
    }

    pub fn floor_scale_px(&self) -> &ExactRational {
        &self.floor_scale_px
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PecNoteKind {
    Tap,
    Hold,
    Flick,
    Drag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PecNoteSide {
    Above,
    Below,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PecSourceBpm {
    beat: ExactDecimal,
    bpm: ExactDecimal,
    line: usize,
}

impl PecSourceBpm {
    pub fn beat(&self) -> &ExactDecimal {
        &self.beat
    }

    pub fn bpm(&self) -> &ExactDecimal {
        &self.bpm
    }

    pub const fn line(&self) -> usize {
        self.line
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PecSourceNote {
    kind: PecNoteKind,
    line_index: ExactDecimal,
    start_beat: ExactDecimal,
    end_beat: Option<ExactDecimal>,
    x: ExactDecimal,
    side: ExactDecimal,
    fake: ExactDecimal,
    speed: Option<ExactDecimal>,
    width: Option<ExactDecimal>,
    line: usize,
}

impl PecSourceNote {
    pub const fn kind(&self) -> PecNoteKind {
        self.kind
    }

    pub fn line_index(&self) -> &ExactDecimal {
        &self.line_index
    }

    pub fn start_beat(&self) -> &ExactDecimal {
        &self.start_beat
    }

    pub fn end_beat(&self) -> Option<&ExactDecimal> {
        self.end_beat.as_ref()
    }

    pub fn x(&self) -> &ExactDecimal {
        &self.x
    }

    pub fn side(&self) -> &ExactDecimal {
        &self.side
    }

    pub fn fake(&self) -> &ExactDecimal {
        &self.fake
    }

    pub fn speed(&self) -> Option<&ExactDecimal> {
        self.speed.as_ref()
    }

    pub fn width(&self) -> Option<&ExactDecimal> {
        self.width.as_ref()
    }

    pub const fn line(&self) -> usize {
        self.line
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PecSourceCommand {
    Bpm(PecSourceBpm),
    Note(PecSourceNote),
    Cv {
        line_index: ExactDecimal,
        beat: ExactDecimal,
        speed: ExactDecimal,
        line: usize,
    },
    Cp {
        line_index: ExactDecimal,
        beat: ExactDecimal,
        x: ExactDecimal,
        y: ExactDecimal,
        line: usize,
    },
    Cd {
        line_index: ExactDecimal,
        beat: ExactDecimal,
        angle: ExactDecimal,
        line: usize,
    },
    Ca {
        line_index: ExactDecimal,
        beat: ExactDecimal,
        alpha: ExactDecimal,
        line: usize,
    },
    Cm {
        line_index: ExactDecimal,
        start_beat: ExactDecimal,
        end_beat: ExactDecimal,
        x: ExactDecimal,
        y: ExactDecimal,
        easing: ExactDecimal,
        line: usize,
    },
    Cr {
        line_index: ExactDecimal,
        start_beat: ExactDecimal,
        end_beat: ExactDecimal,
        angle: ExactDecimal,
        easing: ExactDecimal,
        line: usize,
    },
    Cf {
        line_index: ExactDecimal,
        start_beat: ExactDecimal,
        end_beat: ExactDecimal,
        alpha: ExactDecimal,
        line: usize,
    },
    Unknown {
        raw: String,
        line: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PecSourceDocument {
    artifact_id: LogicalSourceLocator,
    artifact_content_sha256: [u8; 32],
    raw_offset_milliseconds: ExactDecimal,
    commands: Vec<PecSourceCommand>,
}

impl PecSourceDocument {
    pub fn artifact_id(&self) -> &LogicalSourceLocator {
        &self.artifact_id
    }

    pub(crate) const fn artifact_content_sha256(&self) -> [u8; 32] {
        self.artifact_content_sha256
    }

    pub fn raw_offset_milliseconds(&self) -> &ExactDecimal {
        &self.raw_offset_milliseconds
    }

    pub fn commands(&self) -> &[PecSourceCommand] {
        &self.commands
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PecSemanticTime {
    source_beat: ExactRational,
    chart_time_seconds: ExactRational,
}

impl PecSemanticTime {
    pub fn source_beat(&self) -> &ExactRational {
        &self.source_beat
    }

    pub fn chart_time_seconds(&self) -> &ExactRational {
        &self.chart_time_seconds
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PecSemanticBpm {
    start_beat: ExactRational,
    bpm: ExactRational,
}

impl PecSemanticBpm {
    pub fn start_beat(&self) -> &ExactRational {
        &self.start_beat
    }

    pub fn bpm(&self) -> &ExactRational {
        &self.bpm
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PecSemanticNote {
    kind: PecNoteKind,
    line_index: usize,
    side: PecNoteSide,
    judgment_enabled: bool,
    start_time: PecSemanticTime,
    end_time: Option<PecSemanticTime>,
    position_x_px: ExactRational,
    speed_factor: ExactRational,
    width_factor: ExactRational,
}

impl PecSemanticNote {
    pub const fn kind(&self) -> PecNoteKind {
        self.kind
    }

    pub const fn line_index(&self) -> usize {
        self.line_index
    }

    pub const fn side(&self) -> PecNoteSide {
        self.side
    }

    pub const fn judgment_enabled(&self) -> bool {
        self.judgment_enabled
    }

    pub fn start_time(&self) -> &PecSemanticTime {
        &self.start_time
    }

    pub fn end_time(&self) -> Option<&PecSemanticTime> {
        self.end_time.as_ref()
    }

    pub fn position_x_px(&self) -> &ExactRational {
        &self.position_x_px
    }

    pub fn speed_factor(&self) -> &ExactRational {
        &self.speed_factor
    }

    pub fn width_factor(&self) -> &ExactRational {
        &self.width_factor
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PecSemanticDocument {
    artifact_id: LogicalSourceLocator,
    artifact_content_sha256: [u8; 32],
    profile: PecProfile,
    audio_offset_seconds: ExactRational,
    floor_scale_px: ExactRational,
    bpm_points: Vec<PecSemanticBpm>,
    notes: Vec<PecSemanticNote>,
    max_line_index: usize,
}

impl PecSemanticDocument {
    pub fn artifact_id(&self) -> &LogicalSourceLocator {
        &self.artifact_id
    }

    pub(crate) const fn artifact_content_sha256(&self) -> [u8; 32] {
        self.artifact_content_sha256
    }

    pub const fn profile(&self) -> PecProfile {
        self.profile
    }

    pub fn audio_offset_seconds(&self) -> &ExactRational {
        &self.audio_offset_seconds
    }

    pub fn floor_scale_px(&self) -> &ExactRational {
        &self.floor_scale_px
    }

    pub fn bpm_points(&self) -> &[PecSemanticBpm] {
        &self.bpm_points
    }

    pub fn notes(&self) -> &[PecSemanticNote] {
        &self.notes
    }

    pub const fn max_line_index(&self) -> usize {
        self.max_line_index
    }
}

/// Parse a PEC chart artifact using the Phira line-command dialect.
pub fn parse_pec_document(
    artifact: &SourceArtifact,
    limits: PecLimits,
) -> Result<PecSourceDocument, PecError> {
    let text = std::str::from_utf8(artifact.bytes())
        .map_err(|_| PecError::new(SOURCE_INVALID, "artifact", "PEC chart must be valid UTF-8"))?;
    let mut physical_lines = text.lines().enumerate();
    let (offset_line_no, offset_line) = physical_lines.next().ok_or_else(|| {
        PecError::new(
            SOURCE_INVALID,
            "line:1",
            "PEC requires a first-line raw offset",
        )
    })?;
    let offset_tokens = tokenize(offset_line);
    if offset_tokens.len() != 1 {
        return Err(PecError::new(
            SOURCE_INVALID,
            format!("line:{}", offset_line_no + 1),
            "first PEC line must be a single raw offset number",
        ));
    }
    let raw_offset_milliseconds = number(
        offset_tokens[0],
        &format!("line:{}", offset_line_no + 1),
        limits,
    )?;

    let mut commands = Vec::new();
    let mut pending_note: Option<usize> = None;
    for (line_no, line) in physical_lines {
        let line_path = format!("line:{}", line_no + 1);
        let tokens = tokenize(line);
        if tokens.is_empty() {
            continue;
        }
        if commands.len() >= limits.max_commands {
            return Err(PecError::new(
                SOURCE_INVALID,
                &line_path,
                format!(
                    "PEC limit max_commands exceeded: limit {}, observed {}",
                    limits.max_commands,
                    commands.len().saturating_add(1)
                ),
            ));
        }
        match tokens[0] {
            "#" => {
                if tokens.len() != 2 {
                    return Err(PecError::new(
                        SOURCE_INVALID,
                        &line_path,
                        "# speed suffix requires exactly one numeric argument",
                    ));
                }
                let Some(index) = pending_note else {
                    return Err(PecError::new(
                        SOURCE_INVALID,
                        &line_path,
                        "# speed suffix is not associated with a Note",
                    ));
                };
                let speed = number(tokens[1], &format!("{line_path}.#"), limits)?;
                if let PecSourceCommand::Note(note) = &mut commands[index] {
                    note.speed = Some(speed);
                }
            }
            "&" => {
                if tokens.len() != 2 {
                    return Err(PecError::new(
                        SOURCE_INVALID,
                        &line_path,
                        "& width suffix requires exactly one numeric argument",
                    ));
                }
                let Some(index) = pending_note else {
                    return Err(PecError::new(
                        SOURCE_INVALID,
                        &line_path,
                        "& width suffix is not associated with a Note",
                    ));
                };
                let width = number(tokens[1], &format!("{line_path}.&"), limits)?;
                if let PecSourceCommand::Note(note) = &mut commands[index] {
                    note.width = Some(width);
                }
            }
            "bp" => {
                pending_note = None;
                if tokens.len() != 3 {
                    return Err(PecError::new(
                        SOURCE_INVALID,
                        &line_path,
                        "bp requires <beat> <bpm>",
                    ));
                }
                commands.push(PecSourceCommand::Bpm(PecSourceBpm {
                    beat: number(tokens[1], &format!("{line_path}.beat"), limits)?,
                    bpm: number(tokens[2], &format!("{line_path}.bpm"), limits)?,
                    line: line_no + 1,
                }));
            }
            "n1" | "n2" | "n3" | "n4" => {
                let kind = match tokens[0] {
                    "n1" => PecNoteKind::Tap,
                    "n2" => PecNoteKind::Hold,
                    "n3" => PecNoteKind::Flick,
                    _ => PecNoteKind::Drag,
                };
                let note = parse_note(kind, &tokens, &line_path, line_no + 1, limits)?;
                pending_note = Some(commands.len());
                commands.push(PecSourceCommand::Note(note));
            }
            "cv" => {
                pending_note = None;
                require_arity(&tokens, 4, &line_path, "cv <line> <beat> <speed>")?;
                commands.push(PecSourceCommand::Cv {
                    line_index: number(tokens[1], &format!("{line_path}.line"), limits)?,
                    beat: number(tokens[2], &format!("{line_path}.beat"), limits)?,
                    speed: number(tokens[3], &format!("{line_path}.speed"), limits)?,
                    line: line_no + 1,
                });
            }
            "cp" => {
                pending_note = None;
                require_arity(&tokens, 5, &line_path, "cp <line> <beat> <x> <y>")?;
                commands.push(PecSourceCommand::Cp {
                    line_index: number(tokens[1], &format!("{line_path}.line"), limits)?,
                    beat: number(tokens[2], &format!("{line_path}.beat"), limits)?,
                    x: number(tokens[3], &format!("{line_path}.x"), limits)?,
                    y: number(tokens[4], &format!("{line_path}.y"), limits)?,
                    line: line_no + 1,
                });
            }
            "cd" => {
                pending_note = None;
                require_arity(&tokens, 4, &line_path, "cd <line> <beat> <angle>")?;
                commands.push(PecSourceCommand::Cd {
                    line_index: number(tokens[1], &format!("{line_path}.line"), limits)?,
                    beat: number(tokens[2], &format!("{line_path}.beat"), limits)?,
                    angle: number(tokens[3], &format!("{line_path}.angle"), limits)?,
                    line: line_no + 1,
                });
            }
            "ca" => {
                pending_note = None;
                require_arity(&tokens, 4, &line_path, "ca <line> <beat> <alpha>")?;
                commands.push(PecSourceCommand::Ca {
                    line_index: number(tokens[1], &format!("{line_path}.line"), limits)?,
                    beat: number(tokens[2], &format!("{line_path}.beat"), limits)?,
                    alpha: number(tokens[3], &format!("{line_path}.alpha"), limits)?,
                    line: line_no + 1,
                });
            }
            "cm" => {
                pending_note = None;
                require_arity(
                    &tokens,
                    7,
                    &line_path,
                    "cm <line> <start> <end> <x> <y> <easing>",
                )?;
                commands.push(PecSourceCommand::Cm {
                    line_index: number(tokens[1], &format!("{line_path}.line"), limits)?,
                    start_beat: number(tokens[2], &format!("{line_path}.start"), limits)?,
                    end_beat: number(tokens[3], &format!("{line_path}.end"), limits)?,
                    x: number(tokens[4], &format!("{line_path}.x"), limits)?,
                    y: number(tokens[5], &format!("{line_path}.y"), limits)?,
                    easing: number(tokens[6], &format!("{line_path}.easing"), limits)?,
                    line: line_no + 1,
                });
            }
            "cr" => {
                pending_note = None;
                require_arity(
                    &tokens,
                    6,
                    &line_path,
                    "cr <line> <start> <end> <angle> <easing>",
                )?;
                commands.push(PecSourceCommand::Cr {
                    line_index: number(tokens[1], &format!("{line_path}.line"), limits)?,
                    start_beat: number(tokens[2], &format!("{line_path}.start"), limits)?,
                    end_beat: number(tokens[3], &format!("{line_path}.end"), limits)?,
                    angle: number(tokens[4], &format!("{line_path}.angle"), limits)?,
                    easing: number(tokens[5], &format!("{line_path}.easing"), limits)?,
                    line: line_no + 1,
                });
            }
            "cf" => {
                pending_note = None;
                require_arity(&tokens, 5, &line_path, "cf <line> <start> <end> <alpha>")?;
                commands.push(PecSourceCommand::Cf {
                    line_index: number(tokens[1], &format!("{line_path}.line"), limits)?,
                    start_beat: number(tokens[2], &format!("{line_path}.start"), limits)?,
                    end_beat: number(tokens[3], &format!("{line_path}.end"), limits)?,
                    alpha: number(tokens[4], &format!("{line_path}.alpha"), limits)?,
                    line: line_no + 1,
                });
            }
            _ => {
                pending_note = None;
                commands.push(PecSourceCommand::Unknown {
                    raw: line.to_owned(),
                    line: line_no + 1,
                });
            }
        }
    }

    Ok(PecSourceDocument {
        artifact_id: artifact.logical_id().clone(),
        artifact_content_sha256: artifact.content_sha256(),
        raw_offset_milliseconds,
        commands,
    })
}

/// Bind a PEC source document to an explicit profile and produce chartTime + Note semantics.
pub fn interpret_pec(
    source: &PecSourceDocument,
    binding: &PecProfileBinding,
) -> Result<PecSemanticDocument, PecError> {
    let profile = binding.profile();
    let mut bpm_points = Vec::new();
    let mut previous_beat: Option<ExactRational> = None;
    let mut notes = Vec::new();
    let mut max_line_index = 0usize;
    let mut saw_non_bp = false;

    for command in &source.commands {
        match command {
            PecSourceCommand::Bpm(point) => {
                if saw_non_bp && profile == PecProfile::Phira {
                    return Err(PecError::new(
                        SOURCE_INVALID,
                        format!("line:{}", point.line),
                        "Phira PEC dialect rejects bp commands after the first non-bp command",
                    ));
                }
                let beat = point.beat.exact().clone();
                validate_positive_finite(point.bpm.exact(), format!("line:{}", point.line), "BPM")?;
                if let Some(previous) = &previous_beat
                    && beat.value() < previous.value()
                {
                    return Err(PecError::new(
                        SOURCE_INVALID,
                        format!("line:{}", point.line),
                        "bp startBeat must be non-decreasing in source order",
                    ));
                }
                previous_beat = Some(beat.clone());
                bpm_points.push(PecSemanticBpm {
                    start_beat: beat,
                    bpm: point.bpm.exact().clone(),
                });
            }
            PecSourceCommand::Note(note) => {
                saw_non_bp = true;
                let line_index = note
                    .line_index
                    .exact()
                    .to_i64()
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or_else(|| {
                        PecError::new(
                            SOURCE_INVALID,
                            format!("line:{}", note.line),
                            "Note line index must be a non-negative integer",
                        )
                    })?;
                max_line_index = max_line_index.max(line_index);
                if note.kind == PecNoteKind::Hold {
                    let Some(end) = note.end_beat.as_ref() else {
                        return Err(PecError::new(
                            SOURCE_INVALID,
                            format!("line:{}", note.line),
                            "Hold requires endBeat",
                        ));
                    };
                    if end.exact().value() <= note.start_beat.exact().value() {
                        return Err(PecError::new(
                            SOURCE_INVALID,
                            format!("line:{}", note.line),
                            "Hold endBeat must be strictly later than startBeat",
                        ));
                    }
                }
                let side = match note.side.exact().to_i64() {
                    Some(1) => PecNoteSide::Above,
                    Some(_) => PecNoteSide::Below,
                    None => {
                        return Err(PecError::new(
                            SOURCE_INVALID,
                            format!("line:{}", note.line),
                            "Note side must be an exact integer",
                        ));
                    }
                };
                let judgment_enabled = match note.fake.exact().to_i64() {
                    Some(0) => true,
                    Some(1) => false,
                    Some(_) if profile == PecProfile::Phira => {
                        return Err(PecError::new(
                            SOURCE_INVALID,
                            format!("line:{}", note.line),
                            "Phira PEC fake must be 0 or 1",
                        ));
                    }
                    Some(value) => value == 0,
                    None => {
                        return Err(PecError::new(
                            SOURCE_INVALID,
                            format!("line:{}", note.line),
                            "Note fake must be an exact integer",
                        ));
                    }
                };
                notes.push(PecSemanticNote {
                    kind: note.kind,
                    line_index,
                    side,
                    judgment_enabled,
                    start_time: semantic_time(note.start_beat.exact(), &bpm_points, note.line)?,
                    end_time: note
                        .end_beat
                        .as_ref()
                        .map(|end| semantic_time(end.exact(), &bpm_points, note.line))
                        .transpose()?,
                    position_x_px: note_x_relative_2048(note.x.exact())?,
                    speed_factor: note
                        .speed
                        .as_ref()
                        .map(|value| value.exact().clone())
                        .unwrap_or_else(|| ExactRational::from_integer(BigInt::one())),
                    width_factor: note
                        .width
                        .as_ref()
                        .map(|value| value.exact().clone())
                        .unwrap_or_else(|| ExactRational::from_integer(BigInt::one())),
                });
            }
            PecSourceCommand::Unknown { line, .. } => {
                return Err(PecError::new(
                    SOURCE_INVALID,
                    format!("line:{line}"),
                    "unknown PEC command is not accepted in this unit without Repair",
                ));
            }
            PecSourceCommand::Cv { line_index, .. }
            | PecSourceCommand::Cp { line_index, .. }
            | PecSourceCommand::Cd { line_index, .. }
            | PecSourceCommand::Ca { line_index, .. }
            | PecSourceCommand::Cm { line_index, .. }
            | PecSourceCommand::Cr { line_index, .. }
            | PecSourceCommand::Cf { line_index, .. } => {
                saw_non_bp = true;
                let index = line_index
                    .exact()
                    .to_i64()
                    .and_then(|value| usize::try_from(value).ok());
                if let Some(index) = index {
                    max_line_index = max_line_index.max(index);
                }
            }
        }
    }

    if bpm_points.is_empty() {
        return Err(PecError::new(
            SOURCE_INVALID,
            "bp",
            "PEC requires at least one bp command",
        ));
    }
    if max_line_index >= limits_like_max_lines() {
        // soft: only track for later
    }

    let audio_offset_seconds = offset_with_bias(
        source.raw_offset_milliseconds.exact(),
        profile.offset_bias_ms(),
    )?;

    Ok(PecSemanticDocument {
        artifact_id: source.artifact_id.clone(),
        artifact_content_sha256: source.artifact_content_sha256(),
        profile,
        audio_offset_seconds,
        floor_scale_px: binding.floor_scale_px().clone(),
        bpm_points,
        notes,
        max_line_index,
    })
}

pub fn offset_with_bias(
    raw_offset_milliseconds: &ExactRational,
    bias_ms: i64,
) -> Result<ExactRational, PecError> {
    let bias = ExactRational::from_integer(BigInt::from(bias_ms));
    let adjusted = ExactRational(raw_offset_milliseconds.value() - bias.value());
    Ok(ExactRational(adjusted.value() / integer(1000)))
}

pub fn note_x_relative_2048(source_x: &ExactRational) -> Result<ExactRational, PecError> {
    // noteXpx = sourceX * 1920 / 2048 = sourceX * 15 / 16
    Ok(ExactRational(
        source_x.value() * integer(1920) / integer(2048),
    ))
}

pub fn line_x_canvas_2048(source_x: &ExactRational) -> Result<ExactRational, PecError> {
    // (sourceX/2048 - 0.5) * 1920
    Ok(ExactRational(
        (source_x.value() / integer(2048) - half()) * integer(1920),
    ))
}

pub fn line_y_canvas_1400(source_y: &ExactRational) -> Result<ExactRational, PecError> {
    Ok(ExactRational(
        (source_y.value() / integer(1400) - half()) * integer(1080),
    ))
}

pub fn cv_scale(source_cv: &ExactRational, scale: PecCvScale) -> Result<ExactRational, PecError> {
    match scale {
        PecCvScale::Div585 => Ok(ExactRational(
            source_cv.value() / (integer(585) / integer(100)),
        )),
        PecCvScale::Div7 => Ok(ExactRational(source_cv.value() / integer(7))),
        PecCvScale::RpeHeight900 => {
            // rawCv * 900 / 1400 / 4.5 = rawCv / 7
            Ok(ExactRational(
                source_cv.value() * integer(900) / integer(1400) / (integer(9) / integer(2)),
            ))
        }
    }
}

fn semantic_time(
    beat: &ExactRational,
    bpm_points: &[PecSemanticBpm],
    line: usize,
) -> Result<PecSemanticTime, PecError> {
    if bpm_points.is_empty() {
        return Err(PecError::new(
            SOURCE_INVALID,
            format!("line:{line}"),
            "chart time requires at least one bp before non-bp commands",
        ));
    }
    let chart_time_seconds = chart_time_at(beat, bpm_points)?;
    Ok(PecSemanticTime {
        source_beat: beat.clone(),
        chart_time_seconds,
    })
}

fn chart_time_at(
    target: &ExactRational,
    bpm_points: &[PecSemanticBpm],
) -> Result<ExactRational, PecError> {
    let mut segments: Vec<(ExactRational, ExactRational)> = Vec::new();
    for point in bpm_points {
        if let Some((beat, bpm)) = segments.last_mut()
            && beat == &point.start_beat
        {
            *bpm = point.bpm.clone();
            continue;
        }
        segments.push((point.start_beat.clone(), point.bpm.clone()));
    }
    let mut time = BigRational::zero();
    let first_beat = &segments[0].0;
    if target.value() < first_beat.value() {
        let delta = ExactRational(first_beat.value() - target.value());
        let left = beat_delta_seconds(&delta, &segments[0].1)?;
        return Ok(ExactRational(-left.value().clone()));
    }
    for index in 0..segments.len() {
        let (start, bpm) = &segments[index];
        if target.value() <= start.value() {
            break;
        }
        let end = if index + 1 < segments.len() {
            let next = &segments[index + 1].0;
            if target.value() < next.value() {
                target.clone()
            } else {
                next.clone()
            }
        } else {
            target.clone()
        };
        if end.value() <= start.value() {
            continue;
        }
        let delta = ExactRational(end.value() - start.value());
        let step = beat_delta_seconds(&delta, bpm)?;
        time += step.value();
        if end.value() == target.value() {
            break;
        }
    }
    Ok(ExactRational(time))
}

fn beat_delta_seconds(
    beat_delta: &ExactRational,
    bpm: &ExactRational,
) -> Result<ExactRational, PecError> {
    validate_positive_finite(bpm, "bpm", "BPM")?;
    Ok(ExactRational(
        beat_delta.value() * integer(60) / bpm.value(),
    ))
}

fn parse_note(
    kind: PecNoteKind,
    tokens: &[&str],
    path: &str,
    line: usize,
    limits: PecLimits,
) -> Result<PecSourceNote, PecError> {
    let expected = if kind == PecNoteKind::Hold { 6 } else { 5 };
    // command + args; Hold has endBeat
    if tokens.len() != expected + 1 {
        return Err(PecError::new(
            SOURCE_INVALID,
            path,
            format!(
                "{} requires {} numeric fields after the command name",
                tokens[0], expected
            ),
        ));
    }
    if kind == PecNoteKind::Hold {
        Ok(PecSourceNote {
            kind,
            line_index: number(tokens[1], &format!("{path}.line"), limits)?,
            start_beat: number(tokens[2], &format!("{path}.start"), limits)?,
            end_beat: Some(number(tokens[3], &format!("{path}.end"), limits)?),
            x: number(tokens[4], &format!("{path}.x"), limits)?,
            side: number(tokens[5], &format!("{path}.side"), limits)?,
            fake: number(tokens[6], &format!("{path}.fake"), limits)?,
            speed: None,
            width: None,
            line,
        })
    } else {
        Ok(PecSourceNote {
            kind,
            line_index: number(tokens[1], &format!("{path}.line"), limits)?,
            start_beat: number(tokens[2], &format!("{path}.start"), limits)?,
            end_beat: None,
            x: number(tokens[3], &format!("{path}.x"), limits)?,
            side: number(tokens[4], &format!("{path}.side"), limits)?,
            fake: number(tokens[5], &format!("{path}.fake"), limits)?,
            speed: None,
            width: None,
            line,
        })
    }
}

fn require_arity(
    tokens: &[&str],
    expected: usize,
    path: &str,
    usage: &str,
) -> Result<(), PecError> {
    if tokens.len() != expected {
        Err(PecError::new(
            SOURCE_INVALID,
            path,
            format!("expected {usage}"),
        ))
    } else {
        Ok(())
    }
}

fn tokenize(line: &str) -> Vec<&str> {
    line.split_ascii_whitespace().collect()
}

fn number(raw: &str, path: &str, limits: PecLimits) -> Result<ExactDecimal, PecError> {
    ExactDecimal::parse(raw, limits.decimal).map_err(|error| PecError::from_exact(path, error))
}

fn validate_positive_finite(
    value: &ExactRational,
    path: impl Into<String>,
    label: &str,
) -> Result<(), PecError> {
    if !value.is_positive() || value.to_f64().is_err() {
        Err(PecError::new(
            SOURCE_INVALID,
            path,
            format!("{label} must be finite and positive"),
        ))
    } else {
        Ok(())
    }
}

fn limits_like_max_lines() -> usize {
    4096
}

fn integer(value: i64) -> BigRational {
    BigRational::from_integer(BigInt::from(value))
}

fn half() -> BigRational {
    BigRational::new(BigInt::one(), BigInt::from(2))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PecError {
    category: &'static str,
    path: String,
    message: String,
}

impl PecError {
    pub(crate) fn new(
        category: &'static str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
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

impl fmt::Display for PecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at {}: {}",
            self.category, self.path, self.message
        )
    }
}

impl std::error::Error for PecError {}

// Silence unused SourceFormat import intent for future package entry points.
#[allow(dead_code)]
fn _format_is_pec(format: SourceFormat) -> bool {
    format == SourceFormat::Pec
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArtifactRole, SourceArtifact};

    const SIMPLE: &str =
        "0\nbp 0.00 120\nn1 0 1.00 1024 1 0\n# 1.000\n& 1.000\nn2 0 2.00 3.00 0 1 0\n";

    fn artifact(bytes: &str) -> SourceArtifact {
        SourceArtifact::new("charts/main.pec", ArtifactRole::Chart, bytes.as_bytes()).unwrap()
    }

    fn exact(expected: &str) -> ExactRational {
        if expected.contains('/') {
            let (numerator, denominator) = expected.split_once('/').unwrap();
            ExactRational(BigRational::new(
                BigInt::parse_bytes(numerator.as_bytes(), 10).unwrap(),
                BigInt::parse_bytes(denominator.as_bytes(), 10).unwrap(),
            ))
        } else if expected.contains('.') || expected.contains(['e', 'E']) {
            ExactDecimal::parse(expected, DecimalLimits::default())
                .unwrap()
                .exact()
                .clone()
        } else {
            ExactRational(BigRational::new(
                BigInt::parse_bytes(expected.as_bytes(), 10).unwrap(),
                BigInt::one(),
            ))
        }
    }

    #[test]
    fn parse_retains_order_offset_notes_and_suffixes() {
        let source = parse_pec_document(&artifact(SIMPLE), PecLimits::default()).unwrap();
        assert_eq!(source.raw_offset_milliseconds().raw(), "0");
        assert_eq!(source.commands().len(), 3);
        match &source.commands()[1] {
            PecSourceCommand::Note(note) => {
                assert_eq!(note.kind(), PecNoteKind::Tap);
                assert_eq!(note.speed().unwrap().raw(), "1.000");
                assert_eq!(note.width().unwrap().raw(), "1.000");
            }
            other => panic!("expected note, got {other:?}"),
        }
        match &source.commands()[2] {
            PecSourceCommand::Note(note) => {
                assert_eq!(note.kind(), PecNoteKind::Hold);
                assert!(note.end_beat().is_some());
            }
            other => panic!("expected hold, got {other:?}"),
        }
    }

    #[test]
    fn phira_profile_maps_offset_and_note_times() {
        let source = parse_pec_document(&artifact(SIMPLE), PecLimits::default()).unwrap();
        let binding = PecProfileBinding::new(
            PecProfile::Phira,
            ExactDecimal::parse("100", DecimalLimits::default()).unwrap(),
        )
        .unwrap();
        let semantic = interpret_pec(&source, &binding).unwrap();
        // raw 0, bias 150 => -0.15 s
        assert_eq!(semantic.audio_offset_seconds(), &exact("-3/20"));
        assert_eq!(semantic.notes().len(), 2);
        // beat 1 at 120 bpm => 0.5 s
        assert_eq!(
            semantic.notes()[0].start_time().chart_time_seconds(),
            &exact("1/2")
        );
        assert_eq!(semantic.notes()[0].side(), PecNoteSide::Above);
        assert!(semantic.notes()[0].judgment_enabled());
        assert_eq!(semantic.notes()[0].position_x_px(), &exact("960"));
    }

    #[test]
    fn late_bp_is_rejected_for_phira_dialect() {
        let chart = "0\nbp 0 120\nn1 0 1 0 1 0\nbp 2 140\n";
        let source = parse_pec_document(&artifact(chart), PecLimits::default()).unwrap();
        let binding = PecProfileBinding::new(
            PecProfile::Phira,
            ExactDecimal::parse("100", DecimalLimits::default()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            interpret_pec(&source, &binding).unwrap_err().category(),
            SOURCE_INVALID
        );
    }

    #[test]
    fn checked_in_pec_mapping_vectors_execute_exactly() {
        use std::collections::BTreeMap;

        use serde::Deserialize;

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

        let corpus: MappingCorpus = toml::from_str(include_str!(
            "../../../docs/conformance/conversion/mapping-vectors.toml"
        ))
        .unwrap();
        let mut executed = 0;
        for vector in &corpus.vector {
            let actual = match vector.rule_id.as_str() {
                "pec.time.direct-beat" => exact(vector.source["time"].as_str().unwrap()),
                "pec.note-x.relative2048" => {
                    note_x_relative_2048(&exact(vector.source["x"].as_str().unwrap())).unwrap()
                }
                "pec.line-x.canvas2048" => {
                    line_x_canvas_2048(&exact(vector.source["x"].as_str().unwrap())).unwrap()
                }
                "pec.line-y.canvas1400" => {
                    line_y_canvas_1400(&exact(vector.source["y"].as_str().unwrap())).unwrap()
                }
                "pec.offset.bias150ms" => offset_with_bias(
                    &exact(vector.source["raw_offset_milliseconds"].as_str().unwrap()),
                    150,
                )
                .unwrap(),
                "pec.offset.bias175ms" => offset_with_bias(
                    &exact(vector.source["raw_offset_milliseconds"].as_str().unwrap()),
                    175,
                )
                .unwrap(),
                "pec.cv.scale5_85" => cv_scale(
                    &exact(vector.source["cv"].as_str().unwrap()),
                    PecCvScale::Div585,
                )
                .unwrap(),
                "pec.cv.scale7" => cv_scale(
                    &exact(vector.source["cv"].as_str().unwrap()),
                    PecCvScale::Div7,
                )
                .unwrap(),
                "pec.cv.rpe-height900" => cv_scale(
                    &exact(vector.source["cv"].as_str().unwrap()),
                    PecCvScale::RpeHeight900,
                )
                .unwrap(),
                _ => continue,
            };
            assert_eq!(actual, exact(&vector.expected), "{}", vector.id);
            executed += 1;
        }
        assert_eq!(executed, 9);
    }
}
