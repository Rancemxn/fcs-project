//! BPM Timeline AST (§5.3 & §5.5.1).

#[derive(Debug, Clone, PartialEq)]
pub struct BpmEntry { pub beat: f64, pub bpm: f64, pub is_step_before: bool }

#[derive(Debug, Clone, PartialEq)]
pub struct BpmTimeline { pub entries: Vec<BpmEntry> }
