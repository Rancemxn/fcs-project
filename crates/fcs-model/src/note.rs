//! Immutable canonical Note values and deterministic gameplay ordering.

use std::fmt;

use crate::{CanonicalColor, CanonicalTime, CanonicalVec2, EntityKind, StableId};

/// One of the four Core Note kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalNoteKind {
    Tap,
    Hold,
    Flick,
    Drag,
}

/// The canonical side of a Note's gameplay intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalNoteSide {
    Above,
    Below,
}

/// The closed Core judge-shape descriptor.
#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalJudgeShape {
    LineDefault,
    Rectangle {
        center: CanonicalVec2,
        half_extents: CanonicalVec2,
    },
    Circle {
        center: CanonicalVec2,
        radius: f64,
    },
}

/// The canonical sound intent for a judgment-enabled Note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalNoteSoundPolicy {
    Default,
    None,
    Resource(String),
}

/// The canonical score intent for a judgment-enabled Note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalNoteScorePolicy {
    Default,
    None,
    Custom(String),
}

/// Canonical gameplay fields shared by all Note kinds.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalNoteGameplay {
    kind: CanonicalNoteKind,
    line: StableId,
    time: CanonicalTime,
    end_time: Option<CanonicalTime>,
    side: CanonicalNoteSide,
    judgment_enabled: bool,
    judge_shape: CanonicalJudgeShape,
    sound_policy: CanonicalNoteSoundPolicy,
    score_policy: CanonicalNoteScorePolicy,
}

impl CanonicalNoteGameplay {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: CanonicalNoteKind,
        line: StableId,
        time: CanonicalTime,
        end_time: Option<CanonicalTime>,
        side: CanonicalNoteSide,
        judgment_enabled: bool,
        judge_shape: CanonicalJudgeShape,
        sound_policy: CanonicalNoteSoundPolicy,
        score_policy: CanonicalNoteScorePolicy,
    ) -> Result<Self, CanonicalNoteError> {
        if line.namespace() != EntityKind::Line {
            return Err(CanonicalNoteError::WrongLineNamespace { id: line.value() });
        }
        if let Some(end_time) = end_time {
            if kind != CanonicalNoteKind::Hold {
                return Err(CanonicalNoteError::EndTimeOnNonHold);
            }
            if !end_time.chart_time_seconds().is_finite()
                || end_time.chart_time_seconds() <= time.chart_time_seconds()
            {
                return Err(CanonicalNoteError::InvalidHoldInterval);
            }
        } else if kind == CanonicalNoteKind::Hold {
            return Err(CanonicalNoteError::MissingHoldEndTime);
        }
        validate_shape(&judge_shape)?;
        if !judgment_enabled
            && (!matches!(sound_policy, CanonicalNoteSoundPolicy::None)
                || !matches!(score_policy, CanonicalNoteScorePolicy::None))
        {
            return Err(CanonicalNoteError::DisabledJudgmentPolicy);
        }
        if matches!(sound_policy, CanonicalNoteSoundPolicy::Resource(ref value) if value.is_empty())
            || matches!(score_policy, CanonicalNoteScorePolicy::Custom(ref value) if value.is_empty())
        {
            return Err(CanonicalNoteError::EmptyPolicyReference);
        }
        Ok(Self {
            kind,
            line,
            time,
            end_time,
            side,
            judgment_enabled,
            judge_shape,
            sound_policy,
            score_policy,
        })
    }

    pub fn line(&self) -> &StableId {
        &self.line
    }

    pub const fn kind(&self) -> CanonicalNoteKind {
        self.kind
    }

    pub const fn time(&self) -> CanonicalTime {
        self.time
    }

    pub const fn end_time(&self) -> Option<CanonicalTime> {
        self.end_time
    }

    pub const fn side(&self) -> CanonicalNoteSide {
        self.side
    }

    pub const fn judgment_enabled(&self) -> bool {
        self.judgment_enabled
    }

    pub fn judge_shape(&self) -> &CanonicalJudgeShape {
        &self.judge_shape
    }

    pub fn sound_policy(&self) -> &CanonicalNoteSoundPolicy {
        &self.sound_policy
    }

    pub fn score_policy(&self) -> &CanonicalNoteScorePolicy {
        &self.score_policy
    }
}

/// Canonical static presentation defaults and values.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalNotePresentation {
    position_x: f64,
    scroll_factor: f64,
    x_offset: f64,
    y_offset: f64,
    alpha: f64,
    scale_x: f64,
    scale_y: f64,
    rotation: f64,
    color: CanonicalColor,
    texture: Option<String>,
    render_enabled: bool,
    visible_from: Option<CanonicalTime>,
    visible_until: Option<CanonicalTime>,
}

impl CanonicalNotePresentation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        position_x: f64,
        scroll_factor: f64,
        x_offset: f64,
        y_offset: f64,
        alpha: f64,
        scale_x: f64,
        scale_y: f64,
        rotation: f64,
        color: CanonicalColor,
        texture: Option<String>,
        render_enabled: bool,
        visible_from: Option<CanonicalTime>,
        visible_until: Option<CanonicalTime>,
    ) -> Result<Self, CanonicalNoteError> {
        for (field, value) in [
            ("positionX", position_x),
            ("scrollFactor", scroll_factor),
            ("xOffset", x_offset),
            ("yOffset", y_offset),
            ("alpha", alpha),
            ("scaleX", scale_x),
            ("scaleY", scale_y),
            ("rotation", rotation),
        ] {
            if !value.is_finite() {
                return Err(CanonicalNoteError::NonFinitePresentation { field });
            }
        }
        if !(0.0..=1.0).contains(&alpha) {
            return Err(CanonicalNoteError::PresentationOutOfRange { field: "alpha" });
        }
        if let (Some(start), Some(end)) = (visible_from, visible_until)
            && end.chart_time_seconds() <= start.chart_time_seconds()
        {
            return Err(CanonicalNoteError::InvalidVisibilityInterval);
        }
        Ok(Self {
            position_x,
            scroll_factor,
            x_offset,
            y_offset,
            alpha,
            scale_x,
            scale_y,
            rotation,
            color,
            texture,
            render_enabled,
            visible_from,
            visible_until,
        })
    }

    pub const fn position_x(&self) -> f64 {
        self.position_x
    }

    pub const fn scroll_factor(&self) -> f64 {
        self.scroll_factor
    }

    pub const fn x_offset(&self) -> f64 {
        self.x_offset
    }

    pub const fn y_offset(&self) -> f64 {
        self.y_offset
    }

    pub const fn alpha(&self) -> f64 {
        self.alpha
    }

    pub const fn scale_x(&self) -> f64 {
        self.scale_x
    }

    pub const fn scale_y(&self) -> f64 {
        self.scale_y
    }

    pub const fn rotation(&self) -> f64 {
        self.rotation
    }

    pub fn color(&self) -> CanonicalColor {
        self.color
    }

    pub fn texture(&self) -> Option<&str> {
        self.texture.as_deref()
    }

    pub const fn render_enabled(&self) -> bool {
        self.render_enabled
    }

    pub const fn visible_from(&self) -> Option<CanonicalTime> {
        self.visible_from
    }

    pub const fn visible_until(&self) -> Option<CanonicalTime> {
        self.visible_until
    }
}

/// One immutable canonical Note.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalNote {
    id: StableId,
    kind: CanonicalNoteKind,
    document_order: u64,
    gameplay: CanonicalNoteGameplay,
    presentation: CanonicalNotePresentation,
}

impl CanonicalNote {
    pub fn new(
        id: StableId,
        kind: CanonicalNoteKind,
        document_order: u64,
        gameplay: CanonicalNoteGameplay,
        presentation: CanonicalNotePresentation,
    ) -> Result<Self, CanonicalNoteError> {
        if id.namespace() != EntityKind::Note {
            return Err(CanonicalNoteError::WrongNoteNamespace { id: id.value() });
        }
        if gameplay.kind() != kind {
            return Err(CanonicalNoteError::KindMismatch {
                gameplay: gameplay.kind(),
                note: kind,
            });
        }
        Ok(Self {
            id,
            kind,
            document_order,
            gameplay,
            presentation,
        })
    }

    pub fn id(&self) -> &StableId {
        &self.id
    }

    pub const fn kind(&self) -> CanonicalNoteKind {
        self.kind
    }

    pub const fn document_order(&self) -> u64 {
        self.document_order
    }

    pub fn gameplay(&self) -> &CanonicalNoteGameplay {
        &self.gameplay
    }

    pub fn presentation(&self) -> &CanonicalNotePresentation {
        &self.presentation
    }
}

/// An immutable collection of Notes in the normative canonical sort order.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalNoteSet {
    notes: Vec<CanonicalNote>,
}

impl CanonicalNoteSet {
    pub fn new(mut notes: Vec<CanonicalNote>) -> Result<Self, CanonicalNoteError> {
        let mut ids = std::collections::BTreeSet::new();
        for note in &notes {
            if !ids.insert(note.id.value()) {
                return Err(CanonicalNoteError::DuplicateId {
                    id: note.id.value(),
                });
            }
        }
        notes.sort_by(|left, right| {
            left.gameplay
                .time
                .chart_time_seconds()
                .total_cmp(&right.gameplay.time.chart_time_seconds())
                .then_with(|| left.gameplay.line.value().cmp(&right.gameplay.line.value()))
                .then_with(|| left.document_order.cmp(&right.document_order))
                .then_with(|| left.id.value().cmp(&right.id.value()))
        });
        Ok(Self { notes })
    }

    pub fn notes(&self) -> &[CanonicalNote] {
        &self.notes
    }

    pub fn note(&self, id: u64) -> Option<&CanonicalNote> {
        self.notes.iter().find(|note| note.id.value() == id)
    }

    pub fn note_by_textual_id(&self, textual_id: &str) -> Option<&CanonicalNote> {
        self.notes
            .iter()
            .find(|note| note.id.textual().as_str() == textual_id)
    }
}

fn validate_shape(shape: &CanonicalJudgeShape) -> Result<(), CanonicalNoteError> {
    match shape {
        CanonicalJudgeShape::LineDefault => Ok(()),
        CanonicalJudgeShape::Rectangle { half_extents, .. } => {
            if half_extents.x() <= 0.0 || half_extents.y() <= 0.0 {
                Err(CanonicalNoteError::NonPositiveShape)
            } else {
                Ok(())
            }
        }
        CanonicalJudgeShape::Circle { radius, .. } => {
            if !radius.is_finite() || *radius <= 0.0 {
                Err(CanonicalNoteError::NonPositiveShape)
            } else {
                Ok(())
            }
        }
    }
}

/// Invalid canonical Note data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalNoteError {
    WrongNoteNamespace {
        id: u64,
    },
    KindMismatch {
        gameplay: CanonicalNoteKind,
        note: CanonicalNoteKind,
    },
    WrongLineNamespace {
        id: u64,
    },
    DuplicateId {
        id: u64,
    },
    MissingHoldEndTime,
    InvalidHoldInterval,
    EndTimeOnNonHold,
    NonPositiveShape,
    DisabledJudgmentPolicy,
    EmptyPolicyReference,
    NonFinitePresentation {
        field: &'static str,
    },
    PresentationOutOfRange {
        field: &'static str,
    },
    InvalidVisibilityInterval,
}

impl fmt::Display for CanonicalNoteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongNoteNamespace { id } => write!(formatter, "stable ID {id} is not a Note ID"),
            Self::KindMismatch { gameplay, note } => {
                write!(
                    formatter,
                    "Note kind {note:?} does not match gameplay kind {gameplay:?}"
                )
            }
            Self::WrongLineNamespace { id } => write!(formatter, "stable ID {id} is not a Line ID"),
            Self::DuplicateId { id } => write!(formatter, "duplicate canonical Note ID {id}"),
            Self::MissingHoldEndTime => formatter.write_str("Hold Note requires endTime"),
            Self::InvalidHoldInterval => formatter.write_str("Hold endTime must be after time"),
            Self::EndTimeOnNonHold => formatter.write_str("non-Hold Note must not set endTime"),
            Self::NonPositiveShape => formatter.write_str("judge shape geometry must be positive"),
            Self::DisabledJudgmentPolicy => {
                formatter.write_str("disabled judgment requires none sound and score policies")
            }
            Self::EmptyPolicyReference => {
                formatter.write_str("Note policy reference must not be empty")
            }
            Self::NonFinitePresentation { field } => {
                write!(formatter, "Note presentation field {field} must be finite")
            }
            Self::PresentationOutOfRange { field } => {
                write!(formatter, "Note presentation field {field} is out of range")
            }
            Self::InvalidVisibilityInterval => {
                formatter.write_str("Note visibility interval must end after it starts")
            }
        }
    }
}

impl std::error::Error for CanonicalNoteError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Beat, CanonicalTextualId, ChartTimeMap, StableIdRegistry, TempoPoint};

    #[test]
    fn canonical_note_rejects_kind_mismatch_between_layers() {
        let mut ids = StableIdRegistry::new();
        let line = ids
            .insert(
                EntityKind::Line,
                CanonicalTextualId::explicit("main").unwrap(),
            )
            .unwrap();
        let note_id = ids
            .insert(
                EntityKind::Note,
                CanonicalTextualId::explicit("note").unwrap(),
            )
            .unwrap();
        let time_map = ChartTimeMap::new([TempoPoint {
            beat: Beat::zero(),
            bpm: 120.0,
        }])
        .unwrap();
        let time = time_map.chart_time(Beat::zero()).unwrap();
        let gameplay = CanonicalNoteGameplay::new(
            CanonicalNoteKind::Tap,
            line,
            time,
            None,
            CanonicalNoteSide::Above,
            true,
            CanonicalJudgeShape::LineDefault,
            CanonicalNoteSoundPolicy::Default,
            CanonicalNoteScorePolicy::Default,
        )
        .unwrap();
        let presentation = CanonicalNotePresentation::new(
            0.0,
            1.0,
            0.0,
            0.0,
            1.0,
            1.0,
            1.0,
            0.0,
            CanonicalColor::rgba(255, 255, 255, 255),
            None,
            true,
            None,
            None,
        )
        .unwrap();

        assert!(matches!(
            CanonicalNote::new(note_id, CanonicalNoteKind::Hold, 0, gameplay, presentation,),
            Err(CanonicalNoteError::KindMismatch { .. })
        ));
    }
}
