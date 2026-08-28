use std::fs;
use std::path::{Path, PathBuf};

use fcs_fcbc::write_from_compilation;
use fcs_model::{EntityKind, derive_stable_id};
use fcs_render::{DrawOp, NodeKind, evaluate_semantic_draw_list_at, load_render};
use fcs_source::ResourceLimits;
use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RenderManifest {
    fixture: Vec<RenderFixture>,
}

#[derive(Debug, Deserialize)]
struct RenderFixture {
    id: String,
    source: String,
    chart_time_seconds: f64,
    semantic_expected: String,
}

#[derive(Debug, Deserialize)]
struct SemanticExpectation {
    #[serde(rename = "drawOrder")]
    draw_order: Vec<String>,
    #[serde(rename = "nodeKind")]
    node_kind: String,
    #[serde(rename = "worldBounds")]
    world_bounds: [f64; 4],
    #[serde(rename = "fillLinearRgba")]
    fill_linear_rgba: [f64; 4],
    composite: String,
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> T {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    toml::from_str(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> T {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn node_kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Group => "Group",
        NodeKind::ClipGroup => "ClipGroup",
        NodeKind::Rect => "Rect",
        NodeKind::RoundedRect => "RoundedRect",
        NodeKind::Circle => "Circle",
        NodeKind::Ellipse => "Ellipse",
        NodeKind::Line => "Line",
        NodeKind::Polyline => "Polyline",
        NodeKind::Polygon => "Polygon",
        NodeKind::Path => "Path",
        NodeKind::Image => "Image",
        NodeKind::Text => "Text",
    }
}

fn composite_name(composite: u16) -> Option<&'static str> {
    Some(match composite {
        1 => "sourceOver",
        2 => "copy",
        3 => "add",
        4 => "multiply",
        5 => "screen",
        _ => return None,
    })
}

fn float_bits(values: [f64; 4]) -> [u64; 4] {
    values.map(f64::to_bits)
}

fn stable_ids(draw_order: &str, fixture_id: &str) -> (u64, u64) {
    let (layer_name, node_path) = draw_order
        .split_once('/')
        .unwrap_or_else(|| panic!("{fixture_id}: drawOrder entry must contain a layer/name path"));
    let layer_id = derive_stable_id(EntityKind::RenderLayer, &format!("layer/{layer_name}"));
    let node_id = derive_stable_id(
        EntityKind::RenderNode,
        &format!("layer/{layer_name}/{node_path}"),
    );
    (layer_id, node_id)
}

fn assert_draw_op_matches(
    render: &fcs_render::DecodedRenderChart,
    draw: &DrawOp,
    expected_label: &str,
    expected: &SemanticExpectation,
    fixture_id: &str,
) {
    let (layer_id, node_id) = stable_ids(expected_label, fixture_id);
    let layer = render
        .layers
        .get(draw.layer_index as usize)
        .unwrap_or_else(|| panic!("{fixture_id}: DrawOp layer index is out of range"));
    assert_eq!(layer.id, layer_id, "{fixture_id}: semantic layer identity");
    assert_eq!(
        draw.node_id, node_id,
        "{fixture_id}: semantic node identity"
    );
    assert_eq!(
        node_kind_name(draw.kind),
        expected.node_kind,
        "{fixture_id}: semantic node kind"
    );
    assert_eq!(
        float_bits(draw.bounds),
        float_bits(expected.world_bounds),
        "{fixture_id}: semantic world bounds"
    );
    assert_eq!(
        draw.fill_rgba,
        Some(expected.fill_linear_rgba),
        "{fixture_id}: fill"
    );
    assert_eq!(
        composite_name(draw.composite),
        Some(expected.composite.as_str()),
        "{fixture_id}: semantic composite"
    );
}

#[test]
fn declared_render_semantic_fixtures_match_product_output() {
    let render_root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/conformance/render");
    let manifest: RenderManifest = read_toml(&render_root.join("manifest.toml"));
    assert!(
        !manifest.fixture.is_empty(),
        "Render manifest must not have an empty fixture set"
    );

    for fixture in manifest.fixture {
        assert!(
            fixture.chart_time_seconds.is_finite(),
            "{}: chart time must be finite",
            fixture.id
        );
        let source_path = render_root.join(&fixture.source);
        let source = fs::read_to_string(&source_path).unwrap_or_else(|error| {
            panic!(
                "{}: failed to read {}: {error}",
                fixture.id,
                source_path.display()
            )
        });
        let document = parse_document(&source)
            .into_result()
            .unwrap_or_else(|errors| panic!("{}: source parse failed: {errors:?}", fixture.id));
        let compilation = document
            .canonical_compilation_with_source(
                &source,
                CompileTimeLimits::default(),
                &render_root,
                ResourceLimits::default(),
            )
            .unwrap_or_else(|errors| {
                panic!("{}: canonical lowering failed: {errors:?}", fixture.id)
            });
        let bytes = write_from_compilation(&compilation)
            .unwrap_or_else(|error| panic!("{}: FCBC writing failed: {error}", fixture.id));
        let render = load_render(&bytes)
            .unwrap_or_else(|error| panic!("{}: Render loading failed: {error}", fixture.id));
        let draw = evaluate_semantic_draw_list_at(&render, fixture.chart_time_seconds)
            .unwrap_or_else(|error| panic!("{}: semantic evaluation failed: {error}", fixture.id));
        let expected: SemanticExpectation =
            read_json(&render_root.join(&fixture.semantic_expected));

        assert_eq!(
            draw.len(),
            expected.draw_order.len(),
            "{}: semantic draw-order length",
            fixture.id
        );
        assert_eq!(
            draw.len(),
            1,
            "{}: this semantic fixture schema expects one DrawOp",
            fixture.id
        );
        assert_draw_op_matches(
            &render,
            &draw[0],
            &expected.draw_order[0],
            &expected,
            &fixture.id,
        );
    }
}
