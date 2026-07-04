//! Error types — compile errors (E001-E017), warnings (W001-W007), runtime errors (§8).

use std::fmt;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Compile-time fatal errors (§8.1)
// ---------------------------------------------------------------------------

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    #[error("E001: unclosed string or block")]
    UnclosedDelimiter,
    #[error("E002: single-quoted strings are not allowed; use double quotes")]
    SingleQuoteString,
    #[error("E003: type mismatch — {0}")]
    TypeMismatch(String),
    #[error("E004: unknown unit: '{0}'")]
    UnknownUnit(String),
    #[error("E005: undefined template reference: '{0}'")]
    UndefinedTemplate(String),
    #[error("E006: circular template dependency involving '{0}'")]
    CircularTemplate(String),
    #[error("E007: template contains loop or recursion — must be O(1)")]
    TemplateLoop,
    #[error("E008: masterTimeline must start at 0.0b")]
    MasterTimelineNonZeroStart,
    #[error("E009: masterTimeline BPM must be positive")]
    MasterTimelineBpmNonPositive,
    #[error("E010: bpmTimeline must start at 0.0b")]
    BpmTimelineNonZeroStart,
    #[error("E011: bpmTimeline BPM must be positive")]
    BpmTimelineBpmNonPositive,
    #[error("E012: beat denominator cannot be zero")]
    BeatDenominatorZero,
    #[error("E013: parent line '{0}' does not exist")]
    ParentLineNotFound(String),
    #[error("E014: required block missing: {0}")]
    RequiredBlockMissing(String),
    #[error("E015: required meta field missing: '{0}'")]
    RequiredMetaFieldMissing(String),
    #[error("E016: invalid judgeShape: {0}")]
    InvalidJudgeShape(String),
    #[error("E017: hold note endTime ({end_time}) < time ({time})")]
    HoldEndTimeBeforeTime { end_time: String, time: String },
    #[error("syntax error: {0}")]
    Syntax(String),
}

// ---------------------------------------------------------------------------
// Compile-time warnings (§8.2)
// ---------------------------------------------------------------------------

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    #[error("W001: motion interval overlap in layer {layer}, property '{property}'")]
    MotionOverlap { layer: usize, property: String },
    #[error("W002: motion interval gap in layer {layer}, property '{property}'")]
    MotionGap { layer: usize, property: String },
    #[error("W003: unrecognized meta field: '{0}'")]
    UnrecognizedMetaField(String),
    #[error("W004: unrecognized note property: '{0}'")]
    UnrecognizedNoteProperty(String),
    #[error("W005: unused template: '{0}'")]
    UnusedTemplate(String),
    #[error("W006: line '{0}' has no notes and no motion")]
    EmptyLine(String),
    #[error("W007: startTime > endTime for '{0}' — will be auto-swapped")]
    InvertedTimeRange(String),
}

// ---------------------------------------------------------------------------
// Runtime errors (§8.3)
// ---------------------------------------------------------------------------

#[derive(Error, Debug, Clone, PartialEq)]
pub enum RuntimeError {
    #[error("division by zero — entity culled this frame")]
    DivisionByZero,
    #[error("expression produced {0} — entity culled this frame")]
    NonFiniteValue(String),
    #[error("VM stack overflow — limit is 256")]
    StackOverflow,
    #[error("VM stack underflow — insufficient values for operation")]
    StackUnderflow,
    #[error("texture not found: '{0}'")]
    TextureNotFound(String),
    #[error("parent line index out of bounds")]
    ParentIndexOutOfBounds,
}

// ---------------------------------------------------------------------------
// Diagnostic bag
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
pub struct DiagnosticBag {
    pub errors: Vec<CompileError>,
    pub warnings: Vec<Warning>,
}

impl DiagnosticBag {
    pub fn new() -> Self { Self::default() }
    pub fn has_errors(&self) -> bool { !self.errors.is_empty() }
    pub fn error(&mut self, e: CompileError) { self.errors.push(e); }
    pub fn warn(&mut self, w: Warning) { self.warnings.push(w); }
}

impl fmt::Display for DiagnosticBag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for w in &self.warnings { writeln!(f, "warning: {}", w)?; }
        for e in &self.errors { writeln!(f, "error: {}", e)?; }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_bag() {
        let mut bag = DiagnosticBag::new();
        bag.error(CompileError::UnknownUnit("foo".into()));
        bag.warn(Warning::UnusedTemplate("tpl".into()));
        assert!(bag.has_errors());
        assert_eq!(bag.errors.len(), 1);
        assert_eq!(bag.warnings.len(), 1);
    }

    #[test]
    fn test_error_codes() {
        assert!(CompileError::TypeMismatch("test".into()).to_string().contains("E003"));
        assert!(Warning::MotionOverlap { layer: 0, property: "x".into() }.to_string().contains("W001"));
    }
}
