//! PEC (PhiEditer) text format writer — FCS Document → PEC text.
//!
//! Reference: `refer/phispler-ext/src/light_utils.py` (pec2rpe).
//!
//! PEC format:
//! ```text
//! <offset_ms - 150>
//! bp <time_beat> <bpm>
//! n1 <line_id> <time> <x> <above> <fake>          # Tap
//! # <speed>                                         # per-note speed
//! & <size>                                          # per-note size
//! n2 <line_id> <time> <visT> <x> <above> <fake>    # Hold (visT = visibleTime beats)
//! n3 <line_id> <time> <x> <above> <fake> [<hold>]  # Flick (optional holdTime beats)
//! n4 <line_id> <time> <x> <above> <fake>           # Drag
//! cp <line_id> <time> <x> <y>                       # position
//! cd <line_id> <time> <value>                       # rotation
//! ca <line_id> <time> <value>                       # alpha
//! cv <line_id> <time> <value>                       # speed
//! cm <line_id> <st> <et> <endX> <endY> <ease>       # move (interp)
//! cr <line_id> <st> <et> <endValue> <ease>           # rotate (interp)
//! cf <line_id> <time> <value> <ease>                 # (rare)
//! ```

use crate::from_fcs::coord;
use crate::from_fcs::evaluator::{EvalEnv, eval_expr};
use crate::from_fcs::{autofill, flattener};
use fcs_core::ast::{Document, MotionBlock, MotionInterval, NoteInstance, NoteKind};
use std::fmt::Write;

// PEC Y axis uses 1400, not 2048 (from pec2rpe: rpey = (y/1400 - 0.5) * 900)
const PEC_Y_ENC: f64 = 1400.0;

// ---- Public API -----------------------------------------------------------

pub fn fcs_to_pec(doc: &Document) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{}", (doc.meta.offset + 150.0) as i32);

    // Pre-process: flatten parent lines (PEC has no hierarchy)
    let doc = flattener::flatten_parent_lines(doc);
    for entry in &doc.master_timeline.entries {
        let _ = writeln!(out, "bp {} {}", pec_t(entry.beat), entry.bpm as i32);
    }

    for (li, line) in doc.judgelines.lines.iter().enumerate() {
        // Flatten proto inheritance
        let flat_notes = flattener::flatten_note_block(&line.notes);
        for note in &flat_notes.concrete {
            if let Some(s) = note_to_pec(note, li) {
                let _ = writeln!(out, "{s}");
            }
        }

        // Autofill motion
        let motion = line.motion.as_ref().map(|m| MotionBlock {
            layers: m.layers.iter().map(autofill::autofill_layer).collect(),
        });
        for s in motion_to_pec(&motion, li) {
            let _ = writeln!(out, "{s}");
        }
    }
    out
}

// ---- Note helpers ---------------------------------------------------------

fn note_f64(note: &NoteInstance, key: &str, default: f64) -> f64 {
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

fn note_bool(note: &NoteInstance, key: &str, default: bool) -> bool {
    note.properties
        .iter()
        .find(|(k, _)| k.as_str() == key)
        .map(|(_, v)| match v {
            fcs_core::ast::NotePropertyValue::Bool(b) => *b,
            fcs_core::ast::NotePropertyValue::Expr(fcs_core::ast::Expression::Literal(
                fcs_core::ast::Literal::Boolean(b),
            )) => *b,
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

fn pec_prefix(kind: NoteKind) -> &'static str {
    match kind {
        NoteKind::Tap => "n1",
        NoteKind::Hold => "n2",
        NoteKind::Flick => "n3",
        NoteKind::Drag => "n4",
        NoteKind::Fake => "n1",
    }
}

/// PEC time: beat * 2048, output as float with 2 decimal places
/// to preserve sub-tick precision through round-trip.
fn pec_t(beat: f64) -> String {
    format!("{:.2}", beat * 2048.0)
}

/// PEC X: FCS px → PEC x = (fcs_px_to_rpe_x(px) / 1350.0) * 2048.0 = fcs_px_to_pec_x(px)
fn pec_x(px: f64) -> i32 {
    coord::fcs_px_to_pec_x(px)
}

/// PEC Y: uses 1400 denominator. rpe_y = (y/1400 - 0.5) * 900 → y = (rpe_y/900 + 0.5) * 1400
fn pec_y(px: f64) -> i32 {
    let rpe_y = coord::fcs_px_to_rpe_y(px);
    ((rpe_y / 900.0 + 0.5) * PEC_Y_ENC).round() as i32
}

fn note_to_pec(note: &NoteInstance, line_id: usize) -> Option<String> {
    let tb = note_time_beat(note);
    let eb = note_end_beat(note);
    let t = pec_t(tb);
    let x = pec_x(note_f64(note, "positionX", 0.0));
    let above = if note_bool(note, "above", true) { 1 } else { 0 };
    let fake = if note_bool(note, "fake", false) { 1 } else { 0 };
    let speed = note_f64(note, "speed", 1.0);
    let size = note_f64(note, "size", 1.0);
    let prefix = pec_prefix(note.kind);

    let mut lines = Vec::new();

    // PEC note format differs by prefix:
    //   n2 (Hold): <line> <time> <visT> <x> <above> <fake>
    //   n3 (Flick): <line> <time> <x> <above> <fake> [<holdTime>]
    //   n1, n4 (Tap, Drag): <line> <time> <x> <above> <fake>
    match prefix {
        "n2" => {
            let vis = note_f64(note, "visibleTime", 0.0) as i32;
            lines.push(format!("{prefix} {line_id} {t} {vis} {x} {above} {fake}"));
        }
        "n3" if eb > tb => {
            lines.push(format!(
                "{prefix} {line_id} {t} {x} {above} {fake} {}",
                pec_t(eb - tb)
            ));
        }
        _ => {
            lines.push(format!("{prefix} {line_id} {t} {x} {above} {fake}"));
        }
    }
    // # speed, & size
    lines.push(format!("# {}", speed));
    lines.push(format!("& {}", size));

    Some(lines.join("\n"))
}

// ---- Motion → PEC ---------------------------------------------------------

fn motion_to_pec(motion: &Option<MotionBlock>, line_id: usize) -> Vec<String> {
    let mut out = Vec::new();
    let m = match motion {
        Some(m) => m,
        None => return out,
    };
    for layer in &m.layers {
        for iv in &layer.position_x {
            out.extend(pec_cp(iv, line_id));
        }
        for iv in &layer.position_y {
            out.extend(pec_cp(iv, line_id));
        }
        for iv in &layer.rotation {
            out.extend(pec_simple(iv, line_id, "cd"));
        }
        for iv in &layer.alpha {
            out.extend(pec_simple(iv, line_id, "ca"));
        }
        for iv in &layer.speed {
            out.extend(pec_simple(iv, line_id, "cv"));
        }
    }
    out
}

/// `cp <line> <time> <x> <y>` — no easing, just start/end points
fn pec_cp(iv: &MotionInterval, line_id: usize) -> Vec<String> {
    let env = EvalEnv::default();
    let st = pec_t(iv.start_beat);
    let et = pec_t(iv.end_beat);
    let mut es = env;
    es.beat = iv.start_beat;
    let sv = eval_expr(&iv.expression, &es);
    es.beat = iv.end_beat;
    let ev = eval_expr(&iv.expression, &es);
    vec![
        format!("cp {line_id} {st} {} {}", pec_x(sv), pec_y(0.0)),
        format!("cp {line_id} {et} {} {}", pec_x(ev), pec_y(0.0)),
    ]
}

/// `cd/ca/cv <line> <time> <value>` — no easing
fn pec_simple(iv: &MotionInterval, line_id: usize, prefix: &str) -> Vec<String> {
    let env = EvalEnv::default();
    let st = pec_t(iv.start_beat);
    let et = pec_t(iv.end_beat);
    let mut es = env;
    es.beat = iv.start_beat;
    let sv = eval_expr(&iv.expression, &es);
    es.beat = iv.end_beat;
    let ev = eval_expr(&iv.expression, &es);
    // PEC cd/ca/cv store values directly (rotation in degrees, alpha raw, speed raw).
    let fmt = |v: f64| -> String { format!("{v:.5}") };
    vec![
        format!("{prefix} {line_id} {st} {}", fmt(sv)),
        format!("{prefix} {line_id} {et} {}", fmt(ev)),
    ]
}

// ---- Tests ----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fcs_core::parser;

    #[test]
    fn test_convert_sample_fcs_to_pec() {
        let src = include_str!("../../../../examples/fcs/simple.fcs");
        let (_, doc) = parser::parse_document(src).expect("parse");
        let pec = fcs_to_pec(&doc);
        // BPM timeline: single 120 BPM from simple.fcs
        assert!(pec.contains("bp 0.00 120"), "bp0: {pec}");
        assert!(pec.contains("n1 "), "{pec}");
        assert!(pec.contains("# "), "{pec}");
    }

    #[test]
    fn test_pec_prefixes() {
        assert_eq!(pec_prefix(NoteKind::Tap), "n1");
        assert_eq!(pec_prefix(NoteKind::Hold), "n2");
        assert_eq!(pec_prefix(NoteKind::Flick), "n3");
        assert_eq!(pec_prefix(NoteKind::Drag), "n4");
    }
}
