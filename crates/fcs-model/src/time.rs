//! Exact source-beat and deterministic global chart-time normalization.

use std::cmp::Ordering;
use std::fmt;

/// A normalized exact rational chart beat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Beat {
    numerator: i64,
    denominator: i64,
}

impl Beat {
    pub fn new(numerator: i64, denominator: i64) -> Result<Self, TempoError> {
        if denominator == 0 {
            return Err(TempoError::InvalidBeat);
        }
        let mut numerator = numerator as i128;
        let mut denominator = denominator as i128;
        if denominator < 0 {
            numerator = numerator.checked_neg().ok_or(TempoError::InvalidBeat)?;
            denominator = denominator.checked_neg().ok_or(TempoError::InvalidBeat)?;
        }
        let divisor = gcd(numerator.unsigned_abs(), denominator as u128) as i128;
        Ok(Self {
            numerator: i64::try_from(numerator / divisor).map_err(|_| TempoError::InvalidBeat)?,
            denominator: i64::try_from(denominator / divisor)
                .map_err(|_| TempoError::InvalidBeat)?,
        })
    }

    pub const fn numerator(self) -> i64 {
        self.numerator
    }

    pub const fn denominator(self) -> i64 {
        self.denominator
    }

    pub const fn zero() -> Self {
        Self {
            numerator: 0,
            denominator: 1,
        }
    }

    pub fn as_f64(self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }
}

impl PartialOrd for Beat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Beat {
    fn cmp(&self, other: &Self) -> Ordering {
        let left = self.numerator as i128 * other.denominator as i128;
        let right = other.numerator as i128 * self.denominator as i128;
        left.cmp(&right)
    }
}

/// A source tempo point before canonical validation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempoPoint {
    pub beat: Beat,
    pub bpm: f64,
}

/// A canonical chart-time value with optional exact beat provenance.
#[derive(Debug, Clone, Copy)]
pub struct CanonicalTime {
    source_beat: Option<Beat>,
    chart_time_seconds: f64,
}

impl PartialEq for CanonicalTime {
    fn eq(&self, other: &Self) -> bool {
        self.chart_time_seconds == other.chart_time_seconds
    }
}

impl CanonicalTime {
    /// Returns exact source beat provenance when the value originated as a beat.
    pub const fn source_beat(self) -> Option<Beat> {
        self.source_beat
    }

    /// Constructs a canonical time directly from a finite source-time value.
    pub fn from_chart_time_seconds(chart_time_seconds: f64) -> Result<Self, TempoError> {
        if chart_time_seconds.is_finite() {
            Ok(Self {
                source_beat: None,
                chart_time_seconds,
            })
        } else {
            Err(TempoError::NonFiniteChartTime)
        }
    }

    pub const fn chart_time_seconds(self) -> f64 {
        self.chart_time_seconds
    }
}

/// A finite audio offset and the exact affine conversion around chart time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioOffset(f64);

impl AudioOffset {
    pub fn new(seconds: f64) -> Result<Self, TempoError> {
        if seconds.is_finite() {
            Ok(Self(seconds))
        } else {
            Err(TempoError::NonFiniteAudioOffset)
        }
    }

    pub const fn seconds(self) -> f64 {
        self.0
    }

    pub fn audio_time(self, chart_time: f64) -> Result<f64, TempoError> {
        finite_time(chart_time)?;
        let audio_time = chart_time + self.0;
        audio_time
            .is_finite()
            .then_some(audio_time)
            .ok_or(TempoError::NonFiniteChartTime)
    }

    pub fn chart_time(self, audio_time: f64) -> Result<f64, TempoError> {
        finite_time(audio_time)?;
        let chart_time = audio_time - self.0;
        chart_time
            .is_finite()
            .then_some(chart_time)
            .ok_or(TempoError::NonFiniteChartTime)
    }
}

/// A validated deterministic piecewise-constant global tempo map.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartTimeMap {
    segments: Vec<Segment>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Segment {
    beat: Beat,
    chart_time_seconds: f64,
    bpm: f64,
}

impl ChartTimeMap {
    pub fn new(points: impl IntoIterator<Item = TempoPoint>) -> Result<Self, TempoError> {
        let points: Vec<_> = points.into_iter().collect();
        if points.is_empty() {
            return Err(TempoError::EmptyTempoMap);
        }
        if points[0].beat != Beat::zero() {
            return Err(TempoError::FirstPointNotZero);
        }

        let mut segments: Vec<Segment> = Vec::new();
        for point in points {
            if !point.bpm.is_finite() || point.bpm <= 0.0 {
                return Err(TempoError::InvalidBpm);
            }
            if let Some(previous) = segments.last().copied() {
                match point.beat.cmp(&previous.beat) {
                    Ordering::Less => return Err(TempoError::NonMonotonicTempo),
                    Ordering::Equal => {
                        // The last point at a beat is the instantaneous step value.
                        let last = segments.last_mut().expect("segment exists");
                        last.bpm = point.bpm;
                        continue;
                    }
                    Ordering::Greater => {
                        let chart_time_seconds = advance_chart_time(
                            previous.chart_time_seconds,
                            previous.beat,
                            point.beat,
                            previous.bpm,
                        )?;
                        segments.push(Segment {
                            beat: point.beat,
                            chart_time_seconds,
                            bpm: point.bpm,
                        });
                    }
                }
            } else {
                segments.push(Segment {
                    beat: point.beat,
                    chart_time_seconds: 0.0,
                    bpm: point.bpm,
                });
            }
        }

        Ok(Self { segments })
    }

    pub fn chart_time(&self, beat: Beat) -> Result<CanonicalTime, TempoError> {
        let first = self.segments.first().ok_or(TempoError::EmptyTempoMap)?;
        let (chart_time_seconds, source_beat) = if beat < first.beat {
            (
                advance_chart_time(0.0, Beat::zero(), beat, first.bpm)?,
                beat,
            )
        } else {
            let index = self.segment_for_beat(beat);
            let segment = self.segments[index];
            (
                advance_chart_time(segment.chart_time_seconds, segment.beat, beat, segment.bpm)?,
                beat,
            )
        };
        if !chart_time_seconds.is_finite() {
            return Err(TempoError::NonFiniteChartTime);
        }
        Ok(CanonicalTime {
            source_beat: Some(source_beat),
            chart_time_seconds,
        })
    }

    pub fn beat_at_time(&self, chart_time_seconds: f64) -> Result<f64, TempoError> {
        finite_time(chart_time_seconds)?;
        let first = self.segments.first().ok_or(TempoError::EmptyTempoMap)?;
        if chart_time_seconds < first.chart_time_seconds {
            let beat = (chart_time_seconds * first.bpm) / 60.0;
            return beat
                .is_finite()
                .then_some(beat)
                .ok_or(TempoError::NonFiniteBeat);
        }
        let index = self.segment_for_time(chart_time_seconds);
        let segment = self.segments[index];
        let beat = segment.beat.as_f64()
            + ((chart_time_seconds - segment.chart_time_seconds) * segment.bpm) / 60.0;
        if beat.is_finite() {
            Ok(beat)
        } else {
            Err(TempoError::NonFiniteBeat)
        }
    }

    pub fn segments(&self) -> impl Iterator<Item = (Beat, f64, f64)> + '_ {
        self.segments
            .iter()
            .map(|segment| (segment.beat, segment.chart_time_seconds, segment.bpm))
    }

    fn segment_for_beat(&self, beat: Beat) -> usize {
        self.segments
            .partition_point(|segment| segment.beat <= beat)
            - 1
    }

    fn segment_for_time(&self, chart_time_seconds: f64) -> usize {
        self.segments
            .partition_point(|segment| segment.chart_time_seconds <= chart_time_seconds)
            - 1
    }
}

fn finite_time(value: f64) -> Result<(), TempoError> {
    value
        .is_finite()
        .then_some(())
        .ok_or(TempoError::NonFiniteChartTime)
}

/// Advances a chart time using the exact rational beat delta.
///
/// The compensated path mirrors FCBC section 10 so a writer-produced tempo
/// point remains within the loader's two-ULP reference tolerance. The scaled
/// fallback keeps finite results representable when an intermediate product
/// overflows or underflows despite the final quotient being finite.
fn advance_chart_time(
    chart_time: f64,
    start: Beat,
    end: Beat,
    bpm: f64,
) -> Result<f64, TempoError> {
    let delta_numerator = end.numerator as i128 * start.denominator as i128
        - start.numerator as i128 * end.denominator as i128;
    let delta_denominator = start.denominator as i128 * end.denominator as i128;
    let (numerator, numerator_error) = split_integer(delta_numerator);
    let (denominator, denominator_error) = split_integer(delta_denominator);
    let (top, top_error) = two_product(numerator, 60.0);
    let top_error = top_error + numerator_error * 60.0;
    let (bottom, bottom_error) = two_product(denominator, bpm);

    let (quotient, correction) = if top.is_finite() && bottom.is_finite() && bottom != 0.0 {
        let quotient = top / bottom;
        let (product, product_error) = two_product(quotient, bottom);
        let residual = ((top - product) - product_error) + top_error
            - quotient * (bottom_error + denominator_error * bpm);
        let correction = residual / bottom;
        (
            quotient,
            if correction.is_finite() {
                correction
            } else {
                0.0
            },
        )
    } else {
        (
            (delta_numerator as f64 / delta_denominator as f64) * 60.0 / bpm,
            0.0,
        )
    };
    let (seconds, seconds_error) = two_sum(chart_time, quotient);
    let chart_time_seconds = seconds + (seconds_error + correction);
    chart_time_seconds
        .is_finite()
        .then_some(chart_time_seconds)
        .ok_or(TempoError::NonFiniteChartTime)
}

fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let sum = a + b;
    let b_virtual = sum - a;
    let a_virtual = sum - b_virtual;
    (sum, (a - a_virtual) + (b - b_virtual))
}

fn two_product(a: f64, b: f64) -> (f64, f64) {
    let product = a * b;
    (product, a.mul_add(b, -product))
}

fn split_integer(value: i128) -> (f64, f64) {
    let high = value as f64;
    (high, (value - high as i128) as f64)
}

const fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    if a == 0 { 1 } else { a }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempoError {
    InvalidBeat,
    EmptyTempoMap,
    FirstPointNotZero,
    NonMonotonicTempo,
    InvalidBpm,
    NonFiniteChartTime,
    NonFiniteBeat,
    NonFiniteAudioOffset,
}

impl fmt::Display for TempoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidBeat => "invalid exact beat",
            Self::EmptyTempoMap => "tempo map must not be empty",
            Self::FirstPointNotZero => "first tempo point must be at zero beat",
            Self::NonMonotonicTempo => "tempo points must be non-decreasing",
            Self::InvalidBpm => "tempo BPM must be finite and positive",
            Self::NonFiniteChartTime => "chart time must be finite",
            Self::NonFiniteBeat => "chart beat must be finite",
            Self::NonFiniteAudioOffset => "audio offset must be finite",
        })
    }
}

impl std::error::Error for TempoError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn beat(numerator: i64, denominator: i64) -> Beat {
        Beat::new(numerator, denominator).unwrap()
    }

    fn map(points: &[(i64, i64, f64)]) -> ChartTimeMap {
        ChartTimeMap::new(
            points
                .iter()
                .map(|&(numerator, denominator, bpm)| TempoPoint {
                    beat: beat(numerator, denominator),
                    bpm,
                }),
        )
        .unwrap()
    }

    #[test]
    fn exact_beat_maps_through_piecewise_tempo_and_inverts() {
        let map = map(&[(0, 1, 120.0), (4, 1, 240.0)]);
        assert_eq!(
            map.chart_time(beat(2, 1)).unwrap().chart_time_seconds(),
            1.0
        );
        assert_eq!(
            map.chart_time(beat(6, 1)).unwrap().chart_time_seconds(),
            2.5
        );
        assert_eq!(map.beat_at_time(1.0).unwrap(), 2.0);
        assert_eq!(map.beat_at_time(2.5).unwrap(), 6.0);
    }

    #[test]
    fn non_dyadic_tempo_deltas_use_exact_rational_beats() {
        let map = map(&[(0, 1, 960.0), (8001, 10, 60.0), (8003, 10, 120.0)]);
        let segments: Vec<_> = map.segments().collect();
        assert_eq!(segments[1].1, (8001.0 * 60.0) / (10.0 * 960.0));
        assert_eq!(
            segments[2].1,
            segments[1].1 + (20.0 * 60.0) / (100.0 * 60.0)
        );
    }

    #[test]
    fn negative_and_final_point_extrapolation_use_the_active_bpm() {
        let map = map(&[(0, 1, 120.0), (4, 1, 240.0)]);
        assert_eq!(
            map.chart_time(beat(-2, 1)).unwrap().chart_time_seconds(),
            -1.0
        );
        assert_eq!(
            map.chart_time(beat(8, 1)).unwrap().chart_time_seconds(),
            3.0
        );
    }

    #[test]
    fn same_beat_points_use_the_last_bpm_without_a_time_jump() {
        let map = map(&[(0, 1, 120.0), (4, 1, 180.0), (4, 1, 240.0)]);
        let segments: Vec<_> = map.segments().collect();
        assert_eq!(
            segments,
            vec![(beat(0, 1), 0.0, 120.0), (beat(4, 1), 2.0, 240.0)]
        );
    }

    #[test]
    fn invalid_tempo_maps_fail_at_canonical_validation() {
        assert_eq!(
            ChartTimeMap::new(std::iter::empty()),
            Err(TempoError::EmptyTempoMap)
        );
        assert_eq!(
            ChartTimeMap::new([TempoPoint {
                beat: beat(1, 1),
                bpm: 120.0,
            }]),
            Err(TempoError::FirstPointNotZero)
        );
        assert_eq!(
            ChartTimeMap::new([
                TempoPoint {
                    beat: beat(0, 1),
                    bpm: 120.0,
                },
                TempoPoint {
                    beat: beat(-1, 1),
                    bpm: 120.0,
                },
            ]),
            Err(TempoError::NonMonotonicTempo)
        );
        assert_eq!(
            ChartTimeMap::new([TempoPoint {
                beat: beat(0, 1),
                bpm: 0.0,
            }]),
            Err(TempoError::InvalidBpm)
        );
    }

    #[test]
    fn audio_offset_is_an_affine_boundary_not_a_second_clock() {
        // Shared player/converter vectors: audioTime = chartTime + audioOffset.
        let positive = AudioOffset::new(0.1).unwrap();
        assert_eq!(positive.audio_time(1.0).unwrap(), 1.1);
        assert_eq!(positive.chart_time(1.1).unwrap(), 1.0);

        let zero = AudioOffset::new(0.0).unwrap();
        assert_eq!(zero.audio_time(-2.5).unwrap(), -2.5);
        assert_eq!(zero.chart_time(-2.5).unwrap(), -2.5);

        let negative = AudioOffset::new(-0.1).unwrap();
        assert_eq!(negative.audio_time(1.0).unwrap(), 0.9);
        assert_eq!(negative.chart_time(0.9).unwrap(), 1.0);
    }

    #[test]
    fn finite_inputs_cannot_produce_non_finite_offset_or_inverse_values() {
        assert_eq!(
            AudioOffset::new(f64::NAN),
            Err(TempoError::NonFiniteAudioOffset)
        );
        assert_eq!(
            AudioOffset::new(f64::INFINITY),
            Err(TempoError::NonFiniteAudioOffset)
        );

        let offset = AudioOffset::new(f64::MAX).unwrap();
        assert_eq!(
            offset.audio_time(f64::MAX),
            Err(TempoError::NonFiniteChartTime)
        );
        assert_eq!(
            offset.chart_time(f64::NEG_INFINITY),
            Err(TempoError::NonFiniteChartTime)
        );

        let map = map(&[(0, 1, f64::MAX)]);
        assert_eq!(map.beat_at_time(-f64::MAX), Err(TempoError::NonFiniteBeat));
    }

    #[test]
    fn direct_chart_time_has_no_beat_provenance() {
        let time = CanonicalTime::from_chart_time_seconds(1.25).unwrap();
        assert_eq!(time.source_beat(), None);
        assert_eq!(time.chart_time_seconds(), 1.25);
        assert_eq!(
            CanonicalTime::from_chart_time_seconds(f64::NAN),
            Err(TempoError::NonFiniteChartTime)
        );
    }

    #[test]
    fn canonical_time_equality_ignores_source_beat_provenance() {
        let map = map(&[(0, 1, 120.0)]);
        let from_beat = map.chart_time(Beat::new(2, 1).unwrap()).unwrap();
        let from_time = CanonicalTime::from_chart_time_seconds(1.0).unwrap();

        assert_eq!(from_beat, from_time);
        assert_ne!(from_beat.source_beat(), from_time.source_beat());
    }
}
