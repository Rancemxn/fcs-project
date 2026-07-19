//! Exact canonical Line scroll coordinates for the I3.7 constant-speed seam.

use std::fmt;

use crate::{CanonicalScrollTempo, ChartTimeMap, EntityKind, ScrollTempoKey, StableId};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalScrollTempoPoint {
    chart_time: f64,
    bpm: f64,
}

impl CanonicalScrollTempoPoint {
    pub fn new(chart_time: f64, bpm: f64) -> Result<Self, ScrollCoordinateError> {
        if !chart_time.is_finite() {
            return Err(ScrollCoordinateError::NonFinite);
        }
        if !bpm.is_finite() || bpm <= 0.0 {
            return Err(ScrollCoordinateError::InvalidBpm);
        }
        Ok(Self { chart_time, bpm })
    }

    pub const fn chart_time(self) -> f64 {
        self.chart_time
    }

    pub const fn bpm(self) -> f64 {
        self.bpm
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalScrollCoordinate {
    points: Vec<CanonicalScrollTempoPoint>,
}

impl CanonicalScrollCoordinate {
    pub fn new(
        points: impl IntoIterator<Item = CanonicalScrollTempoPoint>,
    ) -> Result<Self, ScrollCoordinateError> {
        let mut normalized = Vec::new();
        for point in points {
            if let Some(previous) = normalized.last_mut() {
                if point.chart_time < previous.chart_time {
                    return Err(ScrollCoordinateError::NonMonotonic);
                }
                if point.chart_time == previous.chart_time {
                    *previous = point;
                    continue;
                }
            }
            normalized.push(point);
        }
        let first = normalized.first().ok_or(ScrollCoordinateError::Empty)?;
        if first.chart_time != 0.0 {
            return Err(ScrollCoordinateError::FirstPointNotZero);
        }
        Ok(Self { points: normalized })
    }

    pub fn points(&self) -> &[CanonicalScrollTempoPoint] {
        &self.points
    }

    pub fn coordinate(&self, chart_time: f64) -> Result<f64, ScrollCoordinateError> {
        let (integral, _) = self.integral_and_bpm(chart_time)?;
        Ok(integral)
    }

    fn integral_and_bpm(&self, chart_time: f64) -> Result<(f64, f64), ScrollCoordinateError> {
        if !chart_time.is_finite() {
            return Err(ScrollCoordinateError::NonFinite);
        }
        let first = self.points.first().ok_or(ScrollCoordinateError::Empty)?;
        let mut integral = 0.0;
        let mut start = first.chart_time;
        let mut bpm = first.bpm;
        if chart_time < start {
            return finite_pair(integral + (chart_time - start) * bpm / 60.0, bpm);
        }
        for point in self.points.iter().skip(1) {
            if chart_time < point.chart_time {
                integral += (chart_time - start) * bpm / 60.0;
                return finite_pair(integral, bpm);
            }
            integral += (point.chart_time - start) * bpm / 60.0;
            start = point.chart_time;
            bpm = point.bpm;
        }
        finite_pair(integral + (chart_time - start) * bpm / 60.0, bpm)
    }

    pub fn bpm(&self, chart_time: f64) -> Result<f64, ScrollCoordinateError> {
        self.integral_and_bpm(chart_time).map(|(_, bpm)| bpm)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalScrollLine {
    line_id: StableId,
    coordinate: CanonicalScrollCoordinate,
    speed: f64,
    allow_reverse_scroll: bool,
    floor_scale: f64,
    integration_origin: f64,
    initial_floor_position: f64,
}

impl CanonicalScrollLine {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        line_id: StableId,
        coordinate: CanonicalScrollCoordinate,
        speed: f64,
        allow_reverse_scroll: bool,
        floor_scale: f64,
        integration_origin: f64,
        initial_floor_position: f64,
    ) -> Result<Self, ScrollCoordinateError> {
        if line_id.namespace() != EntityKind::Line {
            return Err(ScrollCoordinateError::WrongLineNamespace);
        }
        if !speed.is_finite()
            || (!allow_reverse_scroll && speed < 0.0)
            || !floor_scale.is_finite()
            || floor_scale <= 0.0
            || !integration_origin.is_finite()
            || !initial_floor_position.is_finite()
        {
            return Err(ScrollCoordinateError::InvalidLinePolicy);
        }
        Ok(Self {
            line_id,
            coordinate,
            speed,
            allow_reverse_scroll,
            floor_scale,
            integration_origin,
            initial_floor_position,
        })
    }

    pub const fn line_id(&self) -> &StableId {
        &self.line_id
    }

    pub const fn coordinate(&self) -> &CanonicalScrollCoordinate {
        &self.coordinate
    }

    pub const fn speed(&self) -> f64 {
        self.speed
    }

    pub const fn allow_reverse_scroll(&self) -> bool {
        self.allow_reverse_scroll
    }

    pub const fn floor_scale(&self) -> f64 {
        self.floor_scale
    }

    pub const fn integration_origin(&self) -> f64 {
        self.integration_origin
    }

    pub const fn initial_floor_position(&self) -> f64 {
        self.initial_floor_position
    }

    pub fn floor_position(&self, chart_time: f64) -> Result<f64, ScrollCoordinateError> {
        let origin = self.coordinate.coordinate(self.integration_origin)?;
        let current = self.coordinate.coordinate(chart_time)?;
        finite(self.initial_floor_position + self.speed * (current - origin))
    }

    pub fn scroll_bpm(&self, chart_time: f64) -> Result<f64, ScrollCoordinateError> {
        self.coordinate.bpm(chart_time)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalScrollSet {
    lines: Vec<CanonicalScrollLine>,
}

impl CanonicalScrollSet {
    pub fn new(mut lines: Vec<CanonicalScrollLine>) -> Result<Self, ScrollCoordinateError> {
        lines.sort_by_key(|line| line.line_id().value());
        if lines
            .windows(2)
            .any(|pair| pair[0].line_id().value() == pair[1].line_id().value())
        {
            return Err(ScrollCoordinateError::DuplicateLine);
        }
        Ok(Self { lines })
    }

    pub fn lines(&self) -> &[CanonicalScrollLine] {
        &self.lines
    }

    pub fn line(&self, line_id: u64) -> Option<&CanonicalScrollLine> {
        self.lines
            .iter()
            .find(|line| line.line_id().value() == line_id)
    }
}

pub fn global_coordinate(
    time_map: &ChartTimeMap,
) -> Result<CanonicalScrollCoordinate, ScrollCoordinateError> {
    CanonicalScrollCoordinate::new(
        time_map
            .segments()
            .map(|(_, chart_time, bpm)| CanonicalScrollTempoPoint::new(chart_time, bpm))
            .collect::<Result<Vec<_>, _>>()?,
    )
}

pub fn coordinate_for_tempo(
    tempo: &CanonicalScrollTempo,
    time_map: &ChartTimeMap,
) -> Result<CanonicalScrollCoordinate, ScrollCoordinateError> {
    match tempo {
        CanonicalScrollTempo::Global => global_coordinate(time_map),
        CanonicalScrollTempo::Override(map) => CanonicalScrollCoordinate::new(
            map.points()
                .iter()
                .map(|point| {
                    let chart_time = match point.key() {
                        ScrollTempoKey::Beat(beat) => time_map
                            .chart_time(beat)
                            .map_err(|_| ScrollCoordinateError::NonFinite)?
                            .chart_time_seconds(),
                        ScrollTempoKey::Time(time) => time,
                    };
                    CanonicalScrollTempoPoint::new(chart_time, point.bpm())
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
    }
}

fn finite(value: f64) -> Result<f64, ScrollCoordinateError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(ScrollCoordinateError::NonFinite)
}

fn finite_pair(value: f64, bpm: f64) -> Result<(f64, f64), ScrollCoordinateError> {
    finite(value).map(|value| (value, bpm))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollCoordinateError {
    Empty,
    FirstPointNotZero,
    NonMonotonic,
    InvalidBpm,
    NonFinite,
    InvalidLinePolicy,
    DuplicateLine,
    WrongLineNamespace,
}

impl fmt::Display for ScrollCoordinateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "scroll coordinate requires at least one tempo point",
            Self::FirstPointNotZero => "scroll coordinate must start at chartTime zero",
            Self::NonMonotonic => "scroll coordinate points must be non-decreasing",
            Self::InvalidBpm => "scroll coordinate BPM must be finite and positive",
            Self::NonFinite => "scroll coordinate result must be finite",
            Self::InvalidLinePolicy => "Line scroll policy is invalid",
            Self::DuplicateLine => "scroll set contains a duplicate Line",
            Self::WrongLineNamespace => "scroll Line ID has the wrong namespace",
        })
    }
}

impl std::error::Error for ScrollCoordinateError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanonicalScrollTempoMap, CanonicalTextualId, ScrollTempoKey, StableIdRegistry};

    fn point(chart_time: f64, bpm: f64) -> CanonicalScrollTempoPoint {
        CanonicalScrollTempoPoint::new(chart_time, bpm).unwrap()
    }

    fn line_id() -> StableId {
        let mut registry = StableIdRegistry::new();
        registry
            .insert(
                EntityKind::Line,
                CanonicalTextualId::explicit("main").unwrap(),
            )
            .unwrap()
    }

    #[test]
    fn coordinate_is_exact_for_direct_seek_and_extrapolation() {
        let coordinate =
            CanonicalScrollCoordinate::new([point(0.0, 60.0), point(2.0, 120.0)]).unwrap();
        assert_eq!(coordinate.coordinate(3.0), Ok(4.0));
        assert_eq!(coordinate.coordinate(-1.0), Ok(-1.0));
        assert_eq!(coordinate.coordinate(0.0), Ok(0.0));
        assert_eq!(coordinate.coordinate(1.0), Ok(1.0));
        assert_eq!(coordinate.coordinate(3.0), Ok(4.0));
    }

    #[test]
    fn time_override_duplicate_key_uses_the_final_bpm() {
        let map = CanonicalScrollTempoMap::new([
            crate::CanonicalScrollTempoPoint::new(ScrollTempoKey::Time(0.0), 60.0).unwrap(),
            crate::CanonicalScrollTempoPoint::new(ScrollTempoKey::Time(2.0), 90.0).unwrap(),
            crate::CanonicalScrollTempoPoint::new(ScrollTempoKey::Time(2.0), 120.0).unwrap(),
        ])
        .unwrap();
        let global = ChartTimeMap::new([crate::TempoPoint {
            beat: crate::Beat::zero(),
            bpm: 120.0,
        }])
        .unwrap();
        let coordinate =
            coordinate_for_tempo(&CanonicalScrollTempo::Override(map), &global).unwrap();
        assert_eq!(coordinate.points().len(), 2);
        assert_eq!(coordinate.bpm(2.0), Ok(120.0));
        assert_eq!(coordinate.coordinate(3.0), Ok(4.0));
    }

    #[test]
    fn constant_speed_policy_allows_zero_and_gates_reverse() {
        let coordinate = CanonicalScrollCoordinate::new([point(0.0, 60.0)]).unwrap();
        assert!(
            CanonicalScrollLine::new(line_id(), coordinate.clone(), 0.0, false, 120.0, 0.0, 0.0)
                .is_ok()
        );
        assert_eq!(
            CanonicalScrollLine::new(line_id(), coordinate.clone(), -1.0, false, 120.0, 0.0, 0.0,),
            Err(ScrollCoordinateError::InvalidLinePolicy)
        );
        assert!(
            CanonicalScrollLine::new(line_id(), coordinate, -1.0, true, 120.0, 0.0, 0.0,).is_ok()
        );
    }
}
