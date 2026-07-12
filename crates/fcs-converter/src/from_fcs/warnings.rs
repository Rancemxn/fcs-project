//! Warning collector — accumulates non-fatal issues during conversion.

use std::cell::RefCell;
use std::fmt;

thread_local! { static WARNINGS: RefCell<Vec<Warning>> = const { RefCell::new(Vec::new()) }; }

#[derive(Debug, Clone)]
pub struct Warning {
    pub kind: WarningKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarningKind {
    EvalNaN,
    UnknownFunction,
    MissingProperty,
    TypeMismatch,
    FeatureDropped,
    Info,
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}", self.kind, self.message)
    }
}

pub fn warn(kind: WarningKind, msg: impl Into<String>) {
    WARNINGS.with(|w| {
        w.borrow_mut().push(Warning {
            kind,
            message: msg.into(),
        })
    });
}

pub fn take_all() -> Vec<Warning> {
    WARNINGS.with(|w| w.borrow_mut().drain(..).collect())
}

pub fn print_all() {
    let ws = take_all();
    if ws.is_empty() {
        return;
    }
    eprintln!("--- Conversion Warnings ({}) ---", ws.len());
    for w in &ws {
        eprintln!("  {w}");
    }
    let n = ws
        .iter()
        .filter(|w| !matches!(w.kind, WarningKind::Info))
        .count();
    if n > 0 {
        eprintln!("  {n} non-info warning(s).");
    }
}
