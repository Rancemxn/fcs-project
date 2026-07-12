//! Time conversions: FCS beat → PGR T / RPE Beat / PEC time.
//!
//! FCS: time is measured in beats (`b`) relative to a BPM timeline.
//! PGR: 1T = 1.875 / BPM seconds.  T-to-beat: beats = T / 32.
//! RPE: Beat as `[numerator, denominator, _]` where value = a + b/c.
//! PEC: time encoded as int: beat * 2048 (rounded).

/// Convert FCS beats to PGR T units.
/// PGR: 1T = 1.875 / BPM seconds.  1 beat = 60 / BPM seconds.
/// Ratio: T/beat = (60/BPM) / (1.875/BPM) = 60/1.875 = 32.
pub fn beat_to_pgr_t(beat: f64) -> f64 {
    beat * 32.0
}

/// Convert PGR T units back to beats.
pub fn pgr_t_to_beat(t: f64) -> f64 {
    t / 32.0
}

/// Convert FCS beats to seconds (given a BPM).
pub fn beat_to_seconds(beat: f64, bpm: f64) -> f64 {
    beat * 60.0 / bpm
}

// ---------------------------------------------------------------------------
// RPE Beat encoding
// ---------------------------------------------------------------------------

/// RPE Beat representation: `[a, b, c]` means `a + b / c`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RpeBeat {
    pub a: i32,
    pub b: i32,
    pub c: i32,
}

/// Convert a floating-point beat to RPE `[a, b, c]` representation.
///
/// Uses best-fit fraction with common musical denominators.
pub fn beat_to_rpe_beat(beat: f64) -> RpeBeat {
    if beat < 0.0 {
        return RpeBeat { a: 0, b: 0, c: 1 };
    }

    let integer_part = beat.floor() as i32;
    let frac = beat - integer_part as f64;

    if frac < 1e-10 {
        return RpeBeat {
            a: integer_part,
            b: 0,
            c: 1,
        };
    }

    let denominators = [2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192];
    let mut best_d = 1;
    let mut best_n = 0;
    let mut best_err = f64::MAX;

    for &d in &denominators {
        let n = (frac * d as f64).round() as i32;
        if n < 0 || n > d {
            continue;
        }
        let approx = n as f64 / d as f64;
        let err = (frac - approx).abs();
        if err < best_err {
            best_err = err;
            best_d = d;
            best_n = n;
        }
    }

    if best_err < 0.005 {
        RpeBeat {
            a: integer_part,
            b: best_n,
            c: best_d,
        }
    } else {
        RpeBeat {
            a: integer_part,
            b: (frac * 1000.0).round() as i32,
            c: 1000,
        }
    }
}

/// Format an RpeBeat as a JSON array string `[a, b, c]`.
pub fn format_rpe_beat(b: &RpeBeat) -> String {
    format!("[{}, {}, {}]", b.a, b.b, b.c)
}

// ---------------------------------------------------------------------------
// PEC time encoding
// ---------------------------------------------------------------------------

/// Convert FCS beat to PEC time integer (beat * 2048, rounded).
pub fn beat_to_pec_time(beat: f64) -> i32 {
    (beat * 2048.0).round() as i32
}

/// Convert PEC time back to FCS beat.
pub fn pec_time_to_beat(pec_time: i32) -> f64 {
    pec_time as f64 / 2048.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beat_to_pgr_t() {
        assert!((beat_to_pgr_t(1.0) - 32.0).abs() < 1e-10);
        assert!((beat_to_pgr_t(0.5) - 16.0).abs() < 1e-10);
    }

    #[test]
    fn test_pgr_t_roundtrip() {
        let beat = 4.0;
        let t = beat_to_pgr_t(beat);
        let back = pgr_t_to_beat(t);
        assert!((beat - back).abs() < 1e-10);
    }

    #[test]
    fn test_beat_to_rpe_whole() {
        let b = beat_to_rpe_beat(4.0);
        assert_eq!(b.a, 4);
        assert_eq!(b.b, 0);
    }

    #[test]
    fn test_beat_to_rpe_fractional() {
        let b = beat_to_rpe_beat(4.5);
        assert_eq!(b.a, 4);
        assert_eq!(b.b, 1);
        assert_eq!(b.c, 2);
    }

    #[test]
    fn test_beat_to_rpe_third() {
        let b = beat_to_rpe_beat(1.0 / 3.0);
        let val = b.a as f64 + b.b as f64 / b.c as f64;
        assert!((val - 1.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_beat_to_pec_time() {
        assert_eq!(beat_to_pec_time(0.0), 0);
        assert_eq!(beat_to_pec_time(1.0), 2048);
        assert_eq!(beat_to_pec_time(0.5), 1024);
    }

    #[test]
    fn test_pec_time_roundtrip() {
        let beat = 16.5;
        let pec = beat_to_pec_time(beat);
        let back = pec_time_to_beat(pec);
        assert!((beat - back).abs() < 1e-3);
    }
}
