//! IR → FCS Document (AST) conversion with strict unit system.

use crate::ir::*;
use crate::coord;
use fcs_core::ast::*;
use fcs_core::units::{Color, TypedValue, Unit, TimeUnit, LengthUnit, AngleUnit};
use std::collections::BTreeMap;

pub fn ir_to_fcs(chart: &IrChart) -> Document {
    let meta = MetaBlock {
        name: if chart.meta.name.is_empty() {"Untitled".into()} else {chart.meta.name.clone()},
        artists: if chart.meta.artist.is_empty() {vec!["Unknown".into()]} else {vec![chart.meta.artist.clone()]},
        charters: if chart.meta.charter.is_empty() {vec!["Unknown".into()]} else {vec![chart.meta.charter.clone()]},
        offset: chart.offset_seconds * 1000.0,
        offset_unit: "ms".into(),
        version: "4.0.0".into(),
        extra: BTreeMap::new(),
    };

    let bpm = chart.lines.first().map(|l| l.bpm).unwrap_or(120.0);
    let master_timeline = BpmTimeline {
        entries: vec![BpmEntry { beat: 0.0, bpm, is_step_before: false }],
    };

    let lines: Vec<LineDef> = chart.lines.iter().map(build_line).collect();

    Document {
        meta, master_timeline,
        templates: None,
        judgelines: JudgelineBlock { lines },
        shaders: None,
    }
}

fn build_line(line: &IrLine) -> LineDef {
    let bt = BpmTimeline {
        entries: vec![BpmEntry { beat: 0.0, bpm: line.bpm, is_step_before: false }],
    };

    let mut layer = MotionLayer::default();
    for e in &line.events.speed {
        if e.start_beat < e.end_beat {
            // PGR speed: raw multiplier, FCS speed: dimensionless — use as-is
            layer.speed.push(MotionInterval {
                start_beat: e.start_beat, end_beat: e.end_beat, end_inclusive: true,
                expression: Expression::Literal(Literal::Float(e.end_value)),
            });
        }
    }
    for e in &line.events.move_x {
        if e.start_beat < e.end_beat {
            let px = coord::x_to_fcs_px(e.end_value);
            layer.position_x.push(MotionInterval {
                start_beat: e.start_beat, end_beat: e.end_beat, end_inclusive: true,
                expression: q_length(px, LengthUnit::Pixel),
            });
        }
    }
    for e in &line.events.move_y {
        if e.start_beat < e.end_beat {
            let px = coord::y_to_fcs_px(e.end_value);
            layer.position_y.push(MotionInterval {
                start_beat: e.start_beat, end_beat: e.end_beat, end_inclusive: true,
                expression: q_length(px, LengthUnit::Pixel),
            });
        }
    }
    for e in &line.events.rotate {
        if e.start_beat < e.end_beat {
            layer.rotation.push(MotionInterval {
                start_beat: e.start_beat, end_beat: e.end_beat, end_inclusive: true,
                expression: q_angle(e.end_value, AngleUnit::Degree),
            });
        }
    }
    for e in &line.events.alpha {
        if e.start_beat < e.end_beat {
            // PGR alpha: 0-1 → FCS alpha: dimensionless 0-1 — use as-is
            layer.alpha.push(MotionInterval {
                start_beat: e.start_beat, end_beat: e.end_beat, end_inclusive: true,
                expression: Expression::Literal(Literal::Float(e.end_value)),
            });
        }
    }

    let motion = MotionBlock { layers: vec![layer] };

    let mut instances = Vec::new();
    for n in &line.notes_above { instances.push(note_inst(n)); }
    for n in &line.notes_below { instances.push(note_inst(n)); }

    LineDef {
        name: line.name.clone(),
        texture: line.texture.clone(),
        texture_anchor: (0.5, 0.5),
        z_order: line.z_order,
        color: Color::WHITE,
        parent: None,
        inherit: InheritFlags::default(),
        bpm_timeline: bt,
        motion: Some(motion),
        notes: NoteBlock { prototypes: vec![], instances },
    }
}

/// Create a quantified length literal: `value`px
fn q_length(value: f64, unit: LengthUnit) -> Expression {
    let tv = TypedValue::new(value, Unit::Length(unit));
    Expression::Literal(Literal::Quantified { value: tv.value, unit: tv.unit })
}

/// Create a quantified time literal: `value`b
fn q_time_beat(value: f64) -> Expression {
    Expression::Literal(Literal::Quantified {
        value,
        unit: Unit::Time(TimeUnit::Beat),
    })
}

/// Create a quantified angle literal
fn q_angle(value: f64, unit: AngleUnit) -> Expression {
    Expression::Literal(Literal::Quantified {
        value,
        unit: Unit::Angle(unit),
    })
}

fn note_inst(n: &IrNote) -> NoteInstance {
    // IR position_x is already in FCS logical pixels (1920-wide canvas).
    // Each format parser (pgr/rpe/pec) is responsible for converting to this space.
    let x_px = n.position_x;

    let mut props: Vec<(String, NotePropertyValue)> = vec![
        ("time".into(), NotePropertyValue::Expr(q_time_beat(n.time_beat))),
        ("positionX".into(), NotePropertyValue::Expr(q_length(x_px, LengthUnit::Pixel))),
        ("speed".into(), NotePropertyValue::Expr(Expression::Literal(Literal::Float(n.speed)))),
        ("above".into(), NotePropertyValue::Bool(n.above)),
    ];

    let kind = match n.kind {
        IrNoteKind::Tap => NoteKind::Tap,
        IrNoteKind::Drag => NoteKind::Drag,
        IrNoteKind::Hold => {
            props.push(("endTime".into(), NotePropertyValue::Expr(q_time_beat(n.time_beat + n.hold_beat))));
            NoteKind::Hold
        }
        IrNoteKind::Flick => NoteKind::Flick,
        IrNoteKind::Fake => NoteKind::Fake,
    };

    // yOffset: NOT derived from floorPosition.
    // PGR floorPosition determines the line's target floor position at judgment time,
    // which is already handled by the speed event integral in FCS.
    // Setting yOffset = floorPosition would place the note far off the line at judgment.
    // yOffset remains 0 (default) — the note is at Y=0 when judged.

    if (n.alpha - 1.0).abs() > 1e-10 {
        props.push(("alpha".into(), NotePropertyValue::Expr(Expression::Literal(Literal::Float(n.alpha)))));
    }
    if n.size != 1.0 {
        props.push(("scaleX".into(), NotePropertyValue::Expr(Expression::Literal(Literal::Float(n.size)))));
    }
    if n.is_fake || kind == NoteKind::Fake {
        props.push(("fake".into(), NotePropertyValue::Bool(true)));
    }

    NoteInstance { kind, name: None, parent: None, properties: props }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_empty() {
        let doc = ir_to_fcs(&IrChart {
            meta: IrMeta::default(), bpm_list: vec![], offset_seconds: 0.0, lines: vec![],
        });
        assert_eq!(doc.meta.name, "Untitled");
    }
}
