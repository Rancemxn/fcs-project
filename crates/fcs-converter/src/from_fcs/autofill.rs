//! Event gap autofill — per `_init_events()` in phispler-ext/phichart.py.
//!
//! Ensures contiguous event coverage:
//! 1. Sort by startTime
//! 2. Fill gaps with hold events (start=end=previous end value)
//! 3. Add boundary events at ±INF

use fcs_core::ast::{Expression, Literal, MotionInterval, MotionLayer};

/// Margin added before the first and after the last interval (beats).
/// Must be small to avoid event explosion during FCS→PGR adaptive sampling,
/// but large enough to cover notes just outside the motion range.
/// phispler-ext uses INFBEAT (~1e9); we use ~200ms worth of beats at 120 BPM.
const MARGIN: f64 = 0.5;

pub fn autofill_intervals(intervals: &[MotionInterval], default: f64) -> Vec<MotionInterval> {
    if intervals.is_empty() {
        return vec![];
    }
    let mut sorted = intervals.to_vec();
    sorted.sort_by(|a, b| a.start_beat.partial_cmp(&b.start_beat).unwrap());
    let mut out = Vec::new();

    // Small leading margin
    let first_start = sorted[0].start_beat;
    if first_start > 0.0 {
        out.push(MotionInterval {
            start_beat: (first_start - MARGIN).max(0.0),
            end_beat: first_start,
            end_inclusive: false,
            expression: expr_f(default),
        });
    }

    for i in 0..sorted.len() {
        out.push(sorted[i].clone());
        if i + 1 < sorted.len() {
            let cur_end = sorted[i].end_beat;
            let nxt_start = sorted[i + 1].start_beat;
            if cur_end < nxt_start {
                out.push(MotionInterval {
                    start_beat: cur_end,
                    end_beat: nxt_start,
                    end_inclusive: false,
                    expression: expr_f(default),
                });
            }
        }
    }

    // Small trailing margin
    let last_end = out.last().unwrap().end_beat;
    out.push(MotionInterval {
        start_beat: last_end,
        end_beat: last_end + MARGIN,
        end_inclusive: true,
        expression: expr_f(default),
    });

    out
}

fn expr_f(v: f64) -> Expression {
    Expression::Literal(Literal::Float(v))
}

pub fn autofill_layer(layer: &MotionLayer) -> MotionLayer {
    MotionLayer {
        position_x: autofill_intervals(&layer.position_x, 0.0),
        position_y: autofill_intervals(&layer.position_y, 0.0),
        rotation: autofill_intervals(&layer.rotation, 0.0),
        alpha: autofill_intervals(&layer.alpha, 1.0),
        scale_x: autofill_intervals(&layer.scale_x, 1.0),
        scale_y: autofill_intervals(&layer.scale_y, 1.0),
        speed: autofill_intervals(&layer.speed, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iv(s: f64, e: f64, v: f64) -> MotionInterval {
        MotionInterval {
            start_beat: s,
            end_beat: e,
            end_inclusive: true,
            expression: expr_f(v),
        }
    }

    #[test]
    fn test_empty() {
        let r = autofill_intervals(&[], 1.0);
        assert_eq!(
            r.len(),
            0,
            "empty intervals produce empty output (no boundary)"
        );
    }

    #[test]
    fn test_fills_gap() {
        let r = autofill_intervals(&[iv(0.0, 4.0, 1.0), iv(8.0, 12.0, 2.0)], 0.0);
        assert!(
            r.iter().any(|i| i.start_beat == 4.0 && i.end_beat == 8.0),
            "gap at 4→8"
        );
    }
}
