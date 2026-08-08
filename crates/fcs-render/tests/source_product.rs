use std::path::Path;

use fcs_fcbc::write_from_compilation;
use fcs_source::ResourceLimits;
use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;

use fcs_render::{
    GeometryData, NodeKind, PaintData, evaluate_semantic_draw_list_at, load_render,
    rasterize_solid_rgba8_at,
};

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
fn linear_gradient_source_reaches_product_loader_semantics_and_raster() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport {
        width: 8px;
        height: 8px;
    }
    layer main {
        pass: "overlay";
        children {
            rect gradient {
                origin: vec2(-4px, -4px);
                size: vec2(8px, 8px);
                fill: linearGradient(vec2(-4px, 0px), vec2(4px, 0px), [
                    stop(0.0, #FF0000FF),
                    stop(1.0, #0000FFFF),
                ], "repeat");
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("linear gradient source parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| panic!("linear gradient lowering failed: {diagnostics:?}"));
    let scene = compilation
        .chart()
        .render()
        .expect("canonical Render scene");
    assert_eq!(scene.paints().len(), 1);

    let bytes = write_from_compilation(&compilation).expect("linear gradient FCBC writing");
    let render = load_render(&bytes).expect("linear gradient product loader");
    assert!(matches!(
        render.paints[0].data,
        PaintData::LinearGradient { spread: 2, ref stops, .. } if stops.len() == 2
    ));

    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("gradient semantic evaluation");
    assert!(draw.iter().any(|op| op.linear_gradient.is_some()));
    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 8, 8).expect("gradient rasterization");
    assert_eq!(pixels.len(), 8 * 8 * 4);
    let left = &pixels[0..4];
    let right = &pixels[28..32];
    assert_ne!(
        left, right,
        "gradient output must not fall back to one solid color"
    );
    assert!(left[0] > left[2]);
    assert!(right[2] > right[0]);
}

#[test]
fn radial_gradient_source_reaches_product_loader_semantics_and_raster() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport {
        width: 8px;
        height: 8px;
    }
    layer main {
        pass: "overlay";
        children {
            rect gradient {
                origin: vec2(-4px, -4px);
                size: vec2(8px, 8px);
                fill: radialGradient(
                    vec2(0px, 0px),
                    0px,
                    vec2(0px, 0px),
                    4px,
                    [
                        stop(0.0, #FF0000FF),
                        stop(1.0, #0000FFFF),
                    ],
                    "pad"
                );
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("radial gradient source parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| panic!("radial gradient lowering failed: {diagnostics:?}"));
    let scene = compilation
        .chart()
        .render()
        .expect("canonical Render scene");
    assert_eq!(scene.paints().len(), 1);

    let bytes = write_from_compilation(&compilation).expect("radial gradient FCBC writing");
    let render = load_render(&bytes).expect("radial gradient product loader");
    assert!(matches!(
        render.paints[0].data,
        PaintData::RadialGradient {
            spread: 1,
            ref stops,
            ..
        } if stops.len() == 2
    ));

    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("radial gradient semantics");
    assert!(draw.iter().any(|op| op.radial_gradient.is_some()));
    let pixels =
        rasterize_solid_rgba8_at(&render, 0.0, 8, 8).expect("radial gradient rasterization");
    assert_eq!(pixels.len(), 8 * 8 * 4);
    let corner = &pixels[0..4];
    let center_offset = ((3 * 8 + 3) * 4) as usize;
    let center = &pixels[center_offset..center_offset + 4];
    assert_ne!(
        corner, center,
        "radial output must not fall back to one solid color"
    );
    assert!(corner[2] > corner[0]);
    assert!(center[0] > center[2]);
}

#[test]
fn radial_gradient_source_rejects_negative_radius() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 8px; height: 8px; }
    layer main {
        pass: "overlay";
        children {
            rect gradient {
                size: vec2(8px, 8px);
                fill: radialGradient(
                    vec2(0px, 0px),
                    -1px,
                    vec2(0px, 0px),
                    4px,
                    [stop(0.0, #FF0000FF), stop(1.0, #0000FFFF)],
                    "pad"
                );
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("negative-radius source parses");
    let diagnostics = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .expect_err("negative radial radius must fail at canonical lowering");
    assert!(!diagnostics.is_empty());
}

#[test]
fn nested_shape_source_reaches_product_render_loader() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport {
        width: 16px;
        height: 16px;
        colorSpace: "linear-srgb";
    }
    layer main {
        pass: "overlay";
        children {
            group root {
                children {
                    circle circleShape {
                        zOrder: 2;
                        radius: 2px;
                        fill: solid(#0000FFFF);
                    }
                    roundedRect roundedShape {
                        zOrder: 0;
                        size: vec2(6px, 6px);
                        radius: 1px;
                        fill: solid(#FF0000FF);
                    }
                    ellipse ellipseShape {
                        zOrder: 1;
                        radiusX: 3px;
                        radiusY: 2px;
                        fill: solid(#00FF00FF);
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
        .unwrap_or_else(|diagnostics| {
            panic!("nested Render canonical lowering failed: {diagnostics:?}")
        });

    let bytes = write_from_compilation(&compilation).expect("nested Render FCBC writing");
    let render = load_render(&bytes).expect("nested Render product loader");

    assert_eq!(render.layers.len(), 1);
    assert_eq!(render.layers[0].root_count, 1);
    assert_eq!(render.nodes.len(), 4);
    assert_eq!(render.nodes[0].kind, NodeKind::Group);
    assert_eq!(render.nodes[0].parent, None);
    assert_eq!(render.nodes[1].kind, NodeKind::RoundedRect);
    assert_eq!(render.nodes[2].kind, NodeKind::Ellipse);
    assert_eq!(render.nodes[3].kind, NodeKind::Circle);
    assert_eq!(render.nodes[1].parent, Some(0));
    assert_eq!(render.nodes[2].parent, Some(0));
    assert_eq!(render.nodes[3].parent, Some(0));
    assert_eq!(render.geometries.len(), 3);
    assert!(render
        .geometries
        .iter()
        .any(|geometry| matches!(geometry.data, GeometryData::RoundedRect { radii, .. } if radii.len() == 4)));
    assert!(
        render
            .geometries
            .iter()
            .any(|geometry| matches!(geometry.data, GeometryData::Circle { .. }))
    );
    assert!(
        render
            .geometries
            .iter()
            .any(|geometry| matches!(geometry.data, GeometryData::Ellipse { .. }))
    );
    assert_eq!(render.paints.len(), 3);
}

#[test]
fn point_geometry_source_reaches_product_render_loader() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport {
        width: 16px;
        height: 16px;
    }
    layer main {
        pass: "overlay";
        children {
            polyline trace {
                zOrder: 0;
                active: [1beat, 2beat);
                points: [vec2(-6px, -4px), vec2(0px, 4px), vec2(6px, -4px)];
                fill: solid(#00FFFFFF);
            }
            polygon shard {
                zOrder: 1;
                points: [vec2(-2px, -2px), vec2(2px, -2px), vec2(0px, 2px)];
                fill: solid(#FF00FFFF);
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("point geometry source parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| {
            panic!("point geometry canonical lowering failed: {diagnostics:?}")
        });

    let bytes = write_from_compilation(&compilation).expect("point geometry FCBC writing");
    let render = load_render(&bytes).expect("point geometry product loader");

    assert_eq!(render.nodes.len(), 2);
    assert_eq!(render.nodes[0].kind, NodeKind::Polyline);
    assert_eq!(render.nodes[1].kind, NodeKind::Polygon);
    assert_eq!(render.nodes[0].flags & 0b11, 0);
    assert!((render.nodes[0].active_start - 0.5).abs() < f64::EPSILON);
    assert!((render.nodes[0].active_end - 1.0).abs() < f64::EPSILON);
    assert_eq!(render.geometries.len(), 2);
    assert!(render.geometries.iter().any(|geometry| {
        matches!(geometry.data, GeometryData::Polyline { ref points } if points.len() == 3)
    }));
    assert!(render.geometries.iter().any(|geometry| {
        matches!(geometry.data, GeometryData::Polygon { ref points } if points.len() == 3)
    }));
    assert_eq!(render.paints.len(), 2);

    assert_eq!(
        evaluate_semantic_draw_list_at(&render, 0.25)
            .expect("inactive polyline is skipped")
            .len(),
        1
    );
    assert_eq!(
        evaluate_semantic_draw_list_at(&render, 0.75)
            .expect("point geometry semantic evaluation")
            .len(),
        2
    );
    let pixels =
        rasterize_solid_rgba8_at(&render, 0.75, 16, 16).expect("point geometry rasterization");
    assert!(
        pixels.chunks_exact(4).any(|pixel| pixel[3] != 0),
        "filled point geometry should contribute raster coverage"
    );
}

#[test]
fn image_source_reaches_product_render_loader_with_resource_metadata() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
resources {
    image sprite {
        source: "assets/fcs-test-rgba8.png";
        hash: "sha256:a108791d9edc1d9c37644a45ce29d4a20e479711db97daf85375b82924e8fa22";
        mediaType: "image/png";
        colorSpace: "srgb";
        alpha: "straight";
        sampling: "linear";
    }
}
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport {
        width: 4px;
        height: 4px;
        colorSpace: "linear-srgb";
    }
    layer main {
        pass: "overlay";
        children {
            image spriteNode {
                resource: @sprite;
                destination.origin: vec2(-2px, -2px);
                destination.size: vec2(4px, 4px);
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("Image Render source parses");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/conformance/render");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            &workspace,
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| panic!("Image canonical lowering failed: {diagnostics:?}"));

    let bytes = write_from_compilation(&compilation).expect("Image Render FCBC writing");
    let render = load_render(&bytes).expect("Image product Render loader");

    assert_eq!(render.viewport_width, 4.0);
    assert_eq!(render.viewport_height, 4.0);
    assert_eq!(render.nodes.len(), 1);
    assert_eq!(render.nodes[0].kind, NodeKind::Image);
    assert_eq!(render.nodes[0].fill_paint, None);
    assert_eq!(render.geometries.len(), 1);
    let GeometryData::Image {
        resource_id,
        destination,
        source: source_rect,
        sampling,
    } = &render.geometries[0].data
    else {
        panic!("expected Image geometry");
    };
    assert_eq!(destination.len(), 4);
    assert!(source_rect.is_none());
    assert_eq!(*sampling, 2);
    assert_eq!(render.resources.len(), 1);
    assert_eq!(render.resources[0].id, *resource_id);
    assert_eq!(render.resources[0].kind, 2);
    assert_eq!(render.resources[0].media_type, "image/png");
    assert_eq!(
        render.resources[0].data.as_slice(),
        include_bytes!("../../../docs/conformance/render/assets/fcs-test-rgba8.png")
    );
    assert!(render.decoded_images.contains_key(resource_id));

    let draw = evaluate_semantic_draw_list_at(&render, 0.0)
        .expect("Image semantic evaluation")
        .into_iter()
        .next()
        .expect("Image draw op");
    let image = draw.image.expect("Image draw payload");
    assert_eq!(image.resource_id, *resource_id);
    assert_eq!(image.destination, [-2.0, -2.0, 4.0, 4.0]);
    assert_eq!(image.source, [0.0, 0.0, 2.0, 2.0]);
    assert_eq!(image.sampling, 2);

    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 4, 4).expect("Image rasterization");
    assert_eq!(pixels.len(), 4 * 4 * 4);
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
}
