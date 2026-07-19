//! Immutable canonical Track values and interval invariants.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::{CanonicalTime, CanonicalVec2, EntityKind, StableId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalTrackTarget {
    Position,
    Rotation,
    Scale,
    Alpha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalTrackBlend {
    Replace,
    Add,
    Multiply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalTrackFill {
    Base,
    Zero,
    One,
    HoldBefore,
    HoldAfter,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalTrackInterpolation {
    Step,
    Linear,
    Easing(String),
    CubicBezier([f64; 4]),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CanonicalTrackValue {
    Float(f64),
    Angle(f64),
    Vec2Float(CanonicalVec2),
    Vec2Length(CanonicalVec2),
}

impl CanonicalTrackValue {
    fn is_finite(self) -> bool {
        match self {
            Self::Float(value) | Self::Angle(value) => value.is_finite(),
            Self::Vec2Float(value) | Self::Vec2Length(value) => {
                value.x().is_finite() && value.y().is_finite()
            }
        }
    }

    fn matches_target(self, target: CanonicalTrackTarget) -> bool {
        matches!(
            (target, self),
            (CanonicalTrackTarget::Position, Self::Vec2Length(_))
                | (CanonicalTrackTarget::Rotation, Self::Angle(_))
                | (CanonicalTrackTarget::Scale, Self::Vec2Float(_))
                | (CanonicalTrackTarget::Alpha, Self::Float(_))
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalTrackSegment {
    start: CanonicalTime,
    end: CanonicalTime,
    start_value: CanonicalTrackValue,
    end_value: CanonicalTrackValue,
    interpolation: CanonicalTrackInterpolation,
    document_order: u64,
}

impl CanonicalTrackSegment {
    pub fn new(
        start: CanonicalTime,
        end: CanonicalTime,
        start_value: CanonicalTrackValue,
        end_value: CanonicalTrackValue,
        interpolation: CanonicalTrackInterpolation,
        document_order: u64,
    ) -> Result<Self, CanonicalTrackError> {
        if end.chart_time_seconds() <= start.chart_time_seconds() {
            return Err(CanonicalTrackError::InvalidInterval);
        }
        if !start_value.is_finite() || !end_value.is_finite() {
            return Err(CanonicalTrackError::NonFiniteValue);
        }
        validate_interpolation(&interpolation)?;
        Ok(Self {
            start,
            end,
            start_value,
            end_value,
            interpolation,
            document_order,
        })
    }

    pub const fn start(&self) -> CanonicalTime {
        self.start
    }

    pub const fn end(&self) -> CanonicalTime {
        self.end
    }

    pub const fn start_value(&self) -> CanonicalTrackValue {
        self.start_value
    }

    pub const fn end_value(&self) -> CanonicalTrackValue {
        self.end_value
    }

    pub fn interpolation(&self) -> &CanonicalTrackInterpolation {
        &self.interpolation
    }

    pub const fn document_order(&self) -> u64 {
        self.document_order
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalTrackPoint {
    time: CanonicalTime,
    value: CanonicalTrackValue,
    document_order: u64,
}

impl CanonicalTrackPoint {
    pub fn new(
        time: CanonicalTime,
        value: CanonicalTrackValue,
        document_order: u64,
    ) -> Result<Self, CanonicalTrackError> {
        if !value.is_finite() {
            return Err(CanonicalTrackError::NonFiniteValue);
        }
        Ok(Self {
            time,
            value,
            document_order,
        })
    }

    pub const fn time(&self) -> CanonicalTime {
        self.time
    }

    pub const fn value(&self) -> CanonicalTrackValue {
        self.value
    }

    pub const fn document_order(&self) -> u64 {
        self.document_order
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalTrackPiece {
    Segment(CanonicalTrackSegment),
    Point(CanonicalTrackPoint),
}

impl CanonicalTrackPiece {
    fn time(&self) -> f64 {
        match self {
            Self::Segment(segment) => segment.start.chart_time_seconds(),
            Self::Point(point) => point.time.chart_time_seconds(),
        }
    }

    fn document_order(&self) -> u64 {
        match self {
            Self::Segment(segment) => segment.document_order,
            Self::Point(point) => point.document_order,
        }
    }

    fn value_matches_target(&self, target: CanonicalTrackTarget) -> bool {
        match self {
            Self::Segment(segment) => {
                segment.start_value.matches_target(target)
                    && segment.end_value.matches_target(target)
            }
            Self::Point(point) => point.value.matches_target(target),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalTrack {
    owner: StableId,
    name: String,
    target: CanonicalTrackTarget,
    blend: CanonicalTrackBlend,
    priority: i64,
    fill: CanonicalTrackFill,
    extrapolate_before: CanonicalTrackFill,
    extrapolate_after: CanonicalTrackFill,
    pieces: Vec<CanonicalTrackPiece>,
}

impl CanonicalTrack {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        owner: StableId,
        name: impl Into<String>,
        target: CanonicalTrackTarget,
        blend: CanonicalTrackBlend,
        priority: i64,
        fill: CanonicalTrackFill,
        extrapolate_before: CanonicalTrackFill,
        extrapolate_after: CanonicalTrackFill,
        mut pieces: Vec<CanonicalTrackPiece>,
    ) -> Result<Self, CanonicalTrackError> {
        if owner.namespace() != EntityKind::Line {
            return Err(CanonicalTrackError::WrongOwnerNamespace { id: owner.value() });
        }
        let name = name.into();
        if name.is_empty() {
            return Err(CanonicalTrackError::EmptyName);
        }
        if pieces.is_empty() {
            return Err(CanonicalTrackError::Empty);
        }
        if pieces
            .iter()
            .any(|piece| !piece.value_matches_target(target))
        {
            return Err(CanonicalTrackError::TargetTypeMismatch);
        }
        pieces.sort_by(|left, right| {
            left.time()
                .total_cmp(&right.time())
                .then_with(|| match (left, right) {
                    (CanonicalTrackPiece::Segment(_), CanonicalTrackPiece::Point(_)) => {
                        std::cmp::Ordering::Less
                    }
                    (CanonicalTrackPiece::Point(_), CanonicalTrackPiece::Segment(_)) => {
                        std::cmp::Ordering::Greater
                    }
                    _ => left.document_order().cmp(&right.document_order()),
                })
        });
        validate_pieces(&pieces)?;
        Ok(Self {
            owner,
            name,
            target,
            blend,
            priority,
            fill,
            extrapolate_before,
            extrapolate_after,
            pieces,
        })
    }

    pub fn owner(&self) -> &StableId {
        &self.owner
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn target(&self) -> CanonicalTrackTarget {
        self.target
    }

    pub const fn blend(&self) -> CanonicalTrackBlend {
        self.blend
    }

    pub const fn priority(&self) -> i64 {
        self.priority
    }

    pub const fn fill(&self) -> CanonicalTrackFill {
        self.fill
    }

    pub const fn extrapolate_before(&self) -> CanonicalTrackFill {
        self.extrapolate_before
    }

    pub const fn extrapolate_after(&self) -> CanonicalTrackFill {
        self.extrapolate_after
    }

    pub fn pieces(&self) -> &[CanonicalTrackPiece] {
        &self.pieces
    }

    fn active_intervals(&self) -> Vec<(f64, f64)> {
        let mut intervals = Vec::new();
        let fill_is_active =
            |fill| !matches!(fill, CanonicalTrackFill::Base | CanonicalTrackFill::Error);
        let pieces = self
            .pieces
            .iter()
            .filter(|piece| {
                !matches!(piece, CanonicalTrackPiece::Point(point) if self.pieces.iter().any(|other| {
                    matches!(
                        other,
                        CanonicalTrackPiece::Segment(segment)
                            if segment.start.chart_time_seconds()
                                == point.time.chart_time_seconds()
                    )
                }))
            })
            .collect::<Vec<_>>();
        let first = pieces.first().expect("canonical Tracks are nonempty");
        if fill_is_active(self.extrapolate_before) {
            intervals.push((f64::NEG_INFINITY, first.time()));
        }
        for (index, piece) in pieces.iter().copied().enumerate() {
            match piece {
                CanonicalTrackPiece::Segment(segment) => intervals.push((
                    segment.start.chart_time_seconds(),
                    segment.end.chart_time_seconds(),
                )),
                CanonicalTrackPiece::Point(point) => intervals.push((
                    point.time.chart_time_seconds(),
                    pieces
                        .get(index + 1)
                        .map_or(f64::INFINITY, |piece| piece.time()),
                )),
            }
        }
        for pieces in pieces.windows(2) {
            let left_end = match pieces[0] {
                CanonicalTrackPiece::Segment(segment) => segment.end.chart_time_seconds(),
                CanonicalTrackPiece::Point(point) => point.time.chart_time_seconds(),
            };
            let right_start = pieces[1].time();
            if fill_is_active(self.fill) && left_end < right_start {
                intervals.push((left_end, right_start));
            }
        }
        if let Some(CanonicalTrackPiece::Segment(segment)) = pieces.last().copied()
            && fill_is_active(self.extrapolate_after)
        {
            intervals.push((segment.end.chart_time_seconds(), f64::INFINITY));
        }
        intervals
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalTrackSet {
    tracks: Vec<CanonicalTrack>,
}

impl CanonicalTrackSet {
    pub fn new(mut tracks: Vec<CanonicalTrack>) -> Result<Self, CanonicalTrackError> {
        let mut identities = BTreeSet::new();
        for track in &tracks {
            if !identities.insert((track.owner.value(), track.name.clone())) {
                return Err(CanonicalTrackError::DuplicateIdentity {
                    owner: track.owner.value(),
                    name: track.name.clone(),
                });
            }
        }
        let mut replace_groups: BTreeMap<(u64, CanonicalTrackTarget), Vec<&CanonicalTrack>> =
            BTreeMap::new();
        for track in &tracks {
            if track.blend == CanonicalTrackBlend::Replace {
                replace_groups
                    .entry((track.owner.value(), track.target))
                    .or_default()
                    .push(track);
            }
        }
        for group in replace_groups.values_mut() {
            group.sort_by_key(|track| std::cmp::Reverse(track.priority));
            let mut covered = Vec::new();
            let mut index = 0;
            while index < group.len() {
                let priority = group[index].priority;
                let mut effective = Vec::new();
                while index < group.len() && group[index].priority == priority {
                    let mut track_intervals = Vec::new();
                    for interval in group[index].active_intervals() {
                        track_intervals.extend(subtract_intervals(interval, &covered));
                    }
                    effective.push(track_intervals);
                    index += 1;
                }
                if track_intervals_overlap(&effective) {
                    return Err(CanonicalTrackError::ReplaceConflict);
                }
                for track in &group[..index] {
                    if track.priority == priority {
                        covered.extend(track.active_intervals());
                    }
                }
            }
        }
        tracks.sort_by(|left, right| {
            left.owner
                .value()
                .cmp(&right.owner.value())
                .then_with(|| left.target.cmp(&right.target))
                .then_with(|| left.priority.cmp(&right.priority))
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(Self { tracks })
    }

    pub fn tracks(&self) -> &[CanonicalTrack] {
        &self.tracks
    }
}

fn track_intervals_overlap(tracks: &[Vec<(f64, f64)>]) -> bool {
    tracks.iter().enumerate().any(|(index, left)| {
        tracks[index + 1..].iter().any(|right| {
            left.iter().any(|left| {
                right
                    .iter()
                    .any(|right| left.0.max(right.0) < left.1.min(right.1))
            })
        })
    })
}

fn subtract_intervals(interval: (f64, f64), covered: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut remaining = vec![interval];
    for cover in covered {
        let mut next = Vec::new();
        for candidate in remaining {
            if cover.1 <= candidate.0 || cover.0 >= candidate.1 {
                next.push(candidate);
                continue;
            }
            if candidate.0 < cover.0 {
                next.push((candidate.0, cover.0.min(candidate.1)));
            }
            if cover.1 < candidate.1 {
                next.push((cover.1.max(candidate.0), candidate.1));
            }
        }
        remaining = next;
    }
    remaining.retain(|(start, end)| start < end);
    remaining
}

fn validate_interpolation(
    interpolation: &CanonicalTrackInterpolation,
) -> Result<(), CanonicalTrackError> {
    match interpolation {
        CanonicalTrackInterpolation::Step | CanonicalTrackInterpolation::Linear => Ok(()),
        CanonicalTrackInterpolation::Easing(name) => is_core_easing(name)
            .then_some(())
            .ok_or_else(|| CanonicalTrackError::InvalidEasing { name: name.clone() }),
        CanonicalTrackInterpolation::CubicBezier([x1, y1, x2, y2]) => {
            if [*x1, *y1, *x2, *y2].into_iter().all(f64::is_finite)
                && (0.0..=1.0).contains(x1)
                && (0.0..=1.0).contains(x2)
            {
                Ok(())
            } else {
                Err(CanonicalTrackError::InvalidBezier)
            }
        }
    }
}

fn validate_pieces(pieces: &[CanonicalTrackPiece]) -> Result<(), CanonicalTrackError> {
    let segments = pieces.iter().filter_map(|piece| match piece {
        CanonicalTrackPiece::Segment(segment) => Some(segment),
        CanonicalTrackPiece::Point(_) => None,
    });
    let mut previous_end = None;
    for segment in segments {
        let start = segment.start.chart_time_seconds();
        if previous_end.is_some_and(|end| start < end) {
            return Err(CanonicalTrackError::Overlap);
        }
        previous_end = Some(segment.end.chart_time_seconds());
    }
    for (index, point) in pieces
        .iter()
        .filter_map(|piece| match piece {
            CanonicalTrackPiece::Point(point) => Some(point),
            CanonicalTrackPiece::Segment(_) => None,
        })
        .enumerate()
    {
        let time = point.time.chart_time_seconds();
        for segment in pieces.iter().filter_map(|piece| match piece {
            CanonicalTrackPiece::Segment(segment) => Some(segment),
            CanonicalTrackPiece::Point(_) => None,
        }) {
            let start = segment.start.chart_time_seconds();
            if time > start && time < segment.end.chart_time_seconds() {
                return Err(CanonicalTrackError::Overlap);
            }
            if time == start && point.value != segment.start_value {
                return Err(CanonicalTrackError::ReplaceConflict);
            }
        }
        for other in pieces
            .iter()
            .filter_map(|piece| match piece {
                CanonicalTrackPiece::Point(point) => Some(point),
                CanonicalTrackPiece::Segment(_) => None,
            })
            .skip(index + 1)
        {
            if other.time.chart_time_seconds() == time && other.value != point.value {
                return Err(CanonicalTrackError::ReplaceConflict);
            }
        }
    }
    Ok(())
}

fn is_core_easing(name: &str) -> bool {
    matches!(
        name,
        "linear"
            | "easeInSine"
            | "easeOutSine"
            | "easeInOutSine"
            | "easeInQuad"
            | "easeOutQuad"
            | "easeInOutQuad"
            | "easeInCubic"
            | "easeOutCubic"
            | "easeInOutCubic"
            | "easeInQuart"
            | "easeOutQuart"
            | "easeInOutQuart"
            | "easeInQuint"
            | "easeOutQuint"
            | "easeInOutQuint"
            | "easeInExpo"
            | "easeOutExpo"
            | "easeInOutExpo"
            | "easeInCirc"
            | "easeOutCirc"
            | "easeInOutCirc"
            | "easeInBack"
            | "easeOutBack"
            | "easeInOutBack"
            | "easeInElastic"
            | "easeOutElastic"
            | "easeInOutElastic"
            | "easeInBounce"
            | "easeOutBounce"
            | "easeInOutBounce"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalTrackError {
    WrongOwnerNamespace { id: u64 },
    EmptyName,
    Empty,
    DuplicateIdentity { owner: u64, name: String },
    TargetTypeMismatch,
    NonFiniteValue,
    InvalidInterval,
    Overlap,
    ReplaceConflict,
    Gap,
    InvalidEasing { name: String },
    InvalidBezier,
}

impl fmt::Display for CanonicalTrackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOwnerNamespace { id } => {
                write!(formatter, "stable ID {id} is not a Line ID")
            }
            Self::EmptyName => formatter.write_str("Track name must not be empty"),
            Self::Empty => formatter.write_str("Track segments must not be empty"),
            Self::DuplicateIdentity { owner, name } => {
                write!(
                    formatter,
                    "Line {owner} declares Track {name} more than once"
                )
            }
            Self::TargetTypeMismatch => {
                formatter.write_str("Track value does not match its target")
            }
            Self::NonFiniteValue => formatter.write_str("Track value must be finite"),
            Self::InvalidInterval => formatter.write_str("Track segment end must be after start"),
            Self::Overlap => formatter.write_str("Track pieces overlap"),
            Self::ReplaceConflict => formatter.write_str("Track replace values conflict"),
            Self::Gap => formatter.write_str("Track contains a gap with error fill"),
            Self::InvalidEasing { name } => write!(formatter, "unknown Core easing {name}"),
            Self::InvalidBezier => formatter.write_str("invalid cubic Bezier control values"),
        }
    }
}

impl std::error::Error for CanonicalTrackError {}
