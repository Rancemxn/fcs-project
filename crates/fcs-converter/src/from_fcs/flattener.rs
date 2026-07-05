//! AST pre-processing: proto inheritance flattening + parent line motion merge.
//!
//! These transforms run on the Document AST before any writer, so all three
//! output formats (PGR/RPE/PEC) benefit from the flattened result.

use fcs_core::ast::*;
use std::collections::BTreeMap;

// ---- Proto inheritance flattening -----------------------------------------

type RawEntry = (NoteKind, Option<String>, Vec<(String, NotePropertyValue)>);

#[derive(Debug, Clone)]
pub struct FlattenedNotes {
    pub concrete: Vec<NoteInstance>,
    pub fake: Vec<NoteInstance>,
}

pub fn flatten_note_block(block: &NoteBlock) -> FlattenedNotes {
    // Collect ALL named definitions (both prototypes and named instances)
    // into a single inheritance map.
    let mut name_map: BTreeMap<String, Vec<(String, NotePropertyValue)>> = BTreeMap::new();
    // Also track kind for each named entry
    let mut name_kind: BTreeMap<String, NoteKind> = BTreeMap::new();

    // First pass: collect raw properties (no inheritance yet)
    let mut raw_props: BTreeMap<String, RawEntry> = BTreeMap::new();
    for p in &block.prototypes {
        raw_props.insert(
            p.name.clone(),
            (p.kind, p.parent.clone(), p.properties.clone()),
        );
    }
    for inst in &block.instances {
        if let Some(ref name) = inst.name {
            raw_props.insert(
                name.clone(),
                (inst.kind, inst.parent.clone(), inst.properties.clone()),
            );
        }
    }

    // Resolve inheritance chains
    for (name, (kind, parent, _props)) in &raw_props {
        let resolved = resolve_from_raw(name, parent.as_deref(), &raw_props);
        name_map.insert(name.clone(), resolved);
        name_kind.insert(name.clone(), *kind);
    }

    // Process ALL entries: named ones via inheritance, unnamed instances directly
    let mut concrete = Vec::new();
    let mut fake = Vec::new();

    // Named entries: only those with 'time' are playable instances.
    // kind=fake entries with 'time' go to fake list (for discard after inheritance).
    // Templates (no 'time') are never included in output.
    for (name, props) in &name_map {
        let kind = name_kind.get(name).copied().unwrap_or(NoteKind::Tap);
        if props.iter().any(|(k, _)| k == "time") {
            let inst = NoteInstance {
                kind,
                name: Some(name.clone()),
                parent: None,
                properties: props.clone(),
            };
            if kind == NoteKind::Fake {
                fake.push(inst);
            } else {
                concrete.push(inst);
            }
        }
    }

    // Unnamed instances (always concrete)
    for inst in &block.instances {
        if inst.name.is_some() {
            continue;
        } // already handled above
        let props = if let Some(ref parent_name) = inst.parent {
            let mut base = name_map
                .get(parent_name.as_str())
                .cloned()
                .unwrap_or_default();
            merge_props(&mut base, &inst.properties);
            base
        } else {
            inst.properties.clone()
        };
        let flat = NoteInstance {
            kind: inst.kind,
            name: None,
            parent: None,
            properties: props,
        };
        if inst.kind == NoteKind::Fake {
            fake.push(flat);
        } else {
            concrete.push(flat);
        }
    }

    FlattenedNotes { concrete, fake }
}

fn resolve_from_raw(
    name: &str,
    parent: Option<&str>,
    raw: &BTreeMap<String, RawEntry>,
) -> Vec<(String, NotePropertyValue)> {
    let mut props = if let Some(pn) = parent {
        if let Some((_, p_parent, _p_props)) = raw.get(pn) {
            resolve_from_raw(pn, p_parent.as_deref(), raw)
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    if let Some((_, _, own)) = raw.get(name) {
        merge_props(&mut props, own);
    }
    props
}

fn merge_props(
    base: &mut Vec<(String, NotePropertyValue)>,
    overrides: &[(String, NotePropertyValue)],
) {
    for (k, v) in overrides {
        base.retain(|(bk, _)| bk != k);
        base.push((k.clone(), v.clone()));
    }
}

// ---- Parent line AOT flattening -------------------------------------------

pub fn flatten_parent_lines(doc: &Document) -> Document {
    let mut doc = doc.clone();
    let name_to_idx: BTreeMap<String, usize> = doc
        .judgelines
        .lines
        .iter()
        .enumerate()
        .map(|(i, l)| (l.name.clone(), i))
        .collect();
    let order = topological_order(&doc.judgelines.lines, &name_to_idx);
    let mut flattened: Vec<LineDef> = Vec::new();
    let mut idx_map: BTreeMap<usize, usize> = BTreeMap::new();
    for &old_idx in &order {
        let line = &doc.judgelines.lines[old_idx];
        let mut flat = line.clone();
        if let Some(ref pn) = line.parent
            && let Some(&poi) = name_to_idx.get(pn)
            && let Some(&pni) = idx_map.get(&poi)
        {
            merge_parent_motion(&mut flat, &flattened[pni]);
        }
        idx_map.insert(old_idx, flattened.len());
        flattened.push(flat);
    }
    doc.judgelines = JudgelineBlock { lines: flattened };
    doc
}

fn topological_order(lines: &[LineDef], name_to_idx: &BTreeMap<String, usize>) -> Vec<usize> {
    let n = lines.len();
    let mut visited = vec![false; n];
    let mut order = Vec::with_capacity(n);
    fn visit(
        i: usize,
        lines: &[LineDef],
        n2i: &BTreeMap<String, usize>,
        vis: &mut [bool],
        ord: &mut Vec<usize>,
    ) {
        if vis[i] {
            return;
        }
        vis[i] = true;
        if let Some(ref pn) = lines[i].parent
            && let Some(&pi) = n2i.get(pn)
            && pi < lines.len()
        {
            visit(pi, lines, n2i, vis, ord);
        }
        ord.push(i);
    }
    for i in 0..n {
        visit(i, lines, name_to_idx, &mut visited, &mut order);
    }
    order
}

fn merge_parent_motion(child: &mut LineDef, parent: &LineDef) {
    let pm = match &parent.motion {
        Some(m) => m,
        None => return,
    };
    let cm = child.motion.get_or_insert_with(MotionBlock::default);
    let mut merged = pm.layers.clone();
    merged.extend(cm.layers.clone());
    cm.layers = merged;
}

// ---- Tests ----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fcs_core::parser;

    #[test]
    fn test_proto_flatten_inheritance() {
        let src = r#"
meta { name: "T"; artists: ["A"]; charters: ["C"]; offset: 0ms; version: "4.0.0"; }
masterTimeline { 0.0b -> 120.0; }
judgelines { line L { bpmTimeline { 0.0b -> 120.0; }
    notes {
        tap ghost { speed: 2.0; alpha: 0.5; }
        tap n1 : ghost { time: 4.0b; positionX: 100px; alpha: 1.0; }
    }
} }
"#;
        let (_, doc) = parser::parse_document(src).expect("parse");
        let flat = flatten_note_block(&doc.judgelines.lines[0].notes);
        // ghost is a template (no time) → not in concrete; n1 has time → 1 concrete
        assert_eq!(flat.concrete.len(), 1);
        let n1 = &flat.concrete[0];
        assert!(
            n1.properties.iter().any(|(k, _)| k == "speed"),
            "should inherit speed"
        );
    }

    #[test]
    fn test_fake_anchors_filtered() {
        let src = r#"
meta { name: "T"; artists: ["A"]; charters: ["C"]; offset: 0ms; version: "4.0.0"; }
masterTimeline { 0.0b -> 120.0; }
judgelines { line L { bpmTimeline { 0.0b -> 120.0; }
    notes {
        fake anchor { positionX: 200px; speed: 1.5; }
        tap real : anchor { time: 4.0b; speed: 2.0; }
    }
} }
"#;
        let (_, doc) = parser::parse_document(src).expect("parse");
        let flat = flatten_note_block(&doc.judgelines.lines[0].notes);
        // anchor is a template (no time) → not in fake; real inherits anchor's props
        assert_eq!(flat.concrete.len(), 1);
        assert_eq!(flat.fake.len(), 0); // template-only, no playable fake instances
        assert!(
            flat.concrete[0]
                .properties
                .iter()
                .any(|(k, _)| k == "positionX"),
            "should inherit positionX"
        );
    }
}
