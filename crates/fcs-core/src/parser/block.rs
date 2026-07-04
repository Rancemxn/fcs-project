//! Structural block parsers — meta, masterTimeline, templates, judgelines, shaders.

use crate::ast::{
    BpmEntry, BpmTimeline, Document, InheritFlags, JudgelineBlock, LineDef,
    MetaBlock, MetaValue, MotionBlock, NoteBlock, NoteInstance,
    NoteKind, NotePropertyValue, NotePrototype, ShaderBlock, TemplateBlock,
};
use crate::parser::expr::parse_expression;
use crate::parser::literal::{parse_color, parse_string, parse_bool, ws};
use crate::units::Color;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, alphanumeric1, char, digit1},
    combinator::{map, map_res, opt, recognize, value},
    multi::{many0, separated_list0},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ident(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
    ))(input)
}

fn semicolon(input: &str) -> IResult<&str, ()> {
    let (input, _) = preceded(ws, char(';'))(input)?;
    Ok((input, ()))
}

fn colon(input: &str) -> IResult<&str, ()> {
    let (input, _) = preceded(ws, char(':'))(input)?;
    Ok((input, ()))
}

fn parse_key_value(input: &str) -> IResult<&str, (&str, MetaValue)> {
    let (input, key) = preceded(ws, ident)(input)?;
    let (input, _) = preceded(ws, char(':'))(input)?;
    let (input, _) = ws(input)?; // skip whitespace after colon
    let (input, val) = alt((
        map(parse_string, MetaValue::String),
        map(map_res(recognize(tuple((opt(tag("-")), digit1))), |s: &str| s.parse::<i64>()), MetaValue::Int),
        map(parse_bool, MetaValue::Bool),
        map(
            delimited(
                preceded(ws, char('[')),
                separated_list0(preceded(ws, char(',')), preceded(ws, parse_string)),
                preceded(ws, char(']')),
            ),
            MetaValue::StringArray,
        ),
    ))(input)?;
    // Consume optional unit suffix after numeric values
    let (input, _) = opt(alt((tag("ms"), tag("s"), tag("b"), tag("px"), tag("vw"), tag("vh"), tag("deg"), tag("rad"))))(input)?;
    let (input, _) = semicolon(input)?;
    Ok((input, (key, val)))
}

// ---------------------------------------------------------------------------
// Meta block (§5.2)
// ---------------------------------------------------------------------------

fn parse_meta_block(input: &str) -> IResult<&str, MetaBlock> {
    let (input, _) = preceded(ws, tag("meta"))(input)?;
    let (input, _) = preceded(ws, char('{'))(input)?;

    let mut name = String::new();
    let mut artists = Vec::new();
    let mut charters = Vec::new();
    let mut offset = 0.0f64;
    let offset_unit = "ms".to_string();
    let mut version = String::new();
    let mut extra = BTreeMap::new();

    let (input, pairs): (&str, Vec<(&str, MetaValue)>) = many0(parse_key_value)(input)?;

    for (key, val) in pairs {
        match key {
            "name" => { if let MetaValue::String(s) = val { name = s; } }
            "artists" => match val {
                MetaValue::String(s) => artists = vec![s],
                MetaValue::StringArray(arr) => artists = arr,
                _ => {}
            },
            "charters" => match val {
                MetaValue::String(s) => charters = vec![s],
                MetaValue::StringArray(arr) => charters = arr,
                _ => {}
            },
            "offset" => { if let MetaValue::Int(n) = val { offset = n as f64; } }
            "version" => { if let MetaValue::String(s) = val { version = s; } }
            _ => { extra.insert(key.to_string(), val); }
        }
    }

    let (input, _) = preceded(ws, char('}'))(input)?;
    Ok((input, MetaBlock { name, artists, charters, offset, offset_unit, version, extra }))
}

// ---------------------------------------------------------------------------
// BPM Timeline (§5.3)
// ---------------------------------------------------------------------------

fn parse_bpm_entry(input: &str) -> IResult<&str, BpmEntry> {
    let (input, beat_str) = preceded(ws, recognize(digit1))(input)?;
    let (input, _) = preceded(ws, tag(".0b"))(input)?;
    let beat = beat_str.parse::<f64>().unwrap_or(0.0);
    let (input, _) = preceded(ws, tag("->"))(input)?;
    let (input, bpm_str) = preceded(ws, recognize(digit1))(input)?;
    let (input, _) = opt(preceded(ws, char('.')))(input)?;
    let (input, frac) = opt(preceded(ws, digit1))(input)?;
    let bpm = match frac {
        Some(f) => format!("{}.{}", bpm_str, f).parse::<f64>().unwrap_or(0.0),
        None => bpm_str.parse::<f64>().unwrap_or(0.0),
    };
    let (input, _) = semicolon(input)?;
    Ok((input, BpmEntry { beat, bpm, is_step_before: false }))
}

fn parse_bpm_timeline(input: &str) -> IResult<&str, BpmTimeline> {
    let (input, _) = preceded(ws, char('{'))(input)?;
    let (input, entries) = many0(parse_bpm_entry)(input)?;
    let (input, _) = preceded(ws, char('}'))(input)?;
    Ok((input, BpmTimeline { entries }))
}

fn parse_master_timeline(input: &str) -> IResult<&str, BpmTimeline> {
    preceded(ws, preceded(tag("masterTimeline"), parse_bpm_timeline))(input)
}

// ---------------------------------------------------------------------------
// Templates (§5.4) — deferred to compiler phase
// ---------------------------------------------------------------------------

fn take_until_closing_brace(input: &str) -> IResult<&str, &str> {
    let mut depth = 1u32;
    let mut pos = 0usize;
    for (i, c) in input.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => { depth -= 1; if depth == 0 { pos = i; break; } }
            _ => {}
        }
    }
    Ok((&input[pos..], &input[..pos]))
}

fn parse_template_block(input: &str) -> IResult<&str, TemplateBlock> {
    let (input, _) = preceded(ws, tag("templates"))(input)?;
    let (input, _) = preceded(ws, char('{'))(input)?;
    let (input, _) = take_until_closing_brace(input)?;
    let (input, _) = preceded(ws, char('}'))(input)?;
    Ok((input, TemplateBlock::default()))
}

// ---------------------------------------------------------------------------
// Judgelines (§5.5)
// ---------------------------------------------------------------------------

fn parse_note_kind(input: &str) -> IResult<&str, NoteKind> {
    alt((
        value(NoteKind::Tap, tag("tap")),
        value(NoteKind::Hold, tag("hold")),
        value(NoteKind::Flick, tag("flick")),
        value(NoteKind::Drag, tag("drag")),
        value(NoteKind::Fake, tag("fake")),
    ))(input)
}

fn parse_inherit_flags(input: &str) -> IResult<&str, InheritFlags> {
    let (input, _) = preceded(ws, char('['))(input)?;
    let (input, flags) = separated_list0(
        preceded(ws, char(',')),
        alt((
            value((true, false, false, false), tag("position")),
            value((false, true, false, false), tag("rotation")),
            value((false, false, true, false), tag("scale")),
            value((false, false, false, true), tag("alpha")),
        )),
    )(input)?;
    let (input, _) = preceded(ws, char(']'))(input)?;
    let mut result = InheritFlags::default();
    for (pos, rot, scl, a) in flags {
        if pos { result.position = true; }
        if rot { result.rotation = true; }
        if scl { result.scale = true; }
        if a { result.alpha = true; }
    }
    Ok((input, result))
}

fn parse_note_property_value(input: &str) -> IResult<&str, NotePropertyValue> {
    alt((
        map(parse_expression, NotePropertyValue::Expr),
        map(parse_color, NotePropertyValue::Color),
        map(parse_string, |s| NotePropertyValue::String(s)),
        map(parse_bool, NotePropertyValue::Bool),
    ))(input)
}

fn parse_note_property(input: &str) -> IResult<&str, (&str, NotePropertyValue)> {
    let (input, key) = preceded(ws, ident)(input)?;
    let (input, _) = preceded(ws, char(':'))(input)?;
    let (input, _) = ws(input)?;
    let (input, val) = parse_note_property_value(input)?;
    let (input, _) = semicolon(input)?;
    Ok((input, (key, val)))
}

fn parse_note_prototype(input: &str) -> IResult<&str, NotePrototype> {
    let (input, kind) = preceded(ws, parse_note_kind)(input)?;
    let (input, name) = preceded(ws, ident)(input)?;
    let (input, parent) = opt(preceded(preceded(ws, char(':')), preceded(ws, ident)))(input)?;
    let (input, _) = preceded(ws, char('{'))(input)?;
    let (input, properties) = many0(parse_note_property)(input)?;
    let (input, _) = preceded(ws, char('}'))(input)?;
    Ok((input, NotePrototype {
        kind, name: name.to_string(),
        parent: parent.map(|s| s.to_string()),
        properties: properties.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
    }))
}

fn parse_note_instance(input: &str) -> IResult<&str, NoteInstance> {
    let (input, kind) = preceded(ws, parse_note_kind)(input)?;
    let (input, name) = opt(preceded(ws, ident))(input)?;
    let (input, parent) = opt(preceded(preceded(ws, char(':')), preceded(ws, ident)))(input)?;
    let (input, _) = preceded(ws, char('{'))(input)?;
    let (input, properties) = many0(parse_note_property)(input)?;
    let (input, _) = preceded(ws, char('}'))(input)?;
    Ok((input, NoteInstance {
        kind, name: name.map(|s| s.to_string()),
        parent: parent.map(|s| s.to_string()),
        properties: properties.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
    }))
}

fn parse_notes_block(input: &str) -> IResult<&str, NoteBlock> {
    let (input, _) = preceded(ws, tag("notes"))(input)?;
    let (input, _) = preceded(ws, char('{'))(input)?;
    let mut prototypes = Vec::new();
    let mut instances = Vec::new();
    let (input, _) = many0(alt((
        map(parse_note_prototype, |p| { prototypes.push(p); }),
        map(parse_note_instance, |i| { instances.push(i); }),
    )))(input)?;
    let (input, _) = preceded(ws, char('}'))(input)?;
    Ok((input, NoteBlock { prototypes, instances }))
}

fn parse_line_def(input: &str) -> IResult<&str, LineDef> {
    let (input, _) = preceded(ws, tag("line"))(input)?;
    let (input, name) = preceded(ws, ident)(input)?;
    let (input, _) = preceded(ws, char('{'))(input)?;

    let mut texture = None;
    let texture_anchor = (0.5, 0.5);
    let mut z_order: i32 = 0;
    let mut color = Color::WHITE;
    let mut parent = None;
    let mut inherit = InheritFlags::default();
    let mut bpm_timeline = BpmTimeline { entries: vec![] };
    let mut motion = None;
    let mut notes = NoteBlock::default();

    let (input, _) = many0(alt((
        map(preceded(preceded(ws, tag("texture")), preceded(colon, terminated(preceded(ws, parse_string), semicolon))), |t| { texture = Some(t); }),
        map(preceded(preceded(ws, tag("zOrder")), preceded(colon, terminated(preceded(ws, map_res(recognize(digit1), |s: &str| s.parse::<i32>())), semicolon))), |z| { z_order = z; }),
        map(preceded(preceded(ws, tag("color")), preceded(colon, terminated(preceded(ws, parse_color), semicolon))), |c| { color = c; }),
        map(preceded(preceded(ws, tag("parent")), preceded(colon, terminated(preceded(ws, ident), semicolon))), |p: &str| { parent = Some(p.to_string()); }),
        map(preceded(preceded(ws, tag("inherit")), preceded(colon, terminated(parse_inherit_flags, semicolon))), |f| { inherit = f; }),
        map(preceded(preceded(ws, tag("bpmTimeline")), parse_bpm_timeline), |bt| { bpm_timeline = bt; }),
        map(preceded(preceded(ws, tag("motion")), preceded(preceded(ws, char('{')), take_until_closing_brace)), |_raw: &str| { motion = Some(MotionBlock::default()); }),
        map(parse_notes_block, |n| { notes = n; }),
    )))(input)?;

    let (input, _) = preceded(ws, char('}'))(input)?;
    Ok((input, LineDef { name: name.to_string(), texture, texture_anchor, z_order, color, parent, inherit, bpm_timeline, motion, notes }))
}

fn parse_judgelines(input: &str) -> IResult<&str, JudgelineBlock> {
    let (input, _) = preceded(ws, tag("judgelines"))(input)?;
    let (input, _) = preceded(ws, char('{'))(input)?;
    let (input, lines) = many0(parse_line_def)(input)?;
    let (input, _) = preceded(ws, char('}'))(input)?;
    Ok((input, JudgelineBlock { lines }))
}

// ---------------------------------------------------------------------------
// Shaders (§5.7) — deferred
// ---------------------------------------------------------------------------

fn parse_shaders_block(input: &str) -> IResult<&str, ShaderBlock> {
    let (input, _) = preceded(ws, tag("shaders"))(input)?;
    let (input, _) = preceded(ws, char('{'))(input)?;
    let (input, _) = take_until_closing_brace(input)?;
    let (input, _) = preceded(ws, char('}'))(input)?;
    Ok((input, ShaderBlock::default()))
}

// ---------------------------------------------------------------------------
// Top-level document parser
// ---------------------------------------------------------------------------

/// Parse a complete `.fcs` document.
pub fn parse_document(input: &str) -> IResult<&str, Document> {
    let (input, meta) = preceded(ws, parse_meta_block)(input)?;
    let (input, master_timeline) = preceded(ws, parse_master_timeline)(input)?;
    let (input, templates) = opt(preceded(ws, parse_template_block))(input)?;
    // Shaders may appear before or after judgelines (spec says after; accept both)
    let (input, shaders_before) = opt(preceded(ws, parse_shaders_block))(input)?;
    let (input, judgelines) = preceded(ws, parse_judgelines)(input)?;
    let (input, shaders_after) = opt(preceded(ws, parse_shaders_block))(input)?;
    let (input, _) = ws(input)?;
    let shaders = shaders_before.or(shaders_after);
    Ok((input, Document { meta, master_timeline, templates, judgelines, shaders }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_meta() {
        let src = r#"meta { name: "Test"; artists: ["A","B"]; charters: ["C"]; offset: -120ms; version: "2.1.0"; }"#;
        let (_, meta) = parse_meta_block(src).unwrap();
        assert_eq!(meta.name, "Test");
        assert_eq!(meta.artists.len(), 2);
    }

    #[test]
    fn test_parse_bpm_entry() {
        let (_, e) = parse_bpm_entry("0.0b -> 180.0;").unwrap();
        assert!((e.bpm - 180.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_master_timeline() {
        let src = "masterTimeline { 0.0b -> 180.0; 16.0b -> 90.0; }";
        let (_, tl) = parse_master_timeline(src).unwrap();
        assert_eq!(tl.entries.len(), 2);
    }

    #[test]
    fn test_parse_note_kinds() {
        assert_eq!(parse_note_kind("tap").unwrap().1, NoteKind::Tap);
        assert_eq!(parse_note_kind("hold").unwrap().1, NoteKind::Hold);
    }

    #[test]
    fn test_parse_note_instance() {
        let src = "tap { time: 4.0b; positionX: -150px; speed: 1.0; }";
        let result = parse_note_instance(src);
        assert!(result.is_ok(), "err: {:?}", result.err());
        let (_, note) = result.unwrap();
        assert_eq!(note.kind, NoteKind::Tap);
        assert_eq!(note.properties.len(), 3);
    }

    #[test]
    fn test_parse_minimal_document() {
        let src = r#"
meta { name: "T"; artists: ["X"]; charters: ["Y"]; offset: 0ms; version: "4.0.0"; }
masterTimeline { 0.0b -> 120.0; }
judgelines { line L { bpmTimeline { 0.0b -> 120.0; } } }
"#;
        let result = parse_document(src);
        assert!(result.is_ok(), "parse error: {:?}", result.err());
        let (_, doc) = result.unwrap();
        assert_eq!(doc.meta.name, "T");
        assert_eq!(doc.judgelines.lines.len(), 1);
    }
}
