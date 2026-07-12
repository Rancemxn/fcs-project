//! PGR (official Phigros) format parser.
pub mod types;

use crate::ir::*;
use crate::time::t_to_beat;

pub fn parse_pgr(json: &str) -> Result<IrChart, String> {
    let pgr: types::PgrChart =
        serde_json::from_str(json).map_err(|e| format!("PGR parse error: {}", e))?;
    to_ir(&pgr)
}

/// PGR INF sentinel events: both startTime and endTime are sentinel values
/// (startTime=-999999, endTime=1e9), marking "default value for all time".
/// Filter them out — they have no meaningful animation data.
/// Events with only one sentinel bound (e.g., startTime=17952, endTime=1e9)
/// are REAL events that must be preserved.
fn is_inf_event(e: &types::PgrEvent) -> bool {
    e.start_time.abs() > 1e8 && e.end_time.abs() > 1e8
}

fn to_ir(pgr: &types::PgrChart) -> Result<IrChart, String> {
    let mut lines = Vec::new();
    for (i, jl) in pgr.judge_line_list.iter().enumerate() {
        let bpm = jl.bpm;
        let name = format!("line{}", i);
        let notes_above: Vec<IrNote> = jl
            .notes_above
            .iter()
            .map(|n| convert_note(n, true, bpm))
            .collect();
        let notes_below: Vec<IrNote> = jl
            .notes_below
            .iter()
            .map(|n| convert_note(n, false, bpm))
            .collect();

        let events = IrEventBundle {
            speed: jl
                .speed_events
                .iter()
                .filter(|e| !is_inf_event(e))
                .map(|e| speed_event(e, bpm))
                .collect(),
            alpha: jl
                .judge_line_disappear_events
                .iter()
                .filter(|e| !is_inf_event(e))
                .map(|e| simple_event(e, IrEventKind::Alpha, bpm))
                .collect(),
            move_x: jl
                .judge_line_move_events
                .iter()
                .filter(|e| !is_inf_event(e))
                .map(|e| move_event_x(e, bpm, &pgr.format_version))
                .collect(),
            move_y: jl
                .judge_line_move_events
                .iter()
                .filter(|e| !is_inf_event(e))
                .map(|e| move_event_y(e, bpm, &pgr.format_version))
                .collect(),
            rotate: jl
                .judge_line_rotate_events
                .iter()
                .filter(|e| !is_inf_event(e))
                .map(|e| simple_event(e, IrEventKind::Rotate, bpm))
                .collect(),
            ..Default::default()
        };

        lines.push(IrLine {
            name,
            notes_above,
            notes_below,
            events,
            bpm,
            z_order: i as i32,
            texture: None,
        });
    }

    Ok(IrChart {
        meta: IrMeta {
            source_format: "pgr".into(),
            source_version: pgr.format_version as i32,
            ..Default::default()
        },
        bpm_list: vec![],
        offset_seconds: pgr.offset,
        lines,
    })
}

fn convert_note(n: &types::PgrNote, above: bool, bpm: f64) -> IrNote {
    let nt = n.note_type;
    // PGR positionX is in X-units: 1X = 108px at 1920w. Convert to FCS px.
    let x_fcs = n.position_x * 108.0;
    // PGR floorPosition is in Y-units. But per earlier analysis, this is handled
    // by the speed integral, NOT as yOffset. Keep yOffset=0.
    IrNote {
        kind: if (nt - 1.0).abs() < 0.1 {
            IrNoteKind::Tap
        } else if (nt - 2.0).abs() < 0.1 {
            IrNoteKind::Drag
        } else if (nt - 3.0).abs() < 0.1 {
            IrNoteKind::Hold
        } else if (nt - 4.0).abs() < 0.1 {
            IrNoteKind::Flick
        } else {
            IrNoteKind::Fake
        },
        time_beat: t_to_beat(n.time, bpm),
        position_x: x_fcs,
        speed: n.speed,
        hold_beat: if (nt - 3.0).abs() < 0.1 {
            t_to_beat(n.hold_time, bpm)
        } else {
            0.0
        },
        above,
        is_fake: nt > 4.5,
        alpha: 1.0,
        size: 1.0,
        y_offset: 0.0,
        visible_time: 0.0,
    }
}

fn simple_event(e: &types::PgrEvent, kind: IrEventKind, bpm: f64) -> IrEvent {
    IrEvent {
        kind,
        start_beat: t_to_beat(e.start_time, bpm),
        end_beat: t_to_beat(e.end_time, bpm),
        start_value: e.start,
        end_value: e.end,
        easing_type: 0,
        bezier_points: None,
    }
}

fn speed_event(e: &types::PgrEvent, bpm: f64) -> IrEvent {
    IrEvent {
        kind: IrEventKind::Speed,
        start_beat: t_to_beat(e.start_time, bpm),
        end_beat: t_to_beat(e.end_time, bpm),
        start_value: e.value,
        end_value: e.value,
        easing_type: 0,
        bezier_points: None,
    }
}

fn move_event_x(e: &types::PgrEvent, bpm: f64, fv: &types::FormatVersion) -> IrEvent {
    let (sv, ev) = match fv {
        types::FormatVersion::V1 => ((e.start / 1000.0).floor(), (e.end / 1000.0).floor()),
        _ => (e.start, e.end),
    };
    IrEvent {
        kind: IrEventKind::MoveX,
        start_beat: t_to_beat(e.start_time, bpm),
        end_beat: t_to_beat(e.end_time, bpm),
        start_value: sv,
        end_value: ev,
        easing_type: 0,
        bezier_points: None,
    }
}

fn move_event_y(e: &types::PgrEvent, bpm: f64, fv: &types::FormatVersion) -> IrEvent {
    let (sv, ev) = match fv {
        types::FormatVersion::V1 => {
            let sx = (e.start / 1000.0).floor();
            (
                e.start - sx * 1000.0,
                e.end - (e.end / 1000.0).floor() * 1000.0,
            )
        }
        _ => (e.start2, e.end2),
    };
    IrEvent {
        kind: IrEventKind::MoveY,
        start_beat: t_to_beat(e.start_time, bpm),
        end_beat: t_to_beat(e.end_time, bpm),
        start_value: sv,
        end_value: ev,
        easing_type: 0,
        bezier_points: None,
    }
}
