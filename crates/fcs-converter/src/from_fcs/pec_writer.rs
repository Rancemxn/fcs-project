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
use crate::from_fcs::easing_map;
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
        out.extend(pec_move(&layer.position_x, &layer.position_y, line_id));
        for iv in &layer.rotation {
            out.extend(pec_easing_rotation(iv, line_id));
        }
        for iv in &layer.alpha {
            out.extend(pec_easing_alpha(iv, line_id));
        }
        for iv in &layer.speed {
            out.extend(pec_simple(iv, line_id, "cv"));
        }
    }
    out
}

/// `cp + cm` for position movement: sets start position then eases to end.
///
/// Merges positionX and positionY interval boundaries to produce
/// combined `cp` (instant) and `cm` (easing) events, since PEC `cm`
/// takes both endX and endY in a single command.
fn pec_move(ivs_x: &[MotionInterval], ivs_y: &[MotionInterval], line_id: usize) -> Vec<String> {
    let beats = collect_beat_boundaries(ivs_x.iter().chain(ivs_y.iter()));
    let mut out = Vec::new();
    let env = EvalEnv::default();
    for w in beats.windows(2) {
        let st = w[0];
        let et = w[1];
        if (et - st).abs() < 1e-12 {
            continue;
        }

        let x_info = pec_covering(ivs_x, st, et, &env);
        let y_info = pec_covering(ivs_y, st, et, &env);

        let (x_sv, x_ev) = x_info.map(|(s, e, _)| (s, e)).unwrap_or((0.0, 0.0));
        let (y_sv, y_ev) = y_info.map(|(s, e, _)| (s, e)).unwrap_or((0.0, 0.0));

        // Constant within window — single cp is sufficient
        if (x_sv - x_ev).abs() < 1e-9 && (y_sv - y_ev).abs() < 1e-9 {
            out.push(format!(
                "cp {line_id} {} {} {}",
                pec_t(st),
                pec_x(x_sv),
                pec_y(y_sv)
            ));
        } else {
            let easing = x_info
                .map(|(_, _, e)| e)
                .or_else(|| y_info.map(|(_, _, e)| e))
                .unwrap_or(1);
            out.push(format!(
                "cp {line_id} {} {} {}",
                pec_t(st),
                pec_x(x_sv),
                pec_y(y_sv)
            ));
            out.push(format!(
                "cm {line_id} {} {} {} {} {easing}",
                pec_t(st),
                pec_t(et),
                pec_x(x_ev),
                pec_y(y_ev)
            ));
        }
    }
    out
}

/// Build sorted, deduped beat boundaries from interval endpoints.
fn collect_beat_boundaries<'a>(
    intervals: impl IntoIterator<Item = &'a MotionInterval>,
) -> Vec<f64> {
    let mut s = std::collections::BTreeSet::new();
    for iv in intervals {
        s.insert((iv.start_beat * 1e6) as i64);
        s.insert((iv.end_beat * 1e6) as i64);
    }
    s.iter().map(|b| *b as f64 / 1e6).collect()
}

/// For a window [window_start, window_end], check if exactly one interval
/// covers the entire window. If so, evaluate the expression at both endpoints
/// and return (start_value, end_value) and the easing type.
fn pec_covering(
    intervals: &[MotionInterval],
    window_start: f64,
    window_end: f64,
    env: &EvalEnv,
) -> Option<(f64, f64, u8)> {
    let covering: Vec<&MotionInterval> = intervals
        .iter()
        .filter(|iv| iv.start_beat <= window_start && iv.end_beat >= window_end)
        .collect();
    if covering.len() == 1 {
        let expr = &covering[0].expression;
        let mut es = *env;
        es.beat = window_start;
        let sv = eval_expr(expr, &es);
        es.beat = window_end;
        let ev = eval_expr(expr, &es);
        let easing = extract_pec_easing_type(expr);
        Some((sv, ev, easing))
    } else {
        None
    }
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

/// `cd + cr` for rotation easing: sets start value then eases to end.
fn pec_easing_rotation(iv: &MotionInterval, line_id: usize) -> Vec<String> {
    let env = EvalEnv::default();
    let st = pec_t(iv.start_beat);
    let et = pec_t(iv.end_beat);
    let mut es = env;
    es.beat = iv.start_beat;
    let sv = eval_expr(&iv.expression, &es);
    es.beat = iv.end_beat;
    let ev = eval_expr(&iv.expression, &es);
    let fmt = |v: f64| -> String { format!("{v:.5}") };
    if (sv - ev).abs() < 1e-9 || (iv.end_beat - iv.start_beat).abs() < 1e-12 {
        // Constant or zero-width interval — single cd
        vec![format!("cd {line_id} {st} {}", fmt(sv))]
    } else {
        // Easing interval — cd st sv + cr st et ev easeType
        let easing = extract_pec_easing_type(&iv.expression);
        vec![
            format!("cd {line_id} {st} {}", fmt(sv)),
            format!("cr {line_id} {st} {et} {} {}", fmt(ev), easing),
        ]
    }
}

/// `ca + cf` for alpha easing: sets start value then eases to end.
///
/// Note: `cf` does not support a custom easing type — it is always linear.
fn pec_easing_alpha(iv: &MotionInterval, line_id: usize) -> Vec<String> {
    let env = EvalEnv::default();
    let st = pec_t(iv.start_beat);
    let et = pec_t(iv.end_beat);
    let mut es = env;
    es.beat = iv.start_beat;
    let sv = eval_expr(&iv.expression, &es);
    es.beat = iv.end_beat;
    let ev = eval_expr(&iv.expression, &es);
    let fmt = |v: f64| -> String { format!("{v:.5}") };
    if (sv - ev).abs() < 1e-9 || (iv.end_beat - iv.start_beat).abs() < 1e-12 {
        // Constant or zero-width interval — single ca
        vec![format!("ca {line_id} {st} {}", fmt(sv))]
    } else {
        // Easing interval — ca st sv + cf st et ev
        vec![
            format!("ca {line_id} {st} {}", fmt(sv)),
            format!("cf {line_id} {st} {et} {}", fmt(ev)),
        ]
    }
}

/// Extract PEC easing type (1–28) from an FCS easing expression.
fn extract_pec_easing_type(expr: &fcs_core::ast::Expression) -> u8 {
    match expr {
        fcs_core::ast::Expression::Call { name, .. } if name.starts_with("ease") => {
            easing_map::fcs_id_to_rpe_easing_type(easing_map::fcs_easing_id(name).unwrap_or(1))
                .unwrap_or(1)
        }
        _ => 1, // default linear
    }
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
    fn test_position_y_preserved_in_cp() {
        let src = r#"#fcs v4.0.0
meta { name: "PEC Y Test"; offset: 0ms; }
masterTimeline { 0.0b -> 120.0; }
judgelines {
    line Main {
        zOrder: 0;
        bpmTimeline { 0.0b -> 120.0; }
        motion { layer {
            positionY { [0.0b => 8.0b]: easeLinear(b, 0.0b, 8.0b, 0px, 540px, 0.0, 1.0); }
        } }
        notes { tap { time: 0.0b; positionX: 0px; } }
    }
}"#;
        let (_, doc) = parser::parse_document(src).expect("parse");
        let pec = fcs_to_pec(&doc);
        // positionY interval [0, 8] beats, easeLinear from 0px to 540px.
        // At start (0 beats): Y=700 (pec_y(0) = center)
        assert!(
            pec.contains("cp 0 0.00 1024 700"),
            "Y start should be center 700:\n{pec}"
        );
        // cm eases from start position (cp 1024,700) to end (1024,1400) with easing type 1
        assert!(
            pec.contains("cm 0 0.00 16384.00 1024 1400 1"),
            "cm easing event missing:\n{pec}"
        );
        // Note: autofill adds a trailing margin cp at 16384.00 with Y=700 (pec_y(0)=center)
        // This overrides the cm's end value but is limited to the 0.5-beat margin.
    }

    #[test]
    fn test_rotation_uses_cr_for_easing() {
        let src = r#"#fcs v4.0.0
meta { name: "PEC Rot Test"; offset: 0ms; }
masterTimeline { 0.0b -> 120.0; }
judgelines {
    line Main {
        zOrder: 0;
        bpmTimeline { 0.0b -> 120.0; }
        motion { layer {
            rotation { [0.0b => 4.0b]: easeLinear(b, 0.0b, 4.0b, 0deg, 90deg, 0.0, 1.0); }
        } }
        notes { tap { time: 0.0b; positionX: 0px; } }
    }
}"#;
        let (_, doc) = parser::parse_document(src).expect("parse");
        let pec = fcs_to_pec(&doc);
        // Should have cd at 0 (start value) and cr for interpolation
        assert!(pec.contains("cd 0 0.00 0"), "cd start: {pec}");
        assert!(pec.contains("cr 0 0.00 8192.00 90"), "cr event: {pec}");
        // Should NOT have cd with end value 90 at 8192 (replaced by cr)
        // (autofill may add a cd at 8192 with value 0 from default margin)
        assert!(
            !pec.lines().any(|l| l.contains("cd 0 8192.00 90")),
            "cd at end with value 90 should not exist:\n{pec}"
        );
    }

    #[test]
    fn test_alpha_uses_cf_for_easing() {
        let src = r#"#fcs v4.0.0
meta { name: "PEC Alpha Test"; offset: 0ms; }
masterTimeline { 0.0b -> 120.0; }
judgelines {
    line Main {
        zOrder: 0;
        bpmTimeline { 0.0b -> 120.0; }
        motion { layer {
            alpha { [0.0b => 4.0b]: easeLinear(b, 0.0b, 4.0b, 0.0, 1.0, 0.0, 1.0); }
        } }
        notes { tap { time: 0.0b; positionX: 0px; } }
    }
}"#;
        let (_, doc) = parser::parse_document(src).expect("parse");
        let pec = fcs_to_pec(&doc);
        // Should have ca at 0 (start value) and cf for interpolation
        assert!(pec.contains("ca 0 0.00 0"), "ca start: {pec}");
        assert!(pec.contains("cf 0 0.00 8192.00 1"), "cf event: {pec}");
        // Old code would have ca at 8192 from interval end evaluation.
        // New code replaces that with cf — any ca at 8192 is only from margin.
        let ca_at_8192 = pec
            .lines()
            .filter(|l| l.starts_with("ca 0 8192.00"))
            .count();
        assert!(
            ca_at_8192 <= 1,
            "more than 1 ca at 8192 ({ca_at_8192}), old-style double-ca likely present:\n{pec}"
        );
    }

    #[test]
    fn test_pec_prefixes() {
        assert_eq!(pec_prefix(NoteKind::Tap), "n1");
        assert_eq!(pec_prefix(NoteKind::Hold), "n2");
        assert_eq!(pec_prefix(NoteKind::Flick), "n3");
        assert_eq!(pec_prefix(NoteKind::Drag), "n4");
    }
}
