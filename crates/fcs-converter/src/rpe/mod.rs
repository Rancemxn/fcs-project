//! RPE (Re:PhiEdit) → IR converter.
pub mod types;
use crate::ir::*;

pub fn parse_rpe(json: &str) -> Result<IrChart, String> {
    let rpe: types::RpeChart =
        serde_json::from_str(json).map_err(|e| format!("RPE parse error: {}", e))?;
    to_ir(&rpe)
}

fn to_ir(rpe: &types::RpeChart) -> Result<IrChart, String> {
    let bpm_list: Vec<IrBpmPoint> = rpe
        .bpm_list
        .iter()
        .map(|b| IrBpmPoint {
            beat: b.start_time.to_f64(),
            bpm: b.bpm as f64,
        })
        .collect();
    let default_bpm = bpm_list.first().map(|b| b.bpm).unwrap_or(120.0);
    let mut lines = Vec::new();
    for (i, jl) in rpe.judge_line_list.iter().enumerate() {
        let name = if jl.name.is_empty() {
            format!("line{}", i)
        } else {
            jl.name.clone()
        };
        let mut notes_above = Vec::new();
        let mut notes_below = Vec::new();
        for n in &jl.notes {
            // RPE position_x is on a 1350-wide canvas. Map to FCS 1920-wide.
            let x_fcs = n.position_x as f64 / 1350.0 * 1920.0;
            let note = IrNote {
                kind: match n.kind {
                    1 => IrNoteKind::Tap,
                    2 => IrNoteKind::Drag,
                    3 => IrNoteKind::Hold,
                    4 => IrNoteKind::Flick,
                    _ => IrNoteKind::Fake,
                },
                time_beat: n.start_time.to_f64(),
                position_x: x_fcs,
                speed: n.speed as f64,
                hold_beat: if n.kind == 3 {
                    n.end_time.to_f64() - n.start_time.to_f64()
                } else {
                    0.0
                },
                above: n.above >= 1,
                is_fake: n.is_fake == 1,
                alpha: n.alpha as f64 / 255.0,
                size: n.size as f64,
                y_offset: n.y_offset as f64 / 900.0 * 1080.0,
                visible_time: n.visible_time as f64,
            };
            if note.above {
                notes_above.push(note)
            } else {
                notes_below.push(note)
            }
        }
        // Collect ALL event layers (unlike phichain which drops layers beyond the first)
        let events = merge_all_layers(&jl.event_layers);
        lines.push(IrLine {
            name,
            notes_above,
            notes_below,
            events,
            bpm: default_bpm,
            z_order: jl.z_order,
            texture: if jl.texture.is_empty() {
                None
            } else {
                Some(jl.texture.clone())
            },
        });
    }
    Ok(IrChart {
        meta: IrMeta {
            name: rpe.meta.name.clone(),
            artist: rpe.meta.composer.clone(),
            charter: rpe.meta.charter.clone(),
            level: rpe.meta.level.clone(),
            source_format: "rpe".into(),
            source_version: rpe.meta.rpe_version,
            ..Default::default()
        },
        bpm_list,
        offset_seconds: rpe.meta.offset as f64 / 1000.0,
        lines,
    })
}

fn merge_all_layers(layers: &[types::RpeEventLayer]) -> IrEventBundle {
    let mut b = IrEventBundle::default();
    for l in layers {
        for e in &l.speed_events {
            b.speed.push(IrEvent {
                kind: IrEventKind::Speed,
                start_beat: e.start_time.to_f64(),
                end_beat: e.end_time.to_f64(),
                // RPE speed events use a 4.5x scale vs PGR/FCS canonical units
                // (from tool-phi2rpe.py: RPE_value = PGR_value * PGR_UH * 900 / 120 = PGR_value * 4.5)
                // Normalize to canonical unit so the RPE writer's * 4.5 factor
                // produces correct values for both PGR->RPE and RPE->RPE paths.
                start_value: e.start as f64 / 4.5,
                end_value: e.end as f64 / 4.5,
                easing_type: 0,
                bezier_points: None,
            });
        }
        for e in &l.move_x_events {
            b.move_x.push(IrEvent {
                kind: IrEventKind::MoveX,
                start_beat: e.start_time.to_f64(),
                end_beat: e.end_time.to_f64(),
                // RPE x: [-675, 675] → IR [0,1] (0=left, 0.5=center, 1=right)
                start_value: e.start as f64 / 1350.0 + 0.5,
                end_value: e.end as f64 / 1350.0 + 0.5,
                easing_type: e.easing_type as u8,
                bezier_points: if e.bezier == 1 {
                    Some([e.bezier_points[0] as f64; 4])
                } else {
                    None
                },
            });
        }
        for e in &l.move_y_events {
            b.move_y.push(IrEvent {
                kind: IrEventKind::MoveY,
                start_beat: e.start_time.to_f64(),
                end_beat: e.end_time.to_f64(),
                // RPE y: [-450, 450] → IR [0,1] (0=bottom, 0.5=center, 1=top)
                start_value: e.start as f64 / 900.0 + 0.5,
                end_value: e.end as f64 / 900.0 + 0.5,
                easing_type: e.easing_type as u8,
                bezier_points: if e.bezier == 1 {
                    Some([e.bezier_points[0] as f64; 4])
                } else {
                    None
                },
            });
        }
        for e in &l.rotate_events {
            b.rotate.push(IrEvent {
                kind: IrEventKind::Rotate,
                start_beat: e.start_time.to_f64(),
                end_beat: e.end_time.to_f64(),
                start_value: -(e.start as f64),
                end_value: -(e.end as f64),
                easing_type: e.easing_type as u8,
                bezier_points: if e.bezier == 1 {
                    Some([e.bezier_points[0] as f64; 4])
                } else {
                    None
                },
            });
        }
        for e in &l.alpha_events {
            b.alpha.push(IrEvent {
                kind: IrEventKind::Alpha,
                start_beat: e.start_time.to_f64(),
                end_beat: e.end_time.to_f64(),
                start_value: e.start as f64 / 255.0,
                end_value: e.end as f64 / 255.0,
                easing_type: e.easing_type as u8,
                bezier_points: if e.bezier == 1 {
                    Some([e.bezier_points[0] as f64; 4])
                } else {
                    None
                },
            });
        }
    }
    b
}
