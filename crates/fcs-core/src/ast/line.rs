//! Judgeline AST (§5.5).
use super::expr::Expression;
use super::note::NoteBlock;
use super::timeline::BpmTimeline;
use crate::units::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InheritFlags { pub position: bool, pub rotation: bool, pub scale: bool, pub alpha: bool }

impl Default for InheritFlags {
    fn default() -> Self { Self { position: true, rotation: false, scale: false, alpha: false } }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MotionInterval {
    pub start_beat: f64, pub end_beat: f64, pub end_inclusive: bool, pub expression: Expression,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MotionLayer {
    pub position_x: Vec<MotionInterval>, pub position_y: Vec<MotionInterval>,
    pub rotation: Vec<MotionInterval>, pub alpha: Vec<MotionInterval>,
    pub scale_x: Vec<MotionInterval>, pub scale_y: Vec<MotionInterval>,
    pub speed: Vec<MotionInterval>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MotionBlock { pub layers: Vec<MotionLayer> }

#[derive(Debug, Clone, PartialEq)]
pub struct LineDef {
    pub name: String, pub texture: Option<String>, pub texture_anchor: (f64, f64),
    pub z_order: i32, pub color: Color, pub parent: Option<String>,
    pub inherit: InheritFlags, pub bpm_timeline: BpmTimeline,
    pub motion: Option<MotionBlock>, pub notes: NoteBlock,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JudgelineBlock { pub lines: Vec<LineDef> }
