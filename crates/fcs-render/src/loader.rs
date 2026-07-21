//! Independent test-only FCBC RenderSection loader and validator.
//!
//! The loader consumes static bytes. It invokes the already independent Core FCBC loader first,
//! then re-reads Render-owned sections with its own cursor and graph validation. It never imports
//! the Render writer or any expected semantic/raster artifact.

use std::collections::{BTreeMap, BTreeSet};

use fcs_fcbc::{DecodedChart, DescriptorKind, ExpressionNode, PropertyDescriptor, ValueType};

use crate::assets::{AssetError, DecodedImage, TestFont, decode_font, decode_image};

pub const NULL_INDEX: u32 = u32::MAX;
const MAX_TABLE_ITEMS: usize = 4096;

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedRenderChart {
    pub core: DecodedChart,
    pub viewport_width: f64,
    pub viewport_height: f64,
    pub viewport_color_space: u16,
    pub layers: Vec<LayerRecord>,
    pub nodes: Vec<NodeRecord>,
    pub geometries: Vec<GeometryRecord>,
    pub paths: Vec<PathRecord>,
    pub paints: Vec<PaintRecord>,
    pub strokes: Vec<StrokeRecord>,
    pub clips: Vec<ClipRecord>,
    pub glyph_runs: Vec<GlyphRunRecord>,
    pub resources: Vec<ResourceRecord>,
    pub decoded_images: BTreeMap<u64, DecodedImage>,
    pub decoded_fonts: BTreeMap<u64, TestFont>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayerRecord {
    pub id: u64,
    pub pass: u16,
    pub z_order: i32,
    pub document_order: u32,
    pub first_root: u32,
    pub root_count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u16)]
pub enum NodeKind {
    Group = 1,
    ClipGroup = 2,
    Rect = 3,
    RoundedRect = 4,
    Circle = 5,
    Ellipse = 6,
    Line = 7,
    Polyline = 8,
    Polygon = 9,
    Path = 10,
    Image = 11,
    Text = 12,
}

impl NodeKind {
    fn from_u16(value: u16) -> Result<Self, &'static str> {
        match value {
            1 => Ok(Self::Group),
            2 => Ok(Self::ClipGroup),
            3 => Ok(Self::Rect),
            4 => Ok(Self::RoundedRect),
            5 => Ok(Self::Circle),
            6 => Ok(Self::Ellipse),
            7 => Ok(Self::Line),
            8 => Ok(Self::Polyline),
            9 => Ok(Self::Polygon),
            10 => Ok(Self::Path),
            11 => Ok(Self::Image),
            12 => Ok(Self::Text),
            _ => Err("render.invalid-geometry"),
        }
    }

    pub fn is_drawable(self) -> bool {
        !matches!(self, Self::Group | Self::ClipGroup)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Attachment {
    pub kind: u16,
    pub id: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeRecord {
    pub id: u64,
    pub kind: NodeKind,
    pub flags: u16,
    pub parent: Option<u32>,
    pub layer_index: u32,
    pub document_order: u32,
    pub z_order: i32,
    pub attachment: Attachment,
    pub active_start: f64,
    pub active_end: f64,
    pub position_descriptor: u32,
    pub origin_descriptor: u32,
    pub rotation_descriptor: u32,
    pub scale_descriptor: u32,
    pub opacity_descriptor: u32,
    pub visibility_descriptor: u32,
    pub geometry_ref: Option<u32>,
    pub fill_paint: Option<u32>,
    pub stroke_ref: Option<u32>,
    pub clip_ref: Option<u32>,
    pub composite: u16,
}

impl NodeRecord {
    pub fn isolated(&self) -> bool {
        self.flags & (1 << 2) != 0
    }

    fn active_domain(&self) -> fcs_fcbc::Domain {
        fcs_fcbc::Domain {
            start: self.active_start,
            end: self.active_end,
            unbounded_before: self.flags & 1 != 0,
            unbounded_after: self.flags & 2 != 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeometryRecord {
    pub id: u64,
    pub kind: NodeKind,
    pub data: GeometryData,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GeometryData {
    Rect {
        origin: u32,
        size: u32,
    },
    RoundedRect {
        origin: u32,
        size: u32,
        radii: [u32; 4],
    },
    Circle {
        center: u32,
        radius: u32,
    },
    Ellipse {
        center: u32,
        radius_x: u32,
        radius_y: u32,
        rotation: u32,
    },
    Line {
        start: u32,
        end: u32,
    },
    Polyline {
        points: Vec<u32>,
    },
    Polygon {
        points: Vec<u32>,
    },
    Path {
        path_ref: u32,
    },
    Image {
        resource_id: u64,
        destination: [u32; 4],
        source: Option<[u32; 4]>,
        sampling: u16,
    },
    Text {
        glyph_runs: Vec<u32>,
        origin: u32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathRecord {
    pub id: u64,
    pub fill_rule: u16,
    pub commands: Vec<PathCommand>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PathCommand {
    MoveTo(u32),
    LineTo(u32),
    QuadraticTo(u32, u32),
    CubicTo(u32, u32, u32),
    Arc {
        center: u32,
        radius: u32,
        start_angle: u32,
        end_angle: u32,
        direction: u16,
    },
    EllipseArc {
        center: u32,
        radius_x: u32,
        radius_y: u32,
        rotation: u32,
        start_angle: u32,
        end_angle: u32,
        direction: u16,
    },
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GradientStop {
    pub offset: f64,
    pub color_descriptor: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaintRecord {
    pub id: u64,
    pub data: PaintData,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PaintData {
    Solid {
        color: u32,
    },
    LinearGradient {
        start: u32,
        end: u32,
        spread: u16,
        stops: Vec<GradientStop>,
    },
    RadialGradient {
        start_center: u32,
        start_radius: u32,
        end_center: u32,
        end_radius: u32,
        spread: u16,
        stops: Vec<GradientStop>,
    },
    ImagePattern {
        resource_id: u64,
        position: u32,
        origin: u32,
        rotation: u32,
        scale: u32,
        repeat: u16,
        sampling: u16,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrokeRecord {
    pub id: u64,
    pub paint_ref: u32,
    pub width_descriptor: u32,
    pub cap: u16,
    pub join: u16,
    pub miter_limit: f64,
    pub dash_offset_descriptor: u32,
    pub dash: Vec<f64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipRecord {
    pub id: u64,
    pub fill_rule: u16,
    pub geometry_ref: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlyphPlacement {
    pub glyph_id: u32,
    pub x_advance: f64,
    pub y_advance: f64,
    pub x_offset: f64,
    pub y_offset: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRunRecord {
    pub id: u64,
    pub font_resource_id: u64,
    pub face_index: u32,
    pub size_descriptor: u32,
    pub run_offset: [f64; 2],
    pub glyphs: Vec<GlyphPlacement>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResourceRecord {
    pub id: u64,
    pub kind: u16,
    pub media_type: String,
    pub data_offset: u64,
    pub data_length: u64,
    pub data: Vec<u8>,
    metadata: ParsedValue,
}

#[derive(Clone, Debug, PartialEq)]
enum ParsedValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(u32),
    Scalar(u8, f64),
    Beat(i64, i64),
    Color([f64; 4]),
    Vec2(u8, [f64; 2]),
    Resource(u64),
    Contributor(u64),
    Array(u8, Vec<ParsedValue>),
    Object(Vec<(u32, ParsedValue)>),
}

#[derive(Clone, Copy)]
struct RawSection {
    offset: usize,
    length: usize,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
    error: &'static str,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], error: &'static str) -> Self {
        Self {
            bytes,
            position: 0,
            error,
        }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], &'static str> {
        let end = self.position.checked_add(length).ok_or(self.error)?;
        let bytes = self.bytes.get(self.position..end).ok_or(self.error)?;
        self.position = end;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, &'static str> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().map_err(|_| self.error)?,
        ))
    }
    fn u32(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| self.error)?,
        ))
    }
    fn i32(&mut self) -> Result<i32, &'static str> {
        Ok(i32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| self.error)?,
        ))
    }
    fn u64(&mut self) -> Result<u64, &'static str> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| self.error)?,
        ))
    }
    fn i64(&mut self) -> Result<i64, &'static str> {
        Ok(i64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| self.error)?,
        ))
    }
    fn f64(&mut self) -> Result<f64, &'static str> {
        let value = f64::from_bits(self.u64()?);
        value.is_finite().then_some(value).ok_or(self.error)
    }
    fn semantic_f64(&mut self, semantic_error: &'static str) -> Result<f64, &'static str> {
        let value = f64::from_bits(self.u64()?);
        value.is_finite().then_some(value).ok_or(semantic_error)
    }
    fn zeroes(&mut self, length: usize) -> Result<(), &'static str> {
        self.take(length)?
            .iter()
            .all(|byte| *byte == 0)
            .then_some(())
            .ok_or(self.error)
    }
    fn finish(self) -> Result<(), &'static str> {
        (self.position == self.bytes.len())
            .then_some(())
            .ok_or(self.error)
    }
}

pub fn load_render(bytes: &[u8]) -> Result<DecodedRenderChart, &'static str> {
    let core = fcs_fcbc::load_chart(bytes)?;
    let sections = section_map(bytes)?;
    let resources = parse_resources(
        section_payload(bytes, &sections, 6)?,
        section_payload(bytes, &sections, 20)?,
        &core.strings,
    )?;
    let mut decoded =
        parse_render_section(section_payload(bytes, &sections, 14)?, core, resources)?;
    validate_render(&mut decoded)?;
    Ok(decoded)
}

fn section_map(bytes: &[u8]) -> Result<BTreeMap<u32, RawSection>, &'static str> {
    let count = usize::try_from(u32_at(bytes, 36)?).map_err(|_| "fcbc.limit-exceeded")?;
    let table_offset =
        usize::try_from(u64_at(bytes, 40)?).map_err(|_| "fcbc.invalid-section-table")?;
    let table_end = table_offset
        .checked_add(count.checked_mul(40).ok_or("fcbc.limit-exceeded")?)
        .ok_or("fcbc.limit-exceeded")?;
    if table_end > bytes.len() {
        return Err("fcbc.invalid-section-table");
    }
    let mut sections = BTreeMap::new();
    for index in 0..count {
        let entry = table_offset + index * 40;
        let kind = u32_at(bytes, entry)?;
        let offset = usize::try_from(u64_at(bytes, entry + 16)?)
            .map_err(|_| "fcbc.section-out-of-bounds")?;
        let length = usize::try_from(u64_at(bytes, entry + 24)?)
            .map_err(|_| "fcbc.section-out-of-bounds")?;
        let end = offset
            .checked_add(length)
            .ok_or("fcbc.section-out-of-bounds")?;
        if end > bytes.len()
            || sections
                .insert(kind, RawSection { offset, length })
                .is_some()
        {
            return Err("fcbc.invalid-section-table");
        }
    }
    Ok(sections)
}

fn section_payload<'a>(
    bytes: &'a [u8],
    sections: &BTreeMap<u32, RawSection>,
    kind: u32,
) -> Result<&'a [u8], &'static str> {
    let section = sections
        .get(&kind)
        .ok_or("fcbc.profile-requirement-missing")?;
    Ok(&bytes[section.offset..section.offset + section.length])
}

fn parse_resources(
    bytes: &[u8],
    data: &[u8],
    strings: &[String],
) -> Result<Vec<ResourceRecord>, &'static str> {
    let mut cursor = Cursor::new(bytes, "fcbc.invalid-record");
    let count = limited_count(cursor.u32()?)?;
    let mut resources = Vec::with_capacity(count);
    let mut previous = None;
    for _ in 0..count {
        let mut record = take_record(&mut cursor, "fcbc.invalid-record")?;
        let id = record.u64()?;
        if id == 0 || previous.is_some_and(|prior| prior >= id) {
            return Err("fcbc.duplicate-id");
        }
        previous = Some(id);
        let kind = record.u16()?;
        if !(1..=7).contains(&kind) || record.u16()? != 0 {
            return Err("fcbc.invalid-record");
        }
        let media_ref = record.u32()?;
        let media_type = strings
            .get(media_ref as usize)
            .ok_or("fcbc.dangling-reference")?
            .clone();
        if record.u16()? != 1 || record.u16()? != 0 {
            return Err("fcbc.invalid-record");
        }
        let data_offset = record.u64()?;
        let data_length = record.u64()?;
        let hash = parse_counted_bytes(&mut record)?;
        if hash.len() != 32 {
            return Err("fcbc.invalid-record");
        }
        let metadata = parse_value(&mut record, strings.len())?;
        record.finish()?;
        let start = usize::try_from(data_offset).map_err(|_| "fcbc.resource-out-of-bounds")?;
        let length = usize::try_from(data_length).map_err(|_| "fcbc.resource-out-of-bounds")?;
        let end = start
            .checked_add(length)
            .ok_or("fcbc.resource-out-of-bounds")?;
        let payload = data
            .get(start..end)
            .ok_or("fcbc.resource-out-of-bounds")?
            .to_vec();
        resources.push(ResourceRecord {
            id,
            kind,
            media_type,
            data_offset,
            data_length,
            data: payload,
            metadata,
        });
    }
    cursor.finish()?;
    Ok(resources)
}

fn parse_counted_bytes(cursor: &mut Cursor<'_>) -> Result<Vec<u8>, &'static str> {
    let length = usize::try_from(cursor.u32()?).map_err(|_| cursor.error)?;
    let bytes = cursor.take(length)?.to_vec();
    cursor.zeroes((4 - length % 4) % 4)?;
    Ok(bytes)
}

fn parse_value(cursor: &mut Cursor<'_>, string_count: usize) -> Result<ParsedValue, &'static str> {
    let tag = cursor.u8()?;
    if cursor.u8()? != 0 || cursor.u16()? != 0 {
        return Err(cursor.error);
    }
    let payload_length = usize::try_from(cursor.u32()?).map_err(|_| cursor.error)?;
    let payload_bytes = cursor.take(payload_length)?;
    let mut payload = Cursor::new(payload_bytes, cursor.error);
    let value = match tag {
        0 if payload_length == 0 => ParsedValue::Null,
        1 if payload_length == 8 => {
            let value = payload.u8()?;
            payload.zeroes(7)?;
            match value {
                0 => ParsedValue::Bool(false),
                1 => ParsedValue::Bool(true),
                _ => return Err(cursor.error),
            }
        }
        2 if payload_length == 8 => ParsedValue::Int(payload.i64()?),
        3 if payload_length == 8 => ParsedValue::Float(payload.f64()?),
        4 if payload_length == 8 => {
            let reference = payload.u32()?;
            payload.zeroes(4)?;
            if reference as usize >= string_count {
                return Err("fcbc.dangling-reference");
            }
            ParsedValue::String(reference)
        }
        5 | 7 | 8 if payload_length == 8 => ParsedValue::Scalar(tag, payload.f64()?),
        6 if payload_length == 16 => {
            let numerator = payload.i64()?;
            let denominator = payload.i64()?;
            if denominator <= 0 {
                return Err(cursor.error);
            }
            ParsedValue::Beat(numerator, denominator)
        }
        9 if payload_length == 32 => {
            let color = [
                payload.f64()?,
                payload.f64()?,
                payload.f64()?,
                payload.f64()?,
            ];
            if color
                .iter()
                .any(|component| !(0.0..=1.0).contains(component))
            {
                return Err(cursor.error);
            }
            ParsedValue::Color(color)
        }
        10 if matches!(payload_length, 24 | 40) => {
            let element_tag = payload.u8()?;
            payload.zeroes(7)?;
            if !matches!(element_tag, 2 | 3 | 5 | 6 | 7 | 8) {
                return Err(cursor.error);
            }
            let first = parse_bare_scalar(&mut payload, element_tag)?;
            let second = parse_bare_scalar(&mut payload, element_tag)?;
            ParsedValue::Vec2(element_tag, [first, second])
        }
        11 if payload_length == 8 => ParsedValue::Resource(payload.u64()?),
        12 if payload_length == 8 => ParsedValue::Contributor(payload.u64()?),
        13 => {
            let element_tag = payload.u8()?;
            payload.zeroes(3)?;
            if element_tag == 0 {
                return Err(cursor.error);
            }
            let count = limited_count(payload.u32()?)?;
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                let value = parse_value(&mut payload, string_count)?;
                if value_tag(&value) != element_tag {
                    return Err(cursor.error);
                }
                values.push(value);
            }
            ParsedValue::Array(element_tag, values)
        }
        14 => {
            let count = limited_count(payload.u32()?)?;
            let mut fields = Vec::with_capacity(count);
            let mut keys = BTreeSet::new();
            for _ in 0..count {
                let key = payload.u32()?;
                if key as usize >= string_count {
                    return Err("fcbc.dangling-reference");
                }
                if !keys.insert(key) {
                    return Err(cursor.error);
                }
                fields.push((key, parse_value(&mut payload, string_count)?));
            }
            ParsedValue::Object(fields)
        }
        _ => return Err(cursor.error),
    };
    payload.finish()?;
    cursor.zeroes((8 - (8 + payload_length) % 8) % 8)?;
    Ok(value)
}

fn parse_bare_scalar(cursor: &mut Cursor<'_>, tag: u8) -> Result<f64, &'static str> {
    match tag {
        2 => Ok(cursor.i64()? as f64),
        3 | 5 | 7 | 8 => cursor.f64(),
        6 => {
            let numerator = cursor.i64()?;
            let denominator = cursor.i64()?;
            if denominator <= 0 {
                return Err(cursor.error);
            }
            Ok(numerator as f64 / denominator as f64)
        }
        _ => Err(cursor.error),
    }
}

fn value_tag(value: &ParsedValue) -> u8 {
    match value {
        ParsedValue::Null => 0,
        ParsedValue::Bool(_) => 1,
        ParsedValue::Int(_) => 2,
        ParsedValue::Float(_) => 3,
        ParsedValue::String(_) => 4,
        ParsedValue::Scalar(tag, _) => *tag,
        ParsedValue::Beat(_, _) => 6,
        ParsedValue::Color(_) => 9,
        ParsedValue::Vec2(_, _) => 10,
        ParsedValue::Resource(_) => 11,
        ParsedValue::Contributor(_) => 12,
        ParsedValue::Array(_, _) => 13,
        ParsedValue::Object(_) => 14,
    }
}

fn take_record<'a>(
    outer: &mut Cursor<'a>,
    error: &'static str,
) -> Result<Cursor<'a>, &'static str> {
    let length = usize::try_from(outer.u32()?).map_err(|_| error)?;
    if length < 8 || !length.is_multiple_of(4) {
        return Err(error);
    }
    let version = outer.u16()?;
    let flags = outer.u16()?;
    if version != 1 || flags != 0 {
        return Err(error);
    }
    Ok(Cursor::new(outer.take(length - 8)?, error))
}

fn parse_render_section(
    bytes: &[u8],
    core: DecodedChart,
    resources: Vec<ResourceRecord>,
) -> Result<DecodedRenderChart, &'static str> {
    if bytes.len() < 8 {
        return Err("render.invalid-section");
    }
    let declared = usize::try_from(u32_at(bytes, 0).map_err(|_| "render.invalid-section")?)
        .map_err(|_| "render.invalid-section")?;
    if declared != bytes.len() || !declared.is_multiple_of(4) {
        return Err("render.invalid-section");
    }
    let mut outer = Cursor::new(bytes, "render.invalid-record");
    if outer.u32()? as usize != bytes.len() || outer.u16()? != 1 || outer.u16()? != 0 {
        return Err("render.invalid-record");
    }
    let mut cursor = Cursor::new(outer.take(bytes.len() - 8)?, "render.invalid-record");
    let profile = (cursor.u16()?, cursor.u16()?, cursor.u16()?);
    let flags = cursor.u16()?;
    if profile != (1, 0, 0) || flags != 0 {
        return Err("render.unsupported-profile");
    }
    let viewport_width = cursor.f64()?;
    let viewport_height = cursor.f64()?;
    let viewport_color_space = cursor.u16()?;
    if cursor.u16()? != 0
        || viewport_width <= 0.0
        || viewport_height <= 0.0
        || !matches!(viewport_color_space, 1 | 2)
    {
        return Err("render.invalid-section");
    }
    let layer_count = limited_count(cursor.u32()?)?;
    let node_count = limited_count(cursor.u32()?)?;
    let geometry_count = limited_count(cursor.u32()?)?;
    let path_count = limited_count(cursor.u32()?)?;
    let paint_count = limited_count(cursor.u32()?)?;
    let stroke_count = limited_count(cursor.u32()?)?;
    let clip_count = limited_count(cursor.u32()?)?;
    let glyph_count = limited_count(cursor.u32()?)?;

    let mut layers = Vec::with_capacity(layer_count);
    for _ in 0..layer_count {
        layers.push(parse_layer(&mut cursor)?);
    }
    let mut nodes = Vec::with_capacity(node_count);
    for _ in 0..node_count {
        nodes.push(parse_node(&mut cursor, core.strings.len())?);
    }
    let mut geometries = Vec::with_capacity(geometry_count);
    for _ in 0..geometry_count {
        geometries.push(parse_geometry(&mut cursor, &core.strings)?);
    }
    let mut paths = Vec::with_capacity(path_count);
    for _ in 0..path_count {
        paths.push(parse_path(&mut cursor)?);
    }
    let mut paints = Vec::with_capacity(paint_count);
    for _ in 0..paint_count {
        paints.push(parse_paint(&mut cursor)?);
    }
    let mut strokes = Vec::with_capacity(stroke_count);
    for _ in 0..stroke_count {
        strokes.push(parse_stroke(&mut cursor)?);
    }
    let mut clips = Vec::with_capacity(clip_count);
    for _ in 0..clip_count {
        clips.push(parse_clip(&mut cursor)?);
    }
    let mut glyph_runs = Vec::with_capacity(glyph_count);
    for _ in 0..glyph_count {
        glyph_runs.push(parse_glyph_run(&mut cursor)?);
    }
    cursor.finish()?;
    outer.finish()?;

    Ok(DecodedRenderChart {
        core,
        viewport_width,
        viewport_height,
        viewport_color_space,
        layers,
        nodes,
        geometries,
        paths,
        paints,
        strokes,
        clips,
        glyph_runs,
        resources,
        decoded_images: BTreeMap::new(),
        decoded_fonts: BTreeMap::new(),
    })
}

fn parse_layer(cursor: &mut Cursor<'_>) -> Result<LayerRecord, &'static str> {
    let mut record = take_record(cursor, "render.invalid-record")?;
    if record.bytes.len() + 8 != 36 {
        return Err("render.invalid-record");
    }
    let layer = LayerRecord {
        id: record.u64()?,
        pass: record.u16()?,
        z_order: {
            if record.u16()? != 0 {
                return Err("render.invalid-record");
            }
            record.i32()?
        },
        document_order: record.u32()?,
        first_root: record.u32()?,
        root_count: record.u32()?,
    };
    record.finish()?;
    Ok(layer)
}

fn parse_node(cursor: &mut Cursor<'_>, string_count: usize) -> Result<NodeRecord, &'static str> {
    let mut record = take_record(cursor, "render.invalid-record")?;
    let id = record.u64()?;
    let kind = NodeKind::from_u16(record.u16()?)?;
    let flags = record.u16()?;
    let parent = optional_index(record.u32()?);
    let layer_index = record.u32()?;
    let document_order = record.u32()?;
    let z_order = record.i32()?;
    let attachment_kind = record.u16()?;
    if record.u16()? != 0 {
        return Err("render.invalid-record");
    }
    let attachment_id = record.u64()?;
    let active_start = record.f64()?;
    let active_end = record.f64()?;
    let node = NodeRecord {
        id,
        kind,
        flags,
        parent,
        layer_index,
        document_order,
        z_order,
        attachment: Attachment {
            kind: attachment_kind,
            id: attachment_id,
        },
        active_start,
        active_end,
        position_descriptor: record.u32()?,
        origin_descriptor: record.u32()?,
        rotation_descriptor: record.u32()?,
        scale_descriptor: record.u32()?,
        opacity_descriptor: record.u32()?,
        visibility_descriptor: record.u32()?,
        geometry_ref: optional_index(record.u32()?),
        fill_paint: optional_index(record.u32()?),
        stroke_ref: optional_index(record.u32()?),
        clip_ref: optional_index(record.u32()?),
        composite: record.u16()?,
    };
    if record.u16()? != 0
        || !matches!(
            parse_value(&mut record, string_count)?,
            ParsedValue::Object(_)
        )
    {
        return Err("render.invalid-record");
    }
    record.finish()?;
    Ok(node)
}

fn parse_geometry(
    cursor: &mut Cursor<'_>,
    strings: &[String],
) -> Result<GeometryRecord, &'static str> {
    let mut record = take_record(cursor, "render.invalid-record")?;
    let id = record.u64()?;
    let kind = NodeKind::from_u16(record.u16()?)?;
    if !kind.is_drawable() || record.u16()? != 0 {
        return Err("render.invalid-geometry");
    }
    let fields = named_object(parse_value(&mut record, strings.len())?, strings)?;
    record.finish()?;
    let data = geometry_data(kind, fields)?;
    Ok(GeometryRecord { id, kind, data })
}

fn geometry_data(
    kind: NodeKind,
    fields: Vec<(String, ParsedValue)>,
) -> Result<GeometryData, &'static str> {
    let names: Vec<_> = fields.iter().map(|(name, _)| name.as_str()).collect();
    let value = |index: usize| &fields[index].1;
    match kind {
        NodeKind::Rect if names == ["originDescriptor", "sizeDescriptor"] => {
            Ok(GeometryData::Rect {
                origin: expect_u32(value(0))?,
                size: expect_u32(value(1))?,
            })
        }
        NodeKind::RoundedRect
            if names == ["originDescriptor", "sizeDescriptor", "radiiDescriptors"] =>
        {
            Ok(GeometryData::RoundedRect {
                origin: expect_u32(value(0))?,
                size: expect_u32(value(1))?,
                radii: expect_array4(value(2))?,
            })
        }
        NodeKind::Circle if names == ["centerDescriptor", "radiusDescriptor"] => {
            Ok(GeometryData::Circle {
                center: expect_u32(value(0))?,
                radius: expect_u32(value(1))?,
            })
        }
        NodeKind::Ellipse
            if names
                == [
                    "centerDescriptor",
                    "radiusXDescriptor",
                    "radiusYDescriptor",
                    "rotationDescriptor",
                ] =>
        {
            Ok(GeometryData::Ellipse {
                center: expect_u32(value(0))?,
                radius_x: expect_u32(value(1))?,
                radius_y: expect_u32(value(2))?,
                rotation: expect_u32(value(3))?,
            })
        }
        NodeKind::Line if names == ["startDescriptor", "endDescriptor"] => Ok(GeometryData::Line {
            start: expect_u32(value(0))?,
            end: expect_u32(value(1))?,
        }),
        NodeKind::Polyline if names == ["pointDescriptors"] => {
            let points = expect_u32_array(value(0))?;
            (points.len() >= 2)
                .then_some(GeometryData::Polyline { points })
                .ok_or("render.invalid-geometry")
        }
        NodeKind::Polygon if names == ["pointDescriptors"] => {
            let points = expect_u32_array(value(0))?;
            (points.len() >= 3)
                .then_some(GeometryData::Polygon { points })
                .ok_or("render.invalid-geometry")
        }
        NodeKind::Path if names == ["pathRef"] => Ok(GeometryData::Path {
            path_ref: expect_u32(value(0))?,
        }),
        NodeKind::Image if names == ["resourceId", "destinationDescriptors", "sampling"] => {
            Ok(GeometryData::Image {
                resource_id: expect_resource(value(0))?,
                destination: expect_array4(value(1))?,
                source: None,
                sampling: expect_enum(value(2), 1..=2)?,
            })
        }
        NodeKind::Image
            if names
                == [
                    "resourceId",
                    "destinationDescriptors",
                    "sourceDescriptors",
                    "sampling",
                ] =>
        {
            Ok(GeometryData::Image {
                resource_id: expect_resource(value(0))?,
                destination: expect_array4(value(1))?,
                source: Some(expect_array4(value(2))?),
                sampling: expect_enum(value(3), 1..=2)?,
            })
        }
        NodeKind::Text if names == ["glyphRunRefs", "originDescriptor"] => {
            let glyph_runs = expect_u32_array(value(0))?;
            if glyph_runs.is_empty() {
                return Err("render.invalid-geometry");
            }
            Ok(GeometryData::Text {
                glyph_runs,
                origin: expect_u32(value(1))?,
            })
        }
        _ => Err("render.invalid-geometry"),
    }
}

fn named_object(
    value: ParsedValue,
    strings: &[String],
) -> Result<Vec<(String, ParsedValue)>, &'static str> {
    let ParsedValue::Object(fields) = value else {
        return Err("render.invalid-geometry");
    };
    fields
        .into_iter()
        .map(|(key, value)| {
            Ok((
                strings
                    .get(key as usize)
                    .ok_or("fcbc.dangling-reference")?
                    .clone(),
                value,
            ))
        })
        .collect()
}

fn expect_u32(value: &ParsedValue) -> Result<u32, &'static str> {
    let ParsedValue::Int(value) = value else {
        return Err("render.invalid-geometry");
    };
    u32::try_from(*value).map_err(|_| "render.invalid-geometry")
}

fn expect_resource(value: &ParsedValue) -> Result<u64, &'static str> {
    let ParsedValue::Resource(value) = value else {
        return Err("render.invalid-geometry");
    };
    (*value != 0)
        .then_some(*value)
        .ok_or("render.invalid-geometry")
}

fn expect_u32_array(value: &ParsedValue) -> Result<Vec<u32>, &'static str> {
    let ParsedValue::Array(2, values) = value else {
        return Err("render.invalid-geometry");
    };
    values.iter().map(expect_u32).collect()
}

fn expect_array4(value: &ParsedValue) -> Result<[u32; 4], &'static str> {
    expect_u32_array(value)?
        .try_into()
        .map_err(|_| "render.invalid-geometry")
}

fn expect_enum(
    value: &ParsedValue,
    range: std::ops::RangeInclusive<u16>,
) -> Result<u16, &'static str> {
    let value = u16::try_from(expect_u32(value)?).map_err(|_| "render.invalid-geometry")?;
    range
        .contains(&value)
        .then_some(value)
        .ok_or("render.invalid-geometry")
}

fn parse_path(cursor: &mut Cursor<'_>) -> Result<PathRecord, &'static str> {
    let mut record = take_record(cursor, "render.invalid-record")?;
    let id = record.u64()?;
    if record.u16()? != 0 {
        return Err("render.invalid-record");
    }
    let fill_rule = record.u16()?;
    if !matches!(fill_rule, 1 | 2) {
        return Err("render.invalid-geometry");
    }
    let count = limited_count(record.u32()?)?;
    let mut commands = Vec::with_capacity(count);
    let mut open = false;
    let mut closed = false;
    for _ in 0..count {
        let command = parse_path_command(&mut record)?;
        match command {
            PathCommand::MoveTo(_) => {
                open = true;
                closed = false;
            }
            PathCommand::Close if !open || closed => return Err("render.invalid-geometry"),
            PathCommand::Close => closed = true,
            _ if !open => return Err("render.invalid-geometry"),
            _ => closed = false,
        }
        commands.push(command);
    }
    record.finish()?;
    Ok(PathRecord {
        id,
        fill_rule,
        commands,
    })
}

fn parse_path_command(cursor: &mut Cursor<'_>) -> Result<PathCommand, &'static str> {
    let mut record = take_record(cursor, "render.invalid-record")?;
    let kind = record.u16()?;
    if record.u16()? != 0 {
        return Err("render.invalid-record");
    }
    let command = match kind {
        1 => PathCommand::MoveTo(record.u32()?),
        2 => PathCommand::LineTo(record.u32()?),
        3 => PathCommand::QuadraticTo(record.u32()?, record.u32()?),
        4 => PathCommand::CubicTo(record.u32()?, record.u32()?, record.u32()?),
        5 => {
            let command = PathCommand::Arc {
                center: record.u32()?,
                radius: record.u32()?,
                start_angle: record.u32()?,
                end_angle: record.u32()?,
                direction: record.u16()?,
            };
            if record.u16()? != 0 {
                return Err("render.invalid-record");
            }
            command
        }
        6 => {
            let command = PathCommand::EllipseArc {
                center: record.u32()?,
                radius_x: record.u32()?,
                radius_y: record.u32()?,
                rotation: record.u32()?,
                start_angle: record.u32()?,
                end_angle: record.u32()?,
                direction: record.u16()?,
            };
            if record.u16()? != 0 {
                return Err("render.invalid-record");
            }
            command
        }
        7 => PathCommand::Close,
        _ => return Err("render.invalid-geometry"),
    };
    if matches!(command, PathCommand::Arc { direction, .. } | PathCommand::EllipseArc { direction, .. } if !matches!(direction, 1 | 2))
    {
        return Err("render.invalid-geometry");
    }
    record.finish()?;
    Ok(command)
}

fn parse_paint(cursor: &mut Cursor<'_>) -> Result<PaintRecord, &'static str> {
    let mut record = take_record(cursor, "render.invalid-record")?;
    let id = record.u64()?;
    let kind = record.u16()?;
    if record.u16()? != 0 {
        return Err("render.invalid-record");
    }
    let data = match kind {
        1 => PaintData::Solid {
            color: record.u32()?,
        },
        2 => PaintData::LinearGradient {
            start: record.u32()?,
            end: record.u32()?,
            spread: parse_spread(&mut record)?,
            stops: parse_stops(&mut record)?,
        },
        3 => PaintData::RadialGradient {
            start_center: record.u32()?,
            start_radius: record.u32()?,
            end_center: record.u32()?,
            end_radius: record.u32()?,
            spread: parse_spread(&mut record)?,
            stops: parse_stops(&mut record)?,
        },
        4 => {
            let resource_id = record.u64()?;
            let position = record.u32()?;
            let origin = record.u32()?;
            let rotation = record.u32()?;
            let scale = record.u32()?;
            let repeat = record.u16()?;
            let sampling = record.u16()?;
            if !(1..=4).contains(&repeat) || !(1..=2).contains(&sampling) {
                return Err("render.invalid-paint");
            }
            PaintData::ImagePattern {
                resource_id,
                position,
                origin,
                rotation,
                scale,
                repeat,
                sampling,
            }
        }
        _ => return Err("render.invalid-paint"),
    };
    record.finish()?;
    Ok(PaintRecord { id, data })
}

fn parse_spread(record: &mut Cursor<'_>) -> Result<u16, &'static str> {
    let spread = record.u16()?;
    if record.u16()? != 0 {
        return Err("render.invalid-record");
    }
    if !(1..=3).contains(&spread) {
        return Err("render.invalid-paint");
    }
    Ok(spread)
}

fn parse_stops(record: &mut Cursor<'_>) -> Result<Vec<GradientStop>, &'static str> {
    let count = limited_count(record.u32()?)?;
    if count < 2 {
        return Err("render.invalid-paint");
    }
    let mut stops = Vec::with_capacity(count);
    let mut prior = None;
    for _ in 0..count {
        let offset = record.semantic_f64("render.invalid-paint")?;
        let color_descriptor = record.u32()?;
        if record.u32()? != 0 {
            return Err("render.invalid-record");
        }
        if !(0.0..=1.0).contains(&offset) || prior.is_some_and(|value| value > offset) {
            return Err("render.invalid-paint");
        }
        prior = Some(offset);
        stops.push(GradientStop {
            offset,
            color_descriptor,
        });
    }
    Ok(stops)
}

fn parse_stroke(cursor: &mut Cursor<'_>) -> Result<StrokeRecord, &'static str> {
    let mut record = take_record(cursor, "render.invalid-record")?;
    let id = record.u64()?;
    if record.u16()? != 0 || record.u16()? != 0 {
        return Err("render.invalid-record");
    }
    let paint_ref = record.u32()?;
    let width_descriptor = record.u32()?;
    let cap = record.u16()?;
    let join = record.u16()?;
    let miter_limit = record.semantic_f64("render.invalid-stroke")?;
    let dash_offset_descriptor = record.u32()?;
    let count = limited_count(record.u32()?)?;
    if !(1..=3).contains(&cap)
        || !(1..=3).contains(&join)
        || miter_limit < 1.0
        || (!count.is_multiple_of(2) && count != 0)
    {
        return Err("render.invalid-stroke");
    }
    let mut dash = Vec::with_capacity(count);
    for _ in 0..count {
        let value = record.semantic_f64("render.invalid-stroke")?;
        if value < 0.0 {
            return Err("render.invalid-stroke");
        }
        dash.push(value);
    }
    if !dash.is_empty() && dash.iter().sum::<f64>() <= 0.0 {
        return Err("render.invalid-stroke");
    }
    record.finish()?;
    Ok(StrokeRecord {
        id,
        paint_ref,
        width_descriptor,
        cap,
        join,
        miter_limit,
        dash_offset_descriptor,
        dash,
    })
}

fn parse_clip(cursor: &mut Cursor<'_>) -> Result<ClipRecord, &'static str> {
    let mut record = take_record(cursor, "render.invalid-record")?;
    if record.bytes.len() + 8 != 24 {
        return Err("render.invalid-record");
    }
    let id = record.u64()?;
    if record.u16()? != 0 {
        return Err("render.invalid-record");
    }
    let fill_rule = record.u16()?;
    let geometry_ref = record.u32()?;
    if !matches!(fill_rule, 1 | 2) {
        return Err("render.invalid-clip");
    }
    record.finish()?;
    Ok(ClipRecord {
        id,
        fill_rule,
        geometry_ref,
    })
}

fn parse_glyph_run(cursor: &mut Cursor<'_>) -> Result<GlyphRunRecord, &'static str> {
    let mut record = take_record(cursor, "render.invalid-record")?;
    let id = record.u64()?;
    let font_resource_id = record.u64()?;
    let face_index = record.u32()?;
    if record.u16()? != 0 || record.u16()? != 1 {
        return Err("render.invalid-record");
    }
    let size_descriptor = record.u32()?;
    let run_offset = [record.f64()?, record.f64()?];
    let count = limited_count(record.u32()?)?;
    if record.u32()? != 0 {
        return Err("render.invalid-record");
    }
    let mut glyphs = Vec::with_capacity(count);
    for _ in 0..count {
        let glyph_id = record.u32()?;
        if record.u32()? != 0 {
            return Err("render.invalid-record");
        }
        glyphs.push(GlyphPlacement {
            glyph_id,
            x_advance: record.f64()?,
            y_advance: record.f64()?,
            x_offset: record.f64()?,
            y_offset: record.f64()?,
        });
    }
    record.finish()?;
    Ok(GlyphRunRecord {
        id,
        font_resource_id,
        face_index,
        size_descriptor,
        run_offset,
        glyphs,
    })
}

fn optional_index(value: u32) -> Option<u32> {
    (value != NULL_INDEX).then_some(value)
}

fn validate_render(chart: &mut DecodedRenderChart) -> Result<(), &'static str> {
    validate_ids_and_table_order(chart)?;
    validate_node_graph(chart)?;
    let owners = validate_ownership(chart)?;
    validate_descriptor_roots(chart, &owners)?;
    validate_and_decode_resources(chart)?;
    Ok(())
}

fn validate_ids_and_table_order(chart: &DecodedRenderChart) -> Result<(), &'static str> {
    let mut ids = BTreeSet::new();
    for id in chart
        .layers
        .iter()
        .map(|record| record.id)
        .chain(chart.nodes.iter().map(|record| record.id))
        .chain(chart.geometries.iter().map(|record| record.id))
        .chain(chart.paths.iter().map(|record| record.id))
        .chain(chart.paints.iter().map(|record| record.id))
        .chain(chart.strokes.iter().map(|record| record.id))
        .chain(chart.clips.iter().map(|record| record.id))
        .chain(chart.glyph_runs.iter().map(|record| record.id))
    {
        if id == 0 || !ids.insert(id) {
            return Err("render.invalid-graph");
        }
    }
    if chart.layers.windows(2).any(|pair| {
        (
            pair[0].pass,
            pair[0].z_order,
            pair[0].document_order,
            pair[0].id,
        ) >= (
            pair[1].pass,
            pair[1].z_order,
            pair[1].document_order,
            pair[1].id,
        )
    }) || chart
        .layers
        .iter()
        .any(|layer| !(1..=6).contains(&layer.pass))
    {
        return Err("render.invalid-graph");
    }
    for ids in [
        chart
            .geometries
            .iter()
            .map(|record| record.id)
            .collect::<Vec<_>>(),
        chart.paths.iter().map(|record| record.id).collect(),
        chart.paints.iter().map(|record| record.id).collect(),
        chart.strokes.iter().map(|record| record.id).collect(),
        chart.clips.iter().map(|record| record.id).collect(),
        chart.glyph_runs.iter().map(|record| record.id).collect(),
    ] {
        if ids.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("render.invalid-graph");
        }
    }
    Ok(())
}

fn validate_node_graph(chart: &DecodedRenderChart) -> Result<(), &'static str> {
    let root_total = chart
        .layers
        .iter()
        .try_fold(0usize, |total, layer| {
            total.checked_add(layer.root_count as usize)
        })
        .ok_or("render.limit-exceeded")?;
    if root_total > chart.nodes.len() {
        return Err("render.invalid-graph");
    }
    let mut expected_first = 0usize;
    for (layer_index, layer) in chart.layers.iter().enumerate() {
        if layer.root_count == 0 {
            if layer.first_root != NULL_INDEX {
                return Err("render.invalid-graph");
            }
            continue;
        }
        if layer.first_root as usize != expected_first {
            return Err("render.invalid-graph");
        }
        let end = expected_first
            .checked_add(layer.root_count as usize)
            .ok_or("render.limit-exceeded")?;
        if end > root_total {
            return Err("render.invalid-graph");
        }
        let roots = &chart.nodes[expected_first..end];
        if roots
            .iter()
            .any(|node| node.parent.is_some() || node.layer_index as usize != layer_index)
            || roots
                .windows(2)
                .any(|pair| sibling_key(&pair[0]) >= sibling_key(&pair[1]))
        {
            return Err("render.invalid-graph");
        }
        expected_first = end;
    }
    if expected_first != root_total
        || chart.nodes[..root_total]
            .iter()
            .any(|node| node.parent.is_some())
    {
        return Err("render.invalid-graph");
    }
    for (index, node) in chart.nodes.iter().enumerate() {
        if node.id == 0
            || node.layer_index as usize >= chart.layers.len()
            || node.flags & !0b1111 != 0
        {
            return Err("render.invalid-graph");
        }
        if !(1..=5).contains(&node.composite) {
            return Err("render.invalid-composite");
        }
        if node.flags & 1 != 0 && node.active_start.to_bits() != 0 {
            return Err("render.invalid-graph");
        }
        if node.flags & 2 != 0 && node.active_end.to_bits() != 0 {
            return Err("render.invalid-graph");
        }
        if node.flags & 1 == 0 && node.flags & 2 == 0 && node.active_start > node.active_end {
            return Err("render.invalid-graph");
        }
        validate_attachment(node, &chart.core)?;
        match node.parent {
            None if index >= root_total => return Err("render.invalid-graph"),
            Some(parent) => {
                let parent_index = parent as usize;
                if parent_index >= index {
                    return Err("render.invalid-graph");
                }
                let parent_node = &chart.nodes[parent_index];
                if parent_node.layer_index != node.layer_index
                    || parent_node.attachment != node.attachment
                {
                    return Err("render.invalid-graph");
                }
            }
            None => {}
        }
        if node.isolated() && !matches!(node.kind, NodeKind::Group | NodeKind::ClipGroup) {
            return Err("render.invalid-composite");
        }
        if matches!(node.kind, NodeKind::Group | NodeKind::ClipGroup)
            && !node.isolated()
            && node.composite != 1
        {
            return Err("render.invalid-composite");
        }
    }

    let mut collections: BTreeMap<(u32, Option<u32>), Vec<u32>> = BTreeMap::new();
    for node in &chart.nodes {
        collections
            .entry((node.layer_index, node.parent))
            .or_default()
            .push(node.document_order);
    }
    if collections.values_mut().any(|orders| {
        orders.sort_unstable();
        orders
            .iter()
            .enumerate()
            .any(|(index, value)| *value != index as u32)
    }) {
        return Err("render.invalid-graph");
    }

    let mut previous: Option<OrderedNodeKey> = None;
    for index in root_total..chart.nodes.len() {
        let node = &chart.nodes[index];
        let key = (node.layer_index, ancestry_key(chart, index)?);
        if previous.as_ref().is_some_and(|prior| prior >= &key) {
            return Err("render.invalid-graph");
        }
        previous = Some(key);
    }
    Ok(())
}

fn sibling_key(node: &NodeRecord) -> (i32, u32, u64) {
    (node.z_order, node.document_order, node.id)
}

type OrderedNodeKey = (u32, Vec<(i32, u32, u64)>);

fn ancestry_key(
    chart: &DecodedRenderChart,
    mut index: usize,
) -> Result<Vec<(i32, u32, u64)>, &'static str> {
    let mut reverse = Vec::new();
    loop {
        let node = chart.nodes.get(index).ok_or("render.invalid-graph")?;
        reverse.push(sibling_key(node));
        match node.parent {
            Some(parent) => index = parent as usize,
            None => break,
        }
        if reverse.len() > chart.nodes.len() {
            return Err("render.invalid-graph");
        }
    }
    reverse.reverse();
    Ok(reverse)
}

fn validate_attachment(node: &NodeRecord, core: &DecodedChart) -> Result<(), &'static str> {
    if node.flags & (1 << 3) != 0 && node.attachment.kind != 4 {
        return Err("render.invalid-reference");
    }
    match node.attachment.kind {
        1 | 2 if node.attachment.id == 0 => Ok(()),
        3 if node.attachment.id != 0
            && core.lines.iter().any(|line| line.id == node.attachment.id) =>
        {
            Ok(())
        }
        4 if node.attachment.id != 0
            && core.notes.iter().any(|note| note.id == node.attachment.id) =>
        {
            Ok(())
        }
        _ => Err("render.invalid-reference"),
    }
}

struct Ownership {
    geometry_node: Vec<usize>,
    path_node: Vec<usize>,
    paint_node: Vec<usize>,
    stroke_node: Vec<usize>,
    clip_node: Vec<usize>,
    glyph_node: Vec<usize>,
}

fn validate_ownership(chart: &DecodedRenderChart) -> Result<Ownership, &'static str> {
    let mut geometry_owner = vec![None; chart.geometries.len()];
    let mut paint_owner = vec![None; chart.paints.len()];
    let mut stroke_owner = vec![None; chart.strokes.len()];
    let mut clip_owner = vec![None; chart.clips.len()];
    for (node_index, node) in chart.nodes.iter().enumerate() {
        if node.kind.is_drawable() {
            let geometry = node.geometry_ref.ok_or("render.invalid-reference")?;
            claim(&mut geometry_owner, geometry, node_index)?;
        } else if node.geometry_ref.is_some() {
            return Err("render.invalid-reference");
        }
        if let Some(paint) = node.fill_paint {
            claim(&mut paint_owner, paint, node_index)?;
        }
        if let Some(stroke) = node.stroke_ref {
            claim(&mut stroke_owner, stroke, node_index)?;
        }
        if let Some(clip) = node.clip_ref {
            claim(&mut clip_owner, clip, node_index)?;
        }
        match node.kind {
            NodeKind::Group
                if node.fill_paint.is_some()
                    || node.stroke_ref.is_some()
                    || node.clip_ref.is_some() =>
            {
                return Err("render.invalid-reference");
            }
            NodeKind::ClipGroup
                if node.fill_paint.is_some()
                    || node.stroke_ref.is_some()
                    || node.clip_ref.is_none() =>
            {
                return Err("render.invalid-reference");
            }
            NodeKind::Image if node.fill_paint.is_some() || node.stroke_ref.is_some() => {
                return Err("render.invalid-reference");
            }
            NodeKind::Line if node.fill_paint.is_some() || node.stroke_ref.is_none() => {
                return Err("render.invalid-reference");
            }
            kind if kind.is_drawable()
                && !matches!(kind, NodeKind::Image | NodeKind::Line)
                && node.fill_paint.is_none()
                && node.stroke_ref.is_none() =>
            {
                return Err("render.invalid-reference");
            }
            _ => {}
        }
    }
    let mut stroke_node = Vec::with_capacity(chart.strokes.len());
    for (index, stroke) in chart.strokes.iter().enumerate() {
        let node = stroke_owner[index].ok_or("render.invalid-graph")?;
        claim(&mut paint_owner, stroke.paint_ref, node)?;
        stroke_node.push(node);
    }
    let mut clip_node = Vec::with_capacity(chart.clips.len());
    for (index, clip) in chart.clips.iter().enumerate() {
        let node = clip_owner[index].ok_or("render.invalid-graph")?;
        claim(&mut geometry_owner, clip.geometry_ref, node)?;
        clip_node.push(node);
    }
    if geometry_owner.iter().any(Option::is_none)
        || paint_owner.iter().any(Option::is_none)
        || stroke_owner.iter().any(Option::is_none)
        || clip_owner.iter().any(Option::is_none)
    {
        return Err("render.invalid-graph");
    }
    let geometry_node: Vec<_> = geometry_owner.into_iter().map(Option::unwrap).collect();
    let paint_node: Vec<_> = paint_owner.into_iter().map(Option::unwrap).collect();

    let mut path_owner = vec![None; chart.paths.len()];
    let mut glyph_owner = vec![None; chart.glyph_runs.len()];
    for (geometry_index, geometry) in chart.geometries.iter().enumerate() {
        let owner_node = geometry_node[geometry_index];
        let is_clip_geometry = chart
            .clips
            .iter()
            .any(|clip| clip.geometry_ref as usize == geometry_index);
        if is_clip_geometry
            && !matches!(
                geometry.kind,
                NodeKind::Rect
                    | NodeKind::RoundedRect
                    | NodeKind::Circle
                    | NodeKind::Ellipse
                    | NodeKind::Polygon
                    | NodeKind::Path
            )
        {
            return Err("render.invalid-clip");
        }
        if !is_clip_geometry && chart.nodes[owner_node].kind != geometry.kind {
            return Err("render.invalid-reference");
        }
        match &geometry.data {
            GeometryData::Path { path_ref } => claim(&mut path_owner, *path_ref, owner_node)?,
            GeometryData::Text { glyph_runs, .. } => {
                for glyph in glyph_runs {
                    claim(&mut glyph_owner, *glyph, owner_node)?;
                }
            }
            _ => {}
        }
    }
    if path_owner.iter().any(Option::is_none) || glyph_owner.iter().any(Option::is_none) {
        return Err("render.invalid-graph");
    }
    for (clip_index, clip) in chart.clips.iter().enumerate() {
        let geometry = chart
            .geometries
            .get(clip.geometry_ref as usize)
            .ok_or("render.invalid-reference")?;
        if let GeometryData::Path { path_ref } = geometry.data
            && chart
                .paths
                .get(path_ref as usize)
                .ok_or("render.invalid-reference")?
                .fill_rule
                != clip.fill_rule
        {
            return Err("render.invalid-clip");
        }
        if chart.nodes[clip_node[clip_index]].clip_ref != Some(clip_index as u32) {
            return Err("render.invalid-graph");
        }
    }
    Ok(Ownership {
        geometry_node,
        path_node: path_owner.into_iter().map(Option::unwrap).collect(),
        paint_node,
        stroke_node,
        clip_node,
        glyph_node: glyph_owner.into_iter().map(Option::unwrap).collect(),
    })
}

fn claim(owners: &mut [Option<usize>], reference: u32, owner: usize) -> Result<(), &'static str> {
    let slot = owners
        .get_mut(reference as usize)
        .ok_or("render.invalid-reference")?;
    if slot.replace(owner).is_some() {
        return Err("render.invalid-graph");
    }
    Ok(())
}

fn validate_descriptor_roots(
    chart: &DecodedRenderChart,
    owners: &Ownership,
) -> Result<(), &'static str> {
    for node in &chart.nodes {
        for (reference, expected) in [
            (node.position_descriptor, ValueType::Vec2Length),
            (node.origin_descriptor, ValueType::Vec2Length),
            (node.rotation_descriptor, ValueType::Angle),
            (node.scale_descriptor, ValueType::Vec2Float),
            (node.opacity_descriptor, ValueType::Float),
            (node.visibility_descriptor, ValueType::Bool),
        ] {
            check_descriptor(chart, node, reference, expected)?;
        }
    }
    for (index, geometry) in chart.geometries.iter().enumerate() {
        let node = &chart.nodes[owners.geometry_node[index]];
        validate_geometry_descriptors(chart, node, &geometry.data)?;
    }
    for (index, path) in chart.paths.iter().enumerate() {
        let node = &chart.nodes[owners.path_node[index]];
        for command in &path.commands {
            validate_path_command_descriptors(chart, node, command)?;
        }
    }
    for (index, paint) in chart.paints.iter().enumerate() {
        let node = &chart.nodes[owners.paint_node[index]];
        validate_paint_descriptors(chart, node, &paint.data)?;
    }
    for (index, stroke) in chart.strokes.iter().enumerate() {
        let node = &chart.nodes[owners.stroke_node[index]];
        check_descriptor(chart, node, stroke.width_descriptor, ValueType::Length)?;
        check_descriptor(
            chart,
            node,
            stroke.dash_offset_descriptor,
            ValueType::Length,
        )?;
    }
    for (index, glyph) in chart.glyph_runs.iter().enumerate() {
        let node = &chart.nodes[owners.glyph_node[index]];
        check_descriptor(chart, node, glyph.size_descriptor, ValueType::Length)?;
    }
    // Reading the field proves clip ownership contributes the same descriptor environment.
    for node in &owners.clip_node {
        let _ = chart.nodes.get(*node).ok_or("render.invalid-graph")?;
    }
    Ok(())
}

fn validate_geometry_descriptors(
    chart: &DecodedRenderChart,
    node: &NodeRecord,
    data: &GeometryData,
) -> Result<(), &'static str> {
    let mut roots = Vec::new();
    match data {
        GeometryData::Rect { origin, size } => roots.extend([
            (*origin, ValueType::Vec2Length),
            (*size, ValueType::Vec2Length),
        ]),
        GeometryData::RoundedRect {
            origin,
            size,
            radii,
        } => {
            roots.extend([
                (*origin, ValueType::Vec2Length),
                (*size, ValueType::Vec2Length),
            ]);
            roots.extend(radii.iter().map(|value| (*value, ValueType::Length)));
        }
        GeometryData::Circle { center, radius } => roots.extend([
            (*center, ValueType::Vec2Length),
            (*radius, ValueType::Length),
        ]),
        GeometryData::Ellipse {
            center,
            radius_x,
            radius_y,
            rotation,
        } => roots.extend([
            (*center, ValueType::Vec2Length),
            (*radius_x, ValueType::Length),
            (*radius_y, ValueType::Length),
            (*rotation, ValueType::Angle),
        ]),
        GeometryData::Line { start, end } => roots.extend([
            (*start, ValueType::Vec2Length),
            (*end, ValueType::Vec2Length),
        ]),
        GeometryData::Polyline { points } | GeometryData::Polygon { points } => {
            roots.extend(points.iter().map(|value| (*value, ValueType::Vec2Length)))
        }
        GeometryData::Path { .. } => {}
        GeometryData::Image {
            destination,
            source,
            ..
        } => {
            roots.extend(destination.iter().map(|value| (*value, ValueType::Length)));
            if let Some(source) = source {
                roots.extend(source.iter().map(|value| (*value, ValueType::Float)));
            }
        }
        GeometryData::Text { origin, .. } => roots.push((*origin, ValueType::Vec2Length)),
    }
    for (reference, expected) in roots {
        check_descriptor(chart, node, reference, expected)?;
    }
    Ok(())
}

fn validate_path_command_descriptors(
    chart: &DecodedRenderChart,
    node: &NodeRecord,
    command: &PathCommand,
) -> Result<(), &'static str> {
    let mut roots = Vec::new();
    match command {
        PathCommand::MoveTo(point) | PathCommand::LineTo(point) => {
            roots.push((*point, ValueType::Vec2Length))
        }
        PathCommand::QuadraticTo(control, end) => roots.extend([
            (*control, ValueType::Vec2Length),
            (*end, ValueType::Vec2Length),
        ]),
        PathCommand::CubicTo(a, b, end) => roots.extend([
            (*a, ValueType::Vec2Length),
            (*b, ValueType::Vec2Length),
            (*end, ValueType::Vec2Length),
        ]),
        PathCommand::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
        } => roots.extend([
            (*center, ValueType::Vec2Length),
            (*radius, ValueType::Length),
            (*start_angle, ValueType::Angle),
            (*end_angle, ValueType::Angle),
        ]),
        PathCommand::EllipseArc {
            center,
            radius_x,
            radius_y,
            rotation,
            start_angle,
            end_angle,
            ..
        } => roots.extend([
            (*center, ValueType::Vec2Length),
            (*radius_x, ValueType::Length),
            (*radius_y, ValueType::Length),
            (*rotation, ValueType::Angle),
            (*start_angle, ValueType::Angle),
            (*end_angle, ValueType::Angle),
        ]),
        PathCommand::Close => {}
    }
    for (reference, expected) in roots {
        check_descriptor(chart, node, reference, expected)?;
    }
    Ok(())
}

fn validate_paint_descriptors(
    chart: &DecodedRenderChart,
    node: &NodeRecord,
    paint: &PaintData,
) -> Result<(), &'static str> {
    let mut roots = Vec::new();
    match paint {
        PaintData::Solid { color } => roots.push((*color, ValueType::Color)),
        PaintData::LinearGradient {
            start, end, stops, ..
        } => {
            roots.extend([
                (*start, ValueType::Vec2Length),
                (*end, ValueType::Vec2Length),
            ]);
            roots.extend(
                stops
                    .iter()
                    .map(|stop| (stop.color_descriptor, ValueType::Color)),
            );
        }
        PaintData::RadialGradient {
            start_center,
            start_radius,
            end_center,
            end_radius,
            stops,
            ..
        } => {
            roots.extend([
                (*start_center, ValueType::Vec2Length),
                (*start_radius, ValueType::Length),
                (*end_center, ValueType::Vec2Length),
                (*end_radius, ValueType::Length),
            ]);
            roots.extend(
                stops
                    .iter()
                    .map(|stop| (stop.color_descriptor, ValueType::Color)),
            );
        }
        PaintData::ImagePattern {
            position,
            origin,
            rotation,
            scale,
            ..
        } => roots.extend([
            (*position, ValueType::Vec2Length),
            (*origin, ValueType::Vec2Length),
            (*rotation, ValueType::Angle),
            (*scale, ValueType::Vec2Float),
        ]),
    }
    for (reference, expected) in roots {
        check_descriptor(chart, node, reference, expected)?;
    }
    Ok(())
}

fn check_descriptor(
    chart: &DecodedRenderChart,
    owner: &NodeRecord,
    reference: u32,
    expected: ValueType,
) -> Result<(), &'static str> {
    fcs_fcbc::validate_descriptor_env_p_context(
        reference,
        &chart.core.descriptors,
        &chart.core.expressions,
    )
    .map_err(|_| "render.invalid-descriptor")?;
    let descriptor = chart
        .core
        .descriptors
        .get(reference as usize)
        .ok_or("render.invalid-descriptor")?;
    if descriptor.property_type != expected || !descriptor.domain.covers(owner.active_domain()) {
        return Err("render.invalid-descriptor");
    }
    let dependencies = descriptor_environment(
        reference,
        &chart.core.descriptors,
        &chart.core.expressions,
        0,
    )?;
    if dependencies.0 && owner.attachment.kind != 4 {
        return Err("render.invalid-descriptor");
    }
    if dependencies.1 && !matches!(owner.attachment.kind, 3 | 4) {
        return Err("render.invalid-descriptor");
    }
    Ok(())
}

fn descriptor_environment(
    index: u32,
    descriptors: &[PropertyDescriptor],
    expressions: &[ExpressionNode],
    depth: usize,
) -> Result<(bool, bool), &'static str> {
    if depth > descriptors.len() + expressions.len() {
        return Err("render.invalid-descriptor");
    }
    let descriptor = descriptors
        .get(index as usize)
        .ok_or("render.invalid-descriptor")?;
    match &descriptor.kind {
        DescriptorKind::Constant(_) | DescriptorKind::SegmentTrack(_) => Ok((false, false)),
        DescriptorKind::Piecewise(pieces) => {
            let mut result = (false, false);
            for piece in pieces {
                let dependency = descriptor_environment(
                    piece.descriptor_index,
                    descriptors,
                    expressions,
                    depth + 1,
                )?;
                result.0 |= dependency.0;
                result.1 |= dependency.1;
            }
            Ok(result)
        }
        DescriptorKind::Expression(root) => expression_environment(*root, expressions, depth + 1),
    }
}

fn expression_environment(
    index: u32,
    expressions: &[ExpressionNode],
    depth: usize,
) -> Result<(bool, bool), &'static str> {
    if depth > expressions.len() {
        return Err("render.invalid-descriptor");
    }
    let node = expressions
        .get(index as usize)
        .ok_or("render.invalid-descriptor")?;
    let mut result = (node.opcode == 5, node.opcode == 4);
    for operand in &node.operands[..node.arity as usize] {
        let dependency = expression_environment(*operand, expressions, depth + 1)?;
        result.0 |= dependency.0;
        result.1 |= dependency.1;
    }
    Ok(result)
}

fn validate_and_decode_resources(chart: &mut DecodedRenderChart) -> Result<(), &'static str> {
    let image_ids: BTreeSet<_> = chart
        .geometries
        .iter()
        .filter_map(|geometry| match geometry.data {
            GeometryData::Image { resource_id, .. } => Some(resource_id),
            _ => None,
        })
        .chain(chart.paints.iter().filter_map(|paint| match paint.data {
            PaintData::ImagePattern { resource_id, .. } => Some(resource_id),
            _ => None,
        }))
        .collect();
    for id in image_ids {
        let resource = chart
            .resources
            .iter()
            .find(|resource| resource.id == id)
            .ok_or("render.resource-not-found")?;
        if !matches!(resource.kind, 2 | 4) {
            return Err("render.resource-type-mismatch");
        }
        let metadata = image_metadata(resource, &chart.core.strings)?;
        let decoded = decode_image(
            &resource.media_type,
            &metadata.0,
            &metadata.1,
            &resource.data,
        )
        .map_err(asset_error)?;
        chart.decoded_images.insert(id, decoded);
    }
    let font_ids: BTreeSet<_> = chart
        .glyph_runs
        .iter()
        .map(|run| run.font_resource_id)
        .collect();
    for id in font_ids {
        let resource = chart
            .resources
            .iter()
            .find(|resource| resource.id == id)
            .ok_or("render.resource-not-found")?;
        if resource.kind != 3 {
            return Err("render.resource-type-mismatch");
        }
        if resource.media_type != "font/ttf" {
            return Err("render.resource-capability-missing");
        }
        validate_font_metadata(resource, &chart.core.strings)?;
        let font = decode_font(&resource.data).map_err(asset_error)?;
        chart.decoded_fonts.insert(id, font);
    }
    for run in &chart.glyph_runs {
        let font = chart
            .decoded_fonts
            .get(&run.font_resource_id)
            .ok_or("render.resource-not-found")?;
        if run.face_index != 0
            || run
                .glyphs
                .iter()
                .any(|glyph| glyph.glyph_id == 0 || glyph.glyph_id as usize >= font.glyphs.len())
        {
            return Err("render.invalid-geometry");
        }
    }
    Ok(())
}

fn image_metadata(
    resource: &ResourceRecord,
    strings: &[String],
) -> Result<(String, String, String), &'static str> {
    let fields = metadata_fields(&resource.metadata, strings)?;
    if fields.len() != 3
        || fields[0].0 != "colorSpace"
        || fields[1].0 != "alpha"
        || fields[2].0 != "sampling"
    {
        return Err("render.resource-decode-failed");
    }
    let values = fields
        .into_iter()
        .map(|(_, value)| expect_metadata_string(value, strings))
        .collect::<Result<Vec<_>, _>>()?;
    if !matches!(values[0].as_str(), "srgb" | "linear-srgb")
        || !matches!(values[1].as_str(), "straight" | "premultiplied")
        || !matches!(values[2].as_str(), "nearest" | "linear")
    {
        return Err("render.resource-decode-failed");
    }
    if resource.media_type == "image/webp" && (values[0] != "srgb" || values[1] != "straight") {
        return Err("render.resource-decode-failed");
    }
    Ok((values[0].clone(), values[1].clone(), values[2].clone()))
}

fn validate_font_metadata(
    resource: &ResourceRecord,
    strings: &[String],
) -> Result<(), &'static str> {
    let fields = metadata_fields(&resource.metadata, strings)?;
    if fields.len() != 3
        || fields[0].0 != "fontProfile"
        || fields[1].0 != "shapingProfile"
        || fields[2].0 != "faceCount"
    {
        return Err("render.resource-decode-failed");
    }
    if expect_metadata_string(fields[0].1, strings)? != "truetype-glyf-1"
        || expect_metadata_string(fields[1].1, strings)? != "simple-ltr-1"
        || !matches!(fields[2].1, ParsedValue::Int(1))
    {
        return Err("render.resource-decode-failed");
    }
    Ok(())
}

fn metadata_fields<'a>(
    value: &'a ParsedValue,
    strings: &[String],
) -> Result<Vec<(String, &'a ParsedValue)>, &'static str> {
    let ParsedValue::Object(fields) = value else {
        return Err("render.resource-decode-failed");
    };
    fields
        .iter()
        .map(|(key, value)| {
            Ok((
                strings
                    .get(*key as usize)
                    .ok_or("render.resource-decode-failed")?
                    .clone(),
                value,
            ))
        })
        .collect()
}

fn expect_metadata_string(value: &ParsedValue, strings: &[String]) -> Result<String, &'static str> {
    let ParsedValue::String(reference) = value else {
        return Err("render.resource-decode-failed");
    };
    strings
        .get(*reference as usize)
        .cloned()
        .ok_or("render.resource-decode-failed")
}

fn asset_error(error: AssetError) -> &'static str {
    match error {
        AssetError::CapabilityMissing => "render.resource-capability-missing",
        AssetError::DecodeFailed => "render.resource-decode-failed",
    }
}

fn limited_count(value: u32) -> Result<usize, &'static str> {
    let value = value as usize;
    (value <= MAX_TABLE_ITEMS)
        .then_some(value)
        .ok_or("render.limit-exceeded")
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, &'static str> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or("fcbc.invalid-header")?
            .try_into()
            .map_err(|_| "fcbc.invalid-header")?,
    ))
}

fn u64_at(bytes: &[u8], offset: usize) -> Result<u64, &'static str> {
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..offset + 8)
            .ok_or("fcbc.invalid-header")?
            .try_into()
            .map_err(|_| "fcbc.invalid-header")?,
    ))
}
