//! RPE (Re:PhiEdit) format writer — FCS Document → RPE JSON.
//!
//! Reference: `refer/phira-docs/src/chart-standard/chart-format/rpe/`,
//! `refer/phispler-ext/src/tool-phi2rpe.py` (PGR→RPE).

use crate::from_fcs::coord;
use crate::from_fcs::easing_map;
use crate::from_fcs::evaluator::{EvalEnv, eval_expr};
use crate::from_fcs::time;
use crate::from_fcs::{autofill, controls, flattener};
use fcs_core::ast::{
    Document, LineDef, MotionBlock, MotionInterval, MotionLayer, NoteInstance, NoteKind,
};
use serde::Serialize;

// ---- RPE JSON output types ------------------------------------------------

#[derive(Debug, Clone, Serialize)]
struct RpeChart {
    #[serde(rename = "BPMList")]
    bpm_list: Vec<RpeBpmPoint>,
    #[serde(rename = "META")]
    meta: RpeMeta,
    #[serde(rename = "judgeLineGroup")]
    judge_line_group: Vec<String>,
    #[serde(rename = "judgeLineList")]
    judge_line_list: Vec<RpeLine>,
    #[serde(rename = "multiLineString")]
    multi_line_string: String,
    #[serde(rename = "multiScale")]
    multi_scale: f64,
}
#[derive(Debug, Clone, Serialize)]
struct RpeBpmPoint {
    bpm: f32,
    #[serde(rename = "startTime")]
    start_time: [i32; 3],
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RpeMeta {
    #[serde(rename = "RPEVersion")]
    rpe_version: i32,
    name: String,
    composer: String,
    charter: String,
    level: String,
    song: String,
    background: String,
    id: String,
    offset: i32,
}
#[derive(Debug, Clone, Serialize)]
struct RpeLine {
    #[serde(rename = "Group")]
    group: i32,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Texture")]
    texture: String,
    father: i32,
    #[serde(rename = "isCover")]
    is_cover: i32,
    #[serde(rename = "zOrder")]
    z_order: i32,
    #[serde(rename = "bpmfactor")]
    bpm_factor: f64,
    #[serde(rename = "eventLayers")]
    event_layers: Vec<RpeEventLayer>,
    #[serde(rename = "extended")]
    extended: RpeExtended,
    #[serde(rename = "alphaControl", skip_serializing_if = "Vec::is_empty")]
    alpha_control: Vec<controls::AlphaPoint>,
    #[serde(rename = "posControl", skip_serializing_if = "Vec::is_empty")]
    pos_control: Vec<controls::PosPoint>,
    #[serde(rename = "sizeControl", skip_serializing_if = "Vec::is_empty")]
    size_control: Vec<controls::SizePoint>,
    notes: Vec<RpeNote>,
}
#[derive(Debug, Clone, Serialize)]
struct RpeEventLayer {
    #[serde(rename = "speedEvents", skip_serializing_if = "Vec::is_empty")]
    speed_events: Vec<RpeEvent>,
    #[serde(rename = "moveXEvents", skip_serializing_if = "Vec::is_empty")]
    move_x_events: Vec<RpeEvent>,
    #[serde(rename = "moveYEvents", skip_serializing_if = "Vec::is_empty")]
    move_y_events: Vec<RpeEvent>,
    #[serde(rename = "rotateEvents", skip_serializing_if = "Vec::is_empty")]
    rotate_events: Vec<RpeEvent>,
    #[serde(rename = "alphaEvents", skip_serializing_if = "Vec::is_empty")]
    alpha_events: Vec<RpeEvent>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RpeEvent {
    start_time: [i32; 3],
    end_time: [i32; 3],
    start: f32,
    end: f32,
    #[serde(rename = "easingType")]
    easing_type: i32,
    bezier: i32,
    #[serde(rename = "bezierPoints")]
    bezier_points: [f32; 4],
    #[serde(rename = "easingLeft")]
    easing_left: f32,
    #[serde(rename = "easingRight")]
    easing_right: f32,
    #[serde(rename = "linkgroup")]
    link_group: i32,
}
#[derive(Debug, Clone, Serialize)]
struct RpeExtended {
    #[serde(rename = "inclineEvents")]
    incline_events: Vec<RpeEvent>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RpeNote {
    #[serde(rename = "type")]
    kind: i32,
    start_time: [i32; 3],
    end_time: [i32; 3],
    #[serde(rename = "positionX")]
    position_x: f32,
    speed: f32,
    above: i32,
    #[serde(rename = "isFake")]
    is_fake: i32,
    alpha: i32,
    size: f32,
    #[serde(rename = "yOffset")]
    y_offset: f32,
    #[serde(rename = "visibleTime")]
    visible_time: f32,
}

// ---- Public API -----------------------------------------------------------

pub fn fcs_to_rpe_json(doc: &Document) -> String {
    serde_json::to_string_pretty(&fcs_to_rpe(doc)).unwrap_or_else(|_| "{}".into())
}

fn fcs_to_rpe(doc: &Document) -> RpeChart {
    RpeChart {
        bpm_list: doc
            .master_timeline
            .entries
            .iter()
            .map(|e| RpeBpmPoint {
                bpm: e.bpm as f32,
                start_time: beat_arr(e.beat),
            })
            .collect(),
        meta: RpeMeta {
            rpe_version: 140,
            name: doc.meta.name.clone(),
            composer: doc.meta.artists.join(", "),
            charter: doc.meta.charters.join(", "),
            level: doc
                .meta
                .extra
                .get("level")
                .and_then(|v| {
                    if let fcs_core::ast::MetaValue::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_default(),
            song: String::new(),
            background: String::new(),
            id: String::new(),
            offset: doc.meta.offset as i32,
        },
        judge_line_group: vec!["Default".into()],
        multi_line_string: String::new(),
        multi_scale: 1.0,
        judge_line_list: doc.judgelines.lines.iter().map(convert_line).collect(),
    }
}

// ---- Line conversion ------------------------------------------------------

fn convert_line(line: &LineDef) -> RpeLine {
    let bpm = line
        .bpm_timeline
        .entries
        .first()
        .map(|e| e.bpm)
        .unwrap_or(120.0);
    // Flatten proto inheritance, discard kind=fake; autofill motion
    let flat_notes = flattener::flatten_note_block(&line.notes);
    let motion = line.motion.as_ref().map(|m| MotionBlock {
        layers: m.layers.iter().map(autofill::autofill_layer).collect(),
    });
    // Sample RPE Controls from first note with d-dependent expressions
    let note_ctrl = flat_notes
        .concrete
        .iter()
        .find_map(controls::sample_note_controls);
    let (alpha_ctrl, pos_ctrl, size_ctrl) = match note_ctrl {
        Some(c) => (c.alpha_control, c.pos_control, c.size_control),
        None => (vec![], vec![], vec![]),
    };

    // Sort notes by start time then position X to preserve original order
    // (above/below split in IR destroys it)
    let mut sorted_notes = flat_notes.concrete;
    sorted_notes.sort_by(|a, b| {
        let ta = note_time_beat(a);
        let tb = note_time_beat(b);
        ta.partial_cmp(&tb)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                note_f64(a, "positionX", 0.0)
                    .partial_cmp(&note_f64(b, "positionX", 0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    RpeLine {
        group: 0,
        name: line.name.clone(),
        texture: line.texture.clone().unwrap_or_default(),
        father: -1,
        is_cover: 1,
        z_order: line.z_order,
        bpm_factor: {
            let chart_bpm = line
                .bpm_timeline
                .entries
                .first()
                .map(|e| e.bpm)
                .unwrap_or(120.0);
            let bpm_0 = bpm;
            if (chart_bpm - bpm_0).abs() > 0.01 {
                bpm_0 / chart_bpm
            } else {
                1.0
            }
        },
        event_layers: motion_to_rpe_layers(&motion, bpm, &EvalEnv::default()),
        extended: RpeExtended {
            incline_events: vec![RpeEvent {
                start_time: [0, 0, 1],
                end_time: [1, 0, 1],
                start: 0.0,
                end: 0.0,
                easing_type: 0,
                bezier: 0,
                bezier_points: [0.0; 4],
                easing_left: 0.0,
                easing_right: 1.0,
                link_group: 0,
            }],
        },
        alpha_control: alpha_ctrl,
        pos_control: pos_ctrl,
        size_control: size_ctrl,
        notes: sorted_notes.iter().map(convert_note).collect(),
    }
}

// ---- Note conversion ------------------------------------------------------

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

fn note_kind_to_rpe_type(kind: NoteKind) -> i32 {
    match kind {
        NoteKind::Tap => 1,
        NoteKind::Drag => 2,
        NoteKind::Hold => 3,
        NoteKind::Flick => 4,
        NoteKind::Fake => 0,
    }
}

fn beat_arr(beat: f64) -> [i32; 3] {
    let b = time::beat_to_rpe_beat(beat);
    [b.a, b.b, b.c]
}

fn convert_note(note: &NoteInstance) -> RpeNote {
    let tb = note_time_beat(note);
    let eb = note_end_beat(note);
    RpeNote {
        kind: note_kind_to_rpe_type(note.kind),
        start_time: beat_arr(tb),
        end_time: if note.kind == NoteKind::Hold && eb > tb {
            beat_arr(eb)
        } else {
            beat_arr(tb)
        },
        position_x: coord::fcs_px_to_rpe_x(note_f64(note, "positionX", 0.0)) as f32,
        speed: note_f64(note, "speed", 1.0) as f32,
        above: if note_bool(note, "above", true) { 1 } else { 0 },
        is_fake: if note_bool(note, "fake", false) { 1 } else { 0 },
        alpha: (note_f64(note, "alpha", 1.0) * 255.0).round() as i32,
        size: note_f64(note, "size", 1.0) as f32,
        y_offset: coord::fcs_px_to_rpe_y(note_f64(note, "yOffset", 0.0)) as f32,
        visible_time: note_f64(note, "visibleTime", 999999.0) as f32,
    }
}

// ---- Motion → RPE events --------------------------------------------------

/// Controls how the FCS expression value is converted to RPE coordinate space.
#[derive(Clone, Copy)]
enum RpeValueConv {
    /// Pass through as-is (rotate, scale, colors).
    Raw,
    /// Speed: IR canonical × 4.5.
    Speed,
    /// PositionX: FCS px → RPE x (center-origin [-675, 675]).
    PosX,
    /// PositionY: FCS px → RPE y (center-origin [-450, 450]).
    PosY,
    /// Alpha: IR [0,1] → RPE [0,255].
    Alpha,
}

fn motion_to_rpe_layers(
    motion: &Option<MotionBlock>,
    bpm: f64,
    env: &EvalEnv,
) -> Vec<RpeEventLayer> {
    match motion {
        Some(m) => m
            .layers
            .iter()
            .map(|l| convert_layer(l, bpm, env))
            .collect(),
        None => vec![],
    }
}

fn convert_layer(layer: &MotionLayer, bpm: f64, env: &EvalEnv) -> RpeEventLayer {
    RpeEventLayer {
        speed_events: intervals_to_rpe(&layer.speed, bpm, env, RpeValueConv::Speed),
        move_x_events: intervals_to_rpe(&layer.position_x, bpm, env, RpeValueConv::PosX),
        move_y_events: intervals_to_rpe(&layer.position_y, bpm, env, RpeValueConv::PosY),
        rotate_events: intervals_to_rpe(&layer.rotation, bpm, env, RpeValueConv::Raw),
        alpha_events: intervals_to_rpe(&layer.alpha, bpm, env, RpeValueConv::Alpha),
    }
}

fn intervals_to_rpe(
    intervals: &[MotionInterval],
    bpm: f64,
    env: &EvalEnv,
    conv: RpeValueConv,
) -> Vec<RpeEvent> {
    intervals
        .iter()
        .map(|iv| {
            let mut es = *env;
            es.beat = iv.start_beat;
            es.seconds = time::beat_to_seconds(iv.start_beat, bpm);
            let sv = eval_expr(&iv.expression, &es);
            es.beat = iv.end_beat;
            es.seconds = time::beat_to_seconds(iv.end_beat, bpm);
            let ev = eval_expr(&iv.expression, &es);
            let (et, bz, bps) = extract_easing(&iv.expression);
            let (s, e): (f32, f32) = match conv {
                RpeValueConv::Speed => (sv as f32 * 4.5, ev as f32 * 4.5),
                RpeValueConv::PosX => (
                    coord::fcs_px_to_rpe_x(sv) as f32,
                    coord::fcs_px_to_rpe_x(ev) as f32,
                ),
                RpeValueConv::PosY => (
                    coord::fcs_px_to_rpe_y(sv) as f32,
                    coord::fcs_px_to_rpe_y(ev) as f32,
                ),
                RpeValueConv::Alpha => (sv as f32 * 255.0, ev as f32 * 255.0),
                RpeValueConv::Raw => (sv as f32, ev as f32),
            };
            RpeEvent {
                start_time: beat_arr(iv.start_beat),
                end_time: beat_arr(iv.end_beat),
                start: s,
                end: e,
                easing_type: et,
                bezier: bz,
                bezier_points: bps,
                easing_left: 0.0,
                easing_right: 1.0,
                link_group: 0,
            }
        })
        .collect()
}

fn extract_easing(expr: &fcs_core::ast::Expression) -> (i32, i32, [f32; 4]) {
    match expr {
        fcs_core::ast::Expression::Call { name, args } if name.starts_with("ease") => {
            if name == "easeBezier" && args.len() >= 11 {
                let g = |i: usize| -> f32 {
                    match &args[i] {
                        fcs_core::ast::Expression::Literal(fcs_core::ast::Literal::Float(f)) => {
                            *f as f32
                        }
                        fcs_core::ast::Expression::Literal(fcs_core::ast::Literal::Integer(n)) => {
                            *n as f32
                        }
                        _ => 0.0,
                    }
                };
                (0, 1, [g(7), g(8), g(9), g(10)])
            } else {
                match easing_map::fcs_id_to_rpe_easing_type(
                    easing_map::fcs_easing_id(name).unwrap_or(1),
                ) {
                    Some(t) => (t as i32, 0, [0.0; 4]),
                    None => (1, 0, [0.0; 4]),
                }
            }
        }
        _ => (1, 0, [0.0; 4]),
    }
}

// ---- Tests ----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fcs_core::parser;
    #[test]
    fn test_convert_sample_fcs_to_rpe() {
        let src = include_str!("../../../../examples/sample.fcs");
        let (_, doc) = parser::parse_document(src).expect("parse");
        let json = fcs_to_rpe_json(&doc);
        let c: serde_json::Value = serde_json::from_str(&json).expect("json");
        assert_eq!(c["META"]["RPEVersion"], 140);
    }
    #[test]
    fn test_note_types() {
        assert_eq!(note_kind_to_rpe_type(NoteKind::Tap), 1);
        assert_eq!(note_kind_to_rpe_type(NoteKind::Drag), 2);
        assert_eq!(note_kind_to_rpe_type(NoteKind::Hold), 3);
        assert_eq!(note_kind_to_rpe_type(NoteKind::Flick), 4);
    }
}
