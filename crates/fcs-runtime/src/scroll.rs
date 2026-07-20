use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fcs_model::{
    CanonicalLine, CanonicalLineGraph, CanonicalNote, CanonicalScrollLine, CanonicalScrollSet,
    CanonicalTrackInterpolation, CanonicalTrackPiece, CanonicalTrackSet, CanonicalTrackTarget,
    CanonicalTrackValue, EntityKind, ScrollCoordinateError, StableId,
};

use crate::{TrackEvaluationError, evaluate_track_set};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvaluatedLineScroll {
    line_id: u64,
    local_q: f64,
    local_velocity: f64,
    local_floor: f64,
    effective_velocity: f64,
    effective_floor: f64,
}

impl EvaluatedLineScroll {
    pub const fn line_id(self) -> u64 {
        self.line_id
    }

    pub const fn local_q(self) -> f64 {
        self.local_q
    }

    pub const fn local_velocity(self) -> f64 {
        self.local_velocity
    }

    pub const fn local_floor(self) -> f64 {
        self.local_floor
    }

    pub const fn effective_velocity(self) -> f64 {
        self.effective_velocity
    }

    pub const fn effective_floor(self) -> f64 {
        self.effective_floor
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineScrollDistance {
    distance: f64,
    local_y: f64,
}

impl LineScrollDistance {
    pub const fn distance(self) -> f64 {
        self.distance
    }

    pub const fn local_y(self) -> f64 {
        self.local_y
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScrollEvaluationError {
    NonFiniteChartTime,
    WrongLineNamespace {
        id: u64,
    },
    UnknownLine {
        id: u64,
    },
    MissingScrollLine {
        line: u64,
    },
    MissingParentState {
        line: u64,
        parent: u64,
    },
    Coordinate {
        line: u64,
        source: ScrollCoordinateError,
    },
    Track {
        line: u64,
        source: TrackEvaluationError,
    },
    SpeedTypeMismatch {
        line: u64,
    },
    ReverseNotAllowed {
        line: u64,
        speed: f64,
    },
    UnsupportedIntegration {
        line: u64,
    },
    NonFiniteResult {
        line: u64,
        field: &'static str,
    },
    NonFiniteNoteValue {
        note: u64,
        field: &'static str,
    },
}

impl fmt::Display for ScrollEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteChartTime => formatter.write_str("scroll chart time must be finite"),
            Self::WrongLineNamespace { id } => write!(formatter, "stable ID {id} is not a Line"),
            Self::UnknownLine { id } => write!(formatter, "unknown Line {id}"),
            Self::MissingScrollLine { line } => {
                write!(formatter, "Line {line} has no scroll descriptor")
            }
            Self::MissingParentState { line, parent } => {
                write!(
                    formatter,
                    "Line {line} is missing parent scroll state {parent}"
                )
            }
            Self::Coordinate { line, source } => {
                write!(formatter, "Line {line} scroll coordinate: {source}")
            }
            Self::Track { line, source } => {
                write!(formatter, "Line {line} scroll speed Track: {source}")
            }
            Self::SpeedTypeMismatch { line } => {
                write!(
                    formatter,
                    "Line {line} scroll speed Track has the wrong value type"
                )
            }
            Self::ReverseNotAllowed { line, speed } => {
                write!(
                    formatter,
                    "Line {line} local scroll speed {speed} is negative without reverse permission"
                )
            }
            Self::UnsupportedIntegration { line } => {
                write!(
                    formatter,
                    "Line {line} scroll speed is not supported by the bounded integrator"
                )
            }
            Self::NonFiniteResult { line, field } => {
                write!(formatter, "Line {line} scroll {field} is non-finite")
            }
            Self::NonFiniteNoteValue { note, field } => {
                write!(formatter, "Note {note} {field} is non-finite")
            }
        }
    }
}

impl std::error::Error for ScrollEvaluationError {}

pub fn evaluate_line_scroll(
    lines: &CanonicalLineGraph,
    scroll: &CanonicalScrollSet,
    tracks: &CanonicalTrackSet,
    line_id: &StableId,
    chart_time: f64,
) -> Result<EvaluatedLineScroll, ScrollEvaluationError> {
    validate_query(line_id, chart_time, lines)?;
    let required = required_ancestry(lines, line_id.value())?;
    let mut evaluated = BTreeMap::<u64, EvaluatedScrollState>::new();

    for id in lines
        .topological_order()
        .iter()
        .filter(|id| required.contains(&id.value()))
    {
        let line = lines
            .line(id.value())
            .expect("canonical topology only contains graph Lines");
        let descriptor = scroll
            .line(id.value())
            .ok_or(ScrollEvaluationError::MissingScrollLine { line: id.value() })?;
        let local = evaluate_local_scroll(line, descriptor, tracks, chart_time)?;
        let parent = if line.inherit().scroll() {
            line.parent()
                .map(|parent| {
                    evaluated.get(&parent.value()).ok_or(
                        ScrollEvaluationError::MissingParentState {
                            line: id.value(),
                            parent: parent.value(),
                        },
                    )
                })
                .transpose()?
        } else {
            None
        };
        let effective_velocity = match parent {
            Some(parent) => finite(
                id.value(),
                "effective velocity",
                local.local_velocity + parent.public.effective_velocity,
            )?,
            None => local.local_velocity,
        };
        let effective_floor = match parent {
            Some(parent) => parent
                .effective_floor
                .add(DoubleDouble::from_f64(local.local_floor)),
            None => DoubleDouble::from_f64(local.local_floor),
        };
        let public = EvaluatedLineScroll {
            line_id: id.value(),
            local_q: local.local_q,
            local_velocity: local.local_velocity,
            local_floor: local.local_floor,
            effective_velocity,
            effective_floor: finite(id.value(), "effective floor", effective_floor.to_f64())?,
        };
        evaluated.insert(
            id.value(),
            EvaluatedScrollState {
                public,
                effective_floor,
            },
        );
        if id.value() == line_id.value() {
            return Ok(public);
        }
    }

    Err(ScrollEvaluationError::UnknownLine {
        id: line_id.value(),
    })
}

pub fn evaluate_note_distance(
    lines: &CanonicalLineGraph,
    scroll: &CanonicalScrollSet,
    tracks: &CanonicalTrackSet,
    note: &CanonicalNote,
    chart_time: f64,
) -> Result<LineScrollDistance, ScrollEvaluationError> {
    if !chart_time.is_finite() {
        return Err(ScrollEvaluationError::NonFiniteChartTime);
    }
    let line_id = note.gameplay().line();
    if line_id.namespace() != EntityKind::Line {
        return Err(ScrollEvaluationError::WrongLineNamespace {
            id: line_id.value(),
        });
    }
    let line = lines
        .line(line_id.value())
        .ok_or(ScrollEvaluationError::UnknownLine {
            id: line_id.value(),
        })?;
    let scroll_factor = note.presentation().scroll_factor();
    let y_offset = note.presentation().y_offset();
    if !scroll_factor.is_finite() {
        return Err(ScrollEvaluationError::NonFiniteNoteValue {
            note: note.id().value(),
            field: "scroll factor",
        });
    }
    if !y_offset.is_finite() {
        return Err(ScrollEvaluationError::NonFiniteNoteValue {
            note: note.id().value(),
            field: "y offset",
        });
    }
    let at_note = evaluate_line_scroll(
        lines,
        scroll,
        tracks,
        line_id,
        note.gameplay().time().chart_time_seconds(),
    )?;
    let at_query = evaluate_line_scroll(lines, scroll, tracks, line_id, chart_time)?;
    let distance = finite_note(
        note.id().value(),
        "distance",
        (at_note.effective_floor() - at_query.effective_floor())
            * line.base().floor_scale()
            * scroll_factor,
    )?;
    let local_y = finite_note(note.id().value(), "local y", distance + y_offset)?;
    Ok(LineScrollDistance { distance, local_y })
}

#[derive(Debug, Clone, Copy)]
struct LocalScroll {
    local_q: f64,
    local_velocity: f64,
    local_floor: f64,
}

#[derive(Debug, Clone, Copy)]
struct EvaluatedScrollState {
    public: EvaluatedLineScroll,
    effective_floor: DoubleDouble,
}

fn validate_query(
    line_id: &StableId,
    chart_time: f64,
    lines: &CanonicalLineGraph,
) -> Result<(), ScrollEvaluationError> {
    if !chart_time.is_finite() {
        return Err(ScrollEvaluationError::NonFiniteChartTime);
    }
    if line_id.namespace() != EntityKind::Line {
        return Err(ScrollEvaluationError::WrongLineNamespace {
            id: line_id.value(),
        });
    }
    if lines.line(line_id.value()).is_none() {
        return Err(ScrollEvaluationError::UnknownLine {
            id: line_id.value(),
        });
    }
    Ok(())
}

fn required_ancestry(
    lines: &CanonicalLineGraph,
    target: u64,
) -> Result<BTreeSet<u64>, ScrollEvaluationError> {
    let mut required = BTreeSet::new();
    let mut current = Some(target);
    while let Some(id) = current {
        let line = lines
            .line(id)
            .ok_or(ScrollEvaluationError::UnknownLine { id })?;
        required.insert(id);
        current = if line.inherit().scroll() {
            line.parent().map(StableId::value)
        } else {
            None
        };
    }
    Ok(required)
}

fn evaluate_local_scroll(
    line: &CanonicalLine,
    descriptor: &CanonicalScrollLine,
    tracks: &CanonicalTrackSet,
    chart_time: f64,
) -> Result<LocalScroll, ScrollEvaluationError> {
    let line_id = line.id().value();
    let local_q = descriptor
        .coordinate()
        .coordinate(chart_time)
        .map_err(|source| ScrollEvaluationError::Coordinate {
            line: line_id,
            source,
        })?;
    let speed = speed_at(tracks, line.id(), descriptor.speed(), chart_time)?;
    if speed < 0.0 && !descriptor.allow_reverse_scroll() {
        return Err(ScrollEvaluationError::ReverseNotAllowed {
            line: line_id,
            speed,
        });
    }
    let bpm =
        descriptor
            .scroll_bpm(chart_time)
            .map_err(|source| ScrollEvaluationError::Coordinate {
                line: line_id,
                source,
            })?;
    let local_velocity = finite(line_id, "local velocity", speed * bpm / 60.0)?;
    let local_floor = integrate_floor(descriptor, tracks, line.id(), chart_time)?;
    Ok(LocalScroll {
        local_q,
        local_velocity,
        local_floor,
    })
}

fn speed_at(
    tracks: &CanonicalTrackSet,
    line_id: &StableId,
    base_speed: f64,
    chart_time: f64,
) -> Result<f64, ScrollEvaluationError> {
    let value = evaluate_track_set(
        tracks,
        line_id,
        CanonicalTrackTarget::ScrollSpeed,
        chart_time,
        CanonicalTrackValue::Float(base_speed),
    )
    .map_err(|source| ScrollEvaluationError::Track {
        line: line_id.value(),
        source,
    })?;
    match value {
        CanonicalTrackValue::Float(value) if value.is_finite() => Ok(value),
        _ => Err(ScrollEvaluationError::SpeedTypeMismatch {
            line: line_id.value(),
        }),
    }
}

fn integrate_floor(
    descriptor: &CanonicalScrollLine,
    tracks: &CanonicalTrackSet,
    line_id: &StableId,
    chart_time: f64,
) -> Result<f64, ScrollEvaluationError> {
    let origin = descriptor.integration_origin();
    if chart_time == origin {
        return Ok(descriptor.initial_floor_position());
    }
    let (lower, upper, direction) = if chart_time > origin {
        (origin, chart_time, 1.0)
    } else {
        (chart_time, origin, -1.0)
    };
    let mut boundaries = vec![lower, upper];
    for point in descriptor.coordinate().points() {
        if point.chart_time() > lower && point.chart_time() < upper {
            boundaries.push(point.chart_time());
        }
    }
    for track in tracks.tracks().iter().filter(|track| {
        track.owner() == line_id && track.target() == CanonicalTrackTarget::ScrollSpeed
    }) {
        for piece in track.pieces() {
            match piece {
                CanonicalTrackPiece::Segment(segment) => {
                    if segment.start().chart_time_seconds() > lower
                        && segment.start().chart_time_seconds() < upper
                    {
                        boundaries.push(segment.start().chart_time_seconds());
                    }
                    if segment.end().chart_time_seconds() > lower
                        && segment.end().chart_time_seconds() < upper
                    {
                        boundaries.push(segment.end().chart_time_seconds());
                    }
                    if segment.start().chart_time_seconds() < upper
                        && segment.end().chart_time_seconds() > lower
                        && !matches!(segment.interpolation(), CanonicalTrackInterpolation::Step)
                        && segment.start_value() != segment.end_value()
                    {
                        return Err(ScrollEvaluationError::UnsupportedIntegration {
                            line: line_id.value(),
                        });
                    }
                }
                CanonicalTrackPiece::Point(point) => {
                    if point.time().chart_time_seconds() > lower
                        && point.time().chart_time_seconds() < upper
                    {
                        boundaries.push(point.time().chart_time_seconds());
                    }
                }
            }
        }
    }
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup_by(|left, right| *left == *right);
    let mut integral = 0.0;
    for window in boundaries.windows(2) {
        let start = window[0];
        let end = window[1];
        let sample = midpoint(start, end);
        let speed = speed_at(tracks, line_id, descriptor.speed(), sample)?;
        let q_start = descriptor
            .coordinate()
            .coordinate(start)
            .map_err(|source| ScrollEvaluationError::Coordinate {
                line: line_id.value(),
                source,
            })?;
        let q_end = descriptor.coordinate().coordinate(end).map_err(|source| {
            ScrollEvaluationError::Coordinate {
                line: line_id.value(),
                source,
            }
        })?;
        let contribution = finite(
            line_id.value(),
            "floor integration contribution",
            speed * (q_end - q_start),
        )?;
        integral = finite(
            line_id.value(),
            "floor integration",
            integral + contribution,
        )?;
    }
    finite(
        line_id.value(),
        "local floor",
        descriptor.initial_floor_position() + direction * integral,
    )
}

fn midpoint(start: f64, end: f64) -> f64 {
    let difference = end - start;
    if difference.is_finite() {
        start + difference * 0.5
    } else {
        start * 0.5 + end * 0.5
    }
}

fn finite(line: u64, field: &'static str, value: f64) -> Result<f64, ScrollEvaluationError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(ScrollEvaluationError::NonFiniteResult { line, field })
}

fn finite_note(note: u64, field: &'static str, value: f64) -> Result<f64, ScrollEvaluationError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(ScrollEvaluationError::NonFiniteNoteValue { note, field })
}

#[derive(Debug, Clone, Copy)]
struct DoubleDouble {
    hi: f64,
    lo: f64,
}

impl DoubleDouble {
    fn from_f64(value: f64) -> Self {
        Self { hi: value, lo: 0.0 }
    }

    fn add(self, other: Self) -> Self {
        let (sum, error) = two_sum(self.hi, other.hi);
        let correction = self.lo + other.lo + error;
        let (hi, lo) = two_sum(sum, correction);
        Self { hi, lo }
    }

    fn to_f64(self) -> f64 {
        if self.lo == 0.0 {
            self.hi
        } else {
            self.hi + self.lo
        }
    }
}

fn two_sum(left: f64, right: f64) -> (f64, f64) {
    let sum = left + right;
    let virtual_right = sum - left;
    let virtual_left = sum - virtual_right;
    let right_error = right - virtual_right;
    let left_error = left - virtual_left;
    (sum, left_error + right_error)
}

#[cfg(test)]
mod tests {
    use fcs_model::{
        CanonicalLineBase, CanonicalLineInherit, CanonicalScrollCoordinate,
        CanonicalScrollTempoPoint, CanonicalTextualId, CanonicalTime, CanonicalTrack,
        CanonicalTrackBlend, CanonicalTrackFill, CanonicalTrackPiece, CanonicalTrackSegment,
        EntityKind, StableIdRegistry,
    };

    use super::*;

    fn id(registry: &mut StableIdRegistry, name: &str) -> StableId {
        registry
            .insert(
                EntityKind::Line,
                CanonicalTextualId::explicit(name).unwrap(),
            )
            .unwrap()
    }

    fn time(value: f64) -> CanonicalTime {
        CanonicalTime::from_chart_time_seconds(value).unwrap()
    }

    fn speed_track(
        owner: StableId,
        name: &str,
        start: f64,
        end: f64,
        value: f64,
    ) -> CanonicalTrack {
        CanonicalTrack::new(
            owner,
            name,
            CanonicalTrackTarget::ScrollSpeed,
            CanonicalTrackBlend::Replace,
            0,
            CanonicalTrackFill::HoldAfter,
            CanonicalTrackFill::HoldBefore,
            CanonicalTrackFill::HoldAfter,
            vec![CanonicalTrackPiece::Segment(
                CanonicalTrackSegment::new(
                    time(start),
                    time(end),
                    CanonicalTrackValue::Float(value),
                    CanonicalTrackValue::Float(value),
                    CanonicalTrackInterpolation::Step,
                    0,
                )
                .unwrap(),
            )],
        )
        .unwrap()
    }

    fn descriptor(id: StableId, speed: f64, initial: f64, origin: f64) -> CanonicalScrollLine {
        CanonicalScrollLine::new(
            id,
            CanonicalScrollCoordinate::new([
                CanonicalScrollTempoPoint::new(0.0, 120.0).unwrap(),
                CanonicalScrollTempoPoint::new(2.0, 60.0).unwrap(),
            ])
            .unwrap(),
            speed,
            true,
            10.0,
            origin,
            initial,
        )
        .unwrap()
    }

    #[test]
    fn local_floor_is_direct_seek_and_preserves_signed_zero() {
        let mut registry = StableIdRegistry::new();
        let line_id = id(&mut registry, "main");
        let line = CanonicalLine::new(
            line_id.clone(),
            None,
            0,
            CanonicalLineBase::identity(),
            CanonicalLineInherit::default(),
            fcs_model::CanonicalScrollTempo::Global,
        )
        .unwrap();
        let graph = CanonicalLineGraph::new([line]).unwrap();
        let scroll =
            CanonicalScrollSet::new([descriptor(line_id.clone(), 1.0, -0.0, 0.0)]).unwrap();
        let tracks =
            CanonicalTrackSet::new([speed_track(line_id.clone(), "speed", 0.0, 2.0, 0.0)]).unwrap();

        let origin = evaluate_line_scroll(&graph, &scroll, &tracks, &line_id, 0.0).unwrap();
        assert_eq!(origin.effective_floor().to_bits(), 0x8000_0000_0000_0000);
        let later = evaluate_line_scroll(&graph, &scroll, &tracks, &line_id, 1.0).unwrap();
        assert_eq!(later.effective_floor().to_bits(), 0);
        let reverse = evaluate_line_scroll(&graph, &scroll, &tracks, &line_id, -1.0).unwrap();
        assert_eq!(reverse.local_q(), -2.0);
    }

    #[test]
    fn static_speed_is_used_when_no_scroll_track_is_present() {
        let mut registry = StableIdRegistry::new();
        let line_id = id(&mut registry, "static-speed");
        let line = CanonicalLine::new(
            line_id.clone(),
            None,
            0,
            CanonicalLineBase::identity(),
            CanonicalLineInherit::default(),
            fcs_model::CanonicalScrollTempo::Global,
        )
        .unwrap();
        let graph = CanonicalLineGraph::new([line]).unwrap();
        let scroll = CanonicalScrollSet::new([descriptor(line_id.clone(), 2.0, 3.0, 0.0)]).unwrap();
        let tracks = CanonicalTrackSet::new(Vec::<CanonicalTrack>::new()).unwrap();

        let result = evaluate_line_scroll(&graph, &scroll, &tracks, &line_id, 1.0).unwrap();
        assert_eq!(result.local_velocity(), 4.0);
        assert_eq!(result.local_floor(), 7.0);
    }

    #[test]
    fn effective_scroll_uses_only_enabled_actual_ancestry() {
        let mut registry = StableIdRegistry::new();
        let root_id = id(&mut registry, "root");
        let child_id = id(&mut registry, "child");
        let detached_id = id(&mut registry, "detached");
        let root = CanonicalLine::new(
            root_id.clone(),
            None,
            0,
            CanonicalLineBase::identity(),
            CanonicalLineInherit::default(),
            fcs_model::CanonicalScrollTempo::Global,
        )
        .unwrap();
        let child = CanonicalLine::new(
            child_id.clone(),
            Some(root_id.clone()),
            1,
            CanonicalLineBase::identity(),
            CanonicalLineInherit::new(true, true, true, true, true),
            fcs_model::CanonicalScrollTempo::Global,
        )
        .unwrap();
        let detached = CanonicalLine::new(
            detached_id.clone(),
            Some(root_id.clone()),
            2,
            CanonicalLineBase::identity(),
            CanonicalLineInherit::default(),
            fcs_model::CanonicalScrollTempo::Global,
        )
        .unwrap();
        let graph = CanonicalLineGraph::new([root, child, detached]).unwrap();
        let scroll = CanonicalScrollSet::new([
            descriptor(root_id.clone(), 1.0, 2.0, 0.0),
            descriptor(child_id.clone(), 1.0, 3.0, 0.0),
            descriptor(detached_id.clone(), 1.0, 5.0, 0.0),
        ])
        .unwrap();
        let tracks = CanonicalTrackSet::new([
            speed_track(root_id.clone(), "speed", 0.0, 1.0, 1.0),
            speed_track(child_id.clone(), "speed", 0.0, 1.0, 1.0),
            speed_track(detached_id.clone(), "speed", 0.0, 1.0, 1.0),
        ])
        .unwrap();

        let child_result = evaluate_line_scroll(&graph, &scroll, &tracks, &child_id, 0.5).unwrap();
        assert_eq!(child_result.effective_floor(), 6.0);
        assert_eq!(child_result.effective_velocity(), 4.0);
        let detached_result =
            evaluate_line_scroll(&graph, &scroll, &tracks, &detached_id, 0.5).unwrap();
        assert_eq!(detached_result.effective_floor(), 5.5);
        assert_eq!(detached_result.effective_velocity(), 1.0);
    }
}
