//! Compilation context — symbol tables, error collection.
use crate::ast::{Document, NotePrototype, TemplateDef};
use crate::error::{CompileError, DiagnosticBag, Warning};
use crate::bytecode::constant_pool::ConstantPoolBuilder;
use crate::bytecode::string_table::StringTableBuilder;
use std::collections::HashMap;

pub struct CompileContext<'a> {
    pub doc: &'a Document,
    pub diagnostics: DiagnosticBag,
    pub strings: StringTableBuilder,
    pub consts: ConstantPoolBuilder,
    pub templates: HashMap<String, &'a TemplateDef>,
    pub prototypes: HashMap<String, HashMap<String, &'a NotePrototype>>,
    pub used_templates: Vec<String>,
}

impl<'a> CompileContext<'a> {
    pub fn new(doc: &'a Document) -> Self {
        let mut templates = HashMap::new();
        if let Some(ref tb) = doc.templates {
            for def in &tb.definitions { templates.insert(def.name.clone(), def); }
        }
        let mut prototypes = HashMap::new();
        for line in &doc.judgelines.lines {
            let mut pmap = HashMap::new();
            for proto in &line.notes.prototypes { pmap.insert(proto.name.clone(), proto); }
            prototypes.insert(line.name.clone(), pmap);
        }
        Self { doc, diagnostics: DiagnosticBag::new(), strings: StringTableBuilder::new(),
            consts: ConstantPoolBuilder::new(), templates, prototypes, used_templates: Vec::new() }
    }
    pub fn has_errors(&self) -> bool { self.diagnostics.has_errors() }
    pub fn error(&mut self, e: CompileError) { self.diagnostics.error(e); }
    pub fn warn(&mut self, w: Warning) { self.diagnostics.warn(w); }
}
