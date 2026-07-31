use fcs_fcbc::write_from_compilation;
use fcs_source::ResourceLimits;
use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;

use fcs_render::{NodeKind, load_render};

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32"))
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64"))
}

fn mutate_render_section(mut bytes: Vec<u8>, mutate: impl FnOnce(&mut [u8])) -> Vec<u8> {
    let section_count = u32_at(&bytes, 36) as usize;
    let table_offset = u64_at(&bytes, 40) as usize;
    let entry = (0..section_count)
        .map(|index| table_offset + index * 40)
        .find(|entry| u32_at(&bytes, *entry) == 14)
        .expect("Render section");
    let offset = u64_at(&bytes, entry + 16) as usize;
    let length = u64_at(&bytes, entry + 24) as usize;
    mutate(&mut bytes[offset..offset + length]);
    let checksum = fcs_fcbc::section_crc32_iso_hdlc(&bytes[offset..offset + length]);
    bytes[entry + 32..entry + 36].copy_from_slice(&checksum.to_le_bytes());
    bytes
}

fn node_record_offset(section: &[u8]) -> usize {
    let mut offset = 68;
    for _ in 0..u32_at(section, 36) {
        offset += u32_at(section, offset) as usize;
    }
    offset
}

#[test]
fn solid_rect_source_reaches_product_render_loader() {
    let source = include_str!("../../../docs/conformance/render/solid-rect-4x4.fcs");
    let document = parse_document(source)
        .into_result()
        .expect("Render conformance source parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| panic!("Render canonical lowering failed: {diagnostics:?}"));
    let bytes = write_from_compilation(&compilation).expect("Render FCBC writing");
    let render = load_render(&bytes).expect("product Render loader");

    assert_eq!(render.viewport_width, 4.0);
    assert_eq!(render.viewport_height, 4.0);
    assert_eq!(render.layers.len(), 1);
    assert_eq!(render.nodes.len(), 1);
    assert_eq!(render.nodes[0].kind, NodeKind::Rect);
    assert_eq!(render.geometries.len(), 1);
    assert_eq!(render.paints.len(), 1);

    let scene = compilation
        .chart()
        .render()
        .expect("canonical Render scene");
    assert_eq!(render.layers[0].id, scene.layers()[0].id().value());
    assert_eq!(render.nodes[0].id, scene.nodes()[0].id().value());
    assert_eq!(render.geometries[0].id, scene.geometries()[0].id().value());
    assert_eq!(render.paints[0].id, scene.paints()[0].id().value());
    let roots = compilation
        .chart()
        .descriptors()
        .expect("Render descriptors")
        .roots();
    assert!(roots.iter().any(|root| {
        root.target_path() == "render.node.position"
            && root.owner() == scene.nodes()[0].id().value()
    }));
    assert!(roots.iter().any(|root| {
        root.target_path() == "render.geometry.size"
            && root.owner() == scene.geometries()[0].id().value()
    }));

    let malformed = mutate_render_section(bytes, |section| {
        let node = node_record_offset(section);
        // Node record: geometryRef follows the six descriptor references.
        section[node + 88..node + 92].copy_from_slice(&u32::MAX.to_le_bytes());
    });
    assert_eq!(load_render(&malformed), Err("render.invalid-reference"));
}

#[test]
fn nested_group_and_solid_shapes_reach_product_render_loader() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport {
        width: 4px;
        height: 4px;
        colorSpace: "linear-srgb";
    }
    layer main {
        pass: "overlay";
        space: "screen";
        children {
            group container {
                children {
                    rect full {
                        origin: vec2(-2px, -2px);
                        size: vec2(4px, 4px);
                        fill: solid(#FF0000FF);
                    }
                    roundedRect card {
                        origin: vec2(-1px, -1px);
                        size: vec2(2px, 2px);
                        radius: 0.5px;
                        fill: solid(#00FF00FF);
                    }
                    circle dot {
                        center: vec2(0px, 0px);
                        radius: 1px;
                        fill: solid(#0000FFFF);
                    }
                    ellipse oval {
                        center: vec2(0px, 0px);
                        radiusX: 1px;
                        radiusY: 0.5px;
                        rotation: 0rad;
                        fill: solid(#FFFFFFFF);
                    }
                }
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("nested Render source parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| panic!("nested Render lowering failed: {diagnostics:?}"));
    let bytes = write_from_compilation(&compilation).expect("nested Render FCBC writing");
    let render = load_render(&bytes).expect("nested product Render loader");

    assert_eq!(render.nodes.len(), 5);
    assert_eq!(render.nodes[0].kind, NodeKind::Group);
    assert_eq!(render.nodes[1].kind, NodeKind::Rect);
    assert_eq!(render.nodes[2].kind, NodeKind::RoundedRect);
    assert_eq!(render.nodes[3].kind, NodeKind::Circle);
    assert_eq!(render.nodes[4].kind, NodeKind::Ellipse);
    assert!(render.nodes[1..].iter().all(|node| node.parent == Some(0)));
    assert_eq!(render.geometries.len(), 4);
    assert_eq!(render.paints.len(), 4);
    assert_eq!(render.layers[0].first_root, 0);
    assert_eq!(render.layers[0].root_count, 1);
}
