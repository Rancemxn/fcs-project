//! FCS format: parse tests (FCS→FCS round-trip not yet supported as a library).

#[path = "common/paths.rs"]
mod paths;

use fcs_core::parser::parse_document;

fn load_fcs(name: &str) -> fcs_core::ast::Document {
    let path = paths::manifest_path(&format!("examples/fcs/{name}"));
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {name}: {e}"));
    let (rest, doc) =
        parse_document(&src).unwrap_or_else(|e| panic!("failed to parse {name}: {e}"));
    assert!(rest.trim().is_empty(), "{name}: unparsed trailing content");
    doc
}

#[test]
fn test_parse_fcs_empty() {
    let doc = load_fcs("empty.fcs");
    assert_eq!(doc.meta.name, "Empty");
    assert!(doc.judgelines.lines.is_empty(), "expected 0 lines");
}

#[test]
fn test_parse_fcs_simple() {
    let doc = load_fcs("simple.fcs");
    assert_eq!(doc.judgelines.lines.len(), 1);
    let line = &doc.judgelines.lines[0];
    assert_eq!(line.notes.instances.len(), 4);
    use fcs_core::ast::NoteKind;
    assert_eq!(line.notes.instances[0].kind, NoteKind::Tap);
    assert_eq!(line.notes.instances[3].kind, NoteKind::Hold);
    let motion = line.motion.as_ref().expect("expected motion block");
    assert!(!motion.layers[0].position_x.is_empty());
    assert!(!motion.layers[0].rotation.is_empty());
}

#[test]
fn test_parse_fcs_multi_line() {
    let doc = load_fcs("multi-line.fcs");
    assert_eq!(doc.judgelines.lines.len(), 3);
    assert_eq!(doc.judgelines.lines[0].z_order, 10);
    assert_eq!(doc.judgelines.lines[1].z_order, 5);
    assert_eq!(doc.judgelines.lines[2].z_order, 0);
}

#[test]
fn test_parse_fcs_easing() {
    use fcs_core::ast::Expression;
    let doc = load_fcs("easing.fcs");
    let layer = &doc.judgelines.lines[0].motion.as_ref().unwrap().layers[0];
    assert_eq!(layer.position_x.len(), 3);
    match &layer.position_x[0].expression {
        Expression::Call { name, .. } => assert_eq!(name, "easeLinear"),
        _ => panic!("expected easeLinear Call"),
    }
    match &layer.position_x[1].expression {
        Expression::Call { name, .. } => assert_eq!(name, "easeOutSine"),
        _ => panic!("expected easeOutSine Call"),
    }
    match &layer.rotation[0].expression {
        Expression::Call { name, .. } => assert_eq!(name, "easeOutElastic"),
        _ => panic!("expected easeOutElastic Call"),
    }
}

#[test]
fn test_parse_fcs_template() {
    let doc = load_fcs("template.fcs");
    assert!(!doc.judgelines.lines.is_empty());
    assert_eq!(doc.judgelines.lines[0].notes.instances.len(), 3);
    assert!(doc.templates.is_some());
}

#[test]
fn test_parse_fcs_overlapping() {
    let doc = load_fcs("overlapping.fcs");
    let motion = &doc.judgelines.lines[0].motion.as_ref().unwrap().layers[0];
    assert_eq!(motion.position_x.len(), 3);
    assert_eq!(motion.rotation.len(), 1);
    assert_eq!(motion.alpha.len(), 1);
}
