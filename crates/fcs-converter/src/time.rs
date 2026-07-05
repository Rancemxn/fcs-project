//! Time unit conversions: PGR T → beats (FCS b).
/// PGR: 1T = 1.875/BPM seconds. FCS: beats. BPM cancels out: beats = T/32.
pub fn t_to_beat(t: f64, _bpm: f64) -> f64 {
    t / 32.0
}
pub fn t_to_seconds(t: f64, bpm: f64) -> f64 {
    t * 1.875 / bpm
}
pub fn beat_to_seconds(beat: f64, bpm: f64) -> f64 {
    beat * 60.0 / bpm
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_t_to_beat() {
        assert!((t_to_beat(32.0, 120.0) - 1.0).abs() < 1e-10);
    }
}
