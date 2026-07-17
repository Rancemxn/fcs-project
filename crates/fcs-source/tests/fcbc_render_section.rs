use std::fs;
use std::path::{Path, PathBuf};

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
use fcbc_reference_loader::{DistanceClassification, RuntimeValue, ValueType};
use fcbc_render_reference_assets::{
    PNG_PIXELS, WEBP_PIXELS, build_test_font, encode_test_png, encode_test_webp, shape_simple_ltr,
};
use fcbc_render_reference_writer::{
    FONT_RESOURCE_TEXT_ID, PNG_RESOURCE_TEXT_ID, RenderAssets, WEBP_RESOURCE_TEXT_ID, resource_id,
    write_nonempty_render,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
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

fn mutate_render_section(mut bytes: Vec<u8>, mutate: impl FnOnce(&mut [u8])) -> Vec<u8> {
    let section_count = u32_at(&bytes, 36) as usize;
    let table_offset = u64_at(&bytes, 40) as usize;
    let entry = (0..section_count)
        .map(|index| table_offset + index * 40)
        .find(|entry| u32_at(&bytes, *entry) == 14)
        .expect("Render section entry");
    let offset = u64_at(&bytes, entry + 16) as usize;
    let length = u64_at(&bytes, entry + 24) as usize;
    mutate(&mut bytes[offset..offset + length]);
    let checksum = crc32_iso_hdlc(&bytes[offset..offset + length]);
    bytes[entry + 32..entry + 36].copy_from_slice(&checksum.to_le_bytes());
    bytes
}

fn first_node_record_offset(section: &[u8]) -> usize {
    let layer_count = u32_at(section, 36) as usize;
    let mut offset = 68;
    for _ in 0..layer_count {
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
fn independent_render_loader_validates_all_tables_codecs_and_shaping() {
    let chart = fcbc_render_reference_loader::load_render(&generated_fixture())
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

    assert_eq!(
        chart.decoded_images[&resource_id(PNG_RESOURCE_TEXT_ID)].rgba8,
        PNG_PIXELS
    );
    assert_eq!(
        chart.decoded_images[&resource_id(WEBP_RESOURCE_TEXT_ID)].rgba8,
        WEBP_PIXELS
    );
    let font = &chart.decoded_fonts[&resource_id(FONT_RESOURCE_TEXT_ID)];
    let shaped = shape_simple_ltr(font, "A").expect("simple-ltr-1 shaping");
    assert_eq!(shaped.len(), 1);
    assert_eq!(shaped[0].glyph_id, 1);
    assert_eq!(shaped[0].x_advance.to_bits(), 1.0f64.to_bits());
    assert_eq!(shaped[0].y_advance.to_bits(), 0.0f64.to_bits());
    assert_eq!(shaped[0].x_offset.to_bits(), 0.0f64.to_bits());
    assert_eq!(shaped[0].y_offset.to_bits(), 0.0f64.to_bits());
    assert_eq!(chart.glyph_runs[0].glyphs[0].glyph_id, shaped[0].glyph_id);
    assert_eq!(
        chart.glyph_runs[0].glyphs[0].x_advance.to_bits(),
        shaped[0].x_advance.to_bits()
    );
}

#[test]
fn glyph_run_semantic_ids_use_invalid_geometry_after_font_decode() {
    let cases = [
        ("glyph zero", 60usize, 0u32),
        ("glyph at numGlyphs", 60usize, 2u32),
        ("nonzero face", 24usize, 1u32),
    ];
    for (name, relative_offset, value) in cases {
        let bytes = mutate_render_section(generated_fixture(), |section| {
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
fn node_kind_and_attachment_fail_at_render_owned_categories() {
    let invalid_kind = mutate_render_section(generated_fixture(), |section| {
        let node = first_node_record_offset(section);
        section[node + 16..node + 18].copy_from_slice(&99u16.to_le_bytes());
    });
    assert_eq!(
        fcbc_render_reference_loader::load_render(&invalid_kind),
        Err("render.invalid-geometry")
    );

    let invalid_attachment = mutate_render_section(generated_fixture(), |section| {
        let node = first_node_record_offset(section);
        section[node + 36..node + 38].copy_from_slice(&99u16.to_le_bytes());
    });
    assert_eq!(
        fcbc_render_reference_loader::load_render(&invalid_attachment),
        Err("render.invalid-reference")
    );

    let follow_hidden_world = mutate_render_section(generated_fixture(), |section| {
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
