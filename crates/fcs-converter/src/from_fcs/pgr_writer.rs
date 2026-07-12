//! PGR (Phigros Official) format writer — FCS Document → PGR JSON (V1 + V3).
//!
//! Reference: `refer/phispler-ext/src/tool-rpe2phi.py` (RPE→PGR),
//! `refer/phispler-ext/src/tool-phi2rpe.py` (PGR→RPE),
//! `refer/phira-docs/src/chart-standard/chart-format/phi/`.

use crate::from_fcs::coord;
use crate::from_fcs::evaluator::{EvalEnv, eval_expr};
use crate::from_fcs::time;
use crate::from_fcs::{autofill, flattener};
use fcs_core::ast::{
    Document, LineDef, MotionBlock, MotionInterval, MotionLayer, NoteInstance, NoteKind,
};
use serde::Serialize;

// adaptive sampling active

// ---- PGR JSON output types ------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PgrChart {
    #[serde(rename = "formatVersion")]
    format_version: i32,
    offset: f64,
    judge_line_list: Vec<PgrLine>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PgrLine {
    bpm: f64,
    notes_above: Vec<PgrNote>,
    notes_below: Vec<PgrNote>,
    speed_events: Vec<PgrEvent>,
    judge_line_disappear_events: Vec<PgrEvent>,
    judge_line_move_events: Vec<PgrEvent>,
    judge_line_rotate_events: Vec<PgrEvent>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PgrNote {
    #[serde(rename = "type")]
    note_type: u8,
    time: f64,
    #[serde(rename = "positionX")]
    position_x: f64,
    #[serde(rename = "holdTime")]
    hold_time: f64,
    speed: f64,
    #[serde(rename = "floorPosition")]
    floor_position: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PgrEvent {
    #[serde(rename = "startTime")]
    start_time: f64,
    #[serde(rename = "endTime")]
    end_time: f64,
    start: f64,
    end: f64,
    #[serde(rename = "start2")]
    start2: f64,
    #[serde(rename = "end2")]
    end2: f64,
    value: f64,
}

// ---- Public API -----------------------------------------------------------

pub fn fcs_to_pgr_json(doc: &Document, version: i32) -> String {
    let chart = fcs_to_pgr(doc, version);
    serde_json::to_string_pretty(&chart).unwrap_or_else(|_| "{}".into())
}

fn fcs_to_pgr(doc: &Document, version: i32) -> PgrChart {
    // Pre-process: flatten parent lines (PGR has no hierarchy)
    let doc = flattener::flatten_parent_lines(doc);
    let lines: Vec<PgrLine> = doc
        .judgelines
        .lines
        .iter()
        .map(|line| convert_line(line, version))
        .collect();
    PgrChart {
        format_version: version,
        offset: doc.meta.offset,
        judge_line_list: lines,
    }
}

// ---- Line conversion ------------------------------------------------------

fn convert_line(line: &LineDef, version: i32) -> PgrLine {
    let line_bpm = line_bpm(line);
    let env_base = EvalEnv::default();

    // Flatten proto inheritance, discard kind=fake
    let flat_notes = flattener::flatten_note_block(&line.notes);

    // Autofill motion gaps (needed before speed_events for floor positions)
    let motion = line.motion.as_ref().map(|m| MotionBlock {
        layers: m.layers.iter().map(autofill::autofill_layer).collect(),
    });
    let mut speed_events = motion_to_speed_events(&motion, &env_base);
    // PGR requires at least one speed event — emit default if empty
    if speed_events.is_empty() {
        speed_events.push(PgrEvent {
            start_time: 0.0,
            end_time: 1e9,
            start: 0.0,
            end: 0.0,
            start2: 0.0,
            end2: 0.0,
            value: 1.0,
        });
    }

    // Compute floor positions matching sim-phi's speed integral
    let all_notes: Vec<&NoteInstance> = flat_notes.concrete.iter().collect();
    let floor_positions = compute_floor_positions(&all_notes, &speed_events, line_bpm);

    let notes_above: Vec<PgrNote> = flat_notes
        .concrete
        .iter()
        .zip(&floor_positions)
        .filter(|(n, _)| note_is_above(n))
        .map(|(n, fp)| convert_note(n, line_bpm, version, *fp))
        .collect();
    let notes_below: Vec<PgrNote> = flat_notes
        .concrete
        .iter()
        .zip(&floor_positions)
        .filter(|(n, _)| !note_is_above(n))
        .map(|(n, fp)| convert_note(n, line_bpm, version, *fp))
        .collect();

    let (move_ev, rot_ev, alpha_ev) = motion_to_move_rotate_alpha(&motion, line_bpm, &env_base);

    PgrLine {
        bpm: line_bpm,
        notes_above,
        notes_below,
        speed_events,
        judge_line_disappear_events: alpha_ev,
        judge_line_move_events: move_ev,
        judge_line_rotate_events: rot_ev,
    }
}

fn line_bpm(line: &LineDef) -> f64 {
    line.bpm_timeline
        .entries
        .first()
        .map(|e| e.bpm)
        .unwrap_or(120.0)
}

// ---- Note conversion ------------------------------------------------------

fn note_is_above(note: &NoteInstance) -> bool {
    note.properties
        .iter()
        .find(|(k, _)| k.as_str() == "above")
        .map(|(_, v)| is_truthy(v))
        .unwrap_or(true)
}

fn is_truthy(v: &fcs_core::ast::NotePropertyValue) -> bool {
    match v {
        fcs_core::ast::NotePropertyValue::Bool(b) => *b,
        fcs_core::ast::NotePropertyValue::Expr(fcs_core::ast::Expression::Literal(
            fcs_core::ast::Literal::Boolean(b),
        )) => *b,
        _ => true,
    }
}

fn note_get_f64(note: &NoteInstance, key: &str, default: f64) -> f64 {
    use fcs_core::ast::{Literal, NotePropertyValue};
    note.properties
        .iter()
        .find(|(k, _)| k.as_str() == key)
        .map(|(_, v)| match v {
            NotePropertyValue::Expr(e) => eval_expr(e, &EvalEnv::default()),
            NotePropertyValue::Literal(Literal::Float(f)) => *f,
            NotePropertyValue::Literal(Literal::Integer(n)) => *n as f64,
            NotePropertyValue::Literal(Literal::Quantified { value, .. }) => *value,
            _ => default,
        })
        .unwrap_or(default)
}

fn note_time_beat(note: &NoteInstance) -> f64 {
    use fcs_core::ast::NotePropertyValue;
    note.properties
        .iter()
        .find(|(k, _)| k.as_str() == "time")
        .map(|(_, v)| match v {
            NotePropertyValue::Expr(e) => eval_expr(e, &EvalEnv::default()),
            NotePropertyValue::Literal(lit) => lit_to_f64(lit),
            _ => 0.0,
        })
        .unwrap_or(0.0)
}

fn note_end_beat(note: &NoteInstance) -> f64 {
    use fcs_core::ast::NotePropertyValue;
    note.properties
        .iter()
        .find(|(k, _)| k.as_str() == "endTime")
        .map(|(_, v)| match v {
            NotePropertyValue::Expr(e) => eval_expr(e, &EvalEnv::default()),
            NotePropertyValue::Literal(lit) => lit_to_f64(lit),
            _ => 0.0,
        })
        .unwrap_or(0.0)
}

fn lit_to_f64(lit: &fcs_core::ast::Literal) -> f64 {
    match lit {
        fcs_core::ast::Literal::Integer(n) => *n as f64,
        fcs_core::ast::Literal::Float(f) => *f,
        fcs_core::ast::Literal::Quantified { value, .. } => *value,
        _ => 0.0,
    }
}

fn note_kind_to_pgr_type(kind: NoteKind) -> u8 {
    match kind {
        NoteKind::Tap => 1,
        NoteKind::Drag => 2,
        NoteKind::Hold => 3,
        NoteKind::Flick => 4,
        NoteKind::Fake => 0,
    }
}

/// Compute floor position for each note matching sim-phi's speed event integral.
///
/// sim-phi's `prerenderChart` computes:
///   y = firstEvent.startTime / bpm * 1.875
///   for each event: floorPosition = y, then y += (endTime - startTime) / bpm * 1.875 * value
///
/// A note's floorPosition is the accumulated y at its time: the event's floorPosition
/// plus the partial contribution through the event to the note's time.
fn compute_floor_positions(
    notes: &[&NoteInstance],
    speed_events: &[PgrEvent],
    bpm: f64,
) -> Vec<f64> {
    if speed_events.is_empty() {
        return vec![0.0; notes.len()];
    }

    // Precompute cumulative floor at each speed event start boundary.
    // cum[i] = (start_time, floor_at_start, value)
    let mut cum: Vec<(f64, f64, f64)> = Vec::with_capacity(speed_events.len());
    let mut y = speed_events[0].start_time / bpm * 1.875;
    for evt in speed_events {
        cum.push((evt.start_time, y, evt.value));
        let dy = (evt.end_time - evt.start_time) / bpm * 1.875;
        y += dy * evt.value;
    }
    let final_y = y;

    notes
        .iter()
        .map(|note| {
            let pgr_t = time::beat_to_pgr_t(note_time_beat(note));
            // Reverse-scan to find the covering speed event
            for &(st, y0, val) in cum.iter().rev() {
                if pgr_t >= st {
                    let dy = (pgr_t - st) / bpm * 1.875;
                    return y0 + dy * val;
                }
            }
            final_y
        })
        .collect()
}

fn convert_note(note: &NoteInstance, _bpm: f64, version: i32, floor_position: f64) -> PgrNote {
    let time_beat = note_time_beat(note);
    let end_beat = note_end_beat(note);
    let pgr_time = time::beat_to_pgr_t(time_beat);
    let hold_time_t = time::beat_to_pgr_t(end_beat) - pgr_time;
    let x_pgr = coord::fcs_px_to_pgr_x(note_get_f64(note, "positionX", 0.0));
    let speed = note_get_f64(note, "speed", 1.0);

    let (position_x, floor_position) = match version {
        1 => {
            let encoded = coord::encode_pgr_v1_position(x_pgr, 0.0);
            (encoded as f64, 0.0)
        }
        _ => (x_pgr, floor_position),
    };

    PgrNote {
        note_type: note_kind_to_pgr_type(note.kind),
        time: pgr_time,
        position_x,
        hold_time: if note.kind == NoteKind::Hold {
            hold_time_t.max(0.0)
        } else {
            0.0
        },
        speed,
        floor_position,
    }
}

// ---- Motion → PGR events --------------------------------------------------

fn collect_intervals<F>(motion: &Option<MotionBlock>, sel: F) -> Vec<MotionInterval>
where
    F: Fn(&MotionLayer) -> &Vec<MotionInterval>,
{
    let mut out = Vec::new();
    if let Some(m) = motion {
        for layer in &m.layers {
            out.extend(sel(layer).clone());
        }
    }
    out
}

fn motion_to_speed_events(motion: &Option<MotionBlock>, env: &EvalEnv) -> Vec<PgrEvent> {
    let ivs = collect_intervals(motion, |l| &l.speed);
    if ivs.is_empty() {
        return vec![];
    }
    // Speed events are piecewise-constant — convert FCS intervals directly
    // without resampling. This preserves exact event boundaries from the
    // original PGR (including speed=80 tap-notes at 2 T-units wide).
    ivs.iter()
        .map(|iv| {
            let mut es = *env;
            es.beat = iv.start_beat;
            let v = eval_expr(&iv.expression, &es);
            PgrEvent {
                start_time: time::beat_to_pgr_t(iv.start_beat),
                end_time: time::beat_to_pgr_t(iv.end_beat),
                start: 0.0,
                end: 0.0,
                start2: 0.0,
                end2: 0.0,
                value: v,
            }
        })
        .collect()
}

/// Build sorted unique beat boundaries from a list of intervals.
fn build_beats(intervals: &[&[MotionInterval]]) -> Vec<f64> {
    let mut beats = std::collections::BTreeSet::new();
    for ivs in intervals {
        for iv in *ivs {
            beats.insert((iv.start_beat * 1e6) as i64);
            beats.insert((iv.end_beat * 1e6) as i64);
        }
    }
    beats.iter().map(|b| *b as f64 / 1e6).collect()
}

/// Sample a field at a given beat, returning the default if no interval covers it.
fn sample_at(intervals: &[MotionInterval], beat: f64, default: f64, env: &EvalEnv) -> f64 {
    let mut es = *env;
    es.beat = beat;
    for iv in intervals.iter().rev() {
        if beat >= iv.start_beat
            && (beat < iv.end_beat || (iv.end_inclusive && beat <= iv.end_beat))
        {
            return eval_expr(&iv.expression, &es);
        }
    }
    default
}

/// Build move/rotate/alpha events with correct PGR coordinate mapping.
/// Each field uses only its own interval boundaries for event generation,
/// avoiding inflation from the union of all fields.
/// Move X: FCS px → RPE x → PGR [0,1] = (rpe_x + 675) / 1350
/// Move Y: FCS px → RPE y → PGR [0,1] = (rpe_y + 450) / 900
fn motion_to_move_rotate_alpha(
    motion: &Option<MotionBlock>,
    bpm: f64,
    env: &EvalEnv,
) -> (Vec<PgrEvent>, Vec<PgrEvent>, Vec<PgrEvent>) {
    let x_ivs = collect_intervals(motion, |l| &l.position_x);
    let y_ivs = collect_intervals(motion, |l| &l.position_y);
    let rot_ivs = collect_intervals(motion, |l| &l.rotation);
    let alpha_ivs = collect_intervals(motion, |l| &l.alpha);

    // Move events: use x + y boundaries so both fields are well-represented
    let move_beats = build_beats(&[&x_ivs, &y_ivs]);
    let moves: Vec<PgrEvent> = move_beats
        .windows(2)
        .map(|w| {
            let bm = (w[0] + w[1]) * 0.5;
            let mut es = *env;
            es.beat = bm;
            es.seconds = time::beat_to_seconds(bm, bpm);
            let xv = sample_at(&x_ivs, bm, 0.0, &es);
            let yv = sample_at(&y_ivs, bm, 0.0, &es);
            let pgr_x = (coord::fcs_px_to_rpe_x(xv) + 675.0) / 1350.0;
            let pgr_y = (coord::fcs_px_to_rpe_y(yv) + 450.0) / 900.0;
            PgrEvent {
                start_time: time::beat_to_pgr_t(w[0]),
                end_time: time::beat_to_pgr_t(w[1]),
                start: pgr_x,
                end: pgr_x,
                start2: pgr_y,
                end2: pgr_y,
                value: 1.0,
            }
        })
        .collect();

    // Rotate events: use rotation interval boundaries only
    let rot_beats = build_beats(&[&rot_ivs]);
    let rots: Vec<PgrEvent> = rot_beats
        .windows(2)
        .map(|w| {
            let bm = (w[0] + w[1]) * 0.5;
            let mut es = *env;
            es.beat = bm;
            es.seconds = time::beat_to_seconds(bm, bpm);
            let rv = sample_at(&rot_ivs, bm, 0.0, &es);
            PgrEvent {
                start_time: time::beat_to_pgr_t(w[0]),
                end_time: time::beat_to_pgr_t(w[1]),
                start: -rv,
                end: -rv,
                start2: 0.0,
                end2: 0.0,
                value: 1.0,
            }
        })
        .collect();

    // Alpha events: use alpha interval boundaries only
    let alpha_beats = build_beats(&[&alpha_ivs]);
    let alphas: Vec<PgrEvent> = alpha_beats
        .windows(2)
        .map(|w| {
            let bm = (w[0] + w[1]) * 0.5;
            let mut es = *env;
            es.beat = bm;
            es.seconds = time::beat_to_seconds(bm, bpm);
            let av = sample_at(&alpha_ivs, bm, 1.0, &es);
            PgrEvent {
                start_time: time::beat_to_pgr_t(w[0]),
                end_time: time::beat_to_pgr_t(w[1]),
                start: av,
                end: av,
                start2: 0.0,
                end2: 0.0,
                value: 1.0,
            }
        })
        .collect();

    (moves, rots, alphas)
}

// ---- Tests ----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fcs_core::parser;

    #[test]
    fn test_convert_sample_fcs_to_pgr_v3() {
        let src = include_str!("../../../../examples/sample.fcs");
        let (_, doc) = parser::parse_document(src).expect("parse");
        let json = fcs_to_pgr_json(&doc, 3);
        let c: serde_json::Value = serde_json::from_str(&json).expect("json");
        assert_eq!(c["formatVersion"], 3);
        assert_eq!(c["judgeLineList"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_convert_sample_fcs_to_pgr_v1() {
        let src = include_str!("../../../../examples/sample.fcs");
        let (_, doc) = parser::parse_document(src).expect("parse");
        let json = fcs_to_pgr_json(&doc, 1);
        let c: serde_json::Value = serde_json::from_str(&json).expect("json");
        assert_eq!(c["formatVersion"], 1);
    }

    #[test]
    fn test_note_type_mapping() {
        assert_eq!(note_kind_to_pgr_type(NoteKind::Tap), 1);
        assert_eq!(note_kind_to_pgr_type(NoteKind::Drag), 2);
        assert_eq!(note_kind_to_pgr_type(NoteKind::Hold), 3);
        assert_eq!(note_kind_to_pgr_type(NoteKind::Flick), 4);
    }
}
