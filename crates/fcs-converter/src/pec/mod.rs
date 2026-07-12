//! PEC (PhiEditer text) → IR converter. Ref: phispler-ext light_utils.py pec2rpe().
//!
//! PEC stores time in beat·2048 encoding (pec_t = beat * 2048). The parser
//! converts pec_t to beats for IR storage. The PEC writer re-encodes.
//!
//! PEC format (per reference):
//!   <offset_line>                    -- offset = (value - 150) / 1000 seconds
//!   bp <time> <bpm>                 -- BPM point (time in pec_t = beat·2048)
//!   n1 <line> <time> <x> <above> <fake> # <speed> & <size>
//!   n2 <line> <time> <visT> <x> <above> <fake> # <speed> & <size> (visT = visibleTime pec_t) [Hold]
//!   n3 <line> <time> <x> <above> <fake> [# <hold>] # <speed> & <size> (hold in pec_t) [Flick]
//!   n4 <line> <time> <x> <above> <fake> # <speed> & <size> [Drag]
//!   cp <line> <time> <x> <y>         -- position (point event)
//!   cd <line> <time> <v>             -- rotate (point event)
//!   ca <line> <time> <v>             -- alpha (point event)
//!   cv <line> <time> <v>             -- speed (point event)
//!   cm <line> <st> <et> <ex> <ey> <ease> -- move interpolation
//!   cr <line> <st> <et> <ev> <ease>      -- rotate interpolation
//!   cf <line> <st> <et> <ev>             -- alpha fade
//!   All times in pec_t units (beat·2048)
//!   cp <line> <time> <x> <y>         -- position (point event)
//!   cd <line> <time> <v>             -- rotate (point event)
//!   ca <line> <time> <v>             -- alpha (point event)
//!   cv <line> <time> <v>             -- speed (point event)
//!   cm <line> <st> <et> <ex> <ey> <ease> -- move interpolation
//!   cr <line> <st> <et> <ev> <ease>      -- rotate interpolation
//!   cf <line> <st> <et> <ev>             -- alpha fade
//!
//! Coordinate scales (from phispler-ext light_utils.py):
//!   Note x: 2048-scale → RPE: x/2048 * RPE_W. → FCS: (x/2048 - 0.5) * 1920
//!   cp x:   same as note x
//!   cp y:   1400-scale → RPE: (y/1400 - 0.5) * RPE_H
//!   cv speed: 1400-scale → RPE: s/1400 * RPE_H
//!   Time: all times are in beats (float), not beat*2048.

use crate::ir::*;
use std::collections::BTreeMap;

const RPE_H: f64 = 900.0;

fn pec_x_to_fcs(x: f64) -> f64 {
    (x / 2048.0 - 0.5) * 1920.0
}

/// PEC x → IR normalized [0,1] (0=left, 0.5=center, 1=right).
/// Used for move_x events (matching RPE/PGR IR convention).
fn pec_x_to_ir(x: f64) -> f64 {
    x / 2048.0
}

/// PEC y → IR normalized [0,1] (0=bottom, 0.5=center, 1=top).
/// Used for move_y events (matching RPE/PGR IR convention).
fn pec_y_to_ir(y: f64) -> f64 {
    y / 1400.0
}

fn pec_speed_to_ir(v: f64) -> f64 {
    v / 1400.0 * RPE_H
}

fn pec_t_to_beat(t: f64) -> f64 {
    t / 2048.0
}

pub fn parse_pec(text: &str) -> Result<IrChart, String> {
    let lines_raw: Vec<&str> = text.lines().collect();
    if lines_raw.is_empty() {
        return Err("empty PEC file".into());
    }
    let offset = lines_raw[0]
        .trim()
        .parse::<f64>()
        .map_err(|e| format!("PEC offset: {}", e))?;
    let offset_seconds = (offset - 150.0) / 1000.0;

    // Split note-speed/size pairs: "n1 ... # 1.0 & 1.0" → separate lines
    let tokens: Vec<Vec<String>> = lines_raw[1..]
        .iter()
        .flat_map(|l| {
            l.replace(" #", "\n#")
                .replace(" &", "\n&")
                .split('\n')
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .map(|l| {
            l.split(' ')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty())
        .collect();

    let mut bpm_list = Vec::new();
    let mut notes_raw = Vec::new();
    let mut speeds = Vec::new();
    let mut sizes = Vec::new();
    let mut cp_events = Vec::new();
    let mut cd_events = Vec::new();
    let mut ca_events = Vec::new();
    let mut cv_events = Vec::new();
    let mut cm_events = Vec::new();
    let mut cr_events = Vec::new();
    let mut cf_events = Vec::new();

    for tok in &tokens {
        if tok.is_empty() {
            continue;
        }
        match tok[0].as_str() {
            "bp" if tok.len() >= 3 => {
                bpm_list.push(IrBpmPoint {
                    beat: pec_t_to_beat(tok[1].parse::<f64>().unwrap_or(0.0)),
                    bpm: tok[2].parse::<f64>().unwrap_or(120.0),
                });
            }
            s if s.starts_with('n') && s.len() == 2 && tok.len() >= 6 => {
                notes_raw.push(tok.clone());
            }
            "#" if tok.len() >= 2 => speeds.push(tok[1].parse::<f64>().unwrap_or(1.0)),
            "&" if tok.len() >= 2 => sizes.push(tok[1].parse::<f64>().unwrap_or(1.0)),
            "cp" if tok.len() >= 5 => cp_events.push(tok.clone()),
            "cd" if tok.len() >= 4 => cd_events.push(tok.clone()),
            "ca" if tok.len() >= 4 => ca_events.push(tok.clone()),
            "cv" if tok.len() >= 4 => cv_events.push(tok.clone()),
            "cm" if tok.len() >= 7 => cm_events.push(tok.clone()),
            "cr" if tok.len() >= 6 => cr_events.push(tok.clone()),
            "cf" if tok.len() >= 5 => cf_events.push(tok.clone()),
            _ => {}
        }
    }

    cp_events.sort_by(|a, b| cmp_f64(&a[2], &b[2]));
    cd_events.sort_by(|a, b| cmp_f64(&a[2], &b[2]));
    ca_events.sort_by(|a, b| cmp_f64(&a[2], &b[2]));
    cv_events.sort_by(|a, b| cmp_f64(&a[2], &b[2]));
    cm_events.sort_by(|a, b| cmp_f64(&a[2], &b[2]));
    cr_events.sort_by(|a, b| cmp_f64(&a[2], &b[2]));
    cf_events.sort_by(|a, b| cmp_f64(&a[2], &b[2]));

    let default_bpm = bpm_list.first().map(|b| b.bpm).unwrap_or(120.0);

    // Group notes by PEC line index (raw[1]) instead of assigning all notes to all lines
    let mut line_notes: BTreeMap<i32, Vec<IrNote>> = BTreeMap::new();
    for (i, raw) in notes_raw.iter().enumerate() {
        let line_idx: i32 = raw.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let kind = match raw[0].as_str() {
            "n1" => IrNoteKind::Tap,
            "n2" => IrNoteKind::Hold,
            "n3" => IrNoteKind::Flick,
            "n4" => IrNoteKind::Drag,
            _ => IrNoteKind::Fake,
        };
        let note_speed = speeds.get(i).copied().unwrap_or(1.0);
        let size = sizes.get(i).copied().unwrap_or(1.0);
        let time_beat = pec_t_to_beat(raw[2].parse::<f64>().unwrap_or(0.0));

        let (x_pec, visible_time, above, is_fake) = if raw[0].as_str() == "n2" {
            // n2 (Hold): with or without visT — detect by token count
            let (x_pec, vis, above, fake) = if raw.len() >= 7 {
                // Format: n2 <line> <time> <visT> <x> <above> <fake>
                let x_pec = raw[4].parse::<f64>().unwrap_or(0.0);
                let vis = pec_t_to_beat(raw[3].parse::<f64>().unwrap_or(0.0));
                let above = raw[5].parse::<i32>().unwrap_or(1) == 1;
                let fake = raw
                    .get(6)
                    .is_some_and(|s| s.parse::<i32>().unwrap_or(0) == 1);
                (x_pec, vis, above, fake)
            } else {
                // Legacy: n2 <line> <time> <x> <above> <fake>
                let x_pec = raw[3].parse::<f64>().unwrap_or(0.0);
                let above = raw[4].parse::<i32>().unwrap_or(1) == 1;
                let fake = raw
                    .get(5)
                    .is_some_and(|s| s.parse::<i32>().unwrap_or(0) == 1);
                (x_pec, 0.0, above, fake)
            };
            (x_pec, vis, above, fake)
        } else {
            let x_pec = raw[3].parse::<f64>().unwrap_or(0.0);
            let above = raw[4].parse::<i32>().unwrap_or(1) == 1;
            let fake = raw
                .get(5)
                .is_some_and(|s| s.parse::<i32>().unwrap_or(0) == 1);
            (x_pec, 0.0, above, fake)
        };
        let x_fcs = pec_x_to_fcs(x_pec);

        // n3 (Flick): optional holdTime at raw[6], stored as PEC time units (beat*2048)
        let hold_beat = if raw[0].as_str() == "n3" {
            raw.get(6)
                .and_then(|s| s.parse::<i32>().ok())
                .map(|pec_time| pec_time as f64 / 2048.0)
                .unwrap_or(0.0)
        } else {
            0.0
        };

        line_notes.entry(line_idx).or_default().push(IrNote {
            kind,
            above,
            time_beat,
            position_x: x_fcs,
            speed: note_speed,
            hold_beat,
            is_fake,
            alpha: 1.0,
            size,
            y_offset: 0.0,
            visible_time,
        });
    }

    let mut lines_map: BTreeMap<i32, PECEvents> = BTreeMap::new();

    for raw in &cp_events {
        let k: i32 = raw[1].parse().unwrap_or(0);
        let t = pec_t_to_beat(raw[2].parse::<f64>().unwrap_or(0.0));
        let x = raw[3].parse::<f64>().unwrap_or(0.0);
        let y = raw[4].parse::<f64>().unwrap_or(0.0);
        let entry = lines_map.entry(k).or_default();
        entry.move_x.push(IrEvent {
            kind: IrEventKind::MoveX,
            start_beat: t,
            end_beat: t,
            start_value: pec_x_to_ir(x),
            end_value: pec_x_to_ir(x),
            easing_type: 0,
            bezier_points: None,
        });
        entry.move_y.push(IrEvent {
            kind: IrEventKind::MoveY,
            start_beat: t,
            end_beat: t,
            start_value: pec_y_to_ir(y),
            end_value: pec_y_to_ir(y),
            easing_type: 0,
            bezier_points: None,
        });
    }

    for raw in &cd_events {
        let k: i32 = raw[1].parse().unwrap_or(0);
        let t = pec_t_to_beat(raw[2].parse::<f64>().unwrap_or(0.0));
        let v = raw[3].parse::<f64>().unwrap_or(0.0);
        let entry = lines_map.entry(k).or_default();
        entry.rotate.push(IrEvent {
            kind: IrEventKind::Rotate,
            start_beat: t,
            end_beat: t,
            start_value: v,
            end_value: v,
            easing_type: 0,
            bezier_points: None,
        });
    }

    for raw in &ca_events {
        let k: i32 = raw[1].parse().unwrap_or(0);
        let t = pec_t_to_beat(raw[2].parse::<f64>().unwrap_or(0.0));
        let v = raw[3].parse::<f64>().unwrap_or(0.0);
        let entry = lines_map.entry(k).or_default();
        entry.alpha.push(IrEvent {
            kind: IrEventKind::Alpha,
            start_beat: t,
            end_beat: t,
            start_value: v,
            end_value: v,
            easing_type: 0,
            bezier_points: None,
        });
    }

    for raw in &cv_events {
        let k: i32 = raw[1].parse().unwrap_or(0);
        let t = pec_t_to_beat(raw[2].parse::<f64>().unwrap_or(0.0));
        let v = raw[3].parse::<f64>().unwrap_or(0.0);
        let entry = lines_map.entry(k).or_default();
        entry.speed.push(IrEvent {
            kind: IrEventKind::Speed,
            start_beat: t,
            end_beat: t,
            start_value: pec_speed_to_ir(v),
            end_value: pec_speed_to_ir(v),
            easing_type: 0,
            bezier_points: None,
        });
    }

    for raw in &cm_events {
        let k: i32 = raw[1].parse().unwrap_or(0);
        let st = pec_t_to_beat(raw[2].parse::<f64>().unwrap_or(0.0));
        let et = pec_t_to_beat(raw[3].parse::<f64>().unwrap_or(0.0));
        let ex = raw[4].parse::<f64>().unwrap_or(0.0);
        let ey = raw[5].parse::<f64>().unwrap_or(0.0);
        let ease: u8 = raw.get(6).and_then(|s| s.parse().ok()).unwrap_or(0);
        let entry = lines_map.entry(k).or_default();
        let start_x = entry
            .move_x
            .last()
            .map(|e| e.end_value)
            .unwrap_or(pec_x_to_ir(ex));
        let start_y = entry
            .move_y
            .last()
            .map(|e| e.end_value)
            .unwrap_or(pec_y_to_ir(ey));
        entry.move_x.push(IrEvent {
            kind: IrEventKind::MoveX,
            start_beat: st,
            end_beat: et,
            start_value: start_x,
            end_value: pec_x_to_ir(ex),
            easing_type: ease,
            bezier_points: None,
        });
        entry.move_y.push(IrEvent {
            kind: IrEventKind::MoveY,
            start_beat: st,
            end_beat: et,
            start_value: start_y,
            end_value: pec_y_to_ir(ey),
            easing_type: ease,
            bezier_points: None,
        });
    }

    for raw in &cr_events {
        let k: i32 = raw[1].parse().unwrap_or(0);
        let st = pec_t_to_beat(raw[2].parse::<f64>().unwrap_or(0.0));
        let et = pec_t_to_beat(raw[3].parse::<f64>().unwrap_or(0.0));
        let ev = raw[4].parse::<f64>().unwrap_or(0.0);
        let ease: u8 = raw.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
        let entry = lines_map.entry(k).or_default();
        let start_v = entry.rotate.last().map(|e| e.end_value).unwrap_or(ev);
        entry.rotate.push(IrEvent {
            kind: IrEventKind::Rotate,
            start_beat: st,
            end_beat: et,
            start_value: start_v,
            end_value: ev,
            easing_type: ease,
            bezier_points: None,
        });
    }

    for raw in &cf_events {
        let k: i32 = raw[1].parse().unwrap_or(0);
        let st = pec_t_to_beat(raw[2].parse::<f64>().unwrap_or(0.0));
        let et = pec_t_to_beat(raw[3].parse::<f64>().unwrap_or(0.0));
        let ev = raw[4].parse::<f64>().unwrap_or(0.0);
        let entry = lines_map.entry(k).or_default();
        let start_v = entry.alpha.last().map(|e| e.end_value).unwrap_or(ev);
        entry.alpha.push(IrEvent {
            kind: IrEventKind::Alpha,
            start_beat: st,
            end_beat: et,
            start_value: start_v,
            end_value: ev,
            easing_type: 0,
            bezier_points: None,
        });
    }

    let max_line = std::cmp::max(
        lines_map.keys().copied().max().unwrap_or(0),
        line_notes.keys().copied().max().unwrap_or(0),
    );
    let lines: Vec<IrLine> = (0..=max_line)
        .map(|k| {
            let entry = lines_map.remove(&k).unwrap_or_default();
            let notes = line_notes.remove(&k).unwrap_or_default();
            IrLine {
                name: format!("line{}", k),
                notes_above: notes.iter().filter(|n| n.above).cloned().collect(),
                notes_below: notes.iter().filter(|n| !n.above).cloned().collect(),
                events: IrEventBundle {
                    speed: entry.speed,
                    move_x: entry.move_x,
                    move_y: entry.move_y,
                    rotate: entry.rotate,
                    alpha: entry.alpha,
                    ..Default::default()
                },
                bpm: default_bpm,
                z_order: k,
                texture: None,
            }
        })
        .collect();

    Ok(IrChart {
        meta: IrMeta {
            source_format: "pec".into(),
            ..Default::default()
        },
        bpm_list,
        offset_seconds,
        lines,
    })
}

fn cmp_f64(a: &str, b: &str) -> std::cmp::Ordering {
    a.parse::<f64>()
        .unwrap_or(0.0)
        .partial_cmp(&b.parse::<f64>().unwrap_or(0.0))
        .unwrap_or(std::cmp::Ordering::Equal)
}

#[derive(Default)]
struct PECEvents {
    speed: Vec<IrEvent>,
    move_x: Vec<IrEvent>,
    move_y: Vec<IrEvent>,
    rotate: Vec<IrEvent>,
    alpha: Vec<IrEvent>,
}
