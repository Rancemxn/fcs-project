//! Test-only deterministic FCBC Render fixture writer.
//!
//! The writer composes the already fixed nonempty Execution ABI chart with a declarative Render
//! scene. It parses only the FCBC framing needed to preserve Core section payloads and never reads
//! a checked-in golden, manifest, expected semantic output, loader, or product implementation.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

const REQUIRED: u16 = 1;
const NULL_INDEX: u32 = u32::MAX;

pub const PNG_RESOURCE_TEXT_ID: &str = "fixture.render.png";
pub const WEBP_RESOURCE_TEXT_ID: &str = "fixture.render.webp";
pub const FONT_RESOURCE_TEXT_ID: &str = "fixture.render.font";
pub const MALFORMED_RESOURCE_TEXT_ID: &str = "fixture.render.malformed";
pub const UNSUPPORTED_RESOURCE_TEXT_ID: &str = "fixture.render.unsupported";

pub const ANALYTIC_NOTE_TEXT_ID: &str = "fixture.analytic.note";
pub const TEXT_NOTE_TEXT_ID: &str = "fixture.render.text";

#[derive(Clone, Copy, Debug)]
pub struct RenderAssets<'a> {
    pub png: &'a [u8],
    pub webp: &'a [u8],
    pub font: &'a [u8],
    pub malformed: &'a [u8],
}

#[derive(Clone, Debug)]
struct Section {
    kind: u32,
    payload: Vec<u8>,
    offset: u64,
}

#[derive(Clone, Copy)]
struct NodeSpec {
    key: &'static str,
    textual_id: &'static str,
    parent: Option<&'static str>,
    document_order: u32,
    kind: u16,
    attachment_kind: u16,
    attachment_id: u64,
    opacity_descriptor: u32,
    composite: u16,
    isolated: bool,
}

#[derive(Clone, Copy)]
struct ResourceSpec<'a> {
    textual_id: &'static str,
    kind: u16,
    media_type: &'static str,
    bytes: &'a [u8],
    metadata: ResourceMetadata,
}

#[derive(Clone, Copy)]
enum ResourceMetadata {
    Image,
    Font,
}

/// Stable identifier algorithm required by FCBC 2 and Render 1.0.
pub fn stable_id(namespace: &[u8], textual_id: &[u8]) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(namespace);
    hasher.update([0]);
    hasher.update(textual_id);
    u64::from_le_bytes(
        hasher.finalize()[..8]
            .try_into()
            .expect("SHA-256 prefix width"),
    )
}

pub fn resource_id(textual_id: &str) -> u64 {
    stable_id(b"fcs.resource", textual_id.as_bytes())
}

pub fn note_id(textual_id: &str) -> u64 {
    stable_id(b"fcs.note", textual_id.as_bytes())
}

/// Builds the fixed RenderSection conformance fixture from the reviewed nonempty Core fixture.
pub fn write_nonempty_render(core_bytes: &[u8], assets: RenderAssets<'_>) -> Vec<u8> {
    let mut sections = read_core_sections(core_bytes);
    let strings = fixture_strings();
    let string_indices = string_indices(&strings);

    let resources = fixture_resources(assets);
    let (resource_section, resource_data) = resource_sections(&resources, &string_indices);

    replace_section(&mut sections, 1, string_table_section(&strings));
    replace_section(&mut sections, 6, resource_section);
    let notes = sections
        .iter()
        .find(|section| section.kind == 10)
        .expect("Core fixture Notes section")
        .payload
        .clone();
    replace_section(&mut sections, 10, notes_section(notes, &string_indices));
    replace_section(&mut sections, 20, resource_data);
    sections.push(Section::new(14, render_section(&string_indices)));
    sections.sort_by_key(|section| section.kind);

    let table_length = sections.len() * 40;
    let mut bytes = vec![0; 128 + table_length];
    bytes[..128].copy_from_slice(&core_bytes[..128]);
    for section in &mut sections {
        pad_to(&mut bytes, 8);
        section.offset = bytes.len() as u64;
        bytes.extend_from_slice(&section.payload);
    }

    let feature_flags = u64_at(&bytes, 28) | (1 << 1);
    write_u64_at(&mut bytes, 28, feature_flags);
    write_u32_at(&mut bytes, 36, sections.len() as u32);
    write_u64_at(&mut bytes, 40, 128);
    let file_length = bytes.len() as u64;
    write_u64_at(&mut bytes, 48, file_length);
    write_section_table(&mut bytes, &sections);
    bytes
}

impl Section {
    fn new(kind: u32, payload: Vec<u8>) -> Self {
        Self {
            kind,
            payload,
            offset: 0,
        }
    }
}

fn read_core_sections(bytes: &[u8]) -> Vec<Section> {
    assert!(bytes.len() >= 128, "Core fixture header is truncated");
    assert_eq!(&bytes[..4], b"FCSB", "Core fixture magic");
    assert_eq!(u64_at(bytes, 40), 128, "Core fixture table offset");
    let count = u32_at(bytes, 36) as usize;
    assert!(128 + count * 40 <= bytes.len(), "Core section table bounds");

    let mut sections = Vec::with_capacity(count + 1);
    for index in 0..count {
        let entry = 128 + index * 40;
        let kind = u32_at(bytes, entry);
        let offset = usize::try_from(u64_at(bytes, entry + 16)).expect("section offset");
        let length = usize::try_from(u64_at(bytes, entry + 24)).expect("section length");
        let end = offset.checked_add(length).expect("section range overflow");
        assert!(end <= bytes.len(), "Core section range");
        sections.push(Section::new(kind, bytes[offset..end].to_vec()));
    }
    assert_eq!(sections.len(), 14, "reviewed Core fixture section count");
    assert!(sections.iter().all(|section| section.kind != 14));
    sections
}

fn replace_section(sections: &mut [Section], kind: u32, payload: Vec<u8>) {
    sections
        .iter_mut()
        .find(|section| section.kind == kind)
        .unwrap_or_else(|| panic!("Core fixture is missing section {kind}"))
        .payload = payload;
}

fn fixture_strings() -> Vec<&'static str> {
    let mut strings = vec![
        "alpha",
        "centerDescriptor",
        "colorSpace",
        "destinationDescriptors",
        "endDescriptor",
        "faceCount",
        "font/ttf",
        "fontProfile",
        "glyphRunRefs",
        "image/jpeg",
        "image/png",
        "image/webp",
        "kind",
        "lineDefault",
        "nearest",
        "originDescriptor",
        "pathRef",
        "pointDescriptors",
        "radiiDescriptors",
        "radiusDescriptor",
        "radiusXDescriptor",
        "radiusYDescriptor",
        "resourceId",
        "rotationDescriptor",
        "sampling",
        "shapingProfile",
        "simple-ltr-1",
        "sizeDescriptor",
        "srgb",
        "startDescriptor",
        "straight",
        "truetype-glyf-1",
    ];
    strings.sort_unstable_by_key(|value| value.as_bytes());
    strings.dedup();
    strings
}

fn string_indices<'a>(strings: &'a [&'a str]) -> BTreeMap<&'a str, u32> {
    strings
        .iter()
        .enumerate()
        .map(|(index, value)| (*value, index as u32))
        .collect()
}

fn string_table_section(strings: &[&str]) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u32(&mut payload, strings.len() as u32);
    put_u32(&mut payload, 0);
    let mut offset = 0u32;
    for string in strings {
        offset = offset
            .checked_add(string.len() as u32)
            .expect("fixture string bytes");
        put_u32(&mut payload, offset);
    }
    for string in strings {
        payload.extend_from_slice(string.as_bytes());
    }
    pad_to(&mut payload, 8);
    payload
}

fn fixture_resources(assets: RenderAssets<'_>) -> Vec<ResourceSpec<'_>> {
    vec![
        ResourceSpec {
            textual_id: PNG_RESOURCE_TEXT_ID,
            kind: 2,
            media_type: "image/png",
            bytes: assets.png,
            metadata: ResourceMetadata::Image,
        },
        ResourceSpec {
            textual_id: WEBP_RESOURCE_TEXT_ID,
            kind: 2,
            media_type: "image/webp",
            bytes: assets.webp,
            metadata: ResourceMetadata::Image,
        },
        ResourceSpec {
            textual_id: FONT_RESOURCE_TEXT_ID,
            kind: 3,
            media_type: "font/ttf",
            bytes: assets.font,
            metadata: ResourceMetadata::Font,
        },
        ResourceSpec {
            textual_id: MALFORMED_RESOURCE_TEXT_ID,
            kind: 2,
            media_type: "image/png",
            bytes: assets.malformed,
            metadata: ResourceMetadata::Image,
        },
        ResourceSpec {
            textual_id: UNSUPPORTED_RESOURCE_TEXT_ID,
            kind: 2,
            media_type: "image/jpeg",
            bytes: assets.malformed,
            metadata: ResourceMetadata::Image,
        },
    ]
}

fn resource_sections(
    specs: &[ResourceSpec<'_>],
    strings: &BTreeMap<&str, u32>,
) -> (Vec<u8>, Vec<u8>) {
    let mut sorted: Vec<_> = specs
        .iter()
        .map(|spec| (resource_id(spec.textual_id), *spec))
        .collect();
    sorted.sort_by_key(|(id, _)| *id);

    let mut data = Vec::new();
    let mut ranges = BTreeMap::new();
    for (ordinal, (id, spec)) in sorted.iter().enumerate() {
        if ordinal != 0 {
            pad_to(&mut data, 8);
        }
        let offset = data.len() as u64;
        data.extend_from_slice(spec.bytes);
        ranges.insert(*id, (offset, spec.bytes.len() as u64));
    }

    let mut directory = Vec::new();
    put_u32(&mut directory, sorted.len() as u32);
    for (id, spec) in sorted {
        let (offset, length) = ranges[&id];
        let mut payload = Vec::new();
        put_u64(&mut payload, id);
        put_u16(&mut payload, spec.kind);
        put_u16(&mut payload, 0);
        put_u32(&mut payload, strings[spec.media_type]);
        put_u16(&mut payload, 1);
        put_u16(&mut payload, 0);
        put_u64(&mut payload, offset);
        put_u64(&mut payload, length);
        payload.extend_from_slice(&counted_bytes(&Sha256::digest(spec.bytes)));
        payload.extend_from_slice(&resource_metadata(spec.metadata, strings));
        directory.extend_from_slice(&record(payload));
    }
    (directory, data)
}

fn resource_metadata(kind: ResourceMetadata, strings: &BTreeMap<&str, u32>) -> Vec<u8> {
    match kind {
        ResourceMetadata::Image => value_object(
            &[
                ("colorSpace", value_string(strings["srgb"])),
                ("alpha", value_string(strings["straight"])),
                ("sampling", value_string(strings["nearest"])),
            ],
            strings,
        ),
        ResourceMetadata::Font => value_object(
            &[
                ("fontProfile", value_string(strings["truetype-glyf-1"])),
                ("shapingProfile", value_string(strings["simple-ltr-1"])),
                ("faceCount", value_int(1)),
            ],
            strings,
        ),
    }
}

fn notes_section(mut original: Vec<u8>, strings: &BTreeMap<&str, u32>) -> Vec<u8> {
    let old_count = u32_at(&original, 0) as usize;
    assert_eq!(old_count, 2, "reviewed Core fixture note count");
    let mut records = split_records(&original[4..]);
    assert_eq!(records.len(), old_count);
    for record in &mut records {
        rewrite_judge_shape_refs(record, strings["kind"], strings["lineDefault"]);
    }

    let mut render_note = records[0].clone();
    write_u64_at(&mut render_note, 8, note_id(TEXT_NOTE_TEXT_ID));
    write_u64_at(
        &mut render_note,
        16,
        stable_id(b"fcs.line", b"fixture.analytic"),
    );
    write_u32_at(&mut render_note, 24, 2);
    write_u64_at(&mut render_note, 32, 2.0f64.to_bits());
    records.push(render_note);

    original.clear();
    put_u32(&mut original, records.len() as u32);
    for record in records {
        original.extend_from_slice(&record);
    }
    original
}

fn split_records(mut bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut records = Vec::new();
    while !bytes.is_empty() {
        assert!(bytes.len() >= 8, "record prefix");
        let length = u32_at(bytes, 0) as usize;
        assert!(length >= 8 && length.is_multiple_of(4) && length <= bytes.len());
        records.push(bytes[..length].to_vec());
        bytes = &bytes[length..];
    }
    records
}

fn rewrite_judge_shape_refs(record: &mut [u8], key_ref: u32, value_ref: u32) {
    assert!(record.len() >= 76, "NoteRecord judge shape");
    assert_eq!(record[48], 14, "judge shape object tag");
    assert_eq!(u32_at(record, 56), 1, "judge shape field count");
    assert_eq!(record[64], 4, "judge shape nested string tag");
    write_u32_at(record, 60, key_ref);
    write_u32_at(record, 72, value_ref);
}

fn render_section(strings: &BTreeMap<&str, u32>) -> Vec<u8> {
    let layer_id = stable_id(b"fcs.render.layer", b"layer/main");
    let analytic_note = note_id(ANALYTIC_NOTE_TEXT_ID);
    let text_note = note_id(TEXT_NOTE_TEXT_ID);
    let nodes = node_specs(analytic_note, text_note);
    let node_ids: BTreeMap<_, _> = nodes
        .iter()
        .map(|node| {
            (
                node.key,
                stable_id(b"fcs.render.node", node.textual_id.as_bytes()),
            )
        })
        .collect();

    let drawable_keys = [
        "rect", "rounded", "circle", "ellipse", "line", "polyline", "polygon", "path", "image",
        "text",
    ];
    let geometry_ids: BTreeMap<_, _> = drawable_keys
        .iter()
        .map(|key| {
            (
                *key,
                auxiliary_id(b"fcs.render.geometry", node_ids[key], "geometryRef", 0),
            )
        })
        .collect();
    let clip_id = auxiliary_id(b"fcs.render.clip", node_ids["image-clip"], "clipRef", 0);
    let clip_geometry_id = auxiliary_id(b"fcs.render.geometry", clip_id, "geometryRef", 0);
    let path_id = auxiliary_id(b"fcs.render.path", geometry_ids["path"], "pathRef", 0);
    let glyph_id = auxiliary_id(
        b"fcs.render.glyph-run",
        geometry_ids["text"],
        "glyphRunRefs",
        0,
    );
    let stroke_id = auxiliary_id(b"fcs.render.stroke", node_ids["line"], "strokeRef", 0);

    let fill_keys = [
        "rect", "rounded", "circle", "ellipse", "polyline", "polygon", "path", "text",
    ];
    let mut paint_owner_ids = Vec::new();
    for key in fill_keys {
        paint_owner_ids.push((
            key,
            auxiliary_id(b"fcs.render.paint", node_ids[key], "fillPaint", 0),
        ));
    }
    paint_owner_ids.push((
        "line-stroke",
        auxiliary_id(b"fcs.render.paint", stroke_id, "paintRef", 0),
    ));
    let paint_ids: BTreeMap<_, _> = paint_owner_ids.into_iter().collect();

    let mut geometry_order: Vec<_> = geometry_ids.values().copied().collect();
    geometry_order.push(clip_geometry_id);
    geometry_order.sort_unstable();
    let geometry_indices = indices(&geometry_order);
    let mut paint_order: Vec<_> = paint_ids.values().copied().collect();
    paint_order.sort_unstable();
    let paint_indices = indices(&paint_order);
    let path_indices = indices(&[path_id]);
    let stroke_indices = indices(&[stroke_id]);
    let clip_indices = indices(&[clip_id]);
    let glyph_indices = indices(&[glyph_id]);

    let mut payload = Vec::new();
    put_u16(&mut payload, 1);
    put_u16(&mut payload, 0);
    put_u16(&mut payload, 0);
    put_u16(&mut payload, 0);
    put_f64(&mut payload, 12.0);
    put_f64(&mut payload, 12.0);
    put_u16(&mut payload, 2);
    put_u16(&mut payload, 0);
    put_u32(&mut payload, 1);
    put_u32(&mut payload, nodes.len() as u32);
    put_u32(&mut payload, geometry_order.len() as u32);
    put_u32(&mut payload, 1);
    put_u32(&mut payload, paint_order.len() as u32);
    put_u32(&mut payload, 1);
    put_u32(&mut payload, 1);
    put_u32(&mut payload, 1);
    debug_assert_eq!(payload.len(), 60);

    payload.extend_from_slice(&layer_record(layer_id, 3));
    for (index, node) in nodes.iter().enumerate() {
        payload.extend_from_slice(&node_record(
            node,
            index,
            &node_ids,
            &geometry_ids,
            &paint_ids,
            &geometry_indices,
            &paint_indices,
            &stroke_indices,
            &clip_indices,
        ));
    }
    for id in geometry_order {
        payload.extend_from_slice(&geometry_record(
            id,
            &geometry_ids,
            clip_geometry_id,
            &path_indices,
            &glyph_indices,
            strings,
        ));
    }
    payload.extend_from_slice(&path_record(path_id));
    for id in paint_order {
        payload.extend_from_slice(&paint_record(id, &paint_ids));
    }
    payload.extend_from_slice(&stroke_record(
        stroke_id,
        paint_indices[&paint_ids["line-stroke"]],
    ));
    payload.extend_from_slice(&clip_record(clip_id, geometry_indices[&clip_geometry_id]));
    payload.extend_from_slice(&glyph_record(glyph_id));
    record(payload)
}

fn node_specs(analytic_note: u64, text_note: u64) -> Vec<NodeSpec> {
    vec![
        node(
            "world-group",
            "layer/main/node/world-group",
            None,
            0,
            1,
            4,
            analytic_note,
            8,
            1,
            false,
        ),
        node(
            "image-clip",
            "layer/main/node/image-clip",
            None,
            1,
            2,
            4,
            analytic_note,
            8,
            1,
            false,
        ),
        node(
            "text-isolate",
            "layer/main/node/text-isolate",
            None,
            2,
            1,
            4,
            text_note,
            8,
            5,
            true,
        ),
        node(
            "rect",
            "layer/main/node/world-group/node/rect",
            Some("world-group"),
            0,
            3,
            4,
            analytic_note,
            8,
            1,
            false,
        ),
        node(
            "rounded",
            "layer/main/node/world-group/node/rounded",
            Some("world-group"),
            1,
            4,
            4,
            analytic_note,
            5,
            2,
            false,
        ),
        node(
            "circle",
            "layer/main/node/world-group/node/circle",
            Some("world-group"),
            2,
            5,
            4,
            analytic_note,
            11,
            3,
            false,
        ),
        node(
            "ellipse",
            "layer/main/node/world-group/node/ellipse",
            Some("world-group"),
            3,
            6,
            4,
            analytic_note,
            8,
            4,
            false,
        ),
        node(
            "line",
            "layer/main/node/world-group/node/line",
            Some("world-group"),
            4,
            7,
            4,
            analytic_note,
            8,
            5,
            false,
        ),
        node(
            "polyline",
            "layer/main/node/world-group/node/polyline",
            Some("world-group"),
            5,
            8,
            4,
            analytic_note,
            8,
            1,
            false,
        ),
        node(
            "polygon",
            "layer/main/node/world-group/node/polygon",
            Some("world-group"),
            6,
            9,
            4,
            analytic_note,
            8,
            2,
            false,
        ),
        node(
            "path",
            "layer/main/node/world-group/node/path",
            Some("world-group"),
            7,
            10,
            4,
            analytic_note,
            8,
            3,
            false,
        ),
        node(
            "image",
            "layer/main/node/image-clip/node/image",
            Some("image-clip"),
            0,
            11,
            4,
            analytic_note,
            8,
            1,
            false,
        ),
        node(
            "text",
            "layer/main/node/text-isolate/node/text",
            Some("text-isolate"),
            0,
            12,
            4,
            text_note,
            8,
            1,
            false,
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
const fn node(
    key: &'static str,
    textual_id: &'static str,
    parent: Option<&'static str>,
    document_order: u32,
    kind: u16,
    attachment_kind: u16,
    attachment_id: u64,
    opacity_descriptor: u32,
    composite: u16,
    isolated: bool,
) -> NodeSpec {
    NodeSpec {
        key,
        textual_id,
        parent,
        document_order,
        kind,
        attachment_kind,
        attachment_id,
        opacity_descriptor,
        composite,
        isolated,
    }
}

fn auxiliary_id(namespace: &[u8], owner: u64, field: &str, ordinal: u32) -> u64 {
    let textual = format!("owner/{owner:016x}/field/{field}/ordinal/{ordinal}");
    stable_id(namespace, textual.as_bytes())
}

fn indices(ids: &[u64]) -> BTreeMap<u64, u32> {
    ids.iter()
        .enumerate()
        .map(|(index, id)| (*id, index as u32))
        .collect()
}

fn layer_record(id: u64, root_count: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u64(&mut payload, id);
    put_u16(&mut payload, 6);
    put_u16(&mut payload, 0);
    put_i32(&mut payload, 0);
    put_u32(&mut payload, 0);
    put_u32(&mut payload, 0);
    put_u32(&mut payload, root_count);
    let encoded = record(payload);
    debug_assert_eq!(encoded.len(), 36);
    encoded
}

#[allow(clippy::too_many_arguments)]
fn node_record(
    node: &NodeSpec,
    index: usize,
    node_ids: &BTreeMap<&str, u64>,
    geometry_ids: &BTreeMap<&str, u64>,
    paint_ids: &BTreeMap<&str, u64>,
    geometry_indices: &BTreeMap<u64, u32>,
    paint_indices: &BTreeMap<u64, u32>,
    stroke_indices: &BTreeMap<u64, u32>,
    clip_indices: &BTreeMap<u64, u32>,
) -> Vec<u8> {
    let parent = node.parent.map_or(NULL_INDEX, |key| {
        u32::try_from(
            [
                "world-group",
                "image-clip",
                "text-isolate",
                "rect",
                "rounded",
                "circle",
                "ellipse",
                "line",
                "polyline",
                "polygon",
                "path",
                "image",
                "text",
            ]
            .iter()
            .position(|candidate| *candidate == key)
            .expect("parent node order"),
        )
        .expect("node index")
    });
    if node.parent.is_some() {
        debug_assert!(parent < index as u32);
        debug_assert_eq!(node.attachment_kind, 4);
    }

    let geometry = geometry_ids
        .get(node.key)
        .map_or(NULL_INDEX, |id| geometry_indices[id]);
    let fill = paint_ids
        .get(node.key)
        .map_or(NULL_INDEX, |id| paint_indices[id]);
    let stroke = if node.key == "line" {
        stroke_indices
            .values()
            .copied()
            .next()
            .expect("line stroke")
    } else {
        NULL_INDEX
    };
    let clip = if node.key == "image-clip" {
        clip_indices.values().copied().next().expect("image clip")
    } else {
        NULL_INDEX
    };

    let mut payload = Vec::new();
    put_u64(&mut payload, node_ids[node.key]);
    put_u16(&mut payload, node.kind);
    put_u16(&mut payload, 0b11 | u16::from(node.isolated) << 2);
    put_u32(&mut payload, parent);
    put_u32(&mut payload, 0);
    put_u32(&mut payload, node.document_order);
    put_i32(&mut payload, 0);
    put_u16(&mut payload, node.attachment_kind);
    put_u16(&mut payload, 0);
    put_u64(&mut payload, node.attachment_id);
    put_f64(&mut payload, 0.0);
    put_f64(&mut payload, 0.0);
    put_u32(&mut payload, 2);
    put_u32(&mut payload, 2);
    put_u32(&mut payload, 3);
    put_u32(&mut payload, 4);
    put_u32(&mut payload, node.opacity_descriptor);
    put_u32(&mut payload, 12);
    put_u32(&mut payload, geometry);
    put_u32(&mut payload, fill);
    put_u32(&mut payload, stroke);
    put_u32(&mut payload, clip);
    put_u16(&mut payload, node.composite);
    put_u16(&mut payload, 0);
    payload.extend_from_slice(&empty_object());
    let encoded = record(payload);
    debug_assert_eq!(encoded.len(), 124);
    encoded
}

fn geometry_record(
    id: u64,
    geometry_ids: &BTreeMap<&str, u64>,
    clip_geometry_id: u64,
    path_indices: &BTreeMap<u64, u32>,
    glyph_indices: &BTreeMap<u64, u32>,
    strings: &BTreeMap<&str, u32>,
) -> Vec<u8> {
    let (kind, fields) = if id == geometry_ids["rect"] {
        (
            3,
            vec![
                ("originDescriptor", value_int(2)),
                ("sizeDescriptor", value_int(2)),
            ],
        )
    } else if id == geometry_ids["rounded"] {
        (
            4,
            vec![
                ("originDescriptor", value_int(2)),
                ("sizeDescriptor", value_int(2)),
                ("radiiDescriptors", value_int_array(&[13, 13, 13, 13])),
            ],
        )
    } else if id == geometry_ids["circle"] {
        (
            5,
            vec![
                ("centerDescriptor", value_int(2)),
                ("radiusDescriptor", value_int(13)),
            ],
        )
    } else if id == geometry_ids["ellipse"] {
        (
            6,
            vec![
                ("centerDescriptor", value_int(2)),
                ("radiusXDescriptor", value_int(13)),
                ("radiusYDescriptor", value_int(13)),
                ("rotationDescriptor", value_int(3)),
            ],
        )
    } else if id == geometry_ids["line"] {
        (
            7,
            vec![
                ("startDescriptor", value_int(2)),
                ("endDescriptor", value_int(2)),
            ],
        )
    } else if id == geometry_ids["polyline"] {
        (8, vec![("pointDescriptors", value_int_array(&[2, 2]))])
    } else if id == geometry_ids["polygon"] {
        (9, vec![("pointDescriptors", value_int_array(&[2, 2, 2]))])
    } else if id == geometry_ids["path"] {
        (
            10,
            vec![(
                "pathRef",
                value_int(i64::from(
                    path_indices.values().copied().next().expect("path"),
                )),
            )],
        )
    } else if id == geometry_ids["image"] {
        (
            11,
            vec![
                (
                    "resourceId",
                    value_resource(resource_id(PNG_RESOURCE_TEXT_ID)),
                ),
                ("destinationDescriptors", value_int_array(&[13, 13, 10, 10])),
                ("sampling", value_int(1)),
            ],
        )
    } else if id == geometry_ids["text"] {
        (
            12,
            vec![
                (
                    "glyphRunRefs",
                    value_int_array(&[
                        glyph_indices.values().copied().next().expect("glyph") as i64
                    ]),
                ),
                ("originDescriptor", value_int(2)),
            ],
        )
    } else {
        debug_assert_eq!(id, clip_geometry_id);
        (
            5,
            vec![
                ("centerDescriptor", value_int(2)),
                ("radiusDescriptor", value_int(10)),
            ],
        )
    };
    let mut payload = Vec::new();
    put_u64(&mut payload, id);
    put_u16(&mut payload, kind);
    put_u16(&mut payload, 0);
    payload.extend_from_slice(&value_object(&fields, strings));
    record(payload)
}

fn path_record(id: u64) -> Vec<u8> {
    let commands = [
        path_command(1, &[2], None),
        path_command(2, &[2], None),
        path_command(3, &[2, 2], None),
        path_command(4, &[2, 2, 2], None),
        path_command(5, &[2, 10, 3, 3], Some(1)),
        path_command(6, &[2, 10, 10, 3, 3, 3], Some(1)),
        path_command(7, &[], None),
    ];
    let mut payload = Vec::new();
    put_u64(&mut payload, id);
    put_u16(&mut payload, 0);
    put_u16(&mut payload, 1);
    put_u32(&mut payload, commands.len() as u32);
    for command in commands {
        payload.extend_from_slice(&command);
    }
    record(payload)
}

fn path_command(kind: u16, descriptors: &[u32], direction: Option<u16>) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u16(&mut payload, kind);
    put_u16(&mut payload, 0);
    for descriptor in descriptors {
        put_u32(&mut payload, *descriptor);
    }
    if let Some(direction) = direction {
        put_u16(&mut payload, direction);
        put_u16(&mut payload, 0);
    }
    record(payload)
}

fn paint_record(id: u64, paint_ids: &BTreeMap<&str, u64>) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u64(&mut payload, id);
    if id == paint_ids["rounded"] {
        put_u16(&mut payload, 2);
        put_u16(&mut payload, 0);
        put_u32(&mut payload, 2);
        put_u32(&mut payload, 2);
        put_u16(&mut payload, 1);
        put_u16(&mut payload, 0);
        gradient_stops(&mut payload);
    } else if id == paint_ids["circle"] {
        put_u16(&mut payload, 3);
        put_u16(&mut payload, 0);
        put_u32(&mut payload, 2);
        put_u32(&mut payload, 13);
        put_u32(&mut payload, 2);
        put_u32(&mut payload, 13);
        put_u16(&mut payload, 1);
        put_u16(&mut payload, 0);
        gradient_stops(&mut payload);
    } else if id == paint_ids["ellipse"] {
        put_u16(&mut payload, 4);
        put_u16(&mut payload, 0);
        put_u64(&mut payload, resource_id(WEBP_RESOURCE_TEXT_ID));
        put_u32(&mut payload, 2);
        put_u32(&mut payload, 2);
        put_u32(&mut payload, 3);
        put_u32(&mut payload, 4);
        put_u16(&mut payload, 4);
        put_u16(&mut payload, 1);
    } else {
        put_u16(&mut payload, 1);
        put_u16(&mut payload, 0);
        put_u32(&mut payload, 9);
    }
    record(payload)
}

fn gradient_stops(payload: &mut Vec<u8>) {
    put_u32(payload, 2);
    put_f64(payload, 0.0);
    put_u32(payload, 9);
    put_u32(payload, 0);
    put_f64(payload, 1.0);
    put_u32(payload, 9);
    put_u32(payload, 0);
}

fn stroke_record(id: u64, paint_ref: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u64(&mut payload, id);
    put_u16(&mut payload, 0);
    put_u16(&mut payload, 0);
    put_u32(&mut payload, paint_ref);
    put_u32(&mut payload, 13);
    put_u16(&mut payload, 1);
    put_u16(&mut payload, 1);
    put_f64(&mut payload, 2.0);
    put_u32(&mut payload, 13);
    put_u32(&mut payload, 2);
    put_f64(&mut payload, 0.5);
    put_f64(&mut payload, 0.5);
    let encoded = record(payload);
    debug_assert_eq!(encoded.len(), 64);
    encoded
}

fn clip_record(id: u64, geometry_ref: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u64(&mut payload, id);
    put_u16(&mut payload, 0);
    put_u16(&mut payload, 1);
    put_u32(&mut payload, geometry_ref);
    let encoded = record(payload);
    debug_assert_eq!(encoded.len(), 24);
    encoded
}

fn glyph_record(id: u64) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u64(&mut payload, id);
    put_u64(&mut payload, resource_id(FONT_RESOURCE_TEXT_ID));
    put_u32(&mut payload, 0);
    put_u16(&mut payload, 0);
    put_u16(&mut payload, 1);
    put_u32(&mut payload, 10);
    put_f64(&mut payload, 0.0);
    put_f64(&mut payload, 0.0);
    put_u32(&mut payload, 1);
    put_u32(&mut payload, 0);
    put_u32(&mut payload, 1);
    put_u32(&mut payload, 0);
    put_f64(&mut payload, 1.0);
    put_f64(&mut payload, 0.0);
    put_f64(&mut payload, 0.0);
    put_f64(&mut payload, 0.0);
    let encoded = record(payload);
    debug_assert_eq!(encoded.len(), 100);
    encoded
}

fn value_object(fields: &[(&str, Vec<u8>)], strings: &BTreeMap<&str, u32>) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u32(&mut payload, fields.len() as u32);
    for (key, encoded_value) in fields {
        put_u32(&mut payload, strings[key]);
        payload.extend_from_slice(encoded_value);
    }
    value(14, payload)
}

fn empty_object() -> Vec<u8> {
    value(14, 0u32.to_le_bytes().to_vec())
}

fn value_string(reference: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    put_u32(&mut payload, reference);
    put_u32(&mut payload, 0);
    value(4, payload)
}

fn value_int(value_: i64) -> Vec<u8> {
    value(2, value_.to_le_bytes().to_vec())
}

fn value_resource(id: u64) -> Vec<u8> {
    value(11, id.to_le_bytes().to_vec())
}

fn value_int_array(values: &[i64]) -> Vec<u8> {
    let mut payload = vec![2, 0, 0, 0];
    put_u32(&mut payload, values.len() as u32);
    for value_ in values {
        payload.extend_from_slice(&value_int(*value_));
    }
    value(13, payload)
}

fn value(tag: u8, payload: Vec<u8>) -> Vec<u8> {
    let mut bytes = Vec::new();
    put_u8(&mut bytes, tag);
    put_u8(&mut bytes, 0);
    put_u16(&mut bytes, 0);
    put_u32(&mut bytes, payload.len() as u32);
    bytes.extend_from_slice(&payload);
    pad_to(&mut bytes, 8);
    bytes
}

fn counted_bytes(bytes_: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    put_u32(&mut bytes, bytes_.len() as u32);
    bytes.extend_from_slice(bytes_);
    pad_to(&mut bytes, 4);
    bytes
}

fn record(mut payload: Vec<u8>) -> Vec<u8> {
    while !(payload.len() + 8).is_multiple_of(4) {
        payload.push(0);
    }
    let mut bytes = Vec::with_capacity(payload.len() + 8);
    put_u32(&mut bytes, (payload.len() + 8) as u32);
    put_u16(&mut bytes, 1);
    put_u16(&mut bytes, 0);
    bytes.extend_from_slice(&payload);
    bytes
}

fn write_section_table(bytes: &mut [u8], sections: &[Section]) {
    for (index, section) in sections.iter().enumerate() {
        let start = 128 + index * 40;
        write_u32_at(bytes, start, section.kind);
        write_u16_at(bytes, start + 4, 1);
        write_u16_at(bytes, start + 6, 0);
        write_u16_at(bytes, start + 8, 0);
        write_u16_at(bytes, start + 10, REQUIRED);
        bytes[start + 12] = 3;
        bytes[start + 13..start + 16].fill(0);
        write_u64_at(bytes, start + 16, section.offset);
        write_u64_at(bytes, start + 24, section.payload.len() as u64);
        write_u32_at(bytes, start + 32, crc32_iso_hdlc(&section.payload));
        write_u32_at(bytes, start + 36, 0);
    }
}

fn crc32_iso_hdlc(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn align_up(value_: usize, alignment: usize) -> usize {
    (value_ + alignment - 1) & !(alignment - 1)
}

fn pad_to(bytes: &mut Vec<u8>, alignment: usize) {
    bytes.resize(align_up(bytes.len(), alignment), 0);
}

fn put_u8(bytes: &mut Vec<u8>, value_: u8) {
    bytes.push(value_);
}

fn put_u16(bytes: &mut Vec<u8>, value_: u16) {
    bytes.extend_from_slice(&value_.to_le_bytes());
}

fn put_u32(bytes: &mut Vec<u8>, value_: u32) {
    bytes.extend_from_slice(&value_.to_le_bytes());
}

fn put_i32(bytes: &mut Vec<u8>, value_: i32) {
    bytes.extend_from_slice(&value_.to_le_bytes());
}

fn put_u64(bytes: &mut Vec<u8>, value_: u64) {
    bytes.extend_from_slice(&value_.to_le_bytes());
}

fn put_f64(bytes: &mut Vec<u8>, value_: f64) {
    bytes.extend_from_slice(&value_.to_bits().to_le_bytes());
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32 field"))
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64 field"))
}

fn write_u16_at(bytes: &mut [u8], offset: usize, value_: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value_.to_le_bytes());
}

fn write_u32_at(bytes: &mut [u8], offset: usize, value_: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value_.to_le_bytes());
}

fn write_u64_at(bytes: &mut [u8], offset: usize, value_: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value_.to_le_bytes());
}
