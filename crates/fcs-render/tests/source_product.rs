use std::path::Path;

use fcs_fcbc::write_from_compilation;
use fcs_model::{
    CanonicalArcDirection, CanonicalCompilation, CanonicalExpressionType, CanonicalImageRepeat,
    CanonicalImageSampling, CanonicalPathCommand, CanonicalPatternTransform,
    CanonicalRenderFillRule, CanonicalRenderGeometry, CanonicalRenderGeometryData,
    CanonicalRenderNode, CanonicalRenderNodeKind, CanonicalRenderNodeSpec, CanonicalRenderPaint,
    CanonicalRenderPaintData, CanonicalRenderPath, CanonicalRenderScene, CanonicalRenderSceneSpec,
    CanonicalRenderStroke, CanonicalStrokeCap, CanonicalStrokeJoin, CanonicalTextualId, EntityKind,
    StableIdRegistry,
};
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

fn canonical_line_stroke_compilation() -> CanonicalCompilation {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections {
    notes {
        tap {
            id: "writer-stroke";
            line: @main;
            gameplay.time: 1beat;
        };
    }
}
render profile 1.0.0 {
    viewport { width: 16px; height: 16px; }
    layer main {
        pass: "overlay";
        children {
            circle sourceShape {
                center: vec2(0px, 0px);
                radius: 5px;
                fill: solid(#FFFFFFFF);
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("canonical Line writer source parses");
    let base = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .expect("canonical Line writer source lowers");
    let original = base.chart().render().expect("source Render scene");
    let original_node = &original.nodes()[0];
    let original_geometry = &original.geometries()[0];
    let CanonicalRenderGeometryData::Circle { center, .. } = original_geometry.data() else {
        panic!("source fixture must provide a Circle geometry");
    };
    let width_descriptor = base
        .chart()
        .descriptors()
        .expect("source Render descriptors")
        .descriptors()
        .iter()
        .position(|descriptor| descriptor.property_type() == &CanonicalExpressionType::Length)
        .expect("source fixture must provide a Length descriptor");
    let mut ids = StableIdRegistry::new();
    let stroke_id = ids
        .insert(
            EntityKind::RenderStroke,
            CanonicalTextualId::explicit("writer-stroke").expect("stroke textual ID"),
        )
        .expect("stroke stable ID");
    let stroke = CanonicalRenderStroke::new(
        stroke_id,
        0,
        width_descriptor,
        CanonicalStrokeCap::Butt,
        CanonicalStrokeJoin::Miter,
        4.0,
        width_descriptor,
        vec![1.0, 2.0],
    )
    .expect("canonical stroke");
    let node = CanonicalRenderNode::new(CanonicalRenderNodeSpec {
        id: original_node.id().clone(),
        kind: CanonicalRenderNodeKind::Line,
        parent: original_node.parent(),
        layer: original_node.layer(),
        document_order: original_node.document_order(),
        z_order: original_node.z_order(),
        attachment: original_node.attachment().clone(),
        active: original_node.active(),
        isolate: original_node.isolate(),
        follow_hidden_attachment: original_node.follow_hidden_attachment(),
        position: original_node.position(),
        origin: original_node.origin(),
        rotation: original_node.rotation(),
        scale: original_node.scale(),
        opacity: original_node.opacity(),
        visibility: original_node.visibility(),
        geometry: Some(0),
        fill_paint: None,
        stroke: Some(0),
        clip: None,
        composite: original_node.composite(),
    })
    .expect("canonical Line node");
    let scene = CanonicalRenderScene::new(CanonicalRenderSceneSpec {
        viewport: original.viewport(),
        layers: original.layers().to_vec(),
        nodes: vec![node],
        geometries: vec![
            CanonicalRenderGeometry::new(
                original_geometry.id().clone(),
                CanonicalRenderGeometryData::Line {
                    start: *center,
                    end: *center,
                },
            )
            .expect("canonical Line geometry"),
        ],
        paths: Vec::new(),
        paints: vec![original.paints()[0].clone()],
        strokes: vec![stroke],
        clips: Vec::new(),
        glyph_runs: Vec::new(),
    })
    .expect("canonical Line scene");
    CanonicalCompilation::new(
        base.chart().clone().with_render(scene),
        base.resources().clone(),
        base.distribution().clone(),
    )
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
fn canonical_line_stroke_writer_reaches_product_render_loader() {
    let compilation = canonical_line_stroke_compilation();
    let scene = compilation.chart().render().expect("canonical Line scene");
    let bytes = write_from_compilation(&compilation).expect("canonical Line FCBC writing");
    let render = load_render(&bytes).expect("canonical Line product loader");

    assert_eq!(render.nodes.len(), 1);
    assert_eq!(render.nodes[0].kind, NodeKind::Line);
    assert_eq!(render.nodes[0].fill_paint, None);
    assert_eq!(render.nodes[0].stroke_ref, Some(0));
    assert_eq!(render.geometries.len(), 1);
    assert!(matches!(
        render.geometries[0].data,
        GeometryData::Line { start, end } if start != u32::MAX && end != u32::MAX
    ));
    assert_eq!(render.paints.len(), 1);
    assert_eq!(render.strokes.len(), 1);
    assert_eq!(render.strokes[0].id, scene.strokes()[0].id().value());
    assert_eq!(render.strokes[0].cap, 1);
    assert_eq!(render.strokes[0].join, 1);
    assert_eq!(render.strokes[0].miter_limit.to_bits(), 4.0f64.to_bits());
    assert_eq!(render.strokes[0].dash, vec![1.0, 2.0]);
    assert_eq!(
        render.core.descriptors[render.strokes[0].width_descriptor as usize].property_type,
        fcs_fcbc::ValueType::Length
    );
    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("Line semantic draw list");
    assert_eq!(draw.len(), 1);
    assert_eq!(draw[0].kind, NodeKind::Line);
    assert!(draw[0].stroke.is_some());
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

#[test]
fn canonical_image_pattern_writer_reaches_product_render_loader() {
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
    viewport { width: 4px; height: 4px; }
    layer main {
        pass: "overlay";
        children {
            circle patternShape {
                center: vec2(0px, 0px);
                radius: 2px;
                fill: solid(#FFFFFFFF);
            }
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
        .expect("ImagePattern source parses");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/conformance/render");
    let base = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            &workspace,
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| {
            panic!("ImagePattern canonical lowering failed: {diagnostics:?}")
        });
    let original = base.chart().render().expect("source Render scene");
    let pattern_node = &original.nodes()[0];
    let resource_id = original
        .geometries()
        .iter()
        .find_map(|geometry| match geometry.data() {
            CanonicalRenderGeometryData::Image { resource, .. } => Some(resource.clone()),
            _ => None,
        })
        .expect("source fixture must provide an image resource");
    let paint = CanonicalRenderPaint::new(
        original.paints()[0].id().clone(),
        CanonicalRenderPaintData::ImagePattern {
            resource: resource_id.clone(),
            transform: CanonicalPatternTransform {
                position: pattern_node.position(),
                origin: pattern_node.origin(),
                rotation: pattern_node.rotation(),
                scale: pattern_node.scale(),
            },
            repeat: CanonicalImageRepeat::Both,
            sampling: CanonicalImageSampling::Bilinear,
        },
    )
    .expect("canonical ImagePattern paint");
    let scene = CanonicalRenderScene::new(CanonicalRenderSceneSpec {
        viewport: original.viewport(),
        layers: original.layers().to_vec(),
        nodes: original.nodes().to_vec(),
        geometries: original.geometries().to_vec(),
        paths: original.paths().to_vec(),
        paints: vec![paint],
        strokes: original.strokes().to_vec(),
        clips: original.clips().to_vec(),
        glyph_runs: original.glyph_runs().to_vec(),
    })
    .expect("canonical ImagePattern scene");
    let compilation = CanonicalCompilation::new(
        base.chart().clone().with_render(scene),
        base.resources().clone(),
        base.distribution().clone(),
    );
    let bytes = write_from_compilation(&compilation).expect("ImagePattern FCBC writing");
    let render = load_render(&bytes).expect("ImagePattern product Render loader");

    assert_eq!(render.paints.len(), 1);
    let PaintData::ImagePattern {
        resource_id: decoded_resource,
        position,
        origin,
        rotation,
        scale,
        repeat,
        sampling,
    } = render.paints[0].data
    else {
        panic!("expected ImagePattern paint");
    };
    assert_eq!(decoded_resource, resource_id.value());
    assert_eq!(position, render.nodes[0].position_descriptor);
    assert_eq!(origin, render.nodes[0].origin_descriptor);
    assert_eq!(rotation, render.nodes[0].rotation_descriptor);
    assert_eq!(scale, render.nodes[0].scale_descriptor);
    assert_eq!(repeat, 4);
    assert_eq!(sampling, 2);
    assert_eq!(render.resources.len(), 1);
    assert_eq!(render.resources[0].id, decoded_resource);

    let draw =
        evaluate_semantic_draw_list_at(&render, 0.0).expect("ImagePattern semantic evaluation");
    let pattern = draw
        .iter()
        .find_map(|operation| operation.image_pattern)
        .expect("ImagePattern draw payload");
    assert_eq!(pattern.resource_id, decoded_resource);
    assert_eq!(pattern.repeat, 4);
    assert_eq!(pattern.sampling, 2);
    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 4, 4).expect("ImagePattern rasterization");
    assert_eq!(pixels.len(), 4 * 4 * 4);
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
}

#[test]
fn canonical_path_writer_reaches_product_render_loader() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 4px; height: 4px; }
    layer main {
        pass: "overlay";
        children {
            polyline pathShape {
                points: [
                    vec2(-1px, -1px),
                    vec2(1px, -1px),
                    vec2(0px, 1px),
                ];
                fill: solid(#FFFFFFFF);
            }
            circle referenceShape {
                center: vec2(0px, 0px);
                radius: 1px;
                fill: solid(#000000FF);
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("Path Render source parses");
    let base = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| panic!("Path canonical lowering failed: {diagnostics:?}"));
    let original = base.chart().render().expect("source Render scene");
    let path_node_index = original
        .nodes()
        .iter()
        .position(|node| node.kind() == CanonicalRenderNodeKind::Polyline)
        .expect("source fixture must provide a Polyline node");
    let path_node = &original.nodes()[path_node_index];
    let path_geometry_index = path_node.geometry().expect("Polyline geometry");
    let points = match original.geometries()[path_geometry_index].data() {
        CanonicalRenderGeometryData::Polyline { points } => points.clone(),
        _ => panic!("source fixture must provide Polyline points"),
    };
    let reference_node = original
        .nodes()
        .iter()
        .find(|node| node.kind() == CanonicalRenderNodeKind::Circle)
        .expect("source fixture must provide a Circle node");
    let reference_geometry_index = reference_node.geometry().expect("Circle geometry");
    let CanonicalRenderGeometryData::Circle { center, radius } =
        original.geometries()[reference_geometry_index].data()
    else {
        panic!("source fixture must provide Circle geometry");
    };
    let mut ids = StableIdRegistry::new();
    let path_id = ids
        .insert(
            EntityKind::RenderPath,
            CanonicalTextualId::explicit("writer-path").expect("path textual ID"),
        )
        .expect("path stable ID");
    let path = CanonicalRenderPath::new(
        path_id,
        CanonicalRenderFillRule::NonZero,
        vec![
            CanonicalPathCommand::MoveTo(points[0]),
            CanonicalPathCommand::LineTo(points[1]),
            CanonicalPathCommand::QuadraticTo(points[0], points[1]),
            CanonicalPathCommand::CubicTo(points[0], points[1], points[2]),
            CanonicalPathCommand::Arc {
                center: *center,
                radius: *radius,
                start_angle: path_node.rotation(),
                end_angle: path_node.rotation(),
                direction: CanonicalArcDirection::Clockwise,
            },
            CanonicalPathCommand::EllipseArc {
                center: *center,
                radius_x: *radius,
                radius_y: *radius,
                rotation: path_node.rotation(),
                start_angle: path_node.rotation(),
                end_angle: path_node.rotation(),
                direction: CanonicalArcDirection::CounterClockwise,
            },
            CanonicalPathCommand::Close,
        ],
    )
    .expect("canonical Path");
    let node = CanonicalRenderNode::new(CanonicalRenderNodeSpec {
        id: path_node.id().clone(),
        kind: CanonicalRenderNodeKind::Path,
        parent: path_node.parent(),
        layer: path_node.layer(),
        document_order: path_node.document_order(),
        z_order: path_node.z_order(),
        attachment: path_node.attachment().clone(),
        active: path_node.active(),
        isolate: path_node.isolate(),
        follow_hidden_attachment: path_node.follow_hidden_attachment(),
        position: path_node.position(),
        origin: path_node.origin(),
        rotation: path_node.rotation(),
        scale: path_node.scale(),
        opacity: path_node.opacity(),
        visibility: path_node.visibility(),
        geometry: Some(path_geometry_index),
        fill_paint: path_node.fill_paint(),
        stroke: None,
        clip: None,
        composite: path_node.composite(),
    })
    .expect("canonical Path node");
    let geometry = CanonicalRenderGeometry::new(
        original.geometries()[path_geometry_index].id().clone(),
        CanonicalRenderGeometryData::Path { path: 0 },
    )
    .expect("canonical Path geometry");
    let mut nodes = original.nodes().to_vec();
    nodes[path_node_index] = node;
    let mut geometries = original.geometries().to_vec();
    geometries[path_geometry_index] = geometry;
    let scene = CanonicalRenderScene::new(CanonicalRenderSceneSpec {
        viewport: original.viewport(),
        layers: original.layers().to_vec(),
        nodes,
        geometries,
        paths: vec![path],
        paints: original.paints().to_vec(),
        strokes: original.strokes().to_vec(),
        clips: original.clips().to_vec(),
        glyph_runs: original.glyph_runs().to_vec(),
    })
    .expect("canonical Path scene");
    let compilation = CanonicalCompilation::new(
        base.chart().clone().with_render(scene),
        base.resources().clone(),
        base.distribution().clone(),
    );
    let scene = compilation.chart().render().expect("canonical Path scene");
    let bytes = write_from_compilation(&compilation).expect("canonical Path FCBC writing");
    let render = load_render(&bytes).expect("canonical Path product loader");

    let decoded_node_index = render
        .nodes
        .iter()
        .position(|node| node.id == scene.nodes()[path_node_index].id().value())
        .expect("decoded Path node");
    let decoded_geometry_index = render
        .geometries
        .iter()
        .position(|geometry| geometry.kind == NodeKind::Path)
        .expect("decoded Path geometry");
    assert_eq!(render.nodes[decoded_node_index].kind, NodeKind::Path);
    assert_eq!(
        render.nodes[decoded_node_index].geometry_ref,
        Some(decoded_geometry_index as u32)
    );
    assert_eq!(
        render.geometries[decoded_geometry_index].kind,
        NodeKind::Path
    );
    assert!(matches!(
        render.geometries[decoded_geometry_index].data,
        GeometryData::Path { path_ref: 0 }
    ));
    assert_eq!(render.paths.len(), 1);
    assert_eq!(render.paths[0].id, scene.paths()[0].id().value());
    assert_eq!(render.paths[0].fill_rule, 1);
    assert_eq!(render.paths[0].commands.len(), 7);
    assert_eq!(render.paints.len(), 2);

    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("Path semantic evaluation");
    assert!(
        draw.iter()
            .any(|operation| operation.kind == NodeKind::Path)
    );
}
