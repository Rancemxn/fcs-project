//! Unified Intermediate Representation for Phigros chart formats.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrChart {
    pub meta: IrMeta,
    pub bpm_list: Vec<IrBpmPoint>,
    pub offset_seconds: f64,
    pub lines: Vec<IrLine>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IrMeta {
    pub name: String, pub artist: String, pub charter: String,
    pub level: String, pub illustration: String,
    pub source_format: String, pub source_version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrBpmPoint { pub beat: f64, pub bpm: f64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrLine {
    pub name: String,
    pub notes_above: Vec<IrNote>, pub notes_below: Vec<IrNote>,
    pub events: IrEventBundle, pub bpm: f64,
    pub z_order: i32, pub texture: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IrEventBundle {
    pub speed: Vec<IrEvent>, pub move_x: Vec<IrEvent>, pub move_y: Vec<IrEvent>,
    pub rotate: Vec<IrEvent>, pub alpha: Vec<IrEvent>,
    pub scale_x: Vec<IrEvent>, pub scale_y: Vec<IrEvent>, pub color: Vec<IrEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IrNoteKind { Tap=1, Drag=2, Hold=3, Flick=4, Fake=0 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrNote {
    pub kind: IrNoteKind, pub time_beat: f64, pub position_x: f64,
    pub speed: f64, pub hold_beat: f64, pub above: bool,
    pub is_fake: bool, pub alpha: f64, pub size: f64,
    pub y_offset: f64, pub visible_time: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IrEventKind { Speed, MoveX, MoveY, Rotate, Alpha, ScaleX, ScaleY, Color }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrEvent {
    pub kind: IrEventKind, pub start_beat: f64, pub end_beat: f64,
    pub start_value: f64, pub end_value: f64,
    pub easing_type: u8, pub bezier_points: Option<[f64; 4]>,
}
