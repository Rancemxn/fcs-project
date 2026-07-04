//! Note AST (§5.6).
use super::expr::{Expression, Literal};
use crate::units::{Color, TypedValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoteKind { Tap, Hold, Flick, Drag, Fake }

impl NoteKind {
    pub fn as_str(&self) -> &'static str {
        match self { NoteKind::Tap=>"tap", NoteKind::Hold=>"hold", NoteKind::Flick=>"flick", NoteKind::Drag=>"drag", NoteKind::Fake=>"fake" }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum JudgeShape {
    InfiniteY { width: f64 },
    Rectangle { width: TypedValue, height: TypedValue },
    Circle { radius: TypedValue },
}

#[derive(Debug, Clone, PartialEq)]
pub enum NotePropertyValue {
    Expr(Expression), Literal(Literal), JudgeShape(JudgeShape), Color(Color), String(String), Bool(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NotePrototype {
    pub kind: NoteKind, pub name: String, pub parent: Option<String>,
    pub properties: Vec<(String, NotePropertyValue)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoteInstance {
    pub kind: NoteKind, pub name: Option<String>, pub parent: Option<String>,
    pub properties: Vec<(String, NotePropertyValue)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NoteBlock { pub prototypes: Vec<NotePrototype>, pub instances: Vec<NoteInstance> }
