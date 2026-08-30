use std::path::Path;

#[path = "../../fcs-source/tests/support/fcbc_reference_loader.rs"]
#[allow(dead_code)]
mod fcbc_reference_loader;
#[path = "../../fcs-source/tests/support/fcbc_render_reference_assets.rs"]
#[allow(dead_code)]
mod fcbc_render_reference_assets;
#[path = "../../fcs-source/tests/support/fcbc_render_reference_loader.rs"]
#[allow(dead_code)]
mod fcbc_render_reference_loader;

use fcs_fcbc::write_from_compilation;
use fcs_model::{
    CanonicalArcDirection, CanonicalCompilation, CanonicalDescriptorKind,
    CanonicalExpressionEnvironment, CanonicalExpressionType, CanonicalGlyphRun,
    CanonicalPathCommand, CanonicalRenderAttachment, CanonicalRenderError, CanonicalRenderFillRule,
    CanonicalRenderGeometry, CanonicalRenderGeometryData, CanonicalRenderNode,
    CanonicalRenderNodeKind, CanonicalRenderNodeSpec, CanonicalRenderPaint, CanonicalRenderPath,
    CanonicalRenderScene, CanonicalRenderSceneSpec, CanonicalRenderStroke, CanonicalStrokeCap,
    CanonicalStrokeJoin, CanonicalTextualId, EntityKind, StableIdRegistry, derive_stable_id,
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
    let layer_id = derive_stable_id(EntityKind::RenderLayer, "layer/main");
    let node_id = derive_stable_id(EntityKind::RenderNode, "layer/main/node/full");
    let geometry_id = derive_stable_id(
        EntityKind::RenderGeometry,
        &format!("owner/{node_id:016x}/field/geometryRef/ordinal/0"),
    );
    let paint_id = derive_stable_id(
        EntityKind::RenderPaint,
        &format!("owner/{node_id:016x}/field/fillPaint/ordinal/0"),
    );
    assert_eq!(scene.layers()[0].id().value(), layer_id);
    assert_eq!(scene.nodes()[0].id().value(), node_id);
    assert_eq!(scene.geometries()[0].id().value(), geometry_id);
    assert_eq!(scene.paints()[0].id().value(), paint_id);
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
fn render_static_fields_resolve_compile_time_definitions() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
definitions {
    const WIDTH: length = 4px;
    const HEIGHT: length = 4px;
    const VIEWPORT_SPACE: string = "screen";
    const PASS: string = "overlay";
    const POSITION: vec2<length> = vec2(-2px, -2px);
    const SIZE: vec2<length> = vec2(4px, 4px);
    const COLOR: color = #FF0000FF;
    const ACTIVE_START: time = 0s;
    const ACTIVE_END: time = 1s;
}
render profile 1.0.0 {
    viewport {
        width: WIDTH;
        height: HEIGHT;
    }
    layer main {
        pass: PASS;
        space: VIEWPORT_SPACE;
        children {
            rect full {
                position: POSITION;
                size: SIZE;
                active: [ACTIVE_START, ACTIVE_END);
                fill: solid(COLOR);
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("definition-backed Render source parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .expect("definition-backed Render source lowers");
    let bytes =
        write_from_compilation(&compilation).expect("definition-backed Render FCBC writing");
    let render = load_render(&bytes).expect("definition-backed product Render loader");

    assert_eq!(render.viewport_width, 4.0);
    assert_eq!(render.viewport_height, 4.0);
    assert_eq!(render.layers[0].pass, 6);
    assert_eq!(render.nodes[0].active_start, 0.0);
    assert_eq!(render.nodes[0].active_end, 1.0);
}

#[test]
fn source_line_and_note_attachments_reach_product_render_loader() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections {
    notes {
        tap {
            id: "anchor";
            line: @main;
            gameplay.time: 0beat;
        };
    }
}
render profile 1.0.0 {
    viewport { width: 4px; height: 4px; }
    layer lineLayer {
        pass: "overlay";
        space: line(@main);
        children {
            rect lineRect {
                origin: vec2(-2px, -2px);
                size: vec2(1px, 1px);
                fill: solid(#FFFFFFFF);
            }
        }
    }
    layer noteLayer {
        pass: "overlay";
        zOrder: 1;
        space: note(@anchor);
        children {
            rect noteRect {
                origin: vec2(-2px, -2px);
                size: vec2(1px, 1px);
                fill: solid(#FFFFFFFF);
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("line/note attachment source parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .expect("line/note attachment source lowers");
    let scene = compilation
        .chart()
        .render()
        .expect("canonical Render scene");
    let line_id = scene
        .nodes()
        .iter()
        .find_map(|node| match node.attachment() {
            CanonicalRenderAttachment::Line(id) => Some(id.value()),
            CanonicalRenderAttachment::World
            | CanonicalRenderAttachment::Screen
            | CanonicalRenderAttachment::Note(_) => None,
        })
        .expect("canonical line attachment");
    let note_id = scene
        .nodes()
        .iter()
        .find_map(|node| match node.attachment() {
            CanonicalRenderAttachment::Note(id) => Some(id.value()),
            CanonicalRenderAttachment::World
            | CanonicalRenderAttachment::Screen
            | CanonicalRenderAttachment::Line(_) => None,
        })
        .expect("canonical note attachment");

    let bytes = write_from_compilation(&compilation).expect("attachment FCBC writing");
    let render = load_render(&bytes).expect("attachment product Render loader");
    assert!(
        render
            .nodes
            .iter()
            .any(|node| node.attachment.kind == 3 && node.attachment.id == line_id)
    );
    assert!(
        render
            .nodes
            .iter()
            .any(|node| node.attachment.kind == 4 && node.attachment.id == note_id)
    );
}

#[test]
fn source_dynamic_opacity_reaches_product_semantic_and_raster() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 4px; height: 4px; }
    layer main {
        pass: "overlay";
        space: "screen";
        children {
            rect full {
                origin: vec2(-2px, -2px);
                size: vec2(4px, 4px);
                opacity: choose {
                    when s < 1s => 1.0;
                    else => 0.0;
                };
                fill: solid(#FF0000FF);
            }
        }
    }
}
"#;

    let document = parse_document(source)
        .into_result()
        .expect("dynamic Render source parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| panic!("dynamic Render lowering failed: {diagnostics:?}"));
    let descriptors = compilation.chart().descriptors().expect("descriptor table");
    let opacity_root = descriptors
        .roots()
        .iter()
        .find(|root| root.target_path() == "render.node.opacity")
        .expect("opacity root");
    assert!(matches!(
        descriptors.descriptors()[opacity_root.descriptor()].kind(),
        CanonicalDescriptorKind::Expression(expression)
            if expression
                .required_environment()
                .contains(&CanonicalExpressionEnvironment::S)
    ));

    let bytes = write_from_compilation(&compilation).expect("dynamic Render FCBC writing");
    let render = load_render(&bytes).expect("dynamic Render product loader");
    let opaque = evaluate_semantic_draw_list_at(&render, 0.0).expect("opaque semantic query");
    let transparent = evaluate_semantic_draw_list_at(&render, 1.0).expect("transparent query");
    assert_eq!(
        opaque,
        evaluate_semantic_draw_list_at(&render, 0.0).expect("repeat query")
    );
    assert_eq!(opaque.len(), 1);
    assert_eq!(opaque[0].opacity.to_bits(), 1.0f64.to_bits());
    assert_eq!(transparent.len(), 1);
    assert_eq!(transparent[0].opacity.to_bits(), 0.0f64.to_bits());

    let opaque_pixels = rasterize_solid_rgba8_at(&render, 0.0, 4, 4).expect("opaque raster");
    let repeat_pixels = rasterize_solid_rgba8_at(&render, 0.0, 4, 4).expect("repeat raster");
    let transparent_pixels =
        rasterize_solid_rgba8_at(&render, 1.0, 4, 4).expect("transparent raster");
    assert_eq!(opaque_pixels, repeat_pixels);
    assert!(
        opaque_pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] != 0)
    );
    assert!(
        transparent_pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel == &[0; 4])
    );

    let invalid = source.replace("else => 0.0;", "else => 2.0;");
    let document = parse_document(&invalid)
        .into_result()
        .expect("invalid dynamic Render source parses");
    let compilation = document
        .canonical_compilation_with_source(
            &invalid,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .expect("invalid dynamic descriptor remains representable");
    let bytes = write_from_compilation(&compilation).expect("invalid dynamic FCBC writing");
    let render = load_render(&bytes).expect("invalid dynamic Render product loader");
    assert_eq!(
        evaluate_semantic_draw_list_at(&render, 1.0),
        Err("render.invalid-composite")
    );
}

#[test]
fn source_exact_node_and_geometry_descriptors_reach_product_semantics() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 16px; height: 16px; }
    layer main {
        pass: "overlay";
        space: "screen";
        children {
            rect dynamicRect {
                position: choose { when s < 1s => vec2(0px, 0px); else => vec2(1px, 0px); };
                origin: choose { when s < 1s => vec2(-7px, -7px); else => vec2(-6px, -7px); };
                rotation: choose { when s < 1s => 0rad; else => 0.1rad; };
                scale: choose { when s < 1s => vec2(1.0, 1.0); else => vec2(1.5, 1.0); };
                size: choose { when s < 1s => vec2(2px, 2px); else => vec2(3px, 2px); };
                fill: solid(#FFFFFFFF);
            }
            roundedRect dynamicRounded {
                origin: vec2(-3px, -7px);
                size: choose { when s < 1s => vec2(2px, 2px); else => vec2(3px, 3px); };
                radius: choose { when s < 1s => 0.5px; else => 1px; };
                fill: solid(#FFFFFFFF);
            }
            circle dynamicCircle {
                center: choose { when s < 1s => vec2(1px, -6px); else => vec2(2px, -6px); };
                radius: choose { when s < 1s => 1px; else => 2px; };
                fill: solid(#FFFFFFFF);
            }
            ellipse dynamicEllipse {
                center: choose { when s < 1s => vec2(5px, -6px); else => vec2(5px, -5px); };
                radiusX: choose { when s < 1s => 1px; else => 2px; };
                radiusY: choose { when s < 1s => 2px; else => 1px; };
                rotation: choose { when s < 1s => 0rad; else => 0.2rad; };
                fill: solid(#FFFFFFFF);
            }
            line dynamicLine {
                start: choose { when s < 1s => vec2(-7px, 0px); else => vec2(-6px, 0px); };
                end: choose { when s < 1s => vec2(-4px, 0px); else => vec2(-3px, 1px); };
                stroke: solid(#FFFFFFFF);
                width: 1px;
                cap: "butt";
                join: "miter";
                miterLimit: 4.0;
                dash: [];
                dashOffset: 0px;
            }
            polyline dynamicPolyline {
                points: [
                    choose { when s < 1s => vec2(-2px, 0px); else => vec2(-1px, 0px); },
                    choose { when s < 1s => vec2(0px, 2px); else => vec2(0px, 3px); },
                    choose { when s < 1s => vec2(2px, 0px); else => vec2(3px, 0px); },
                ];
                fill: solid(#FFFFFFFF);
            }
            polygon dynamicPolygon {
                points: [
                    choose { when s < 1s => vec2(4px, 0px); else => vec2(4px, 1px); },
                    choose { when s < 1s => vec2(6px, 2px); else => vec2(7px, 2px); },
                    choose { when s < 1s => vec2(7px, 0px); else => vec2(7px, 1px); },
                ];
                fill: solid(#FFFFFFFF);
            }
            clipGroup dynamicRectClip {
                clip.kind: "rect";
                clip.fillRule: "nonzero";
                clip.origin: choose {
                    when s < 1s => vec2(-7px, 4px);
                    else => vec2(-6px, 4px);
                };
                clip.size: choose {
                    when s < 1s => vec2(2px, 2px);
                    else => vec2(3px, 2px);
                };
                children {
                    rect clippedByRect {
                        origin: vec2(-7px, 4px);
                        size: vec2(4px, 4px);
                        fill: solid(#FFFFFFFF);
                    }
                }
            }
            clipGroup dynamicEllipseClip {
                clip.kind: "ellipse";
                clip.fillRule: "nonzero";
                clip.center: choose {
                    when s < 1s => vec2(2px, 5px);
                    else => vec2(3px, 5px);
                };
                clip.radiusX: choose { when s < 1s => 2px; else => 3px; };
                clip.radiusY: choose { when s < 1s => 1px; else => 2px; };
                clip.rotation: choose { when s < 1s => 0rad; else => 0.2rad; };
                children {
                    rect clippedByEllipse {
                        origin: vec2(0px, 3px);
                        size: vec2(6px, 4px);
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
        .expect("exact descriptor Render source parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| {
            panic!("exact descriptor Render lowering failed: {diagnostics:?}")
        });
    let descriptors = compilation.chart().descriptors().expect("descriptor table");
    let is_expression = |descriptor: usize| {
        matches!(
            descriptors.descriptors()[descriptor].kind(),
            CanonicalDescriptorKind::Expression(expression)
                if expression.required_environment().contains(&CanonicalExpressionEnvironment::S)
        )
    };
    let scene = compilation
        .chart()
        .render()
        .expect("canonical Render scene");
    let rect_node = scene
        .nodes()
        .iter()
        .find(|node| node.kind() == CanonicalRenderNodeKind::Rect)
        .expect("dynamic Rect node");
    for descriptor in [
        rect_node.position(),
        rect_node.origin(),
        rect_node.rotation(),
        rect_node.scale(),
    ] {
        assert!(is_expression(descriptor));
    }

    let mut seen = [false; 7];
    for geometry in scene.geometries() {
        match geometry.data() {
            CanonicalRenderGeometryData::Rect { size, .. } => {
                assert!(is_expression(*size));
                seen[0] = true;
            }
            CanonicalRenderGeometryData::RoundedRect { size, radii, .. } => {
                assert!(is_expression(*size));
                assert!(radii.iter().all(|descriptor| is_expression(*descriptor)));
                seen[1] = true;
            }
            CanonicalRenderGeometryData::Circle { center, radius } => {
                assert!(is_expression(*center));
                assert!(is_expression(*radius));
                seen[2] = true;
            }
            CanonicalRenderGeometryData::Ellipse {
                center,
                radius_x,
                radius_y,
                rotation,
            } => {
                assert!(is_expression(*center));
                assert!(is_expression(*radius_x));
                assert!(is_expression(*radius_y));
                assert!(is_expression(*rotation));
                seen[3] = true;
            }
            CanonicalRenderGeometryData::Line { start, end } => {
                assert!(is_expression(*start));
                assert!(is_expression(*end));
                seen[4] = true;
            }
            CanonicalRenderGeometryData::Polyline { points } => {
                assert!(points.iter().all(|descriptor| is_expression(*descriptor)));
                seen[5] = true;
            }
            CanonicalRenderGeometryData::Polygon { points } => {
                assert!(points.iter().all(|descriptor| is_expression(*descriptor)));
                seen[6] = true;
            }
            _ => {}
        }
    }
    assert!(seen.into_iter().all(|present| present));
    let mut seen_rect_clip = false;
    let mut seen_ellipse_clip = false;
    for clip in scene.clips() {
        match scene.geometries()[clip.geometry()].data() {
            CanonicalRenderGeometryData::Rect { origin, size } => {
                assert!(is_expression(*origin));
                assert!(is_expression(*size));
                seen_rect_clip = true;
            }
            CanonicalRenderGeometryData::Ellipse {
                center,
                radius_x,
                radius_y,
                rotation,
            } => {
                for descriptor in [center, radius_x, radius_y, rotation] {
                    assert!(is_expression(*descriptor));
                }
                seen_ellipse_clip = true;
            }
            other => panic!("unexpected dynamic Clip geometry {other:?}"),
        }
    }
    assert!(seen_rect_clip && seen_ellipse_clip);

    let bytes = write_from_compilation(&compilation).expect("exact descriptor FCBC writing");
    let render = load_render(&bytes).expect("exact descriptor product loader");
    let before = evaluate_semantic_draw_list_at(&render, 0.0).expect("first semantic query");
    let after = evaluate_semantic_draw_list_at(&render, 2.0).expect("second semantic query");
    assert_eq!(before.len(), 9);
    assert_eq!(after.len(), 9);
    assert!(
        before
            .iter()
            .zip(&after)
            .any(|(before, after)| before.bounds != after.bounds
                || before.world_matrix != after.world_matrix)
    );
}

#[test]
fn source_exact_image_and_text_descriptors_reach_product_semantics() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
resources {
    image sprite {
        source: "assets/fcs-test-rgba8.png";
        hash: "sha256:a108791d9edc1d9c37644a45ce29d4a20e479711db97daf85375b82924e8fa22";
        mediaType: "image/png";
        colorSpace: "srgb";
        alpha: "straight";
        sampling: "nearest";
    }
    font primary {
        source: "assets/fcs-test-font.ttf";
        mediaType: "font/ttf";
    }
}
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 16px; height: 16px; }
    layer main {
        pass: "overlay";
        children {
            image dynamicImage {
                resource: @sprite;
                destination.origin: choose {
                    when s < 1s => vec2(-2px, -2px);
                    else => vec2(-1px, -1px);
                };
                destination.size: choose {
                    when s < 1s => vec2(4px, 4px);
                    else => vec2(3px, 2px);
                };
                sourceRect.origin: choose {
                    when s < 1s => vec2(0.0, 0.0);
                    else => vec2(1.0, 1.0);
                };
                sourceRect.size: choose {
                    when s < 1s => vec2(2.0, 2.0);
                    else => vec2(1.0, 1.0);
                };
            }
            text dynamicText {
                content: "A";
                font: @primary;
                origin: choose {
                    when s < 1s => vec2(0px, 0px);
                    else => vec2(1px, 0px);
                };
                size: choose { when s < 2s => 4px; else => 8px; };
                fill: solid(#FFFFFFFF);
            }
        }
    }
}
"#;
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/conformance/render");
    let compile = |source: &str| {
        let document = parse_document(source)
            .into_result()
            .expect("exact Image/Text source parses");
        document
            .canonical_compilation_with_source(
                source,
                CompileTimeLimits::default(),
                &workspace,
                ResourceLimits::default(),
            )
            .unwrap_or_else(|diagnostics| {
                panic!("exact Image/Text lowering failed: {diagnostics:?}")
            })
    };
    let product = |compilation: &CanonicalCompilation| {
        let bytes = write_from_compilation(compilation).expect("exact Image/Text FCBC writing");
        load_render(&bytes).expect("exact Image/Text product loader")
    };

    let compilation = compile(source);
    let descriptors = compilation.chart().descriptors().expect("descriptor table");
    let is_expression = |descriptor: usize| {
        matches!(
            descriptors.descriptors()[descriptor].kind(),
            CanonicalDescriptorKind::Expression(expression)
                if expression.required_environment().contains(&CanonicalExpressionEnvironment::S)
        )
    };
    let scene = compilation
        .chart()
        .render()
        .expect("canonical Render scene");
    let image_geometry = scene
        .geometries()
        .iter()
        .find_map(|geometry| match geometry.data() {
            CanonicalRenderGeometryData::Image {
                destination,
                source: Some(source),
                ..
            } => Some((destination, source)),
            _ => None,
        })
        .expect("dynamic Image geometry");
    assert!(
        image_geometry
            .0
            .iter()
            .chain(image_geometry.1)
            .all(|descriptor| is_expression(*descriptor))
    );
    let text_node = scene
        .nodes()
        .iter()
        .find(|node| node.kind() == CanonicalRenderNodeKind::Text)
        .expect("dynamic Text node");
    assert!(is_expression(text_node.origin()));
    let text_geometry = scene
        .geometries()
        .iter()
        .find_map(|geometry| match geometry.data() {
            CanonicalRenderGeometryData::Text { glyph_runs, origin } => Some((glyph_runs, origin)),
            _ => None,
        })
        .expect("dynamic Text geometry");
    assert_eq!(*text_geometry.1, text_node.origin());
    assert!(is_expression(*text_geometry.1));
    assert!(
        text_geometry
            .0
            .iter()
            .all(|run| is_expression(scene.glyph_runs()[*run].size()))
    );

    let render = product(&compilation);
    let before = evaluate_semantic_draw_list_at(&render, 0.0).expect("first Image/Text query");
    let moved = evaluate_semantic_draw_list_at(&render, 1.5).expect("moved Image/Text query");
    let after = evaluate_semantic_draw_list_at(&render, 3.0).expect("resized Image/Text query");
    let before_image = before
        .iter()
        .find(|operation| operation.kind == NodeKind::Image)
        .and_then(|operation| operation.image)
        .expect("first Image payload");
    let after_image = moved
        .iter()
        .find(|operation| operation.kind == NodeKind::Image)
        .and_then(|operation| operation.image)
        .expect("second Image payload");
    assert_eq!(before_image.destination, [-2.0, -2.0, 4.0, 4.0]);
    assert_eq!(after_image.destination, [-1.0, -1.0, 3.0, 2.0]);
    assert_eq!(before_image.source, [0.0, 0.0, 2.0, 2.0]);
    assert_eq!(after_image.source, [1.0, 1.0, 1.0, 1.0]);
    let before_text = before
        .iter()
        .find(|operation| operation.kind == NodeKind::Text)
        .expect("first Text draw operation");
    let moved_text = moved
        .iter()
        .find(|operation| operation.kind == NodeKind::Text)
        .expect("moved Text draw operation");
    let after_text = after
        .iter()
        .find(|operation| operation.kind == NodeKind::Text)
        .expect("resized Text draw operation");
    assert!(moved_text.bounds[0] > before_text.bounds[0]);
    assert!(moved_text.bounds[2] > before_text.bounds[2]);
    assert!(
        after_text.bounds[2] - after_text.bounds[0] > moved_text.bounds[2] - moved_text.bounds[0]
    );
    assert!(
        after_text.bounds[3] - after_text.bounds[1] > moved_text.bounds[3] - moved_text.bounds[1]
    );
    let before_pixels =
        rasterize_solid_rgba8_at(&render, 0.0, 16, 16).expect("first Image/Text raster");
    let after_pixels =
        rasterize_solid_rgba8_at(&render, 3.0, 16, 16).expect("second Image/Text raster");
    assert_ne!(before_pixels, after_pixels);

    let invalid_image = source.replace("else => vec2(3px, 2px);", "else => vec2(-1px, 2px);");
    let invalid_image = product(&compile(&invalid_image));
    assert_eq!(
        evaluate_semantic_draw_list_at(&invalid_image, 2.0),
        Err("render.invalid-geometry")
    );
    let invalid_text = source.replace("else => 8px;", "else => 0px;");
    let invalid_text = product(&compile(&invalid_text));
    assert_eq!(
        evaluate_semantic_draw_list_at(&invalid_text, 3.0),
        Err("render.invalid-geometry")
    );
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

fn canonical_circle_stroke_compilation(dash: Vec<f64>, keep_fill: bool) -> CanonicalCompilation {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 16px; height: 16px; }
    layer main {
        pass: "overlay";
        children {
            circle ring {
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
        .expect("canonical Circle stroke source parses");
    let base = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .expect("canonical Circle stroke source lowers");
    let original = base.chart().render().expect("source Render scene");
    let original_node = &original.nodes()[0];
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
            CanonicalTextualId::explicit("circle-stroke").expect("stroke textual ID"),
        )
        .expect("stroke stable ID");
    let stroke_paint_id = ids
        .insert(
            EntityKind::RenderPaint,
            CanonicalTextualId::explicit("circle-stroke-paint").expect("stroke paint textual ID"),
        )
        .expect("stroke paint stable ID");
    // Render section 14.2 forbids sharing one paint record between a fill and a stroke, so
    // the stroke gets its own table entry with the same payload.
    let stroke_paint =
        CanonicalRenderPaint::new(stroke_paint_id, original.paints()[0].data().clone())
            .expect("canonical Circle stroke paint");
    let (paints, fill_paint, stroke_paint_index) = if keep_fill {
        (
            vec![original.paints()[0].clone(), stroke_paint],
            Some(0usize),
            1usize,
        )
    } else {
        (vec![stroke_paint], None, 0usize)
    };
    let stroke = CanonicalRenderStroke::new(
        stroke_id,
        stroke_paint_index,
        width_descriptor,
        CanonicalStrokeCap::Butt,
        CanonicalStrokeJoin::Miter,
        4.0,
        width_descriptor,
        dash,
    )
    .expect("canonical Circle stroke");
    let node = CanonicalRenderNode::new(CanonicalRenderNodeSpec {
        id: original_node.id().clone(),
        kind: CanonicalRenderNodeKind::Circle,
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
        fill_paint,
        stroke: Some(0),
        clip: None,
        composite: original_node.composite(),
    })
    .expect("canonical Circle node");
    let scene = CanonicalRenderScene::new(CanonicalRenderSceneSpec {
        viewport: original.viewport(),
        layers: original.layers().to_vec(),
        nodes: vec![node],
        geometries: original.geometries().to_vec(),
        paths: Vec::new(),
        paints,
        strokes: vec![stroke],
        clips: Vec::new(),
        glyph_runs: Vec::new(),
    })
    .expect("canonical Circle scene");
    CanonicalCompilation::new(
        base.chart().clone().with_render(scene),
        base.resources().clone(),
        base.distribution().clone(),
    )
}

/// The sample whose centre is nearest the viewport centre, as a premultiplied RGBA8 quad.
fn centre_pixel(pixels: &[u8], width: usize) -> [u8; 4] {
    let index = ((width / 2) * width + (width / 2)) * 4;
    [
        pixels[index],
        pixels[index + 1],
        pixels[index + 2],
        pixels[index + 3],
    ]
}

#[test]
fn canonical_circle_stroke_writer_reaches_product_render_loader() {
    let compilation = canonical_circle_stroke_compilation(Vec::new(), false);
    let scene = compilation
        .chart()
        .render()
        .expect("canonical Circle scene");
    let bytes = write_from_compilation(&compilation).expect("canonical Circle FCBC writing");
    let render = load_render(&bytes).expect("canonical Circle product loader");

    assert_eq!(render.nodes.len(), 1);
    assert_eq!(render.nodes[0].kind, NodeKind::Circle);
    assert_eq!(render.nodes[0].fill_paint, None);
    assert_eq!(render.nodes[0].stroke_ref, Some(0));
    assert!(matches!(
        render.geometries[0].data,
        GeometryData::Circle { center, radius } if center != u32::MAX && radius != u32::MAX
    ));
    assert_eq!(render.paints.len(), 1);
    assert_eq!(render.strokes.len(), 1);
    assert_eq!(render.strokes[0].id, scene.strokes()[0].id().value());
    assert_eq!(render.strokes[0].cap, 1);
    assert_eq!(render.strokes[0].join, 1);
    assert_eq!(render.strokes[0].miter_limit.to_bits(), 4.0f64.to_bits());
    assert!(render.strokes[0].dash.is_empty());
    assert_eq!(
        render.core.descriptors[render.strokes[0].width_descriptor as usize].property_type,
        fcs_fcbc::ValueType::Length
    );

    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("Circle semantic draw list");
    assert_eq!(draw.len(), 1);
    assert_eq!(draw[0].kind, NodeKind::Circle);
    let stroke = draw[0].stroke.as_ref().expect("Circle stroke draw op");
    // The only Length descriptor in the fixture is the 5px radius, so the ring the raster
    // assertions below rely on is exactly `[radius - width/2, radius + width/2]`.
    assert_eq!(stroke.width.to_bits(), 5.0f64.to_bits());

    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 16, 16).expect("Circle stroke raster");
    assert_eq!(pixels.len(), 16 * 16 * 4);
    // Render section 15.2 dilates the centre line, so a stroke-only Circle covers the
    // annulus and leaves the interior empty.
    assert_eq!(centre_pixel(&pixels, 16)[3], 0);
    assert!(
        pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0),
        "a solid Circle stroke must contribute annulus coverage"
    );
}

#[test]
fn canonical_circle_fill_and_stroke_both_reach_the_raster() {
    let compilation = canonical_circle_stroke_compilation(Vec::new(), true);
    let bytes = write_from_compilation(&compilation).expect("canonical Circle FCBC writing");
    let render = load_render(&bytes).expect("canonical Circle product loader");

    // The writer orders the paint table by stable id, so the encoded indices are not the
    // canonical table order. Assert the invariant that matters: both references resolve and
    // Render section 14.2 keeps the fill and the stroke on separate paint records.
    assert_eq!(render.paints.len(), 2);
    assert_eq!(render.strokes.len(), 1);
    assert_eq!(render.nodes[0].stroke_ref, Some(0));
    let fill_paint = render.nodes[0].fill_paint.expect("Circle fill paint");
    assert_ne!(fill_paint, render.strokes[0].paint_ref);

    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 16, 16).expect("Circle raster");
    // Render section 7 emits the fill draw op before the stroke draw op for the same node,
    // so declaring both keeps the interior covered instead of replacing it.
    assert_ne!(centre_pixel(&pixels, 16)[3], 0);
}

#[test]
fn canonical_circle_without_fill_or_stroke_is_rejected() {
    let compilation = canonical_circle_stroke_compilation(Vec::new(), false);
    let scene = compilation
        .chart()
        .render()
        .expect("canonical Circle scene");
    let node = &scene.nodes()[0];
    let error = CanonicalRenderNode::new(CanonicalRenderNodeSpec {
        id: node.id().clone(),
        kind: CanonicalRenderNodeKind::Circle,
        parent: node.parent(),
        layer: node.layer(),
        document_order: node.document_order(),
        z_order: node.z_order(),
        attachment: node.attachment().clone(),
        active: node.active(),
        isolate: node.isolate(),
        follow_hidden_attachment: node.follow_hidden_attachment(),
        position: node.position(),
        origin: node.origin(),
        rotation: node.rotation(),
        scale: node.scale(),
        opacity: node.opacity(),
        visibility: node.visibility(),
        geometry: Some(0),
        fill_paint: None,
        stroke: None,
        clip: None,
        composite: node.composite(),
    })
    .expect_err("a Circle with neither fill nor stroke must be rejected");
    assert_eq!(error, CanonicalRenderError::DrawableWithoutPaint);
}

#[test]
fn canonical_dashed_circle_stroke_reaches_the_product_raster() {
    let solid = write_from_compilation(&canonical_circle_stroke_compilation(Vec::new(), false))
        .expect("solid Circle FCBC writing");
    let solid = load_render(&solid).expect("solid Circle product loader");
    let solid = rasterize_solid_rgba8_at(&solid, 0.0, 16, 16).expect("solid Circle raster");

    // Render section 15.2 fixes the closed subpath's start at the local `+X` crossing and its
    // direction as clockwise, so the writer no longer has an undefined dash origin to reject.
    let compilation = canonical_circle_stroke_compilation(vec![1.0, 2.0], false);
    let bytes = write_from_compilation(&compilation).expect("dashed Circle FCBC writing");
    let render = load_render(&bytes).expect("dashed Circle product loader");
    assert_eq!(render.strokes[0].dash.len(), 2);
    let dashed = rasterize_solid_rgba8_at(&render, 0.0, 16, 16).expect("dashed Circle raster");

    let covered = |pixels: &[u8]| {
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] != 0)
            .count()
    };
    // One-on two-off leaves gaps in the annulus that the solid stroke covers, and the dash
    // phase origin is what decides where those gaps fall.
    assert!(covered(&dashed) > 0, "a dashed Circle stroke must raster");
    assert!(covered(&dashed) < covered(&solid));
}

#[test]
fn source_line_stroke_reaches_product_render_loader() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 4px; height: 4px; }
    layer main {
        pass: "overlay";
        children {
            line guide {
                start: vec2(-1px, 0px);
                end: vec2(1px, 0px);
                stroke: solid(choose {
                    when s < 1s => #FFFFFFFF;
                    else => #FF0000FF;
                });
                width: choose {
                    when s < 1s => 1px;
                    else => 2px;
                };
                cap: "round";
                join: "bevel";
                miterLimit: 4.0;
                dash: [1px, 1px, 1px];
                dashOffset: choose {
                    when s < 1s => 0px;
                    else => 0.5px;
                };
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("source Line stroke parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| {
            panic!("source Line stroke lowering failed: {diagnostics:?}")
        });
    let scene = compilation
        .chart()
        .render()
        .expect("canonical Render scene");
    let node_id = derive_stable_id(EntityKind::RenderNode, "layer/main/node/guide");
    let geometry_id = derive_stable_id(
        EntityKind::RenderGeometry,
        &format!("owner/{node_id:016x}/field/geometryRef/ordinal/0"),
    );
    let stroke_id = derive_stable_id(
        EntityKind::RenderStroke,
        &format!("owner/{node_id:016x}/field/strokeRef/ordinal/0"),
    );
    let paint_id = derive_stable_id(
        EntityKind::RenderPaint,
        &format!("owner/{stroke_id:016x}/field/paintRef/ordinal/0"),
    );
    assert_eq!(
        scene.layers()[0].id().value(),
        derive_stable_id(EntityKind::RenderLayer, "layer/main")
    );
    assert_eq!(scene.nodes()[0].id().value(), node_id);
    assert_eq!(scene.geometries()[0].id().value(), geometry_id);
    assert_eq!(scene.strokes()[0].id().value(), stroke_id);
    assert_eq!(
        scene.paints()[scene.strokes()[0].paint()].id().value(),
        paint_id
    );
    assert_eq!(scene.nodes()[0].kind(), CanonicalRenderNodeKind::Line);
    assert_eq!(scene.nodes()[0].fill_paint(), None);
    assert_eq!(scene.nodes()[0].stroke(), Some(0));
    assert!(matches!(
        scene.geometries()[0].data(),
        CanonicalRenderGeometryData::Line { .. }
    ));
    assert_eq!(scene.strokes().len(), 1);
    assert_eq!(scene.strokes()[0].cap(), CanonicalStrokeCap::Round);
    assert_eq!(scene.strokes()[0].join(), CanonicalStrokeJoin::Bevel);
    assert_eq!(scene.strokes()[0].miter_limit().to_bits(), 4.0f64.to_bits());
    assert_eq!(scene.strokes()[0].dash(), &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
    let descriptors = compilation
        .chart()
        .descriptors()
        .expect("Render descriptors");
    for path in ["render.stroke.width", "render.stroke.dashOffset"] {
        let root = descriptors
            .roots()
            .iter()
            .find(|root| {
                root.target_path() == path && root.owner() == scene.strokes()[0].id().value()
            })
            .expect("dynamic stroke descriptor root");
        assert!(matches!(
            descriptors.descriptors()[root.descriptor()].kind(),
            CanonicalDescriptorKind::Expression(expression)
                if expression.required_environment().contains(&CanonicalExpressionEnvironment::S)
        ));
    }
    let paint_root = descriptors
        .roots()
        .iter()
        .find(|root| {
            root.target_path() == "render.paint.color"
                && root.owner() == scene.paints()[0].id().value()
        })
        .expect("dynamic solid color root");
    assert!(matches!(
        descriptors.descriptors()[paint_root.descriptor()].kind(),
        CanonicalDescriptorKind::Expression(expression)
            if expression.required_environment().contains(&CanonicalExpressionEnvironment::S)
    ));

    let bytes = write_from_compilation(&compilation).expect("source Line stroke FCBC writing");
    let render = load_render(&bytes).expect("source Line stroke product loader");
    assert_eq!(render.nodes[0].kind, NodeKind::Line);
    assert_eq!(render.nodes[0].fill_paint, None);
    assert_eq!(render.nodes[0].stroke_ref, Some(0));
    assert_eq!((render.strokes[0].cap, render.strokes[0].join), (2, 3));
    assert_eq!(render.strokes[0].dash, vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("Line semantic draw list");
    assert_eq!(draw.len(), 1);
    let stroke = draw[0].stroke.as_ref().expect("Line stroke payload");
    assert_eq!((stroke.width, stroke.dash_offset), (1.0, 0.0));
    assert_eq!(stroke.fill_rgba, Some([1.0, 1.0, 1.0, 1.0]));
    let later = evaluate_semantic_draw_list_at(&render, 2.0).expect("later Line draw list");
    let stroke = later[0].stroke.as_ref().expect("later Line stroke payload");
    assert_eq!((stroke.width, stroke.dash_offset), (2.0, 0.5));
    assert_eq!(stroke.fill_rgba, Some([1.0, 0.0, 0.0, 1.0]));
    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 4, 4).expect("Line rasterization");
    assert!(pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0));

    let invalid = source.replace(
        "width: choose {\n                    when s < 1s => 1px;\n                    else => 2px;\n                };",
        "width: -1px;",
    );
    let document = parse_document(&invalid)
        .into_result()
        .expect("negative-width Line source parses");
    assert!(
        document
            .canonical_compilation_with_source(
                &invalid,
                CompileTimeLimits::default(),
                env!("CARGO_MANIFEST_DIR"),
                ResourceLimits::default(),
            )
            .is_err(),
        "negative stroke width must fail at canonical lowering"
    );
}

/// A source `circle` with the flat stroke fields Render uses for `line`.
fn source_circle(fill: &str, stroke: &str) -> String {
    format!(
        r#"#fcs 5.0.0
format {{ profile: renderable; }}
tempoMap {{ 0beat -> 120bpm; }}
render profile 1.0.0 {{
    viewport {{ width: 16px; height: 16px; }}
    layer main {{
        pass: "overlay";
        children {{
            circle ring {{
                center: vec2(0px, 0px);
                radius: 5px;
{fill}{stroke}            }}
        }}
    }}
}}
"#
    )
}

const SOURCE_CIRCLE_FILL: &str = "                fill: solid(#FF0000FF);\n";
const SOURCE_CIRCLE_STROKE: &str = "                stroke: solid(#FFFFFFFF);
                width: 2px;
                cap: \"butt\";
                join: \"miter\";
                miterLimit: 4.0;
                dash: [];
                dashOffset: 0px;
";

fn lower_source_circle(fill: &str, stroke: &str) -> Result<CanonicalCompilation, String> {
    let source = source_circle(fill, stroke);
    let document = parse_document(&source)
        .into_result()
        .expect("source Circle parses");
    document
        .canonical_compilation_with_source(
            &source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .map_err(|diagnostics| format!("{diagnostics:?}"))
}

#[test]
fn source_circle_stroke_reaches_product_render_loader() {
    let compilation = lower_source_circle("", SOURCE_CIRCLE_STROKE)
        .expect("stroke-only source Circle must lower");
    let scene = compilation.chart().render().expect("Render scene");
    // Render section 14.2 requires a fill paint or a stroke, so a declared stroke is what
    // makes `fill` optional. Before this path existed the stroke was silently dropped.
    assert_eq!(scene.strokes().len(), 1);
    assert!(scene.strokes()[0].dash().is_empty());
    assert_eq!(scene.nodes()[0].stroke(), Some(0));
    assert_eq!(scene.nodes()[0].fill_paint(), None);

    let bytes = write_from_compilation(&compilation).expect("source Circle stroke FCBC writing");
    let render = load_render(&bytes).expect("source Circle stroke product loader");
    assert_eq!(render.nodes[0].kind, NodeKind::Circle);
    assert_eq!(render.nodes[0].fill_paint, None);
    assert_eq!(render.nodes[0].stroke_ref, Some(0));
    assert!(render.strokes[0].dash.is_empty());

    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("Circle semantic draw list");
    assert_eq!(draw.len(), 1);
    assert!(draw[0].stroke.is_some());
    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 16, 16).expect("Circle stroke raster");
    // The 2px stroke dilates the 5px centre line into the ring `[4, 6]`, so the interior is
    // empty and the continuous ring is not.
    assert_eq!(centre_pixel(&pixels, 16)[3], 0);
    assert!(pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0));
}

#[test]
fn source_circle_fill_and_stroke_reach_the_product_raster() {
    let compilation = lower_source_circle(SOURCE_CIRCLE_FILL, SOURCE_CIRCLE_STROKE)
        .expect("fill-and-stroke source Circle must lower");
    let scene = compilation.chart().render().expect("Render scene");
    // Section 14.2 forbids sharing one paint record between a fill and a stroke.
    assert_eq!(scene.paints().len(), 2);
    assert!(scene.nodes()[0].fill_paint().is_some());
    assert_eq!(scene.nodes()[0].stroke(), Some(0));

    let bytes = write_from_compilation(&compilation).expect("source Circle FCBC writing");
    let render = load_render(&bytes).expect("source Circle product loader");
    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 16, 16).expect("source Circle raster");
    assert_ne!(centre_pixel(&pixels, 16)[3], 0);
}

#[test]
fn source_circle_without_fill_or_stroke_is_rejected() {
    let error = lower_source_circle("", "")
        .expect_err("a Circle with neither fill nor stroke must be rejected at lowering");
    assert!(error.contains("fill"), "{error}");
}

#[test]
fn source_ellipse_and_rounded_rect_strokes_reach_the_product_raster() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 20px; height: 12px; }
    layer main {
        pass: "overlay";
        children {
            ellipse oval {
                center: vec2(-4px, 0px);
                radiusX: 3px;
                radiusY: 2px;
                stroke: solid(#FFFFFFFF);
                width: 1px;
                cap: "round";
                join: "miter";
                miterLimit: 4.0;
                dash: [2px, 1px];
                dashOffset: 0px;
            }
            roundedRect box {
                origin: vec2(1px, -3px);
                size: vec2(6px, 6px);
                radius: 1px;
                stroke: solid(#FF0000FF);
                width: 1px;
                cap: "butt";
                join: "miter";
                miterLimit: 4.0;
                dash: [];
                dashOffset: 0px;
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("parametric stroke source parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| panic!("parametric strokes must lower: {diagnostics:?}"));
    let scene = compilation.chart().render().expect("Render scene");
    assert_eq!(scene.strokes().len(), 2);
    assert!(
        scene
            .nodes()
            .iter()
            .all(|node| node.stroke().is_some() && node.fill_paint().is_none())
    );

    let bytes = write_from_compilation(&compilation).expect("parametric stroke FCBC writing");
    let render = load_render(&bytes).expect("parametric stroke product loader");
    for kind in [NodeKind::Ellipse, NodeKind::RoundedRect] {
        let node = render
            .nodes
            .iter()
            .find(|node| node.kind == kind)
            .expect("decoded parametric node");
        assert_eq!(node.fill_paint, None);
        assert!(node.stroke_ref.is_some());
    }
    let ellipse = render
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Ellipse)
        .expect("decoded Ellipse");
    assert_eq!(
        render.strokes[ellipse.stroke_ref.expect("Ellipse stroke") as usize].dash,
        vec![2.0, 1.0]
    );
    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("parametric semantic draw list");
    assert_eq!(draw.len(), 2);
    assert!(draw.iter().all(|operation| operation.stroke.is_some()));
    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 20, 12).expect("parametric stroke raster");
    assert!(pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0));
}

/// Lower a source fixture whose hidden `circle ruler` supplies the scalar Length descriptor a
/// stroke needs, then attach a canonical stroke to the second node in place. Replacing the node
/// rather than rebuilding the scene keeps the layer roots and geometry indices valid, and
/// `CanonicalRenderScene` rejects a node whose kind does not match its geometry.
fn canonical_stroked_shape_compilation(
    keyword: &str,
    body: &str,
    dash: Vec<f64>,
) -> CanonicalCompilation {
    let source = format!(
        r#"#fcs 5.0.0
format {{ profile: renderable; }}
tempoMap {{ 0beat -> 120bpm; }}
render profile 1.0.0 {{
    viewport {{ width: 16px; height: 16px; }}
    layer main {{
        pass: "overlay";
        children {{
            circle ruler {{
                center: vec2(0px, 0px);
                radius: 2px;
                visibility: false;
                fill: solid(#FFFFFFFF);
            }}
            {keyword} target {{
                {body}
                fill: solid(#FFFFFFFF);
            }}
        }}
    }}
}}
"#
    );
    let document = parse_document(&source)
        .into_result()
        .expect("stroked shape source parses");
    let base = document
        .canonical_compilation_with_source(
            &source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| {
            panic!("stroked shape source lowering failed: {diagnostics:?}")
        });
    let original = base.chart().render().expect("source Render scene");
    let target = &original.nodes()[1];
    let width_descriptor = base
        .chart()
        .descriptors()
        .expect("source Render descriptors")
        .descriptors()
        .iter()
        .position(|descriptor| descriptor.property_type() == &CanonicalExpressionType::Length)
        .expect("the hidden ruler must provide a Length descriptor");
    let mut ids = StableIdRegistry::new();
    let stroke_id = ids
        .insert(
            EntityKind::RenderStroke,
            CanonicalTextualId::explicit("target-stroke").expect("stroke textual ID"),
        )
        .expect("stroke stable ID");
    // Section 14.2 forbids sharing a paint between a fill and a stroke, so the stroke takes over
    // the target's own paint record while the node drops its fill.
    let stroke_paint = target.fill_paint().expect("target fill paint");
    let stroke = CanonicalRenderStroke::new(
        stroke_id,
        stroke_paint,
        width_descriptor,
        CanonicalStrokeCap::Butt,
        CanonicalStrokeJoin::Miter,
        4.0,
        width_descriptor,
        dash,
    )
    .expect("canonical stroke");
    let stroked = CanonicalRenderNode::new(CanonicalRenderNodeSpec {
        id: target.id().clone(),
        kind: target.kind(),
        parent: target.parent(),
        layer: target.layer(),
        document_order: target.document_order(),
        z_order: target.z_order(),
        attachment: target.attachment().clone(),
        active: target.active(),
        isolate: target.isolate(),
        follow_hidden_attachment: target.follow_hidden_attachment(),
        position: target.position(),
        origin: target.origin(),
        rotation: target.rotation(),
        scale: target.scale(),
        opacity: target.opacity(),
        visibility: target.visibility(),
        geometry: target.geometry(),
        fill_paint: None,
        stroke: Some(0),
        clip: None,
        composite: target.composite(),
    })
    .expect("stroked canonical node");
    let mut nodes = original.nodes().to_vec();
    nodes[1] = stroked;
    let scene = CanonicalRenderScene::new(CanonicalRenderSceneSpec {
        viewport: original.viewport(),
        layers: original.layers().to_vec(),
        nodes,
        geometries: original.geometries().to_vec(),
        paths: Vec::new(),
        paints: original.paints().to_vec(),
        strokes: vec![stroke],
        clips: Vec::new(),
        glyph_runs: Vec::new(),
    })
    .expect("stroked canonical scene");
    CanonicalCompilation::new(
        base.chart().clone().with_render(scene),
        base.resources().clone(),
        base.distribution().clone(),
    )
}

const POLYLINE_POINTS: &str = "points: [vec2(-5px, 0px), vec2(0px, 0px), vec2(0px, 5px)];";

#[test]
fn canonical_polyline_and_polygon_strokes_reach_the_product_raster() {
    let raster = |keyword: &str, dash: Vec<f64>| {
        let compilation = canonical_stroked_shape_compilation(keyword, POLYLINE_POINTS, dash);
        let bytes = write_from_compilation(&compilation).expect("polyline stroke FCBC writing");
        let render = load_render(&bytes).expect("polyline stroke product loader");
        let pixels =
            rasterize_solid_rgba8_at(&render, 0.0, 16, 16).expect("polyline stroke raster");
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] != 0)
            .count()
    };

    let polyline = raster("polyline", Vec::new());
    let polygon = raster("polygon", Vec::new());
    let dashed = raster("polyline", vec![1.0, 2.0]);

    // Render section 15.2 keeps a Polyline stroke open and closes a Polygon stroke, so the
    // Polygon additionally strokes the implicit closing segment. The hidden ruler circle
    // contributes nothing, so every covered pixel comes from the stroke.
    assert!(polyline > 0, "a Polyline stroke must raster");
    assert!(
        polygon > polyline,
        "polygon {polygon} vs polyline {polyline}"
    );
    // One-on two-off leaves gaps in the same open path.
    assert!(dashed > 0, "a dashed Polyline stroke must raster");
    assert!(dashed < polyline, "dashed {dashed} vs polyline {polyline}");
}

#[test]
fn canonical_rect_strokes_reach_the_product_raster() {
    let compilation = canonical_stroked_shape_compilation(
        "rect",
        "origin: vec2(-3px, -3px);
                size: vec2(6px, 6px);",
        Vec::new(),
    );
    let bytes = write_from_compilation(&compilation).expect("solid Rect stroke FCBC writing");
    let render = load_render(&bytes).expect("solid Rect stroke product loader");
    assert_eq!(render.nodes[1].stroke_ref, Some(0));
    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("solid Rect stroke semantics");
    assert!(draw[0].stroke.is_some());
    let pixels =
        rasterize_solid_rgba8_at(&render, 0.0, 16, 16).expect("solid Rect stroke rasterization");
    let solid_coverage = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] != 0)
        .count();
    assert!(solid_coverage > 0);

    let dashed = canonical_stroked_shape_compilation(
        "rect",
        "origin: vec2(-3px, -3px);
                size: vec2(6px, 6px);",
        vec![2.0, 3.0],
    );
    let bytes = write_from_compilation(&dashed).expect("dashed Rect FCBC writing");
    let render = load_render(&bytes).expect("dashed Rect product loader");
    assert_eq!(render.strokes[0].dash, vec![2.0, 3.0]);
    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 16, 16).expect("dashed Rect rasterization");
    let dashed_coverage = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] != 0)
        .count();
    assert!(dashed_coverage > 0);
    assert!(dashed_coverage < solid_coverage);
}

/// A source `polyline` or `polygon` with the flat stroke fields Render uses for `line`.
fn source_points_shape(keyword: &str, fill: &str, stroke: &str) -> String {
    format!(
        r#"#fcs 5.0.0
format {{ profile: renderable; }}
tempoMap {{ 0beat -> 120bpm; }}
render profile 1.0.0 {{
    viewport {{ width: 16px; height: 16px; }}
    layer main {{
        pass: "overlay";
        children {{
            {keyword} trace {{
                points: [vec2(-5px, 0px), vec2(0px, 0px), vec2(0px, 5px)];
{fill}{stroke}            }}
        }}
    }}
}}
"#
    )
}

const SOURCE_POINTS_FILL: &str = "                fill: solid(#FF0000FF);
";
const SOURCE_POINTS_STROKE: &str = "                stroke: solid(#FFFFFFFF);
                width: 2px;
                cap: \"butt\";
                join: \"miter\";
                miterLimit: 4.0;
                dash: [3px, 2px];
                dashOffset: 0px;
";

fn lower_source_points_shape(
    keyword: &str,
    fill: &str,
    stroke: &str,
) -> Result<CanonicalCompilation, String> {
    let source = source_points_shape(keyword, fill, stroke);
    let document = parse_document(&source)
        .into_result()
        .expect("source points shape parses");
    document
        .canonical_compilation_with_source(
            &source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .map_err(|diagnostics| format!("{diagnostics:?}"))
}

#[test]
fn source_polyline_and_polygon_strokes_reach_the_product_raster() {
    for keyword in ["polyline", "polygon"] {
        let compilation = lower_source_points_shape(keyword, "", SOURCE_POINTS_STROKE)
            .unwrap_or_else(|error| panic!("stroke-only source {keyword} must lower: {error}"));
        let scene = compilation.chart().render().expect("Render scene");
        // The stroke used to be silently dropped, because canonical lowering computed one only
        // for Line and Circle.
        assert_eq!(scene.strokes().len(), 1);
        assert_eq!(scene.nodes()[0].stroke(), Some(0));
        assert_eq!(scene.nodes()[0].fill_paint(), None);

        let bytes = write_from_compilation(&compilation).expect("source stroke FCBC writing");
        let render = load_render(&bytes).expect("source stroke product loader");
        assert_eq!(render.nodes[0].fill_paint, None);
        assert_eq!(render.nodes[0].stroke_ref, Some(0));
        let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("semantic draw list");
        assert!(draw[0].stroke.is_some());
        let pixels = rasterize_solid_rgba8_at(&render, 0.0, 16, 16).expect("stroke raster");
        assert!(
            pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0),
            "a source {keyword} stroke must raster"
        );
    }
}

#[test]
fn source_polygon_fill_and_stroke_keep_separate_paint_records() {
    let compilation =
        lower_source_points_shape("polygon", SOURCE_POINTS_FILL, SOURCE_POINTS_STROKE)
            .expect("fill-and-stroke source polygon must lower");
    let scene = compilation.chart().render().expect("Render scene");
    // Section 14.2 forbids sharing one paint record between a fill and a stroke.
    assert_eq!(scene.paints().len(), 2);
    assert!(scene.nodes()[0].fill_paint().is_some());
    assert_eq!(scene.nodes()[0].stroke(), Some(0));
    write_from_compilation(&compilation).expect("fill-and-stroke source polygon FCBC writing");
}

#[test]
fn source_polyline_without_fill_or_stroke_is_rejected() {
    let error = lower_source_points_shape("polyline", "", "")
        .expect_err("a polyline with neither fill nor stroke must be rejected at lowering");
    assert!(error.contains("fill"), "{error}");
}

#[test]
fn source_line_without_a_stroke_is_still_rejected() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 4px; height: 4px; }
    layer main {
        pass: "overlay";
        children {
            line guide {
                start: vec2(-1px, 0px);
                end: vec2(1px, 0px);
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("stroke-less Line source parses");
    // Making the Circle stroke optional must not make the Line stroke optional.
    assert!(
        document
            .canonical_compilation_with_source(
                source,
                CompileTimeLimits::default(),
                env!("CARGO_MANIFEST_DIR"),
                ResourceLimits::default(),
            )
            .is_err()
    );
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
                fill: linearGradient(choose {
                    when s < 1s => vec2(-4px, 0px);
                    else => vec2(-2px, 0px);
                }, choose {
                    when s < 1s => vec2(4px, 0px);
                    else => vec2(2px, 0px);
                }, [
                    stop(0.0, choose {
                        when s < 1s => #FF0000FF;
                        else => #00FF00FF;
                    }),
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
    let descriptors = compilation
        .chart()
        .descriptors()
        .expect("gradient descriptors");
    for path in [
        "render.paint.start",
        "render.paint.end",
        "render.paint.stop[0].color",
    ] {
        let root = descriptors
            .roots()
            .iter()
            .find(|root| root.target_path() == path)
            .expect("dynamic LinearGradient root");
        assert!(matches!(
            descriptors.descriptors()[root.descriptor()].kind(),
            CanonicalDescriptorKind::Expression(expression)
                if expression.required_environment().contains(&CanonicalExpressionEnvironment::S)
        ));
    }

    let bytes = write_from_compilation(&compilation).expect("linear gradient FCBC writing");
    let render = load_render(&bytes).expect("linear gradient product loader");
    assert!(matches!(
        render.paints[0].data,
        PaintData::LinearGradient { spread: 2, ref stops, .. } if stops.len() == 2
    ));

    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("gradient semantic evaluation");
    let gradient = draw
        .iter()
        .find_map(|op| op.linear_gradient.as_ref())
        .expect("LinearGradient payload");
    assert_eq!((gradient.start, gradient.end), ([-4.0, 0.0], [4.0, 0.0]));
    assert_eq!(gradient.stops[0].color, [1.0, 0.0, 0.0, 1.0]);
    let later = evaluate_semantic_draw_list_at(&render, 2.0).expect("later gradient semantics");
    let gradient = later
        .iter()
        .find_map(|op| op.linear_gradient.as_ref())
        .expect("later LinearGradient payload");
    assert_eq!((gradient.start, gradient.end), ([-2.0, 0.0], [2.0, 0.0]));
    assert_eq!(gradient.stops[0].color, [0.0, 1.0, 0.0, 1.0]);
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
                    choose {
                        when s < 1s => vec2(0px, 0px);
                        else => vec2(-1px, 0px);
                    },
                    choose {
                        when s < 1s => 0px;
                        else => 1px;
                    },
                    choose {
                        when s < 1s => vec2(0px, 0px);
                        else => vec2(1px, 0px);
                    },
                    choose {
                        when s < 1s => 4px;
                        else => 3px;
                    },
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
    let descriptors = compilation
        .chart()
        .descriptors()
        .expect("radial gradient descriptors");
    for path in [
        "render.paint.startCenter",
        "render.paint.startRadius",
        "render.paint.endCenter",
        "render.paint.endRadius",
    ] {
        let root = descriptors
            .roots()
            .iter()
            .find(|root| root.target_path() == path)
            .expect("dynamic RadialGradient root");
        assert!(matches!(
            descriptors.descriptors()[root.descriptor()].kind(),
            CanonicalDescriptorKind::Expression(expression)
                if expression.required_environment().contains(&CanonicalExpressionEnvironment::S)
        ));
    }

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
    let gradient = draw
        .iter()
        .find_map(|op| op.radial_gradient.as_ref())
        .expect("RadialGradient payload");
    assert_eq!(
        (
            gradient.start_center,
            gradient.start_radius,
            gradient.end_center,
            gradient.end_radius,
        ),
        ([0.0, 0.0], 0.0, [0.0, 0.0], 4.0)
    );
    let later = evaluate_semantic_draw_list_at(&render, 2.0).expect("later radial semantics");
    let gradient = later
        .iter()
        .find_map(|op| op.radial_gradient.as_ref())
        .expect("later RadialGradient payload");
    assert_eq!(
        (
            gradient.start_center,
            gradient.start_radius,
            gradient.end_center,
            gradient.end_radius,
        ),
        ([-1.0, 0.0], 1.0, [1.0, 0.0], 3.0)
    );
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

    let invalid = source.replace("else => 1px;", "else => -1px;");
    let document = parse_document(&invalid)
        .into_result()
        .expect("dynamic negative-radius source parses");
    let compilation = document
        .canonical_compilation_with_source(
            &invalid,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .expect("dynamic radius is validated at query time");
    let bytes = write_from_compilation(&compilation).expect("dynamic negative-radius FCBC writing");
    let render = load_render(&bytes).expect("dynamic negative-radius product loader");
    assert_eq!(
        evaluate_semantic_draw_list_at(&render, 2.0).expect_err("negative radius must fail"),
        "render.invalid-paint"
    );
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
    let scene = compilation
        .chart()
        .render()
        .expect("canonical Render scene");
    assert_eq!(
        scene.layers()[0].id().value(),
        derive_stable_id(EntityKind::RenderLayer, "layer/main")
    );
    for (kind, textual) in [
        (CanonicalRenderNodeKind::Group, "layer/main/node/root"),
        (
            CanonicalRenderNodeKind::Circle,
            "layer/main/node/root/node/circleShape",
        ),
        (
            CanonicalRenderNodeKind::RoundedRect,
            "layer/main/node/root/node/roundedShape",
        ),
        (
            CanonicalRenderNodeKind::Ellipse,
            "layer/main/node/root/node/ellipseShape",
        ),
    ] {
        let node = scene
            .nodes()
            .iter()
            .find(|node| node.kind() == kind)
            .expect("nested source node kind");
        assert_eq!(
            node.id().value(),
            derive_stable_id(EntityKind::RenderNode, textual)
        );
    }

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
fn nested_isolated_groups_composite_offscreen_and_preserve_empty_copy() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 2px; height: 2px; colorSpace: "linear-srgb"; }
    layer main {
        pass: "overlay";
        children {
            rect background {
                origin: vec2(-1px, -1px);
                size: vec2(2px, 2px);
                fill: solid(#0000FFFF);
            }
            group outer {
                isolate: true;
                opacity: 0.5;
                composite: "copy";
                children {
                    group inner {
                        isolate: true;
                        opacity: 0.5;
                        composite: "sourceOver";
                        children {
                            rect first {
                                origin: vec2(-1px, -1px);
                                size: vec2(2px, 2px);
                                visibility: choose {
                                    when s < 1s => true;
                                    else => false;
                                };
                                fill: solid(#FF0000FF);
                            }
                            rect second {
                                origin: vec2(-1px, -1px);
                                size: vec2(2px, 2px);
                                visibility: choose {
                                    when s < 1s => true;
                                    else => false;
                                };
                                fill: solid(#FF0000FF);
                            }
                        }
                    }
                }
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("nested isolation source parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| panic!("nested isolation lowering failed: {diagnostics:?}"));
    let bytes = write_from_compilation(&compilation).expect("nested isolation FCBC writing");
    let render = load_render(&bytes).expect("nested isolation product loader");

    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("nested isolation semantics");
    let isolated: Vec<_> = draw
        .iter()
        .filter(|operation| operation.isolation_chain.len() == 2)
        .collect();
    assert_eq!(isolated.len(), 2);
    assert!(isolated.iter().all(|operation| operation.opacity == 1.0));
    assert!(isolated.iter().all(|operation| {
        operation
            .isolation_chain
            .iter()
            .map(|boundary| (boundary.opacity, boundary.composite))
            .eq([(0.5, 2), (0.5, 1)])
    }));
    assert_eq!(
        rasterize_solid_rgba8_at(&render, 0.0, 1, 1).expect("nested isolation raster"),
        vec![255, 0, 0, 64]
    );
    assert_eq!(
        rasterize_solid_rgba8_at(&render, 2.0, 1, 1).expect("empty isolation raster"),
        vec![0, 0, 0, 0]
    );
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
        pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0),
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
    assert!(pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0));
}

#[test]
fn source_image_pattern_sibling_fields_reach_product_raster() {
    let source = include_str!("../../../docs/conformance/render/image-pattern-sibling-fields.fcs");
    let document = parse_document(source)
        .into_result()
        .expect("ImagePattern source parses");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/conformance/render");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            &workspace,
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| {
            panic!("ImagePattern canonical lowering failed: {diagnostics:?}")
        });
    let scene = compilation.chart().render().expect("source Render scene");
    let node = &scene.nodes()[0];
    let fill_index = node.fill_paint().expect("ImagePattern fill");
    let stroke_index = node.stroke().expect("ImagePattern stroke");
    let stroke_paint_index = scene.strokes()[stroke_index].paint();
    assert_eq!(scene.strokes()[stroke_index].dash(), &[1.0, 2.0]);
    let (fill_resource, fill_transform, fill_repeat, fill_sampling) =
        match scene.paints()[fill_index].data() {
            fcs_model::CanonicalRenderPaintData::ImagePattern {
                resource,
                transform,
                repeat,
                sampling,
            } => (resource.value(), *transform, *repeat, *sampling),
            _ => panic!("expected source ImagePattern fill"),
        };
    let (stroke_resource, stroke_transform, stroke_repeat, stroke_sampling) =
        match scene.paints()[stroke_paint_index].data() {
            fcs_model::CanonicalRenderPaintData::ImagePattern {
                resource,
                transform,
                repeat,
                sampling,
            } => (resource.value(), *transform, *repeat, *sampling),
            _ => panic!("expected source ImagePattern stroke"),
        };
    assert_ne!(fill_resource, stroke_resource);
    assert_eq!(fill_transform, stroke_transform);
    assert_eq!(fill_repeat.ordinal(), 4);
    assert_eq!(stroke_repeat.ordinal(), 4);
    assert_eq!(fill_sampling.ordinal(), 2);
    assert_eq!(stroke_sampling.ordinal(), 1);

    let descriptors = compilation
        .chart()
        .descriptors()
        .expect("ImagePattern descriptors");
    let position_roots = descriptors
        .roots()
        .iter()
        .filter(|root| root.target_path() == "render.paint.position")
        .collect::<Vec<_>>();
    assert_eq!(position_roots.len(), 2);
    assert_eq!(
        position_roots[0].descriptor(),
        position_roots[1].descriptor()
    );
    assert!(matches!(
        descriptors.descriptors()[position_roots[0].descriptor()].kind(),
        CanonicalDescriptorKind::Expression(expression)
            if expression.required_environment().contains(&CanonicalExpressionEnvironment::S)
    ));

    let bytes = write_from_compilation(&compilation).expect("ImagePattern FCBC writing");
    let render = load_render(&bytes).expect("ImagePattern product Render loader");
    let node = &render.nodes[0];
    let fill_index = node.fill_paint.expect("decoded ImagePattern fill") as usize;
    let stroke = &render.strokes[node.stroke_ref.expect("decoded ImagePattern stroke") as usize];
    assert_eq!(stroke.dash, vec![1.0, 2.0]);
    let stroke_index = stroke.paint_ref as usize;
    let PaintData::ImagePattern {
        resource_id: decoded_fill_resource,
        position: fill_position,
        origin: fill_origin,
        rotation: fill_rotation,
        scale: fill_scale,
        repeat: decoded_fill_repeat,
        sampling: decoded_fill_sampling,
    } = render.paints[fill_index].data
    else {
        panic!("expected decoded ImagePattern fill");
    };
    let PaintData::ImagePattern {
        resource_id: decoded_stroke_resource,
        position: stroke_position,
        origin: stroke_origin,
        rotation: stroke_rotation,
        scale: stroke_scale,
        repeat: decoded_stroke_repeat,
        sampling: decoded_stroke_sampling,
    } = render.paints[stroke_index].data
    else {
        panic!("expected decoded ImagePattern stroke");
    };
    assert_eq!(decoded_fill_resource, fill_resource);
    assert_eq!(decoded_stroke_resource, stroke_resource);
    assert_eq!(
        [fill_position, fill_origin, fill_rotation, fill_scale],
        [
            stroke_position,
            stroke_origin,
            stroke_rotation,
            stroke_scale
        ]
    );
    assert_eq!((decoded_fill_repeat, decoded_fill_sampling), (4, 2));
    assert_eq!((decoded_stroke_repeat, decoded_stroke_sampling), (4, 1));
    assert_eq!(render.resources.len(), 2);

    let draw =
        evaluate_semantic_draw_list_at(&render, 0.0).expect("ImagePattern semantic evaluation");
    let operation = draw
        .iter()
        .find(|operation| operation.kind == NodeKind::Rect)
        .expect("ImagePattern draw operation");
    let fill = operation.image_pattern.expect("ImagePattern fill payload");
    let stroke_payload = operation
        .stroke
        .as_ref()
        .expect("ImagePattern stroke payload");
    assert_eq!(stroke_payload.dash, vec![1.0, 2.0]);
    let stroke = stroke_payload
        .image_pattern
        .expect("ImagePattern stroke payload");
    assert_eq!(
        (fill.resource_id, fill.repeat, fill.sampling),
        (fill_resource, 4, 2)
    );
    assert_eq!(
        (stroke.resource_id, stroke.repeat, stroke.sampling),
        (stroke_resource, 4, 1)
    );
    assert_eq!(fill.position, [0.0, 0.0]);
    assert_eq!(fill.position, stroke.position);
    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 4, 4).expect("ImagePattern rasterization");
    assert_eq!(pixels.len(), 4 * 4 * 4);
    assert!(pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0));
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
    let stroke_paint_id = ids
        .insert(
            EntityKind::RenderPaint,
            CanonicalTextualId::explicit("writer-path-stroke-paint")
                .expect("Path stroke paint textual ID"),
        )
        .expect("Path stroke paint stable ID");
    let stroke_paint = CanonicalRenderPaint::new(
        stroke_paint_id,
        original.paints()[path_node.fill_paint().expect("Path fill paint")]
            .data()
            .clone(),
    )
    .expect("Path stroke paint");
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
    let stroke_id = ids
        .insert(
            EntityKind::RenderStroke,
            CanonicalTextualId::explicit("writer-path-stroke").expect("Path stroke textual ID"),
        )
        .expect("Path stroke stable ID");
    let stroke = CanonicalRenderStroke::new(
        stroke_id,
        original.paints().len(),
        *radius,
        CanonicalStrokeCap::Round,
        CanonicalStrokeJoin::Bevel,
        4.0,
        *radius,
        vec![1.0, 1.0],
    )
    .expect("canonical Path stroke");
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
        stroke: Some(0),
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
        paints: original
            .paints()
            .iter()
            .cloned()
            .chain([stroke_paint])
            .collect(),
        strokes: vec![stroke],
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
    let reference = fcbc_render_reference_loader::load_render(&bytes)
        .expect("canonical Path independent loader");

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
    assert_eq!(render.paints.len(), 3);
    assert_eq!(render.nodes[decoded_node_index].stroke_ref, Some(0));
    assert_eq!(render.strokes.len(), 1);
    let decoded_stroke = &render.strokes[0];
    assert_eq!(decoded_stroke.id, scene.strokes()[0].id().value());
    assert_eq!(
        render.paints[decoded_stroke.paint_ref as usize].id,
        scene.paints()[scene.strokes()[0].paint()].id().value()
    );
    assert_eq!((decoded_stroke.cap, decoded_stroke.join), (2, 3));
    assert_eq!(decoded_stroke.miter_limit.to_bits(), 4.0f64.to_bits());
    assert_eq!(
        decoded_stroke.width_descriptor,
        decoded_stroke.dash_offset_descriptor
    );
    assert_eq!(decoded_stroke.dash, vec![1.0, 1.0]);

    let reference_node = reference
        .nodes
        .iter()
        .find(|node| node.id == scene.nodes()[path_node_index].id().value())
        .expect("independently decoded Path node");
    let reference_geometry_index = reference
        .geometries
        .iter()
        .position(|geometry| geometry.id == scene.geometries()[path_geometry_index].id().value())
        .expect("independently decoded Path geometry");
    assert_eq!(
        reference_node.kind,
        fcbc_render_reference_loader::NodeKind::Path
    );
    assert_eq!(
        reference_node.geometry_ref,
        Some(reference_geometry_index as u32)
    );
    assert!(matches!(
        reference.geometries[reference_geometry_index].data,
        fcbc_render_reference_loader::GeometryData::Path { path_ref: 0 }
    ));
    assert_eq!(reference.paths.len(), 1);
    assert_eq!(reference.paths[0].id, scene.paths()[0].id().value());
    assert_eq!(reference.paths[0].fill_rule, 1);
    assert_eq!(reference_node.stroke_ref, Some(0));
    assert_eq!(reference.strokes.len(), 1);
    let reference_stroke = &reference.strokes[0];
    assert_eq!(reference_stroke.id, scene.strokes()[0].id().value());
    assert_eq!(
        reference.paints[reference_stroke.paint_ref as usize].id,
        scene.paints()[scene.strokes()[0].paint()].id().value()
    );
    assert_eq!((reference_stroke.cap, reference_stroke.join), (2, 3));
    assert_eq!(reference_stroke.miter_limit.to_bits(), 4.0f64.to_bits());
    assert_eq!(
        reference_stroke.width_descriptor,
        reference_stroke.dash_offset_descriptor
    );
    assert_eq!(reference_stroke.dash, vec![1.0, 1.0]);
    use fcbc_render_reference_loader::PathCommand as ReferencePathCommand;
    let [
        ReferencePathCommand::MoveTo(move_to),
        ReferencePathCommand::LineTo(line_to),
        ReferencePathCommand::QuadraticTo(quadratic_control, quadratic_end),
        ReferencePathCommand::CubicTo(cubic_control_1, cubic_control_2, _),
        ReferencePathCommand::Arc {
            center: arc_center,
            radius: arc_radius,
            start_angle: arc_start,
            end_angle: arc_end,
            direction: arc_direction,
        },
        ReferencePathCommand::EllipseArc {
            center: ellipse_center,
            radius_x,
            radius_y,
            rotation,
            start_angle: ellipse_start,
            end_angle: ellipse_end,
            direction: ellipse_direction,
        },
        ReferencePathCommand::Close,
    ] = reference.paths[0].commands.as_slice()
    else {
        panic!("independent loader must recover every canonical Path command in order");
    };
    assert_eq!((quadratic_control, quadratic_end), (move_to, line_to));
    assert_eq!((cubic_control_1, cubic_control_2), (move_to, line_to));
    assert_eq!(arc_start, arc_end);
    assert_eq!(*arc_direction, 1);
    assert_eq!(ellipse_center, arc_center);
    assert_eq!(radius_x, arc_radius);
    assert_eq!(radius_y, arc_radius);
    assert_eq!(rotation, arc_start);
    assert_eq!(ellipse_start, arc_start);
    assert_eq!(ellipse_end, arc_start);
    assert_eq!(*ellipse_direction, 2);
    assert_eq!(
        format!("{:?}", reference.paths[0].commands),
        format!("{:?}", render.paths[0].commands),
        "product and independent loaders must recover the same command sequence and descriptor references"
    );

    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("Path semantic evaluation");
    let path_draw = draw
        .iter()
        .find(|operation| operation.kind == NodeKind::Path)
        .expect("Path semantic draw op");
    assert!(path_draw.bounds[0] < path_draw.bounds[2]);
    assert!(path_draw.bounds[1] < path_draw.bounds[3]);
    assert!(path_draw.fill_rgba.is_some());
    assert!(path_draw.stroke.is_some());

    let mut stroke_only = render.clone();
    for node in &mut stroke_only.nodes {
        if node.kind == NodeKind::Path {
            node.fill_paint = None;
        } else {
            node.geometry_ref = None;
            node.fill_paint = None;
            node.stroke_ref = None;
        }
    }
    let pixels =
        rasterize_solid_rgba8_at(&stroke_only, 0.0, 4, 4).expect("Path stroke rasterization");
    assert_eq!(pixels.len(), 4 * 4 * 4);
    assert!(
        pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0),
        "written Path stroke should contribute raster coverage without its fill"
    );
}

#[test]
fn source_path_commands_reach_the_product_raster() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 20px; height: 16px; }
    layer main {
        pass: "overlay";
        children {
            path outline {
                commands: [
                    moveTo(vec2(-6px, -4px)),
                    lineTo(choose {
                        when s < 1s => vec2(6px, -4px);
                        else => vec2(5px, -4px);
                    }),
                    quadraticTo(vec2(7px, 0px), vec2(6px, 4px)),
                    cubicTo(vec2(2px, 6px), vec2(-2px, 6px), vec2(-6px, 4px)),
                    arc(vec2(-6px, 2px), 2px, 1.5707963267948966rad, 3.141592653589793rad, "counterClockwise"),
                    ellipseArc(vec2(-6px, 0px), 2px, 1px, 0rad, 0rad, -1.5707963267948966rad, "clockwise"),
                    close(),
                ];
                fillRule: "evenodd";
                fill: solid(#FF0000FF);
                stroke: solid(#FFFFFFFF);
                width: 1px;
                cap: "round";
                join: "bevel";
                miterLimit: 4.0;
                dash: [];
                dashOffset: 0px;
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("source Path parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| panic!("source Path lowering failed: {diagnostics:?}"));
    let scene = compilation.chart().render().expect("source Path scene");
    assert_eq!(scene.paths().len(), 1);
    assert_eq!(
        scene.paths()[0].fill_rule(),
        CanonicalRenderFillRule::EvenOdd
    );
    assert!(matches!(
        scene.paths()[0].commands(),
        [
            CanonicalPathCommand::MoveTo(_),
            CanonicalPathCommand::LineTo(_),
            CanonicalPathCommand::QuadraticTo(_, _),
            CanonicalPathCommand::CubicTo(_, _, _),
            CanonicalPathCommand::Arc { .. },
            CanonicalPathCommand::EllipseArc { .. },
            CanonicalPathCommand::Close,
        ]
    ));
    let descriptors = compilation.chart().descriptors().expect("Path descriptors");
    let dynamic_point = descriptors
        .roots()
        .iter()
        .find(|root| root.target_path() == "render.path.command[1].point")
        .expect("dynamic lineTo point root");
    assert!(matches!(
        descriptors.descriptors()[dynamic_point.descriptor()].kind(),
        CanonicalDescriptorKind::Expression(expression)
            if expression.required_environment().contains(&CanonicalExpressionEnvironment::S)
    ));

    let bytes = write_from_compilation(&compilation).expect("source Path FCBC writing");
    let render = load_render(&bytes).expect("source Path product loader");
    let node = render
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Path)
        .expect("decoded Path node");
    assert!(node.fill_paint.is_some());
    assert!(node.stroke_ref.is_some());
    assert_eq!(render.paths[0].fill_rule, 2);
    assert_eq!(render.paths[0].commands.len(), 7);
    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("source Path semantics");
    let path = draw
        .iter()
        .find(|operation| operation.kind == NodeKind::Path)
        .expect("source Path draw operation");
    assert!(path.fill_rgba.is_some());
    assert!(path.stroke.is_some());
    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 20, 16).expect("source Path raster");
    assert!(pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0));
}

const SOURCE_TEXT_FILL: &str = r#"#fcs 5.0.0
format { profile: renderable; }
resources {
    font primary {
        source: "assets/fcs-test-font.ttf";
        mediaType: "font/ttf";
    }
    binary blob {
        source: "assets/fcs-test-font.ttf";
        mediaType: "application/octet-stream";
    }
}
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 16px; height: 16px; }
    layer main {
        pass: "overlay";
        children {
            text label {
                content: "A";
                font: @primary;
                size: 8px;
                fill: solid(#FFFFFFFF);
                stroke: solid(#FF0000FF);
                width: 1px;
                cap: "butt";
                join: "miter";
                miterLimit: 4.0;
                dash: [];
                dashOffset: 0px;
            }
        }
    }
}
"#;

fn source_text_compilation(content: &str) -> CanonicalCompilation {
    let source = SOURCE_TEXT_FILL.replace("content: \"A\";", &format!("content: {content:?};"));
    let document = parse_document(&source)
        .into_result()
        .expect("source Text parses");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/conformance/render");
    document
        .canonical_compilation_with_source(
            &source,
            CompileTimeLimits::default(),
            workspace,
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| panic!("source Text lowering failed: {diagnostics:?}"))
}

#[test]
fn checked_in_text_fixture_reaches_product_raster() {
    let compilation = source_text_compilation("A");
    let scene = compilation.chart().render().expect("canonical Text scene");
    let CanonicalRenderGeometryData::Text { glyph_runs, .. } = scene.geometries()[0].data() else {
        panic!("source Text geometry");
    };
    assert_eq!(glyph_runs.as_slice(), [0]);
    let canonical_run = &scene.glyph_runs()[0];

    let bytes = write_from_compilation(&compilation).expect("source Text FCBC writing");
    let render = load_render(&bytes).expect("source Text product loader");
    assert_eq!(render.nodes.len(), 1);
    assert_eq!(render.nodes[0].kind, NodeKind::Text);
    assert!(render.nodes[0].fill_paint.is_some());
    assert!(render.nodes[0].stroke_ref.is_some());
    assert!(matches!(
        &render.geometries[0].data,
        GeometryData::Text { glyph_runs, origin }
            if glyph_runs.as_slice() == [0] && *origin != u32::MAX
    ));
    let loaded_run = &render.glyph_runs[0];
    assert_eq!(loaded_run.id, canonical_run.id().value());
    assert_eq!(loaded_run.font_resource_id, canonical_run.font().value());
    assert_eq!(loaded_run.face_index, canonical_run.face_index());
    assert_eq!(
        loaded_run.run_offset[0].to_bits(),
        canonical_run.run_offset()[0].to_bits()
    );
    assert_eq!(
        loaded_run.run_offset[1].to_bits(),
        canonical_run.run_offset()[1].to_bits()
    );
    assert_eq!(
        render.core.descriptors[loaded_run.size_descriptor as usize].property_type,
        fcs_fcbc::ValueType::Length
    );
    assert_eq!(loaded_run.glyphs.len(), canonical_run.glyphs().len());
    for (loaded, canonical) in loaded_run.glyphs.iter().zip(canonical_run.glyphs()) {
        assert_eq!(loaded.glyph_id, canonical.glyph_id);
        assert_eq!(loaded.x_advance.to_bits(), canonical.x_advance.to_bits());
        assert_eq!(loaded.y_advance.to_bits(), canonical.y_advance.to_bits());
        assert_eq!(loaded.x_offset.to_bits(), canonical.x_offset.to_bits());
        assert_eq!(loaded.y_offset.to_bits(), canonical.y_offset.to_bits());
    }

    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("Text semantic evaluation");
    let text = draw
        .iter()
        .find(|operation| operation.kind == NodeKind::Text)
        .expect("Text draw op");
    assert_eq!(
        draw,
        evaluate_semantic_draw_list_at(&render, 0.0).expect("repeat Text query")
    );
    assert_eq!(text.fill_rgba, Some([1.0, 1.0, 1.0, 1.0]));
    assert_eq!(
        text.stroke.as_ref().and_then(|stroke| stroke.fill_rgba),
        Some([1.0, 0.0, 0.0, 1.0])
    );
    assert!(text.bounds[2] > text.bounds[0]);
    assert!(text.bounds[3] > text.bounds[1]);

    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 16, 16).expect("Text rasterization");
    assert!(pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0));

    let mut stroke_only = render.clone();
    stroke_only.nodes[0].fill_paint = None;
    let pixels =
        rasterize_solid_rgba8_at(&stroke_only, 0.0, 16, 16).expect("Text stroke rasterization");
    assert!(pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0));

    let empty = source_text_compilation("");
    let empty_bytes = write_from_compilation(&empty).expect("empty source Text FCBC writing");
    let empty_render = load_render(&empty_bytes).expect("empty source Text product loader");
    assert_eq!(empty_render.glyph_runs.len(), 1);
    assert!(empty_render.glyph_runs[0].glyphs.is_empty());
}

#[test]
fn canonical_text_writer_remaps_stable_glyph_run_order() {
    let compilation = source_text_compilation("A");
    let original = compilation.chart().render().expect("canonical Text scene");
    let original_run = &original.glyph_runs()[0];
    let CanonicalRenderGeometryData::Text { origin, .. } = original.geometries()[0].data() else {
        panic!("source Text geometry");
    };
    let mut ids = StableIdRegistry::new();
    let first = ids
        .insert(
            EntityKind::RenderGlyphRun,
            CanonicalTextualId::explicit("writer-glyph-first").expect("first GlyphRun textual ID"),
        )
        .expect("first GlyphRun stable ID");
    let second = ids
        .insert(
            EntityKind::RenderGlyphRun,
            CanonicalTextualId::explicit("writer-glyph-second")
                .expect("second GlyphRun textual ID"),
        )
        .expect("second GlyphRun stable ID");
    let (low, high) = if first.value() < second.value() {
        (first, second)
    } else {
        (second, first)
    };
    let high_run = CanonicalGlyphRun::new(
        high.clone(),
        original_run.font().clone(),
        original_run.face_index(),
        original_run.size(),
        original_run.run_offset(),
        original_run.glyphs().to_vec(),
    )
    .expect("high-ID GlyphRun");
    let low_run = CanonicalGlyphRun::new(
        low.clone(),
        original_run.font().clone(),
        original_run.face_index(),
        original_run.size(),
        [1.0, 0.0],
        Vec::new(),
    )
    .expect("low-ID GlyphRun");
    let geometry = CanonicalRenderGeometry::new(
        original.geometries()[0].id().clone(),
        CanonicalRenderGeometryData::Text {
            glyph_runs: vec![0, 1],
            origin: *origin,
        },
    )
    .expect("two-run Text geometry");
    let scene = CanonicalRenderScene::new(CanonicalRenderSceneSpec {
        viewport: original.viewport(),
        layers: original.layers().to_vec(),
        nodes: original.nodes().to_vec(),
        geometries: vec![geometry],
        paths: original.paths().to_vec(),
        paints: original.paints().to_vec(),
        strokes: original.strokes().to_vec(),
        clips: original.clips().to_vec(),
        glyph_runs: vec![high_run, low_run],
    })
    .expect("two-run canonical Text scene");
    let compilation = CanonicalCompilation::new(
        compilation.chart().clone().with_render(scene),
        compilation.resources().clone(),
        compilation.distribution().clone(),
    );

    let bytes = write_from_compilation(&compilation).expect("two-run Text FCBC writing");
    let render = load_render(&bytes).expect("two-run Text product loader");
    assert_eq!(render.glyph_runs[0].id, low.value());
    assert_eq!(render.glyph_runs[1].id, high.value());
    assert!(matches!(
        &render.geometries[0].data,
        GeometryData::Text { glyph_runs, .. } if glyph_runs.as_slice() == [1, 0]
    ));
}

#[test]
fn canonical_text_writer_rejects_missing_or_non_font_resources() {
    let compilation = source_text_compilation("A");
    let original = compilation.chart().render().expect("canonical Text scene");
    let original_run = &original.glyph_runs()[0];
    let with_font = |font| {
        let glyph_run = CanonicalGlyphRun::new(
            original_run.id().clone(),
            font,
            original_run.face_index(),
            original_run.size(),
            original_run.run_offset(),
            original_run.glyphs().to_vec(),
        )
        .expect("replacement GlyphRun");
        let scene = CanonicalRenderScene::new(CanonicalRenderSceneSpec {
            viewport: original.viewport(),
            layers: original.layers().to_vec(),
            nodes: original.nodes().to_vec(),
            geometries: original.geometries().to_vec(),
            paths: original.paths().to_vec(),
            paints: original.paints().to_vec(),
            strokes: original.strokes().to_vec(),
            clips: original.clips().to_vec(),
            glyph_runs: vec![glyph_run],
        })
        .expect("canonical Text scene with replacement font");
        CanonicalCompilation::new(
            compilation.chart().clone().with_render(scene),
            compilation.resources().clone(),
            compilation.distribution().clone(),
        )
    };
    let mut ids = StableIdRegistry::new();
    let missing = ids
        .insert(
            EntityKind::Resource,
            CanonicalTextualId::explicit("missing").expect("missing resource textual ID"),
        )
        .expect("missing resource stable ID");
    let blob = ids
        .insert(
            EntityKind::Resource,
            CanonicalTextualId::explicit("blob").expect("blob resource textual ID"),
        )
        .expect("blob resource stable ID");

    for (font, category) in [
        (missing, "fcbc.render-resource-not-found"),
        (blob, "fcbc.render-resource-type-mismatch"),
    ] {
        let error = write_from_compilation(&with_font(font))
            .expect_err("invalid GlyphRun font resource must fail closed");
        assert_eq!(error.category(), category);
    }
}

#[test]
fn source_clip_group_reaches_product_semantic_and_raster_paths() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 8px; height: 8px; }
    layer main {
        pass: "overlay";
        children {
            clipGroup root {
                clip.kind: "rect";
                clip.fillRule: "nonzero";
                clip.origin: vec2(-1px, -3px);
                clip.size: vec2(2px, 6px);
                children {
                    rect target {
                        origin: vec2(-3px, -3px);
                        size: vec2(6px, 6px);
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
        .expect("Clip Render source parses");
    let base = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .expect("Clip canonical lowering");
    let scene = base.chart().render().expect("canonical Clip scene");
    let clip_group = &scene.nodes()[0];
    assert_eq!(clip_group.kind(), CanonicalRenderNodeKind::ClipGroup);
    assert_eq!(clip_group.geometry(), None);
    let clip = &scene.clips()[clip_group.clip().expect("canonical Clip reference")];
    assert_eq!(clip.fill_rule(), CanonicalRenderFillRule::NonZero);
    assert!(matches!(
        scene.geometries()[clip.geometry()].data(),
        CanonicalRenderGeometryData::Rect { .. }
    ));

    let bytes = write_from_compilation(&base).expect("canonical Clip FCBC writing");
    let render = load_render(&bytes).expect("canonical Clip product loader");

    let clip_node = render
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::ClipGroup)
        .expect("decoded ClipGroup node");
    let clip_index = clip_node.clip_ref.expect("decoded Clip reference") as usize;
    let decoded_clip = &render.clips[clip_index];
    assert_eq!(decoded_clip.id, clip.id().value());
    assert_eq!(decoded_clip.fill_rule, 1);
    assert_eq!(
        render.geometries[decoded_clip.geometry_ref as usize].id,
        scene.geometries()[clip.geometry()].id().value()
    );
    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("Clip semantic evaluation");
    let target = draw
        .iter()
        .find(|operation| operation.kind == NodeKind::Rect)
        .expect("clipped target draw op");
    assert_eq!(target.clip_chain, vec![clip.id().value()]);
    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 8, 8).expect("Clip rasterization");
    assert_eq!(
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] != 0)
            .count(),
        12
    );
}

#[test]
fn source_clip_group_rejects_invalid_kind_and_fill_rule_with_clip_category() {
    for (kind, fill_rule) in [("line", "nonzero"), ("rect", "winding")] {
        let source = format!(
            r#"#fcs 5.0.0
format {{ profile: renderable; }}
tempoMap {{ 0beat -> 120bpm; }}
render profile 1.0.0 {{
    viewport {{ width: 8px; height: 8px; }}
    layer main {{
        pass: "overlay";
        children {{
            clipGroup root {{
                clip.kind: "{kind}";
                clip.fillRule: "{fill_rule}";
                clip.origin: vec2(-1px, -1px);
                clip.size: vec2(2px, 2px);
            }}
        }}
    }}
}}
"#
        );
        let document = parse_document(&source)
            .into_result()
            .expect("invalid Clip source still parses");
        let diagnostics = document
            .canonical_chart_with_source(&source, CompileTimeLimits::default())
            .expect_err("invalid Clip source must fail during canonical lowering");
        assert_eq!(diagnostics.len(), 1, "{kind}/{fill_rule}");
        assert_eq!(
            diagnostics[0].code().as_str(),
            "render.invalid-clip",
            "{kind}/{fill_rule}"
        );
    }
}

#[test]
fn source_clip_group_lowers_every_allowed_geometry_kind() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 8px; height: 8px; }
    layer main {
        pass: "overlay";
        children {
            clipGroup rectClip {
                clip.kind: "rect";
                clip.fillRule: "nonzero";
                clip.size: vec2(2px, 2px);
            }
            clipGroup roundedRectClip {
                clip.kind: "roundedRect";
                clip.fillRule: "nonzero";
                clip.size: vec2(2px, 2px);
                clip.radius: 0.5px;
            }
            clipGroup circleClip {
                clip.kind: "circle";
                clip.fillRule: "nonzero";
                clip.radius: 1px;
            }
            clipGroup ellipseClip {
                clip.kind: "ellipse";
                clip.fillRule: "nonzero";
                clip.radiusX: 1px;
                clip.radiusY: 0.5px;
            }
            clipGroup polygonClip {
                clip.kind: "polygon";
                clip.fillRule: "nonzero";
                clip.points: [vec2(-1px, -1px), vec2(1px, -1px), vec2(0px, 1px)];
            }
            clipGroup pathClip {
                clip.kind: "path";
                clip.fillRule: "evenodd";
                clip.commands: [
                    moveTo(vec2(-1px, -1px)),
                    lineTo(vec2(1px, -1px)),
                    lineTo(vec2(0px, 1px)),
                    close(),
                ];
            }
        }
    }
}
"#;
    let scene = parse_document(source)
        .into_result()
        .expect("Clip geometry source parses")
        .canonical_chart_with_source(source, CompileTimeLimits::default())
        .expect("every allowed Clip geometry lowers")
        .render()
        .expect("canonical Clip scene")
        .clone();
    assert_eq!(scene.clips().len(), 6);
    assert!(
        scene
            .nodes()
            .iter()
            .all(|node| node.kind() == CanonicalRenderNodeKind::ClipGroup)
    );
    assert_eq!(
        scene
            .clips()
            .iter()
            .map(|clip| scene.geometries()[clip.geometry()].kind())
            .collect::<Vec<_>>(),
        [
            CanonicalRenderNodeKind::Rect,
            CanonicalRenderNodeKind::RoundedRect,
            CanonicalRenderNodeKind::Circle,
            CanonicalRenderNodeKind::Ellipse,
            CanonicalRenderNodeKind::Polygon,
            CanonicalRenderNodeKind::Path,
        ]
    );
    let path_clip = scene.clips().last().expect("Path Clip");
    assert_eq!(path_clip.fill_rule(), CanonicalRenderFillRule::EvenOdd);
    let CanonicalRenderGeometryData::Path { path } =
        scene.geometries()[path_clip.geometry()].data()
    else {
        panic!("last Clip owns Path geometry");
    };
    assert_eq!(
        scene.paths()[*path].fill_rule(),
        CanonicalRenderFillRule::EvenOdd
    );
}
