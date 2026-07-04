//! AOT compiler — orchestrates the full .fcs → .fcbc pipeline.

pub mod context;
pub mod emit;
pub mod expr;
pub mod prototype;
pub mod speed;
pub mod timeline;

use crate::ast::Document;
use crate::bytecode::FcbcFile;
use crate::error::DiagnosticBag;
use context::CompileContext;
use emit::emit;

/// Compile a parsed FCS document into .fcbc bytecode.
///
/// Returns the FcbcFile on success, or a DiagnosticBag with all errors and warnings.
pub fn compile(doc: &Document) -> Result<FcbcFile, DiagnosticBag> {
    let mut ctx = CompileContext::new(doc);

    // Run semantic validation
    validate(&mut ctx);

    if ctx.has_errors() {
        return Err(ctx.diagnostics);
    }

    // Emit bytecode
    emit(&mut ctx).map_err(|_| ctx.diagnostics)
}

/// Semantic validation — checks all rules from §8.1/§8.2.
fn validate(ctx: &mut CompileContext) {
    use crate::error::CompileError;

    let doc = ctx.doc;

    // Check required blocks
    if doc.judgelines.lines.is_empty() {
        ctx.error(CompileError::RequiredBlockMissing("judgelines".into()));
    }

    // Validate masterTimeline
    if doc.master_timeline.entries.is_empty() || doc.master_timeline.entries[0].beat != 0.0 {
        ctx.error(CompileError::MasterTimelineNonZeroStart);
    } else {
        for e in &doc.master_timeline.entries {
            if e.bpm <= 0.0 {
                ctx.error(CompileError::MasterTimelineBpmNonPositive);
            }
        }
    }

    // Validate each line's bpmTimeline
    for line in &doc.judgelines.lines {
        let bt = &line.bpm_timeline;
        if bt.entries.is_empty() || bt.entries[0].beat != 0.0 {
            ctx.error(CompileError::BpmTimelineNonZeroStart);
        } else {
            for e in &bt.entries {
                if e.bpm <= 0.0 {
                    ctx.error(CompileError::BpmTimelineBpmNonPositive);
                }
            }
        }

        // Validate parent line reference
        if let Some(ref parent_name) = line.parent {
            let exists = doc.judgelines.lines.iter().any(|l| &l.name == parent_name);
            if !exists {
                ctx.error(CompileError::ParentLineNotFound(parent_name.clone()));
            }
        }

        // Validate note instances — check template references
        for inst in &line.notes.instances {
            if let Some(ref parent_name) = inst.parent {
                let exists = ctx.prototypes
                    .get(&line.name)
                    .map(|pmap| pmap.contains_key(parent_name.as_str()))
                    .unwrap_or(false);
                if !exists {
                    ctx.error(CompileError::UndefinedTemplate(parent_name.clone()));
                }
            }
        }
    }

    // Detect unused templates (W005)
    if let Some(ref tb) = doc.templates {
        for def in &tb.definitions {
            if !ctx.used_templates.contains(&def.name) {
                ctx.warn(crate::error::Warning::UnusedTemplate(def.name.clone()));
            }
        }
    }

    // Detect empty lines (W006)
    for line in &doc.judgelines.lines {
        if line.notes.instances.is_empty() && line.motion.is_none() {
            ctx.warn(crate::error::Warning::EmptyLine(line.name.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_document;

    #[test]
    fn test_compile_minimal() {
        let src = r#"
meta { name: "Test"; artists: ["X"]; charters: ["Y"]; offset: 0ms; version: "4.0.0"; }
masterTimeline { 0.0b -> 120.0; }
judgelines { line L { bpmTimeline { 0.0b -> 120.0; } } }
"#;
        let (_, doc) = parse_document(src).unwrap();
        let result = compile(&doc);
        assert!(result.is_ok(), "compile error: {:?}", result.err());
    }

    #[test]
    fn test_compile_with_notes() {
        let src = r#"
meta { name: "T"; artists: ["A"]; charters: ["C"]; offset: 0ms; version: "4.0.0"; }
masterTimeline { 0.0b -> 180.0; }
judgelines {
    line L {
        bpmTimeline { 0.0b -> 180.0; }
        notes {
            tap {
                time: 4.0b;
                positionX: -150px;
                speed: 1.0;
            }
            hold {
                time: 8.0b;
                endTime: 12.0b;
                positionX: 0px;
                speed: 1.5;
            }
        }
    }
}
"#;
        let (_, doc) = parse_document(src).unwrap();
        let result = compile(&doc);
        assert!(result.is_ok(), "compile error: {:?}", result.err());
        let file = result.unwrap();
        assert_eq!(file.lines.len(), 1);
        assert_eq!(file.lines[0].notes.len(), 2);
    }

    #[test]
    fn test_compile_with_prototype() {
        let src = r#"
meta { name: "T"; artists: ["A"]; charters: ["C"]; offset: 0ms; version: "4.0.0"; }
masterTimeline { 0.0b -> 120.0; }
judgelines {
    line L {
        bpmTimeline { 0.0b -> 120.0; }
        notes {
            tap templateGhost {
                above: true;
                speed: 1.0;
            }
            tap myNote : templateGhost {
                time: 4.0b;
                positionX: -150px;
            }
        }
    }
}
"#;
        let (_, doc) = parse_document(src).unwrap();
        let result = compile(&doc);
        assert!(result.is_ok(), "compile error: {:?}", result.err());
    }
}
