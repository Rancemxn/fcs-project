//! AST → FcbcFile emission — assembles all bytecode sections.
use crate::ast::NotePropertyValue;
use crate::bytecode::FcbcFile;
use crate::bytecode::property::PropertyDescriptor;
use crate::bytecode::sections::{
    LineHeader, LineSection, MasterTimelineSection, MetaSection, NoteEncoding,
};
use crate::compiler::context::CompileContext;
use crate::compiler::expr::compile_expression;
use crate::compiler::prototype::{flatten_prototype, get_property};
use crate::compiler::timeline::build_bpm_lut;
use crate::error::{CompileError, DiagnosticBag};

pub fn emit(ctx: &mut CompileContext) -> Result<FcbcFile, DiagnosticBag> {
    if ctx.has_errors() {
        return Err(ctx.diagnostics.clone());
    }
    if let Err(()) = validate_meta(ctx) {
        return Err(ctx.diagnostics.clone());
    }
    let master_lut = match build_bpm_lut(&ctx.doc.master_timeline) {
        Ok(l) => l,
        Err(e) => {
            ctx.error(e);
            return Err(ctx.diagnostics.clone());
        }
    };
    let master_timeline = MasterTimelineSection {
        entries: master_lut,
    };

    let mut lines = Vec::new();
    for line_def in &ctx.doc.judgelines.lines {
        lines.push(match emit_line(line_def, ctx) {
            Ok(l) => l,
            Err(()) => return Err(ctx.diagnostics.clone()),
        });
    }

    let name_off = ctx.strings.intern(&ctx.doc.meta.name);
    let artist_offs: Vec<u32> = ctx
        .doc
        .meta
        .artists
        .iter()
        .map(|a| ctx.strings.intern(a))
        .collect();
    let charter_offs: Vec<u32> = ctx
        .doc
        .meta
        .charters
        .iter()
        .map(|c| ctx.strings.intern(c))
        .collect();
    let offset_sec = match ctx.doc.meta.offset_unit.as_str() {
        "ms" => ctx.doc.meta.offset / 1000.0,
        _ => ctx.doc.meta.offset,
    };

    Ok(FcbcFile {
        header: crate::bytecode::header::FcbcHeader::new(28, 0, 0, 0, 0),
        string_table: ctx.strings.clone(),
        const_pool: ctx.consts.clone(),
        meta: MetaSection {
            name_st_off: name_off,
            artist_st_offs: artist_offs,
            charter_st_offs: charter_offs,
            offset_seconds: offset_sec,
        },
        master_timeline,
        lines,
        expressions: vec![],
        shaders: vec![],
    })
}

fn validate_meta(ctx: &mut CompileContext) -> Result<(), ()> {
    let m = &ctx.doc.meta;
    if m.name.is_empty() {
        ctx.error(CompileError::RequiredMetaFieldMissing("name".into()));
    }
    if m.artists.is_empty() {
        ctx.error(CompileError::RequiredMetaFieldMissing("artists".into()));
    }
    if m.charters.is_empty() {
        ctx.error(CompileError::RequiredMetaFieldMissing("charters".into()));
    }
    if m.version.is_empty() {
        ctx.error(CompileError::RequiredMetaFieldMissing("version".into()));
    }
    if ctx.has_errors() { Err(()) } else { Ok(()) }
}

fn emit_line(line_def: &crate::ast::LineDef, ctx: &mut CompileContext) -> Result<LineSection, ()> {
    let name_off = ctx.strings.intern(&line_def.name);
    let tex_off = line_def
        .texture
        .as_ref()
        .map(|t| ctx.strings.intern(t))
        .unwrap_or(0);
    let bpm_lut = build_bpm_lut(&line_def.bpm_timeline).map_err(|e| {
        ctx.error(e);
    })?;
    let mlc = line_def
        .motion
        .as_ref()
        .map(|m| m.layers.len() as u32)
        .unwrap_or(0);

    let mut notes = Vec::new();
    for inst in &line_def.notes.instances {
        let props = flatten_prototype(inst, &line_def.name, ctx).map_err(|e| {
            ctx.error(e);
        })?;
        notes.push(emit_note(inst, &props, ctx)?);
    }

    Ok(LineSection {
        header: LineHeader {
            name_st_off: name_off,
            texture_st_off: tex_off,
            texture_anchor_x: line_def.texture_anchor.0 as f32,
            texture_anchor_y: line_def.texture_anchor.1 as f32,
            z_order: line_def.z_order,
            color_rgba: [
                line_def.color.r,
                line_def.color.g,
                line_def.color.b,
                line_def.color.a,
            ],
            parent_line_index: -1,
            inherit_flags: if line_def.inherit.position { 1 } else { 0 }
                | if line_def.inherit.rotation { 2 } else { 0 }
                | if line_def.inherit.scale { 4 } else { 0 }
                | if line_def.inherit.alpha { 8 } else { 0 },
            bpm_lut_offset: 0,
            bpm_lut_entry_count: bpm_lut.len() as u32,
            motion_layer_count: mlc,
            note_count: notes.len() as u32,
        },
        bpm_lut,
        motion_keyframes: vec![],
        notes,
    })
}

fn emit_note(
    inst: &crate::ast::NoteInstance,
    props: &[(String, NotePropertyValue)],
    ctx: &mut CompileContext,
) -> Result<NoteEncoding, ()> {
    let mut n = NoteEncoding::default();
    n.kind = kind_u8(inst.kind);
    n.flags = (if get_bool(props, "above", true) { 1 } else { 0 })
        | (if get_bool(props, "fake", false) { 2 } else { 0 });
    n.time_beat = get_f64(props, "time", 0.0);
    n.end_time_beat = get_f64(props, "endTime", n.time_beat);
    if inst.kind == crate::ast::NoteKind::Hold && n.end_time_beat < n.time_beat {
        ctx.error(CompileError::HoldEndTimeBeforeTime {
            end_time: format!("{}b", n.end_time_beat),
            time: format!("{}b", n.time_beat),
        });
        return Err(());
    }
    n.position_x = emit_pd(props, "positionX", 0.0, ctx);
    n.speed = emit_pd(props, "speed", 1.0, ctx);
    n.scale_x = emit_pd(props, "scaleX", 1.0, ctx);
    n.scale_y = emit_pd(props, "scaleY", 1.0, ctx);
    n.x_offset = emit_pd(props, "xOffset", 0.0, ctx);
    n.y_offset = emit_pd(props, "yOffset", 0.0, ctx);
    n.alpha = emit_pd(props, "alpha", 1.0, ctx);
    Ok(n)
}

fn emit_pd(
    props: &[(String, NotePropertyValue)],
    key: &str,
    def: f32,
    ctx: &mut CompileContext,
) -> PropertyDescriptor {
    match get_property(props, key) {
        Some(NotePropertyValue::Expr(expr)) => {
            let _bc = compile_expression(expr, &mut ctx.consts);
            PropertyDescriptor::new_expr(0) // TODO: store bc + return real offset
        }
        Some(NotePropertyValue::Literal(lit)) => {
            PropertyDescriptor::new_const(lit_to_f32(lit, def as f64) as f32)
        }
        _ => PropertyDescriptor::new_const(def),
    }
}

fn get_f64(props: &[(String, NotePropertyValue)], key: &str, def: f64) -> f64 {
    match get_property(props, key) {
        Some(NotePropertyValue::Literal(l)) => lit_to_f32(l, def),
        _ => def,
    }
}
fn get_bool(props: &[(String, NotePropertyValue)], key: &str, def: bool) -> bool {
    match get_property(props, key) {
        Some(NotePropertyValue::Bool(b)) => *b,
        _ => def,
    }
}
fn lit_to_f32(lit: &crate::ast::Literal, def: f64) -> f64 {
    match lit {
        crate::ast::Literal::Integer(n) => *n as f64,
        crate::ast::Literal::Float(f) => *f,
        crate::ast::Literal::Quantified { value, .. } => *value,
        crate::ast::Literal::Boolean(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        _ => def,
    }
}
fn kind_u8(k: crate::ast::NoteKind) -> u8 {
    match k {
        crate::ast::NoteKind::Tap => 0,
        crate::ast::NoteKind::Drag => 1,
        crate::ast::NoteKind::Hold => 2,
        crate::ast::NoteKind::Flick => 3,
        crate::ast::NoteKind::Fake => 4,
    }
}
