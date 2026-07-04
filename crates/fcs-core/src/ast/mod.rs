//! Abstract Syntax Tree types for FCS documents (§5).

pub mod expr;
pub mod line;
pub mod meta;
pub mod note;
pub mod shader;
pub mod template;
pub mod timeline;

pub use expr::{BinaryOp, CompareOp, Expression, Literal, UnaryOp, ValueType};
pub use line::{InheritFlags, JudgelineBlock, LineDef, MotionBlock, MotionInterval, MotionLayer};
pub use meta::{MetaBlock, MetaValue};
pub use note::{JudgeShape, NoteBlock, NoteInstance, NoteKind, NotePropertyValue, NotePrototype};
pub use shader::{ShaderBlock, ShaderDef, UniformBind};
pub use template::{TemplateBlock, TemplateDef, TemplateParam, TemplateStatement};
pub use timeline::{BpmEntry, BpmTimeline};

/// Top-level FCS document — the result of parsing a `.fcs` file.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub meta: MetaBlock,
    pub master_timeline: BpmTimeline,
    pub templates: Option<TemplateBlock>,
    pub judgelines: JudgelineBlock,
    pub shaders: Option<ShaderBlock>,
}
