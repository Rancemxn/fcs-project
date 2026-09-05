use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

#[path = "support/fcbc_reference_evaluator.rs"]
mod fcbc_reference_evaluator;
#[path = "support/fcbc_reference_loader.rs"]
mod fcbc_reference_loader;
#[path = "support/fcbc_reference_writer.rs"]
mod fcbc_reference_writer;
#[path = "support/fcbc_render_reference_assets.rs"]
mod fcbc_render_reference_assets;
#[path = "support/fcbc_render_reference_loader.rs"]
mod fcbc_render_reference_loader;
#[path = "support/fcbc_render_reference_writer.rs"]
mod fcbc_render_reference_writer;

use fcbc_reference_evaluator::{
    EvaluationEnvironment, query_descriptor, query_distance, query_scroll_coordinate,
};
use fcbc_reference_loader::{DescriptorKind, DistanceClassification, RuntimeValue, ValueType};
use fcbc_render_reference_assets::{
    PNG_PIXELS, WEBP_PIXELS, build_test_font, encode_test_png, encode_test_webp, shape_simple_ltr,
};
use fcbc_render_reference_loader::{PaintData, ParsedValue, PathCommand};
use fcbc_render_reference_writer::{
    FONT_RESOURCE_TEXT_ID, PNG_RESOURCE_TEXT_ID, RenderAssets, WEBP_RESOURCE_TEXT_ID, resource_id,
    write_nonempty_render,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn render_golden_path() -> PathBuf {
    repository_root().join("docs/conformance/render/nonempty-render.hex")
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderGolden {
    schema_version: u32,
    id: String,
    fcs_version: String,
    fcbc_version: String,
    execution_abi_version: String,
    render_profile_version: String,
    path: String,
    expect: String,
    decoded_length: usize,
    sha256: String,
    viewport_width_bits: String,
    viewport_height_bits: String,
    viewport_color_space: u16,
    table_counts: [usize; 8],
    section: Vec<GoldenSection>,
    resource: Vec<GoldenResource>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenSection {
    r#type: u32,
    offset: u64,
    length: u64,
    crc32: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenResource {
    canonical_textual_id: String,
    id: String,
    kind: u16,
    media_type: String,
    data_offset: u64,
    data_length: u64,
    sha256: String,
    asset: String,
    metadata: BTreeMap<String, toml::Value>,
}

fn load_toml<T: DeserializeOwned>(path: &Path) -> T {
    toml::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

fn static_fixture() -> Vec<u8> {
    decode_hex_file(&render_golden_path())
}

fn decode_hex_file(path: &Path) -> Vec<u8> {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    decode_lower_hex(&source, &path.display().to_string())
}

fn decode_lower_hex(source: &str, description: &str) -> Vec<u8> {
    let compact: String = source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert!(
        !compact.is_empty()
            && compact.len().is_multiple_of(2)
            && compact
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{description} must be nonempty even-length lowercase hex"
    );
    compact
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).expect("ASCII hex"), 16).unwrap())
        .collect()
}

fn encode_hex_lines(bytes: &[u8]) -> String {
    let mut output = String::new();
    for (index, byte) in bytes.iter().enumerate() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
        if index % 48 == 47 || index + 1 == bytes.len() {
            output.push('\n');
        }
    }
    output
}

fn static_golden_or_dump(path: &Path, generated: &[u8]) -> Vec<u8> {
    if path.is_file() {
        return decode_hex_file(path);
    }
    panic!(
        "static golden {} is missing; candidate bytes follow ({} bytes):\n{}",
        path.display(),
        generated.len(),
        encode_hex_lines(generated)
    );
}

fn generated_fixture() -> Vec<u8> {
    let png = encode_test_png();
    let webp = encode_test_webp();
    let font = build_test_font();
    let malformed =
        include_bytes!("../../../docs/conformance/render/binding/assets/opaque-image.bin");
    write_nonempty_render(
        &fcbc_reference_writer::write_nonempty_execution(),
        RenderAssets {
            png: &png,
            webp: &webp,
            font: &font,
            malformed,
        },
    )
}

#[test]
fn binary_golden_matches_reference_writer_byte_for_byte() {
    let generated = generated_fixture();
    let golden = static_golden_or_dump(&render_golden_path(), &generated);
    assert_eq!(generated, golden);
}

#[test]
fn static_render_manifest_pins_sections_resources_and_tables() {
    let root = repository_root().join("docs/conformance/render");
    let suite: toml::Value = load_toml(&root.join("manifest.toml"));
    let entries = suite["binary_fixture"].as_array().unwrap();
    assert!(!entries.is_empty());
    for entry in entries {
        let golden: RenderGolden = load_toml(&root.join(entry["manifest"].as_str().unwrap()));
        assert_eq!(golden.schema_version, 1);
        assert_eq!(golden.id, entry["id"].as_str().unwrap());
        assert_eq!(golden.fcs_version, "5.0.0");
        assert_eq!(golden.fcbc_version, "2.0.0");
        assert_eq!(golden.execution_abi_version, "1.0.0");
        assert_eq!(golden.render_profile_version, "1.0.0");
        assert_eq!(golden.expect, "success");
        let bytes = decode_hex_file(&root.join(&golden.path));
        assert_eq!(bytes.len(), golden.decoded_length);
        assert_eq!(sha256_hex(&bytes), golden.sha256);
        let chart = fcbc_render_reference_loader::load_render(&bytes).unwrap();
        assert_eq!(
            format!("{:016x}", chart.viewport_width.to_bits()),
            golden.viewport_width_bits
        );
        assert_eq!(
            format!("{:016x}", chart.viewport_height.to_bits()),
            golden.viewport_height_bits
        );
        assert_eq!(chart.viewport_color_space, golden.viewport_color_space);
        assert_eq!(
            [
                chart.layers.len(),
                chart.nodes.len(),
                chart.geometries.len(),
                chart.paths.len(),
                chart.paints.len(),
                chart.strokes.len(),
                chart.clips.len(),
                chart.glyph_runs.len()
            ],
            golden.table_counts,
        );
        assert!(golden.table_counts.iter().all(|count| *count > 0));
        assert_eq!(golden.section.len(), chart.core.sections.len());
        assert_eq!(
            golden
                .section
                .iter()
                .map(|section| section.r#type)
                .collect::<Vec<_>>(),
            (1..=14).chain(std::iter::once(20)).collect::<Vec<_>>(),
        );
        for (expected, actual) in golden.section.iter().zip(&chart.core.sections) {
            assert_eq!(actual.section_type, expected.r#type);
            assert_eq!(actual.offset, expected.offset);
            assert_eq!(actual.length, expected.length);
            assert_eq!(format!("{:08x}", actual.checksum), expected.crc32);
            let payload = &bytes[actual.offset as usize..(actual.offset + actual.length) as usize];
            assert_eq!(crc32_iso_hdlc(payload), actual.checksum);
        }
        assert_eq!(golden.resource.len(), chart.resources.len());
        for (expected, actual) in golden.resource.iter().zip(&chart.resources) {
            assert_eq!(format!("{:016x}", actual.id), expected.id);
            assert_eq!(actual.id, resource_id(&expected.canonical_textual_id));
            assert_eq!(actual.kind, expected.kind);
            assert_eq!(actual.media_type, expected.media_type);
            assert_eq!(actual.data_offset, expected.data_offset);
            assert_eq!(actual.data_length, expected.data_length);
            assert_eq!(actual.data.len() as u64, expected.data_length);
            assert_eq!(sha256_hex(&actual.data), expected.sha256);
            assert_eq!(actual.data, fs::read(root.join(&expected.asset)).unwrap());
            let ParsedValue::Object(fields) = &actual.metadata else {
                panic!("resource metadata must be an object");
            };
            let metadata: BTreeMap<_, _> = fields
                .iter()
                .map(|(key, value)| {
                    let value = match value {
                        ParsedValue::String(index) => {
                            toml::Value::String(chart.core.strings[*index as usize].clone())
                        }
                        ParsedValue::Int(value) => toml::Value::Integer(*value),
                        _ => panic!("unexpected fixture metadata value"),
                    };
                    (chart.core.strings[*key as usize].clone(), value)
                })
                .collect();
            assert_eq!(metadata, expected.metadata);
        }
    }
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32 bytes"))
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64 bytes"))
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

fn section_entry(bytes: &[u8], section_type: u32) -> usize {
    let section_count = u32_at(bytes, 36) as usize;
    let table_offset = u64_at(bytes, 40) as usize;
    (0..section_count)
        .map(|index| table_offset + index * 40)
        .find(|entry| u32_at(bytes, *entry) == section_type)
        .expect("section entry")
}

fn mutate_section(
    mut bytes: Vec<u8>,
    section_type: u32,
    mutate: impl FnOnce(&mut [u8]),
) -> Vec<u8> {
    let entry = section_entry(&bytes, section_type);
    let offset = u64_at(&bytes, entry + 16) as usize;
    let length = u64_at(&bytes, entry + 24) as usize;
    mutate(&mut bytes[offset..offset + length]);
    let checksum = crc32_iso_hdlc(&bytes[offset..offset + length]);
    bytes[entry + 32..entry + 36].copy_from_slice(&checksum.to_le_bytes());
    bytes
}

fn mutate_render_section(bytes: Vec<u8>, mutate: impl FnOnce(&mut [u8])) -> Vec<u8> {
    mutate_section(bytes, 14, mutate)
}

#[test]
fn independent_core_loader_checks_resource_coverage_before_hash() {
    let bytes = mutate_section(static_fixture(), 6, |section| {
        let count = u32_at(section, 0) as usize;
        let mut record_offset = 4;
        let mut data_length_offset = None;
        for _ in 0..count {
            let record_length = u32_at(section, record_offset) as usize;
            assert!(
                record_length >= 44,
                "resource record must contain data length"
            );
            data_length_offset = Some(record_offset + 36);
            record_offset += record_length;
        }
        assert_eq!(
            record_offset,
            section.len(),
            "resource records must cover section"
        );
        let data_length_offset = data_length_offset.expect("fixture must contain resources");
        let data_length = u64_at(section, data_length_offset);
        assert!(data_length > 0, "fixture resource must contain data");
        section[data_length_offset..data_length_offset + 8]
            .copy_from_slice(&(data_length - 1).to_le_bytes());
    });
    assert_eq!(
        fcbc_reference_loader::load(&bytes),
        Err("fcbc.invalid-resource-data")
    );
}

fn first_node_record_offset(section: &[u8]) -> usize {
    let mut offset = 68usize;
    for _ in 0..u32_at(section, 36) {
        offset += u32_at(section, offset) as usize;
    }
    offset
}

#[test]
fn checked_in_project_assets_match_deterministic_generators() {
    let assets = repository_root().join("docs/conformance/render/assets");
    let generated = [
        ("fcs-test-rgba8.png", encode_test_png()),
        ("fcs-test-lossless.webp", encode_test_webp()),
        ("fcs-test-font.ttf", build_test_font()),
    ];
    if std::env::var_os("FCS_REGENERATE_RENDER_ASSETS").is_some() {
        fs::create_dir_all(&assets).expect("create Render asset directory");
        for (name, bytes) in &generated {
            fs::write(assets.join(name), bytes).expect("write generated Render asset");
        }
    }
    for (name, expected) in generated {
        let path = assets.join(name);
        let actual = fs::read(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        assert_eq!(actual, expected, "generated asset drift: {name}");
    }
}

#[test]
fn render_writer_produces_a_core_valid_self_contained_container() {
    let bytes = generated_fixture();
    let chart = fcbc_reference_loader::load(&bytes).expect("Render fixture Core envelope");
    assert_eq!(chart.feature_flags & (1 << 1), 1 << 1);
    assert_eq!(chart.notes.len(), 3);
    assert_eq!(chart.sections.len(), 15);
    assert_eq!(
        chart
            .sections
            .iter()
            .map(|section| section.section_type)
            .collect::<Vec<_>>(),
        (1..=14).chain(std::iter::once(20)).collect::<Vec<_>>()
    );
    assert!(repository_root().join("docs/conformance/render").is_dir());

    let identity = query_descriptor(&chart, 8, 0.0, EvaluationEnvironment::at_time(0.0))
        .expect("Render fixture Core identity descriptor");
    assert_eq!(
        identity.value,
        RuntimeValue::Scalar {
            ty: ValueType::Float,
            value: 1.0,
        }
    );

    let distance = query_distance(&chart, 0, 0.0).expect("Render fixture Core distance");
    assert_eq!(distance.floor_position.to_bits(), 20.0f64.to_bits());
    assert_eq!(
        distance.classification,
        DistanceClassification::PortableEvaluable
    );
    assert!(distance.visited_nodes.is_empty());

    let line = &chart.lines[0];
    assert_eq!(
        query_scroll_coordinate(&chart, line.scroll_tempo_descriptor, 2.0)
            .expect("Render fixture Core scroll coordinate")
            .to_bits(),
        2.0f64.to_bits()
    );
}

#[test]
fn independent_render_loader_validates_static_golden_tables_codecs_and_shaping() {
    let chart = fcbc_render_reference_loader::load_render(&static_fixture())
        .expect("independent Render loader");
    assert_eq!(chart.layers.len(), 1);
    assert_eq!(chart.nodes.len(), 13);
    assert_eq!(chart.geometries.len(), 11);
    assert_eq!(chart.paths.len(), 1);
    assert_eq!(chart.paints.len(), 9);
    assert_eq!(chart.strokes.len(), 1);
    assert_eq!(chart.clips.len(), 1);
    assert_eq!(chart.glyph_runs.len(), 1);
    assert_eq!(chart.resources.len(), 5);

    let mut node_kinds: Vec<_> = chart.nodes.iter().map(|node| node.kind as u16).collect();
    node_kinds.sort_unstable();
    node_kinds.dedup();
    assert_eq!(node_kinds, (1..=12).collect::<Vec<_>>());

    let mut geometry_kinds: Vec<_> = chart
        .geometries
        .iter()
        .map(|geometry| geometry.kind as u16)
        .collect();
    geometry_kinds.sort_unstable();
    geometry_kinds.dedup();
    assert_eq!(geometry_kinds, (3..=12).collect::<Vec<_>>());

    let path_kinds: Vec<_> = chart.paths[0]
        .commands
        .iter()
        .map(|command| match command {
            PathCommand::MoveTo(_) => 1,
            PathCommand::LineTo(_) => 2,
            PathCommand::QuadraticTo(_, _) => 3,
            PathCommand::CubicTo(_, _, _) => 4,
            PathCommand::Arc { .. } => 5,
            PathCommand::EllipseArc { .. } => 6,
            PathCommand::Close => 7,
        })
        .collect();
    assert_eq!(path_kinds, (1..=7).collect::<Vec<_>>());

    let mut paint_kinds: Vec<_> = chart
        .paints
        .iter()
        .map(|paint| match paint.data {
            PaintData::Solid { .. } => 1,
            PaintData::LinearGradient { .. } => 2,
            PaintData::RadialGradient { .. } => 3,
            PaintData::ImagePattern { .. } => 4,
        })
        .collect();
    paint_kinds.sort_unstable();
    paint_kinds.dedup();
    assert_eq!(paint_kinds, [1, 2, 3, 4]);

    let node_roots: Vec<_> = chart
        .nodes
        .iter()
        .flat_map(|node| {
            [
                node.position_descriptor,
                node.origin_descriptor,
                node.rotation_descriptor,
                node.scale_descriptor,
                node.opacity_descriptor,
                node.visibility_descriptor,
            ]
        })
        .collect();
    for root in [2, 4, 5, 11] {
        assert!(node_roots.contains(&root), "missing direct root {root}");
    }
    assert!(matches!(
        &chart.core.descriptors[4].kind,
        DescriptorKind::Constant(_)
    ));
    assert!(matches!(
        &chart.core.descriptors[5].kind,
        DescriptorKind::SegmentTrack(_)
    ));
    assert!(matches!(
        &chart.core.descriptors[11].kind,
        DescriptorKind::Piecewise(_)
    ));
    assert!(matches!(
        &chart.core.descriptors[2].kind,
        DescriptorKind::Expression(_)
    ));

    assert_eq!(
        chart.decoded_images[&resource_id(PNG_RESOURCE_TEXT_ID)].rgba8,
        PNG_PIXELS
    );
    assert_eq!(
        chart.decoded_images[&resource_id(WEBP_RESOURCE_TEXT_ID)].rgba8,
        WEBP_PIXELS
    );
    let font = &chart.decoded_fonts[&resource_id(FONT_RESOURCE_TEXT_ID)];
    let root = repository_root().join("docs/conformance/render");
    let suite: toml::Value = load_toml(&root.join("manifest.toml"));
    let fixture = suite["binary_fixture"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"].as_str() == Some("render.nonempty-render"))
        .unwrap();
    let vector: toml::Value = load_toml(&root.join(fixture["vector"].as_str().unwrap()));
    let shaping = &vector["shaping"];
    assert_eq!(shaping["profile"].as_str(), Some("simple-ltr-1"));
    assert_eq!(
        shaping["font_resource_id"].as_str().unwrap(),
        format!("{:016x}", resource_id(FONT_RESOURCE_TEXT_ID))
    );
    let shaped = shape_simple_ltr(font, shaping["input"].as_str().unwrap()).unwrap();
    let expected = shaping["glyph"].as_array().unwrap();
    assert_eq!(shaped.len(), expected.len());
    assert_eq!(chart.glyph_runs[0].glyphs.len(), expected.len());
    for ((actual, placed), expected) in shaped.iter().zip(&chart.glyph_runs[0].glyphs).zip(expected)
    {
        assert_eq!(
            actual.glyph_id as i64,
            expected["glyph_id"].as_integer().unwrap()
        );
        assert_eq!(placed.glyph_id, actual.glyph_id);
        for (field, value, placed_value) in [
            ("x_advance_bits", actual.x_advance, placed.x_advance),
            ("y_advance_bits", actual.y_advance, placed.y_advance),
            ("x_offset_bits", actual.x_offset, placed.x_offset),
            ("y_offset_bits", actual.y_offset, placed.y_offset),
        ] {
            assert_eq!(
                format!("{:016x}", value.to_bits()),
                expected[field].as_str().unwrap()
            );
            assert_eq!(placed_value.to_bits(), value.to_bits());
        }
    }
}

#[test]
fn static_render_deep_mutations_keep_stable_categories() {
    let root = repository_root().join("docs/conformance/render");
    let suite: toml::Value = load_toml(&root.join("manifest.toml"));
    for fixture in suite["binary_fixture"].as_array().unwrap() {
        let mutations: toml::Value = load_toml(&root.join(fixture["mutations"].as_str().unwrap()));
        assert_eq!(mutations["schema_version"].as_integer(), Some(1));
        let bytes = decode_hex_file(&root.join(mutations["base"].as_str().unwrap()));
        let cases = mutations["mutation"].as_array().unwrap();
        assert!(!cases.is_empty());
        for case in cases {
            let id = case["id"].as_str().unwrap();
            let mut mutated = bytes.clone();
            let patches = case["patch"].as_array().unwrap();
            assert!(!patches.is_empty());
            let mut end = 0;
            for patch in patches {
                let offset = usize::try_from(patch["offset"].as_integer().unwrap()).unwrap();
                let replacement = decode_lower_hex(patch["replace_hex"].as_str().unwrap(), id);
                assert!(
                    offset >= end && offset + replacement.len() <= bytes.len(),
                    "{id}: patch range"
                );
                end = offset + replacement.len();
                mutated[offset..end].copy_from_slice(&replacement);
            }
            assert_eq!(
                fcbc_render_reference_loader::load_render(&mutated),
                Err(case["diagnostic"].as_str().unwrap()),
                "{id}",
            );
        }
    }
}

#[test]
fn glyph_run_semantic_ids_use_invalid_geometry_after_font_decode() {
    let cases = [
        ("glyph zero", 60usize, 0u32),
        ("glyph at numGlyphs", 60usize, 2u32),
        ("nonzero face", 24usize, 1u32),
    ];
    for (name, relative_offset, value) in cases {
        let bytes = mutate_render_section(static_fixture(), |section| {
            let glyph_record = section.len() - 100;
            assert_eq!(u32_at(section, glyph_record), 100);
            section[glyph_record + relative_offset..glyph_record + relative_offset + 4]
                .copy_from_slice(&value.to_le_bytes());
        });
        assert_eq!(
            fcbc_render_reference_loader::load_render(&bytes),
            Err("render.invalid-geometry"),
            "{name}"
        );
    }
}

#[test]
fn node_attachment_failures_use_the_render_owned_category() {
    let invalid_attachment = mutate_render_section(static_fixture(), |section| {
        let node = first_node_record_offset(section);
        section[node + 36..node + 38].copy_from_slice(&99u16.to_le_bytes());
    });
    assert_eq!(
        fcbc_render_reference_loader::load_render(&invalid_attachment),
        Err("render.invalid-reference")
    );

    let follow_hidden_world = mutate_render_section(static_fixture(), |section| {
        let node = first_node_record_offset(section);
        let flags = u16::from_le_bytes(section[node + 18..node + 20].try_into().expect("flags"));
        section[node + 18..node + 20].copy_from_slice(&(flags | (1 << 3)).to_le_bytes());
        section[node + 36..node + 38].copy_from_slice(&1u16.to_le_bytes());
        section[node + 40..node + 48].copy_from_slice(&0u64.to_le_bytes());
    });
    assert_eq!(
        fcbc_render_reference_loader::load_render(&follow_hidden_world),
        Err("render.invalid-reference")
    );
}
