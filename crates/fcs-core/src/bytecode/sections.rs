//! .fcbc section types (§9.6–§9.10).
use crate::bytecode::property::PropertyDescriptor;

#[derive(Debug, Clone)]
pub struct MetaSection { pub name_st_off: u32, pub artist_st_offs: Vec<u32>, pub charter_st_offs: Vec<u32>, pub offset_seconds: f64 }

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TimelineLutEntry { pub beat: f64, pub accumulated_sec: f64, pub bpm: f64 }

#[derive(Debug, Clone)]
pub struct MasterTimelineSection { pub entries: Vec<TimelineLutEntry> }

#[derive(Debug, Clone)]
pub struct LineHeader {
    pub name_st_off: u32, pub texture_st_off: u32,
    pub texture_anchor_x: f32, pub texture_anchor_y: f32,
    pub z_order: i32, pub color_rgba: [u8; 4],
    pub parent_line_index: i32, pub inherit_flags: u8,
    pub bpm_lut_offset: u32, pub bpm_lut_entry_count: u32,
    pub motion_layer_count: u32, pub note_count: u32,
}

impl Default for LineHeader {
    fn default() -> Self { Self { name_st_off:0, texture_st_off:0, texture_anchor_x:0.5, texture_anchor_y:0.5, z_order:0, color_rgba:[255,255,255,255], parent_line_index:-1, inherit_flags:1, bpm_lut_offset:0, bpm_lut_entry_count:0, motion_layer_count:0, note_count:0 } }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct Keyframe { pub time_beat: f64, pub easing_id: u8, pub clamp_left: f32, pub clamp_right: f32, pub bezier_params: [f32; 4], pub target_value: f32 }

#[derive(Debug, Clone)]
#[repr(C)]
pub struct NoteEncoding {
    pub kind: u8, pub flags: u8, pub judge_shape_kind: u8, pub padding1: [u8; 5],
    pub time_beat: f64, pub end_time_beat: f64,
    pub position_x: PropertyDescriptor, pub speed: PropertyDescriptor,
    pub scale_x: PropertyDescriptor, pub scale_y: PropertyDescriptor,
    pub x_offset: PropertyDescriptor, pub y_offset: PropertyDescriptor,
    pub alpha: PropertyDescriptor,
    pub judge_shape_param1: PropertyDescriptor, pub judge_shape_param2: PropertyDescriptor,
    pub color_rgba: [u8; 4], pub padding2: [u8; 4],
}

impl Default for NoteEncoding {
    fn default() -> Self { Self { kind:0, flags:1, judge_shape_kind:0, padding1:[0;5], time_beat:0.0, end_time_beat:0.0, position_x:PropertyDescriptor::new_const(0.0), speed:PropertyDescriptor::new_const(1.0), scale_x:PropertyDescriptor::new_const(1.0), scale_y:PropertyDescriptor::new_const(1.0), x_offset:PropertyDescriptor::new_const(0.0), y_offset:PropertyDescriptor::new_const(0.0), alpha:PropertyDescriptor::new_const(1.0), judge_shape_param1:PropertyDescriptor::new_const(1.0), judge_shape_param2:PropertyDescriptor::new_const(0.0), color_rgba:[255,255,255,255], padding2:[0;4] } }
}

#[derive(Debug, Clone)]
pub struct LineSection { pub header: LineHeader, pub bpm_lut: Vec<TimelineLutEntry>, pub motion_keyframes: Vec<Vec<Vec<Keyframe>>>, pub notes: Vec<NoteEncoding> }

#[derive(Debug, Clone)]
pub struct ExpressionEntry { pub bytecode: Vec<u8> }

#[derive(Debug, Clone)]
pub struct UniformBind { pub uniform_name_st_off: u32, pub value: PropertyDescriptor }

#[derive(Debug, Clone)]
pub struct ShaderEntry { pub name_st_off: u32, pub vertex_path_st_off: u32, pub fragment_path_st_off: u32, pub binds: Vec<UniformBind> }
