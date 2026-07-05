//! Event gap autofill — per `_init_events()` in phispler-ext/phichart.py.
//!
//! Ensures contiguous event coverage:
//! 1. Sort by startTime
//! 2. Fill gaps with hold events (start=end=previous end value)
//! 3. Add boundary events at ±INF

use fcs_core::ast::{Expression, Literal, MotionInterval, MotionLayer};

const INF: f64 = 1e9;

pub fn autofill_intervals(intervals: &[MotionInterval], default: f64) -> Vec<MotionInterval> {
    if intervals.is_empty() {
        return vec![MotionInterval {
            start_beat: -INF,
            end_beat: INF,
            end_inclusive: true,
            expression: Expression::Literal(Literal::Float(default)),
        }];
    }
    let mut sorted = intervals.to_vec();
    sorted.sort_by(|a, b| a.start_beat.partial_cmp(&b.start_beat).unwrap());
    let mut out = Vec::new();

    let first = &sorted[0];
    if first.start_beat > -INF {
        out.push(MotionInterval {
            start_beat: -INF,
            end_beat: first.start_beat,
            end_inclusive: false,
            expression: expr_f(default),
        });
    }
    for i in 0..sorted.len() {
        out.push(sorted[i].clone());
        if i + 1 < sorted.len() {
            let (cur, nxt) = (&sorted[i], &sorted[i + 1]);
            if cur.end_beat < nxt.start_beat {
                out.push(MotionInterval {
                    start_beat: cur.end_beat,
                    end_beat: nxt.start_beat,
                    end_inclusive: false,
                    expression: expr_f(default),
                });
            }
        }
    }
    let last = out.last().unwrap();
    if last.end_beat < INF {
        out.push(MotionInterval {
            start_beat: last.end_beat,
            end_beat: INF,
            end_inclusive: true,
            expression: expr_f(default),
        });
    }
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
        assert_eq!(r.len(), 1);
        assert!(r[0].start_beat < -1e8);
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
