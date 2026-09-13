use std::fs;
use std::path::{Path, PathBuf};

use fcs_fcbc::write_from_compilation;
use fcs_model::{EntityKind, derive_stable_id};
use fcs_render::{
    DrawOp, LinearGradientDrawOp, NodeKind, RadialGradientDrawOp, StrokeDrawOp,
    evaluate_semantic_draw_list_at, load_render,
};
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
#[serde(deny_unknown_fields)]
struct SemanticExpectation {
    #[serde(rename = "drawOrder")]
    draw_order: Vec<String>,
    #[serde(rename = "drawOps")]
    draw_ops: Vec<SemanticDrawExpectation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticDrawExpectation {
    #[serde(rename = "nodeKind")]
    node_kind: String,
    #[serde(rename = "worldBounds")]
    world_bounds: [f64; 4],
    #[serde(default, rename = "fillLinearRgba")]
    fill_linear_rgba: Option<[f64; 4]>,
    #[serde(default, rename = "linearGradient")]
    linear_gradient: Option<SemanticLinearGradientExpectation>,
    #[serde(default, rename = "radialGradient")]
    radial_gradient: Option<SemanticRadialGradientExpectation>,
    #[serde(default, rename = "imagePattern")]
    image_pattern: Option<SemanticImagePatternExpectation>,
    #[serde(default)]
    stroke: Option<SemanticStrokeExpectation>,
    composite: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticStrokeExpectation {
    width: f64,
    cap: String,
    join: String,
    #[serde(rename = "miterLimit")]
    miter_limit: f64,
    #[serde(rename = "dashOffset")]
    dash_offset: f64,
    dash: Vec<f64>,
    #[serde(default, rename = "fillLinearRgba")]
    fill_linear_rgba: Option<[f64; 4]>,
    #[serde(default, rename = "linearGradient")]
    linear_gradient: Option<SemanticLinearGradientExpectation>,
    #[serde(default, rename = "radialGradient")]
    radial_gradient: Option<SemanticRadialGradientExpectation>,
    #[serde(default, rename = "imagePattern")]
    image_pattern: Option<SemanticImagePatternExpectation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticLinearGradientExpectation {
    start: [f64; 2],
    end: [f64; 2],
    spread: String,
    stops: Vec<SemanticGradientStopExpectation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticRadialGradientExpectation {
    #[serde(rename = "startCenter")]
    start_center: [f64; 2],
    #[serde(rename = "startRadius")]
    start_radius: f64,
    #[serde(rename = "endCenter")]
    end_center: [f64; 2],
    #[serde(rename = "endRadius")]
    end_radius: f64,
    spread: String,
    stops: Vec<SemanticGradientStopExpectation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticGradientStopExpectation {
    offset: f64,
    color: [f64; 4],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticImagePatternExpectation {
    #[serde(rename = "resourceId")]
    resource_id: u64,
    position: [f64; 2],
    origin: [f64; 2],
    rotation: f64,
    scale: [f64; 2],
    repeat: String,
    sampling: String,
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

fn stroke_cap_name(cap: u16) -> Option<&'static str> {
    Some(match cap {
        1 => "butt",
        2 => "round",
        3 => "square",
        _ => return None,
    })
}

fn stroke_join_name(join: u16) -> Option<&'static str> {
    Some(match join {
        1 => "miter",
        2 => "round",
        3 => "bevel",
        _ => return None,
    })
}

fn gradient_spread_name(spread: u16) -> Option<&'static str> {
    Some(match spread {
        1 => "pad",
        2 => "repeat",
        3 => "reflect",
        _ => return None,
    })
}

fn image_repeat_name(repeat: u16) -> Option<&'static str> {
    Some(match repeat {
        1 => "none",
        2 => "x",
        3 => "y",
        4 => "both",
        _ => return None,
    })
}

fn image_sampling_name(sampling: u16) -> Option<&'static str> {
    Some(match sampling {
        1 => "nearest",
        2 => "linear",
        _ => return None,
    })
}

fn assert_linear_gradient_matches(
    actual: &LinearGradientDrawOp,
    expected: &SemanticLinearGradientExpectation,
    fixture_id: &str,
) {
    assert_eq!(
        actual.start.map(f64::to_bits),
        expected.start.map(f64::to_bits),
        "{fixture_id}: linear gradient start"
    );
    assert_eq!(
        actual.end.map(f64::to_bits),
        expected.end.map(f64::to_bits),
        "{fixture_id}: linear gradient end"
    );
    assert_eq!(
        gradient_spread_name(actual.spread),
        Some(expected.spread.as_str()),
        "{fixture_id}: linear gradient spread"
    );
    assert_eq!(
        actual.stops.len(),
        expected.stops.len(),
        "{fixture_id}: linear gradient stop count"
    );
    for (index, (actual, expected)) in actual.stops.iter().zip(&expected.stops).enumerate() {
        assert_eq!(
            actual.offset.to_bits(),
            expected.offset.to_bits(),
            "{fixture_id}: linear gradient stop {index} offset"
        );
        assert_eq!(
            actual.color.map(f64::to_bits),
            expected.color.map(f64::to_bits),
            "{fixture_id}: linear gradient stop {index} color"
        );
    }
}

fn assert_radial_gradient_matches(
    actual: &RadialGradientDrawOp,
    expected: &SemanticRadialGradientExpectation,
    fixture_id: &str,
) {
    assert_eq!(
        actual.start_center.map(f64::to_bits),
        expected.start_center.map(f64::to_bits),
        "{fixture_id}: radial gradient startCenter"
    );
    assert_eq!(
        actual.start_radius.to_bits(),
        expected.start_radius.to_bits(),
        "{fixture_id}: radial gradient startRadius"
    );
    assert_eq!(
        actual.end_center.map(f64::to_bits),
        expected.end_center.map(f64::to_bits),
        "{fixture_id}: radial gradient endCenter"
    );
    assert_eq!(
        actual.end_radius.to_bits(),
        expected.end_radius.to_bits(),
        "{fixture_id}: radial gradient endRadius"
    );
    assert_eq!(
        gradient_spread_name(actual.spread),
        Some(expected.spread.as_str()),
        "{fixture_id}: radial gradient spread"
    );
    assert_eq!(
        actual.stops.len(),
        expected.stops.len(),
        "{fixture_id}: radial gradient stop count"
    );
    for (index, (actual, expected)) in actual.stops.iter().zip(&expected.stops).enumerate() {
        assert_eq!(
            actual.offset.to_bits(),
            expected.offset.to_bits(),
            "{fixture_id}: radial gradient stop {index} offset"
        );
        assert_eq!(
            actual.color.map(f64::to_bits),
            expected.color.map(f64::to_bits),
            "{fixture_id}: radial gradient stop {index} color"
        );
    }
}

fn assert_image_pattern_matches(
    actual: &fcs_render::ImagePatternDrawOp,
    expected: &SemanticImagePatternExpectation,
    fixture_id: &str,
) {
    assert_eq!(
        actual.resource_id, expected.resource_id,
        "{fixture_id}: ImagePattern resourceId"
    );
    assert_eq!(
        actual.position.map(f64::to_bits),
        expected.position.map(f64::to_bits),
        "{fixture_id}: ImagePattern position"
    );
    assert_eq!(
        actual.origin.map(f64::to_bits),
        expected.origin.map(f64::to_bits),
        "{fixture_id}: ImagePattern origin"
    );
    assert_eq!(
        actual.rotation.to_bits(),
        expected.rotation.to_bits(),
        "{fixture_id}: ImagePattern rotation"
    );
    assert_eq!(
        actual.scale.map(f64::to_bits),
        expected.scale.map(f64::to_bits),
        "{fixture_id}: ImagePattern scale"
    );
    assert_eq!(
        image_repeat_name(actual.repeat),
        Some(expected.repeat.as_str()),
        "{fixture_id}: ImagePattern repeat"
    );
    assert_eq!(
        image_sampling_name(actual.sampling),
        Some(expected.sampling.as_str()),
        "{fixture_id}: ImagePattern sampling"
    );
}

fn assert_stroke_matches(
    actual: &StrokeDrawOp,
    expected: &SemanticStrokeExpectation,
    fixture_id: &str,
) {
    assert_eq!(
        actual.width.to_bits(),
        expected.width.to_bits(),
        "{fixture_id}: stroke width"
    );
    assert_eq!(
        stroke_cap_name(actual.cap),
        Some(expected.cap.as_str()),
        "{fixture_id}: stroke cap"
    );
    assert_eq!(
        stroke_join_name(actual.join),
        Some(expected.join.as_str()),
        "{fixture_id}: stroke join"
    );
    assert_eq!(
        actual.miter_limit.to_bits(),
        expected.miter_limit.to_bits(),
        "{fixture_id}: stroke miterLimit"
    );
    assert_eq!(
        actual.dash_offset.to_bits(),
        expected.dash_offset.to_bits(),
        "{fixture_id}: stroke dashOffset"
    );
    assert_eq!(
        actual
            .dash
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        expected
            .dash
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        "{fixture_id}: stroke dash"
    );
    assert_eq!(
        actual.fill_rgba.map(float_bits),
        expected.fill_linear_rgba.map(float_bits),
        "{fixture_id}: stroke fill"
    );
    assert_eq!(
        usize::from(expected.fill_linear_rgba.is_some())
            + usize::from(expected.linear_gradient.is_some())
            + usize::from(expected.radial_gradient.is_some())
            + usize::from(expected.image_pattern.is_some()),
        1,
        "{fixture_id}: stroke expectation must select one paint payload"
    );
    if let Some(expected) = &expected.linear_gradient {
        let actual = actual
            .linear_gradient
            .as_ref()
            .unwrap_or_else(|| panic!("{fixture_id}: linear gradient stroke payload"));
        assert_linear_gradient_matches(actual, expected, fixture_id);
    } else {
        assert!(
            actual.linear_gradient.is_none(),
            "{fixture_id}: unexpected linear gradient stroke payload"
        );
    }
    if let Some(expected) = &expected.radial_gradient {
        let actual = actual
            .radial_gradient
            .as_ref()
            .unwrap_or_else(|| panic!("{fixture_id}: radial gradient stroke payload"));
        assert_radial_gradient_matches(actual, expected, fixture_id);
    } else {
        assert!(
            actual.radial_gradient.is_none(),
            "{fixture_id}: unexpected radial gradient stroke payload"
        );
    }
    if let Some(expected) = &expected.image_pattern {
        let actual = actual
            .image_pattern
            .as_ref()
            .unwrap_or_else(|| panic!("{fixture_id}: ImagePattern stroke payload"));
        assert_image_pattern_matches(actual, expected, fixture_id);
    } else {
        assert!(
            actual.image_pattern.is_none(),
            "{fixture_id}: unexpected ImagePattern stroke payload"
        );
    }
}

fn stable_ids(draw_order: &str, fixture_id: &str) -> (u64, u64) {
    let (layer_name, node_path) = draw_order
        .split_once('/')
        .unwrap_or_else(|| panic!("{fixture_id}: drawOrder entry must contain a layer/name path"));
    let layer_id = derive_stable_id(EntityKind::RenderLayer, &format!("layer/{layer_name}"));
    let node_id = derive_stable_id(
        EntityKind::RenderNode,
        &format!(
            "layer/{layer_name}/node/{}",
            node_path.replace('/', "/node/")
        ),
    );
    (layer_id, node_id)
}

fn assert_draw_op_matches(
    render: &fcs_render::DecodedRenderChart,
    draw: &DrawOp,
    expected_label: &str,
    expected: &SemanticDrawExpectation,
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
        draw.fill_rgba.map(float_bits),
        expected.fill_linear_rgba.map(float_bits),
        "{fixture_id}: fill"
    );
    let fill_payloads = usize::from(expected.fill_linear_rgba.is_some())
        + usize::from(expected.linear_gradient.is_some())
        + usize::from(expected.radial_gradient.is_some())
        + usize::from(expected.image_pattern.is_some());
    assert!(
        fill_payloads <= 1,
        "{fixture_id}: fill expectation payload count"
    );
    if let Some(expected) = &expected.linear_gradient {
        let actual = draw
            .linear_gradient
            .as_ref()
            .unwrap_or_else(|| panic!("{fixture_id}: linear gradient fill payload"));
        assert_linear_gradient_matches(actual, expected, fixture_id);
    } else {
        assert!(
            draw.linear_gradient.is_none(),
            "{fixture_id}: unexpected linear gradient fill payload"
        );
    }
    if let Some(expected) = &expected.radial_gradient {
        let actual = draw
            .radial_gradient
            .as_ref()
            .unwrap_or_else(|| panic!("{fixture_id}: radial gradient fill payload"));
        assert_radial_gradient_matches(actual, expected, fixture_id);
    } else {
        assert!(
            draw.radial_gradient.is_none(),
            "{fixture_id}: unexpected radial gradient fill payload"
        );
    }
    if let Some(expected) = &expected.image_pattern {
        let actual = draw
            .image_pattern
            .as_ref()
            .unwrap_or_else(|| panic!("{fixture_id}: ImagePattern fill payload"));
        assert_image_pattern_matches(actual, expected, fixture_id);
    } else {
        assert!(
            draw.image_pattern.is_none(),
            "{fixture_id}: unexpected ImagePattern fill payload"
        );
    }
    match (&draw.stroke, &expected.stroke) {
        (Some(actual), Some(expected)) => assert_stroke_matches(actual, expected, fixture_id),
        (None, None) => {}
        _ => panic!("{fixture_id}: stroke presence"),
    }
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
            expected.draw_ops.len(),
            expected.draw_order.len(),
            "{}: semantic drawOps length",
            fixture.id
        );
        for (index, draw) in draw.iter().enumerate() {
            assert_draw_op_matches(
                &render,
                draw,
                &expected.draw_order[index],
                &expected.draw_ops[index],
                &fixture.id,
            );
        }
    }
}
