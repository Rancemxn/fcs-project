use std::path::Path;

use fcs_fcbc::write_from_compilation;
use fcs_model::{
    CanonicalArcDirection, CanonicalCompilation, CanonicalExpressionType, CanonicalGlyphPlacement,
    CanonicalGlyphRun, CanonicalImageRepeat, CanonicalImageSampling, CanonicalPathCommand,
    CanonicalPatternTransform, CanonicalRenderClip, CanonicalRenderFillRule,
    CanonicalRenderGeometry, CanonicalRenderGeometryData, CanonicalRenderNode,
    CanonicalRenderNodeKind, CanonicalRenderNodeSpec, CanonicalRenderPaint,
    CanonicalRenderPaintData, CanonicalRenderPath, CanonicalRenderScene, CanonicalRenderSceneSpec,
    CanonicalRenderStroke, CanonicalStrokeCap, CanonicalStrokeJoin, CanonicalTextualId, EntityKind,
    StableIdRegistry,
};
use fcs_source::ResourceLimits;
use fcs_source::diagnostic::DiagnosticCode;
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

fn canonical_text_compilation() -> CanonicalCompilation {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
resources {
    font textFont {
        source: "assets/fcs-test-font.ttf";
        hash: "sha256:f603c8bcf005ee2a53ea78acae8002e91285ca26fee32ace235684b636706800";
        mediaType: "font/ttf";
    }
}
tempoMap { 0beat -> 120bpm; }
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
        .expect("canonical Text writer source parses");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/conformance/render");
    let base = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            &workspace,
            ResourceLimits::default(),
        )
        .expect("canonical Text writer source lowers");
    let original = base.chart().render().expect("source Render scene");
    let original_node = &original.nodes()[0];
    let CanonicalRenderGeometryData::Circle { center, .. } = original.geometries()[0].data() else {
        panic!("source fixture must provide a Circle geometry");
    };
    let size_descriptor = base
        .chart()
        .descriptors()
        .expect("source Render descriptors")
        .descriptors()
        .iter()
        .position(|descriptor| descriptor.property_type() == &CanonicalExpressionType::Length)
        .expect("source fixture must provide a Length descriptor");
    let mut ids = StableIdRegistry::new();
    let font_id = ids
        .insert(
            EntityKind::Resource,
            CanonicalTextualId::explicit("textFont").expect("font textual ID"),
        )
        .expect("font stable ID");
    let glyph_run_id = ids
        .insert(
            EntityKind::RenderGlyphRun,
            CanonicalTextualId::explicit("writer-text-run").expect("glyph run textual ID"),
        )
        .expect("glyph run stable ID");
    let glyph_run = CanonicalGlyphRun::new(
        glyph_run_id,
        font_id,
        0,
        size_descriptor,
        [0.0, 0.0],
        vec![CanonicalGlyphPlacement {
            glyph_id: 1,
            x_advance: 1.0,
            y_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        }],
    )
    .expect("canonical glyph run");
    let node = CanonicalRenderNode::new(CanonicalRenderNodeSpec {
        id: original_node.id().clone(),
        kind: CanonicalRenderNodeKind::Text,
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
        fill_paint: Some(0),
        stroke: None,
        clip: None,
        composite: original_node.composite(),
    })
    .expect("canonical Text node");
    let geometry = CanonicalRenderGeometry::new(
        original.geometries()[0].id().clone(),
        CanonicalRenderGeometryData::Text {
            glyph_runs: vec![0],
            origin: *center,
        },
    )
    .expect("canonical Text geometry");
    let scene = CanonicalRenderScene::new(CanonicalRenderSceneSpec {
        viewport: original.viewport(),
        layers: original.layers().to_vec(),
        nodes: vec![node],
        geometries: vec![geometry],
        paths: Vec::new(),
        paints: original.paints().to_vec(),
        strokes: Vec::new(),
        clips: Vec::new(),
        glyph_runs: vec![glyph_run],
    })
    .expect("canonical Text scene");
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
fn canonical_text_writer_reaches_product_render_loader() {
    let compilation = canonical_text_compilation();
    let scene = compilation.chart().render().expect("canonical Text scene");
    let bytes = write_from_compilation(&compilation).expect("canonical Text FCBC writing");
    let render = load_render(&bytes).expect("canonical Text product loader");

    assert_eq!(render.nodes.len(), 1);
    assert_eq!(render.nodes[0].kind, NodeKind::Text);
    assert_eq!(render.nodes[0].fill_paint, Some(0));
    assert_eq!(render.glyph_runs.len(), 1);
    assert_eq!(render.glyph_runs[0].id, scene.glyph_runs()[0].id().value());
    assert_eq!(render.glyph_runs[0].glyphs.len(), 1);
    assert_eq!(render.glyph_runs[0].glyphs[0].glyph_id, 1);
    assert!(render.decoded_fonts.contains_key(&render.resources[0].id));
    assert!(matches!(
        render.geometries[0].data,
        GeometryData::Text { ref glyph_runs, .. } if glyph_runs == &vec![0]
    ));
    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("Text semantic evaluation");
    assert_eq!(draw.len(), 1);
    assert_eq!(draw[0].kind, NodeKind::Text);
    assert!(draw[0].bounds[2] > draw[0].bounds[0]);
    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 16, 16).expect("Text rasterization");
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
}

#[test]
fn source_line_stroke_reaches_product_render_loader() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
definitions {
    const gradientStart: length = -4px;
    const initialColor: color = #FF0000FF;
}
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 16px; height: 16px; }
    layer main {
        pass: "overlay";
        children {
            line guide {
                start: vec2(-6px, 0px);
                end: vec2(6px, 0px);
                stroke: solid(#FFFFFFFF);
                width: 2px;
                cap: "round";
                join: "bevel";
                miterLimit: 4.0;
                dash: [2px, 1px, 3px];
                dashOffset: 0px;
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
    assert_eq!(scene.strokes()[0].dash(), &[2.0, 1.0, 3.0, 2.0, 1.0, 3.0]);

    let bytes = write_from_compilation(&compilation).expect("source Line stroke FCBC writing");
    let render = load_render(&bytes).expect("source Line stroke product loader");
    assert_eq!(render.nodes[0].kind, NodeKind::Line);
    assert_eq!(render.nodes[0].fill_paint, None);
    assert_eq!(render.nodes[0].stroke_ref, Some(0));
    assert_eq!(render.strokes[0].cap, 2);
    assert_eq!(render.strokes[0].join, 3);
    assert_eq!(render.strokes[0].dash, vec![2.0, 1.0, 3.0, 2.0, 1.0, 3.0]);
    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("Line semantic draw list");
    assert_eq!(draw.len(), 1);
    assert!(draw[0].stroke.is_some());
}

#[test]
fn source_render_if_selects_concrete_children() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
definitions {
    const showCircle: bool = true;
}
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 8px; height: 8px; }
    layer main {
        pass: "overlay";
        children {
            if showCircle {
                circle selected {
                    radius: 2px;
                    fill: solid(#FFFFFFFF);
                }
            } else {
                rect unselected {
                    size: vec2(4px, 4px);
                    fill: solid(#FFFFFFFF);
                }
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("Render if source parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .expect("Render if lowering");
    let scene = compilation
        .chart()
        .render()
        .expect("canonical Render scene");
    assert_eq!(scene.nodes().len(), 1);
    assert_eq!(scene.nodes()[0].kind(), CanonicalRenderNodeKind::Circle);
}

#[test]
fn source_render_if_does_not_validate_unselected_tracks() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
definitions {
    const showHidden: bool = false;
}
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 8px; height: 8px; }
    layer main {
        pass: "overlay";
        children {
            if showHidden {
                group hidden {
                    tracks {
                        track fade -> opacity: float {
                            segments { point 0beat: 0.0; }
                        }
                    }
                }
            } else {
                circle visible {
                    radius: 2px;
                    fill: solid(#FFFFFFFF);
                }
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("Render lazy branch source parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .expect("unselected Render tracks must not be lowered");
    let scene = compilation
        .chart()
        .render()
        .expect("canonical Render scene");
    assert_eq!(scene.nodes().len(), 1);
    assert_eq!(scene.nodes()[0].kind(), CanonicalRenderNodeKind::Circle);
}

#[test]
fn source_render_rejects_duplicate_body_fields() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 8px; height: 8px; }
    layer main {
        pass: "overlay";
        children {
            circle duplicate {
                radius: 1px;
                radius: 2px;
                fill: solid(#FFFFFFFF);
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("duplicate Render field source parses");
    let diagnostics = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .expect_err("duplicate Render fields must fail closed");
    assert_eq!(
        diagnostics[0].code(),
        DiagnosticCode::SCHEMA_DUPLICATE_FIELD
    );
}

#[test]
fn source_render_active_interval_resolves_definitions() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
definitions {
    const activeStart: beat = 1beat;
    const activeEnd: beat = 2beat;
}
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 8px; height: 8px; }
    layer main {
        pass: "overlay";
        children {
            circle active {
                active: [activeStart, activeEnd);
                radius: 2px;
                fill: solid(#FFFFFFFF);
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("definition-backed active interval source parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .expect("definition-backed active interval lowers");
    let node = &compilation
        .chart()
        .render()
        .expect("canonical Render scene")
        .nodes()[0];
    assert_eq!(node.active().start().to_bits(), 0.5f64.to_bits());
    assert_eq!(node.active().end().to_bits(), 1.0f64.to_bits());
}

#[test]
fn source_polyline_stroke_reaches_product_render_loader() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 16px; height: 16px; }
    layer main {
        pass: "overlay";
        children {
            polyline guide {
                points: [vec2(-6px, -2px), vec2(0px, 4px), vec2(6px, -2px)];
                stroke: solid(#FFFFFFFF);
                width: 2px;
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
        .expect("source Polyline stroke parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| {
            panic!("source Polyline stroke lowering failed: {diagnostics:?}")
        });
    let scene = compilation
        .chart()
        .render()
        .expect("canonical Polyline scene");
    assert_eq!(scene.nodes()[0].kind(), CanonicalRenderNodeKind::Polyline);
    assert_eq!(scene.nodes()[0].fill_paint(), None);
    assert_eq!(scene.nodes()[0].stroke(), Some(0));
    assert_eq!(scene.strokes().len(), 1);

    let bytes = write_from_compilation(&compilation).expect("source Polyline stroke FCBC writing");
    let render = load_render(&bytes).expect("source Polyline stroke product loader");
    assert_eq!(render.nodes[0].kind, NodeKind::Polyline);
    assert_eq!(render.nodes[0].fill_paint, None);
    assert_eq!(render.nodes[0].stroke_ref, Some(0));
    let draw =
        evaluate_semantic_draw_list_at(&render, 0.0).expect("Polyline stroke semantic draw list");
    assert_eq!(draw.len(), 1);
    assert!(draw[0].stroke.is_some());
    let pixels =
        rasterize_solid_rgba8_at(&render, 0.0, 16, 16).expect("Polyline stroke rasterization");
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
}

#[test]
fn source_curved_shape_strokes_reach_product_render_loader() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 20px; height: 20px; }
    layer main {
        pass: "overlay";
        children {
            roundedRect rounded {
                origin: vec2(-9px, 3px);
                size: vec2(6px, 4px);
                radius: 1px;
                stroke: solid(#FFFFFFFF);
                width: 1px;
                cap: "butt";
                join: "round";
                miterLimit: 4.0;
                dash: [];
                dashOffset: 0px;
            }
            circle circular {
                center: vec2(0px, 0px);
                radius: 3px;
                stroke: solid(#FFFFFFFF);
                width: 1px;
                cap: "butt";
                join: "round";
                miterLimit: 4.0;
                dash: [];
                dashOffset: 0px;
            }
            ellipse oval {
                center: vec2(6px, -4px);
                radiusX: 3px;
                radiusY: 2px;
                stroke: solid(#FFFFFFFF);
                width: 1px;
                cap: "butt";
                join: "round";
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
        .expect("curved shape stroke source parses");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            env!("CARGO_MANIFEST_DIR"),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| {
            panic!("curved shape stroke lowering failed: {diagnostics:?}")
        });
    let scene = compilation
        .chart()
        .render()
        .expect("canonical curved shape scene");
    assert_eq!(scene.nodes().len(), 3);
    assert!(scene.nodes().iter().all(|node| node.fill_paint().is_none()));
    assert!(scene.nodes().iter().all(|node| node.stroke().is_some()));

    let bytes = write_from_compilation(&compilation).expect("curved shape stroke FCBC writing");
    let render = load_render(&bytes).expect("curved shape stroke product loader");
    assert_eq!(render.nodes.len(), 3);
    assert!(render.nodes.iter().all(|node| node.fill_paint.is_none()));
    assert!(render.nodes.iter().all(|node| node.stroke_ref.is_some()));
    let draw = evaluate_semantic_draw_list_at(&render, 0.0)
        .expect("curved shape stroke semantic evaluation");
    assert_eq!(draw.len(), 3);
    assert!(draw.iter().all(|operation| operation.stroke.is_some()));
    let pixels =
        rasterize_solid_rgba8_at(&render, 0.0, 20, 20).expect("curved shape stroke rasterization");
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
}

#[test]
fn source_image_pattern_reaches_product_render_loader() {
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
                fill: imagePattern(
                    @sprite,
                    { position: vec2(seconds(s) * 1px, 0px) },
                    "both",
                    "linear"
                );
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("source ImagePattern parses");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/conformance/render");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            &workspace,
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| {
            panic!("source ImagePattern lowering failed: {diagnostics:?}")
        });
    let scene = compilation
        .chart()
        .render()
        .expect("canonical Render scene");
    let resource_id = match scene.paints()[0].data() {
        CanonicalRenderPaintData::ImagePattern {
            resource,
            transform,
            repeat,
            sampling,
        } => {
            assert_eq!(*repeat, CanonicalImageRepeat::Both);
            assert_eq!(*sampling, CanonicalImageSampling::Bilinear);
            assert_ne!(transform.position, scene.nodes()[0].position());
            assert_eq!(transform.origin, scene.nodes()[0].origin());
            assert_eq!(transform.rotation, scene.nodes()[0].rotation());
            assert_eq!(transform.scale, scene.nodes()[0].scale());
            resource.value()
        }
        other => panic!("expected source ImagePattern paint, found {other:?}"),
    };

    let bytes = write_from_compilation(&compilation).expect("source ImagePattern FCBC writing");
    let render = load_render(&bytes).expect("source ImagePattern product loader");
    let PaintData::ImagePattern {
        resource_id: decoded_resource,
        repeat,
        sampling,
        ..
    } = render.paints[0].data
    else {
        panic!("expected product ImagePattern paint");
    };
    assert_eq!(decoded_resource, resource_id);
    assert_eq!(repeat, 4);
    assert_eq!(sampling, 2);
    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("ImagePattern draw list");
    assert!(
        draw.iter()
            .any(|operation| operation.image_pattern.is_some())
    );
    let draw_at_end =
        evaluate_semantic_draw_list_at(&render, 1.0).expect("dynamic ImagePattern draw list");
    let pattern_at_start = draw
        .iter()
        .find_map(|operation| operation.image_pattern)
        .expect("start ImagePattern semantic payload");
    let pattern_at_end = draw_at_end
        .iter()
        .find_map(|operation| operation.image_pattern)
        .expect("end ImagePattern semantic payload");
    assert_eq!(pattern_at_start.position[0], 0.0);
    assert_eq!(pattern_at_end.position[0], 1.0);
    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 4, 4).expect("ImagePattern rasterization");
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
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
                fill: linearGradient(vec2(gradientStart + seconds(s) * 1px, 0px), vec2(4px, 0px), [
                    stop(0.0, choose(s < 1s, initialColor, #00FF00FF)),
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
    let draw_at_end =
        evaluate_semantic_draw_list_at(&render, 1.0).expect("dynamic gradient evaluation");
    let gradient_at_start = draw
        .iter()
        .find_map(|op| op.linear_gradient.as_ref())
        .expect("start gradient semantic payload");
    let gradient_at_end = draw_at_end
        .iter()
        .find_map(|op| op.linear_gradient.as_ref())
        .expect("end gradient semantic payload");
    assert_eq!(gradient_at_start.start[0], -4.0);
    assert_eq!(gradient_at_end.start[0], -3.0);
    assert_ne!(
        gradient_at_start.stops[0].color,
        gradient_at_end.stops[0].color
    );
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
                opacity: 0.5;
                isolate: true;
                children {
                    circle circleShape {
                        zOrder: 2;
                        radius: 2px * (1.0 + s / 2s);
                        opacity: 1.0 - s / 2s;
                        fill: solid(choose(s < 1s, #0000FFFF, #00FFFFFF));
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

    let isolated_group_id = render.nodes[0].id;
    let draw = evaluate_semantic_draw_list_at(&render, 0.0)
        .expect("isolated nested Render semantic evaluation");
    assert_eq!(draw.len(), 3);
    assert!(draw.iter().all(|operation| {
        operation.isolation_chain.iter().any(|boundary| {
            boundary.node_id == isolated_group_id
                && (boundary.opacity - 0.5).abs() < f64::EPSILON
                && boundary.composite == 1
        })
    }));

    let circle_id = render.nodes[3].id;
    let circle_start = draw
        .iter()
        .find(|operation| operation.node_id == circle_id)
        .expect("circle draw at start");
    assert!((circle_start.opacity - 1.0).abs() < f64::EPSILON);
    let draw_end = evaluate_semantic_draw_list_at(&render, 1.0)
        .expect("dynamic nested Render semantic evaluation");
    let circle_end = draw_end
        .iter()
        .find(|operation| operation.node_id == circle_id)
        .expect("circle draw at end");
    assert!((circle_end.opacity - 0.5).abs() < f64::EPSILON);
    assert_ne!(circle_start.fill_rgba, circle_end.fill_rgba);
    assert!(
        circle_end.bounds[2] - circle_end.bounds[0]
            > circle_start.bounds[2] - circle_start.bounds[0]
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

    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("Path semantic evaluation");
    assert!(
        draw.iter()
            .any(|operation| operation.kind == NodeKind::Path)
    );
    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 16, 16).expect("Path rasterization");
    assert!(
        pixels
            .chunks_exact(4)
            .any(|pixel| pixel[0] > 200 && pixel[1] > 200 && pixel[2] > 200 && pixel[3] > 0)
    );
}

#[test]
fn source_path_commands_reach_product_render_loader() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 8px; height: 8px; }
    layer main {
        pass: "overlay";
        children {
            path outline {
                commands: [
                    moveTo(vec2(-2px, -2px)),
                    lineTo(vec2(2px, -2px)),
                    quadraticTo(vec2(3px, 0px), vec2(2px, 2px)),
                    lineTo(vec2(-2px, 2px)),
                    close(),
                ];
                fillRule: "nonzero";
                fill: solid(#FFFFFFFF);
            }
        }
    }
}
"#;
    let document = parse_document(source)
        .into_result()
        .expect("source Path commands parse");
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
    assert!(matches!(
        scene.geometries()[0].data(),
        CanonicalRenderGeometryData::Path { path: 0 }
    ));
    assert_eq!(
        scene.paths()[0].fill_rule(),
        CanonicalRenderFillRule::NonZero
    );
    assert_eq!(scene.paths()[0].commands().len(), 5);

    let bytes = write_from_compilation(&compilation).expect("source Path FCBC writing");
    let render = load_render(&bytes).expect("source Path product loader");
    assert_eq!(render.paths.len(), 1);
    assert_eq!(render.paths[0].commands.len(), 5);
    assert!(
        evaluate_semantic_draw_list_at(&render, 0.0)
            .expect("source Path semantic evaluation")
            .iter()
            .any(|operation| operation.kind == NodeKind::Path)
    );
    assert!(
        rasterize_solid_rgba8_at(&render, 0.0, 16, 16)
            .expect("source Path rasterization")
            .chunks_exact(4)
            .any(|pixel| pixel[3] != 0)
    );
}

#[test]
fn canonical_clip_writer_reaches_product_render_loader() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 8px; height: 8px; }
    layer main {
        pass: "overlay";
        children {
            group root {
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
    let original = base.chart().render().expect("source Render scene");
    let mut ids = StableIdRegistry::new();
    let clip_geometry_id = ids
        .insert(
            EntityKind::RenderGeometry,
            CanonicalTextualId::explicit("writer-clip-geometry").expect("clip geometry ID"),
        )
        .expect("clip geometry stable ID");
    let clip_id = ids
        .insert(
            EntityKind::RenderClip,
            CanonicalTextualId::explicit("writer-clip").expect("clip ID"),
        )
        .expect("clip stable ID");
    let clip_geometry =
        CanonicalRenderGeometry::new(clip_geometry_id, original.geometries()[0].data().clone())
            .expect("canonical clip geometry");
    let clip = CanonicalRenderClip::new(
        clip_id,
        CanonicalRenderFillRule::NonZero,
        original.geometries().len(),
    )
    .expect("canonical clip");
    let root = &original.nodes()[0];
    let clip_group = CanonicalRenderNode::new(CanonicalRenderNodeSpec {
        id: root.id().clone(),
        kind: CanonicalRenderNodeKind::ClipGroup,
        parent: root.parent(),
        layer: root.layer(),
        document_order: root.document_order(),
        z_order: root.z_order(),
        attachment: root.attachment().clone(),
        active: root.active(),
        isolate: root.isolate(),
        follow_hidden_attachment: root.follow_hidden_attachment(),
        position: root.position(),
        origin: root.origin(),
        rotation: root.rotation(),
        scale: root.scale(),
        opacity: root.opacity(),
        visibility: root.visibility(),
        geometry: None,
        fill_paint: None,
        stroke: None,
        clip: Some(0),
        composite: root.composite(),
    })
    .expect("canonical ClipGroup node");
    let mut nodes = original.nodes().to_vec();
    nodes[0] = clip_group;
    let mut geometries = original.geometries().to_vec();
    geometries.push(clip_geometry);
    let scene = CanonicalRenderScene::new(CanonicalRenderSceneSpec {
        viewport: original.viewport(),
        layers: original.layers().to_vec(),
        nodes,
        geometries,
        paths: original.paths().to_vec(),
        paints: original.paints().to_vec(),
        strokes: original.strokes().to_vec(),
        clips: vec![clip],
        glyph_runs: original.glyph_runs().to_vec(),
    })
    .expect("canonical Clip scene");
    let compilation = CanonicalCompilation::new(
        base.chart().clone().with_render(scene),
        base.resources().clone(),
        base.distribution().clone(),
    );
    let scene = compilation.chart().render().expect("canonical Clip scene");
    let bytes = write_from_compilation(&compilation).expect("canonical Clip FCBC writing");
    let render = load_render(&bytes).expect("canonical Clip product loader");

    let clip_node = render
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::ClipGroup)
        .expect("decoded ClipGroup node");
    let clip_index = clip_node.clip_ref.expect("decoded Clip reference") as usize;
    let decoded_clip = &render.clips[clip_index];
    assert_eq!(decoded_clip.id, scene.clips()[0].id().value());
    assert_eq!(decoded_clip.fill_rule, 1);
    assert_eq!(
        render.geometries[decoded_clip.geometry_ref as usize].id,
        scene.geometries()[scene.clips()[0].geometry()].id().value()
    );
    let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("Clip semantic evaluation");
    let target = draw
        .iter()
        .find(|operation| operation.kind == NodeKind::Rect)
        .expect("clipped target draw op");
    assert_eq!(target.clip_chain, vec![scene.clips()[0].id().value()]);
    let pixels = rasterize_solid_rgba8_at(&render, 0.0, 8, 8).expect("Clip rasterization");
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
}
