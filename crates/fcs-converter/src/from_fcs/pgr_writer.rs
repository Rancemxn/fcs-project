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
    let notes_above: Vec<PgrNote> = flat_notes
        .concrete
        .iter()
        .filter(|n| note_is_above(n))
        .map(|n| convert_note(n, line_bpm, version))
        .collect();
    let notes_below: Vec<PgrNote> = flat_notes
        .concrete
        .iter()
        .filter(|n| !note_is_above(n))
        .map(|n| convert_note(n, line_bpm, version))
        .collect();

    // Autofill motion gaps
    let motion = line.motion.as_ref().map(|m| MotionBlock {
        layers: m.layers.iter().map(autofill::autofill_layer).collect(),
    });
    let mut speed_events = motion_to_speed_events(&motion, line_bpm, &env_base);
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

fn convert_note(note: &NoteInstance, _bpm: f64, version: i32) -> PgrNote {
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
        _ => {
            // floorPosition = accumulated position from speed * time
            // Simplified: use speed * time_beat as rough estimate
            let fp = speed * time_beat;
            (x_pgr, fp)
        },
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

fn sample_to_events(
    intervals: &[MotionInterval],
    bpm: f64,
    env: &EvalEnv,
    default: f64,
) -> Vec<PgrEvent> {
    if intervals.is_empty() {
        return vec![];
    }
    let mut min_beat = f64::MAX;
    let mut max_beat = f64::MIN;
    for iv in intervals {
        min_beat = min_beat.min(iv.start_beat);
        max_beat = max_beat.max(iv.end_beat);
    }
    // Adaptive step: larger intervals → coarser sampling (per tool-rpe2phi.py)
    let dt_t = (max_beat - min_beat) * 32.0; // convert to PGR T units
    let step = if dt_t >= 512.0 { 16.0 / 32.0 } else if dt_t >= 256.0 { 8.0 / 32.0 } else if dt_t >= 128.0 { 4.0 / 32.0 } else { 1.0 / 32.0 };
    let n = ((max_beat - min_beat) / step).ceil() as usize + 1;
    let mut samples: Vec<(f64, f64)> = Vec::with_capacity(n);
    for i in 0..=n {
        let beat = min_beat + i as f64 * step;
        if beat > max_beat {
            break;
        }
        let mut es = *env;
        es.beat = beat;
        es.seconds = time::beat_to_seconds(beat, bpm);
        let mut v = default;
        for iv in intervals.iter().rev() {
            if beat >= iv.start_beat
                && (beat < iv.end_beat || (iv.end_inclusive && beat <= iv.end_beat))
            {
                v = eval_expr(&iv.expression, &es);
                break;
            }
        }
        samples.push((beat, v));
    }
    let mut events = Vec::new();
    for w in samples.windows(2) {
        let (b0, v0) = w[0];
        let (b1, v1) = w[1];
        events.push(PgrEvent {
            start_time: time::beat_to_pgr_t(b0),
            end_time: time::beat_to_pgr_t(b1),
            start: v0,
            end: v1,
            start2: 0.0,
            end2: 0.0,
            value: 1.0,
        });
    }
    events
}

fn motion_to_speed_events(motion: &Option<MotionBlock>, bpm: f64, env: &EvalEnv) -> Vec<PgrEvent> {
    let ivs = collect_intervals(motion, |l| &l.speed);
    sample_to_events(&ivs, bpm, env, 1.0)
        .into_iter()
        .map(|mut e| {
            e.value = e.end;
            e.start = 0.0;
            e.end = 0.0;
            e
        })
        .collect()
}

/// Build move/rotate/alpha events with correct PGR coordinate mapping.
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

    let mut beats = std::collections::BTreeSet::new();
    for iv in x_ivs.iter().chain(&y_ivs).chain(&rot_ivs).chain(&alpha_ivs) {
        beats.insert((iv.start_beat * 1e6) as i64);
        beats.insert((iv.end_beat * 1e6) as i64);
    }
    let beat_list: Vec<f64> = beats.iter().map(|b| *b as f64 / 1e6).collect();
    if beat_list.is_empty() {
        return (vec![], vec![], vec![]);
    }

    let mut moves = Vec::new();
    let mut rots = Vec::new();
    let mut alphas = Vec::new();

    for w in beat_list.windows(2) {
        let bm = (w[0] + w[1]) * 0.5;
        let mut es = *env;
        es.beat = bm;
        es.seconds = time::beat_to_seconds(bm, bpm);

        let xv = eval_at_beat(&x_ivs, bm, 0.0, &es);
        let yv = eval_at_beat(&y_ivs, bm, 0.0, &es);
        let rv = eval_at_beat(&rot_ivs, bm, 0.0, &es);
        let av = eval_at_beat(&alpha_ivs, bm, 1.0, &es);

        let t0 = time::beat_to_pgr_t(w[0]);
        let t1 = time::beat_to_pgr_t(w[1]);

        // PGR [0,1] normalization (matching tool-rpe2phi.py)
        let pgr_x = (coord::fcs_px_to_rpe_x(xv) + 675.0) / 1350.0;
        let pgr_y = (coord::fcs_px_to_rpe_y(yv) + 450.0) / 900.0;

        moves.push(PgrEvent {
            start_time: t0,
            end_time: t1,
            start: pgr_x,
            end: pgr_x,
            start2: pgr_y,
            end2: pgr_y,
            value: 1.0,
        });
        rots.push(PgrEvent {
            start_time: t0,
            end_time: t1,
            start: rv,
            end: rv,
            start2: 0.0,
            end2: 0.0,
            value: 1.0,
        });
        alphas.push(PgrEvent {
            start_time: t0,
            end_time: t1,
            start: av,
            end: av,
            start2: 0.0,
            end2: 0.0,
            value: 1.0,
        });
    }

    // Chain start = previous end
    for i in 1..moves.len() {
        moves[i].start = moves[i - 1].end;
        moves[i].start2 = moves[i - 1].end2;
    }
    for i in 1..rots.len() {
        rots[i].start = rots[i - 1].end;
    }
    for i in 1..alphas.len() {
        alphas[i].start = alphas[i - 1].end;
    }

    (moves, rots, alphas)
}

fn eval_at_beat(intervals: &[MotionInterval], beat: f64, default: f64, env: &EvalEnv) -> f64 {
    for iv in intervals.iter().rev() {
        if beat >= iv.start_beat
            && (beat < iv.end_beat || (iv.end_inclusive && beat <= iv.end_beat))
        {
            let mut es = *env;
            es.beat = beat;
            return eval_expr(&iv.expression, &es);
        }
    }
    default
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
