//! Shared note property accessors — used by all three format writers.

use crate::from_fcs::evaluator::{EvalEnv, eval_expr};
use fcs_core::ast::{Literal, NoteInstance, NoteKind, NotePropertyValue};

pub fn note_f64(note: &NoteInstance, key: &str, default: f64) -> f64 {
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

/// Handles both `Bool(true)` and `Expr(Literal(Boolean(true)))` (parser wraps
/// `true`/`false` in an expression literal).
pub fn note_bool(note: &NoteInstance, key: &str, default: bool) -> bool {
    note.properties
        .iter()
        .find(|(k, _)| k.as_str() == key)
        .map(|(_, v)| match v {
            NotePropertyValue::Bool(b) => *b,
            NotePropertyValue::Expr(fcs_core::ast::Expression::Literal(Literal::Boolean(b))) => *b,
            _ => default,
        })
        .unwrap_or(default)
}

pub fn note_is_above(note: &NoteInstance) -> bool {
    note.properties
        .iter()
        .find(|(k, _)| k.as_str() == "above")
        .map(|(_, v)| is_truthy(v))
        .unwrap_or(true)
}

fn is_truthy(v: &NotePropertyValue) -> bool {
    match v {
        NotePropertyValue::Bool(b) => *b,
        NotePropertyValue::Expr(fcs_core::ast::Expression::Literal(Literal::Boolean(b))) => *b,
        _ => true,
    }
}

pub fn note_time_beat(note: &NoteInstance) -> f64 {
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

pub fn note_end_beat(note: &NoteInstance) -> f64 {
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

pub fn lit_to_f64(lit: &fcs_core::ast::Literal) -> f64 {
    match lit {
        fcs_core::ast::Literal::Integer(n) => *n as f64,
        fcs_core::ast::Literal::Float(f) => *f,
        fcs_core::ast::Literal::Quantified { value, .. } => *value,
        _ => 0.0,
    }
}

pub fn kind_to_pgr_type(kind: NoteKind) -> u8 {
    match kind {
        NoteKind::Tap => 1,
        NoteKind::Drag => 2,
        NoteKind::Hold => 3,
        NoteKind::Flick => 4,
        NoteKind::Fake => 0,
    }
}

pub fn kind_to_rpe_type(kind: NoteKind) -> i32 {
    match kind {
        NoteKind::Tap => 1,
        NoteKind::Hold => 2,
        NoteKind::Flick => 3,
        NoteKind::Drag => 4,
        NoteKind::Fake => 0,
    }
}

pub fn kind_to_pec_prefix(kind: NoteKind) -> &'static str {
    match kind {
        NoteKind::Tap => "n1",
        NoteKind::Drag => "n2",
        NoteKind::Hold => "n3",
        NoteKind::Flick => "n4",
        NoteKind::Fake => "n1",
    }
}
