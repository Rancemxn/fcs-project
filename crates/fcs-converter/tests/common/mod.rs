//! Shared test helpers for round-trip validation.
//!
//! Provides time-sampled event comparison (interpolating event values at
//! evenly spaced times) and per-field note comparison.

use fcs_converter::ir::*;

/// Linear interpolation: value at time_beat given a time-sorted event list.
/// Returns None if no event covers the time.
pub fn sample_event_value(events: &[IrEvent], time_beat: f64) -> Option<f64> {
    for e in events {
        if (e.start_beat - time_beat).abs() < 1e-12 && e.start_beat == e.end_beat {
            return Some(e.start_value);
        }
        if time_beat >= e.start_beat && time_beat < e.end_beat {
            if (e.end_beat - e.start_beat).abs() < 1e-12 {
                return Some(e.start_value);
            }
            let t = (time_beat - e.start_beat) / (e.end_beat - e.start_beat);
            return Some(e.start_value + (e.end_value - e.start_value) * t);
        }
    }
    None
}

/// Find the time range of a chart (earliest to latest event end + note time).
pub fn chart_time_range(chart: &IrChart) -> (f64, f64) {
    let mut min_t = f64::MAX;
    let mut max_t = f64::MIN;
    for line in &chart.lines {
        for note in line.notes_above.iter().chain(&line.notes_below) {
            let end = note.time_beat + note.hold_beat;
            if note.time_beat < min_t {
                min_t = note.time_beat;
            }
            if end > max_t {
                max_t = end;
            }
        }
        let bundle = &line.events;
        for ev in bundle
            .speed
            .iter()
            .chain(&bundle.move_x)
            .chain(&bundle.move_y)
            .chain(&bundle.rotate)
            .chain(&bundle.alpha)
            .chain(&bundle.scale_x)
            .chain(&bundle.scale_y)
            .chain(&bundle.color)
        {
            if ev.start_beat < min_t {
                min_t = ev.start_beat;
            }
            if ev.end_beat > max_t {
                max_t = ev.end_beat;
            }
        }
    }
    if min_t == f64::MAX {
        (0.0, 0.0)
    } else {
        (min_t, max_t)
    }
}

/// Per-event-type tolerance limits for sampled validation.
pub struct EventTolerances {
    pub move_x: f64,
    pub move_y: f64,
    pub rotate: f64,
    pub alpha: f64,
    pub speed: f64,
}

impl Default for EventTolerances {
    fn default() -> Self {
        Self {
            move_x: 200.0,
            move_y: 200.0,
            rotate: 90.0,
            alpha: 0.01,
            speed: 1.0,
        }
    }
}

/// Compare event values by sampling at N evenly spaced times.
/// Asserts that max diff per event type stays below its tolerance.
pub fn compare_events_sampled(
    orig: &IrChart,
    rt: &IrChart,
    num_samples: usize,
    tol: EventTolerances,
) {
    let (t_start, t_end) = chart_time_range(orig);
    let rt_range = chart_time_range(rt);
    let t_end = t_end.max(rt_range.1);
    let span = if (t_end - t_start).abs() < 1e-12 {
        1.0
    } else {
        t_end - t_start
    };

    let sample_times: Vec<f64> = (0..num_samples)
        .map(|i| t_start + span * (i as f64 / (num_samples.max(1) - 1) as f64))
        .collect();

    let mut max_move_x = 0.0f64;
    let mut max_move_y = 0.0f64;
    let mut max_rotate = 0.0f64;
    let mut max_alpha = 0.0f64;
    let mut max_speed = 0.0f64;

    for (ol, rl) in orig.lines.iter().zip(&rt.lines) {
        for &t in &sample_times {
            if let Some(ov) = sample_event_value(&ol.events.move_x, t)
                && let Some(rv) = sample_event_value(&rl.events.move_x, t)
            {
                max_move_x = max_move_x.max((ov - rv).abs());
            }
            if let Some(ov) = sample_event_value(&ol.events.move_y, t)
                && let Some(rv) = sample_event_value(&rl.events.move_y, t)
            {
                max_move_y = max_move_y.max((ov - rv).abs());
            }
            if let Some(ov) = sample_event_value(&ol.events.rotate, t)
                && let Some(rv) = sample_event_value(&rl.events.rotate, t)
            {
                max_rotate = max_rotate.max((ov - rv).abs());
            }
            if let Some(ov) = sample_event_value(&ol.events.alpha, t)
                && let Some(rv) = sample_event_value(&rl.events.alpha, t)
            {
                max_alpha = max_alpha.max((ov - rv).abs());
            }
            if let Some(ov) = sample_event_value(&ol.events.speed, t)
                && let Some(rv) = sample_event_value(&rl.events.speed, t)
            {
                max_speed = max_speed.max((ov - rv).abs());
            }
        }
    }

    assert!(
        max_move_x < tol.move_x,
        "moveX sampled max diff {max_move_x:.4} >= tolerance {}",
        tol.move_x
    );
    assert!(
        max_move_y < tol.move_y,
        "moveY sampled max diff {max_move_y:.4} >= tolerance {}",
        tol.move_y
    );
    assert!(
        max_rotate < tol.rotate,
        "rotate sampled max diff {max_rotate:.4} >= tolerance {}",
        tol.rotate
    );
    assert!(
        max_alpha < tol.alpha,
        "alpha sampled max diff {max_alpha:.4} >= tolerance {}",
        tol.alpha
    );
    assert!(
        max_speed < tol.speed,
        "speed sampled max diff {max_speed:.4} >= tolerance {}",
        tol.speed
    );
}

/// Per-field note comparison. Checks all notes across all lines
/// for time_beat, position_x, speed, kind, above, hold_beat.
///
/// Notes are sorted by (time_beat, position_x, kind) before comparison
/// to handle ordering differences introduced by flattener time-sorting.
pub fn compare_notes_exact(orig: &IrChart, rt: &IrChart, tolerance: f64) {
    assert_eq!(orig.lines.len(), rt.lines.len(), "line count mismatch");
    for (i, (ol, rl)) in orig.lines.iter().zip(&rt.lines).enumerate() {
        let mut o_notes: Vec<&IrNote> = ol.notes_above.iter().chain(&ol.notes_below).collect();
        let mut r_notes: Vec<&IrNote> = rl.notes_above.iter().chain(&rl.notes_below).collect();
        o_notes.sort_by(|a, b| {
            a.time_beat
                .partial_cmp(&b.time_beat)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    a.position_x
                        .partial_cmp(&b.position_x)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| (a.kind as u8).cmp(&(b.kind as u8)))
        });
        r_notes.sort_by(|a, b| {
            a.time_beat
                .partial_cmp(&b.time_beat)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    a.position_x
                        .partial_cmp(&b.position_x)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| (a.kind as u8).cmp(&(b.kind as u8)))
        });
        assert_eq!(o_notes.len(), r_notes.len(), "line {} total note count", i);
        for (j, (on, rn)) in o_notes.iter().zip(&r_notes).enumerate() {
            assert!(
                (on.time_beat - rn.time_beat).abs() < tolerance,
                "line {i} note {j} time_beat: {} vs {}",
                on.time_beat,
                rn.time_beat
            );
            assert!(
                (on.position_x - rn.position_x).abs() < tolerance,
                "line {i} note {j} position_x: {} vs {}",
                on.position_x,
                rn.position_x
            );
            assert!(
                (on.speed - rn.speed).abs() < tolerance,
                "line {i} note {j} speed: {} vs {}",
                on.speed,
                rn.speed
            );
            assert_eq!(on.kind, rn.kind, "line {i} note {j} kind");
            assert_eq!(on.above, rn.above, "line {i} note {j} above");
            assert!(
                (on.hold_beat - rn.hold_beat).abs() < tolerance,
                "line {i} note {j} hold_beat: {} vs {}",
                on.hold_beat,
                rn.hold_beat
            );
        }
    }
}
