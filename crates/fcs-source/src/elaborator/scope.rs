//! Immutable lexical binding scopes.

use std::collections::BTreeMap;

use crate::ast::{SourceSpan, Type, TypedValue};

use super::ElaboratorError as Diagnostic;

pub(super) const BUILTIN_NAMES: &[&str] = &[
    "abs", "min", "max", "clamp", "floor", "ceil", "round", "sqrt", "exp", "ln", "pow", "sin",
    "cos", "tan", "asin", "acos", "atan", "atan2", "approxEq", "toFloat", "seconds", "radians",
];

#[derive(Debug, Clone)]
pub(super) struct Binding {
    pub(super) ty: Type,
    pub(super) value: Option<TypedValue>,
    pub(super) span: SourceSpan,
}

#[derive(Debug, Clone)]
pub(super) struct Scope {
    frames: Vec<BTreeMap<String, Binding>>,
    reserved: BTreeMap<String, SourceSpan>,
}

impl Scope {
    pub(super) fn root() -> Self {
        Self {
            frames: vec![BTreeMap::new()],
            reserved: BTreeMap::new(),
        }
    }

    pub(super) fn root_with_builtins() -> Result<Self, Diagnostic> {
        let builtin_span = SourceSpan::new(0, 0);
        let mut scope = Self::root();
        scope.declare(
            "pi".to_owned(),
            Binding {
                ty: Type::Float,
                value: Some(TypedValue::Float(std::f64::consts::PI)),
                span: builtin_span,
            },
        )?;
        scope.declare(
            "tau".to_owned(),
            Binding {
                ty: Type::Float,
                value: Some(TypedValue::Float(std::f64::consts::TAU)),
                span: builtin_span,
            },
        )?;
        for builtin in BUILTIN_NAMES {
            scope.reserve((*builtin).to_owned(), builtin_span);
        }
        Ok(scope)
    }

    pub(super) fn child(&self) -> Self {
        let mut scope = self.clone();
        scope.frames.push(BTreeMap::new());
        scope
    }

    pub(super) fn declare(&mut self, name: String, binding: Binding) -> Result<(), Diagnostic> {
        if let Some(previous) = self
            .frames
            .last()
            .expect("a scope always has a frame")
            .get(&name)
        {
            return Err(Diagnostic::DuplicateBinding {
                name,
                span: binding.span,
                previous_span: previous.span,
            });
        }
        if let Some(previous_span) = self.lookup_enclosing_span(&name) {
            return Err(Diagnostic::ShadowedBinding {
                name,
                span: binding.span,
                previous_span,
            });
        }
        self.frames
            .last_mut()
            .expect("a scope always has a frame")
            .insert(name, binding);
        Ok(())
    }

    pub(super) fn lookup(&self, name: &str) -> Option<&Binding> {
        self.frames.iter().rev().find_map(|frame| frame.get(name))
    }

    pub(super) fn contains(&self, name: &str) -> bool {
        self.lookup(name).is_some() || self.reserved.contains_key(name)
    }

    pub(super) fn reserve(&mut self, name: String, span: SourceSpan) {
        self.reserved.insert(name, span);
    }

    fn lookup_enclosing_span(&self, name: &str) -> Option<SourceSpan> {
        self.frames
            .iter()
            .rev()
            .skip(1)
            .find_map(|frame| frame.get(name).map(|binding| binding.span))
            .or_else(|| self.reserved.get(name).copied())
    }
}
