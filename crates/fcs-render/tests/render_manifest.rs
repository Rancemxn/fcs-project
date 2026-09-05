use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use fcs_fcbc::write_from_compilation;
use fcs_render::{
    DrawOp, GradientStopDrawOp, ImagePatternDrawOp, LinearGradientDrawOp, RadialGradientDrawOp,
    evaluate_semantic_draw_list_at, load_render, rasterize_solid_rgba8_at,
};
use fcs_source::ResourceLimits;
use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn load_toml(path: &Path) -> toml::Value {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    toml::from_str(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn decode_hex_file(path: &Path) -> Vec<u8> {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    decode_hex(&source)
}

fn decode_hex(source: &str) -> Vec<u8> {
    let filtered: String = source
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect();
    assert!(
        filtered.len().is_multiple_of(2) && filtered.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "fixture must contain complete ASCII hex bytes"
    );
    (0..filtered.len())
        .step_by(2)
        .map(|index| {
            let pair = &filtered[index..index + 2];
            u8::from_str_radix(pair, 16).unwrap()
        })
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut encoded, "{byte:02x}").unwrap();
    }
    encoded
}

fn bits(values: &[f64]) -> Vec<String> {
    values
        .iter()
        .map(|value| format!("{:016x}", value.to_bits()))
        .collect()
}

fn stops_snapshot(stops: &[GradientStopDrawOp]) -> Vec<Value> {
    stops
        .iter()
        .map(|stop| {
            json!({
                "offsetBits": format!("{:016x}", stop.offset.to_bits()),
                "colorBits": bits(&stop.color),
            })
        })
        .collect()
}

fn paint_snapshot(
    color: Option<[f64; 4]>,
    linear: Option<&LinearGradientDrawOp>,
    radial: Option<&RadialGradientDrawOp>,
    pattern: Option<&ImagePatternDrawOp>,
) -> Value {
    match (color, linear, radial, pattern) {
        (Some(color), None, None, None) => json!({"kind": 1, "colorBits": bits(&color)}),
        (None, Some(paint), None, None) => json!({
            "kind": 2, "startBits": bits(&paint.start), "endBits": bits(&paint.end),
            "spread": paint.spread, "stops": stops_snapshot(&paint.stops),
        }),
        (None, None, Some(paint), None) => json!({
            "kind": 3, "startCenterBits": bits(&paint.start_center),
            "startRadiusBits": format!("{:016x}", paint.start_radius.to_bits()),
            "endCenterBits": bits(&paint.end_center),
            "endRadiusBits": format!("{:016x}", paint.end_radius.to_bits()),
            "spread": paint.spread, "stops": stops_snapshot(&paint.stops),
        }),
        (None, None, None, Some(paint)) => json!({
            "kind": 4, "resourceId": format!("{:016x}", paint.resource_id),
            "positionBits": bits(&paint.position), "originBits": bits(&paint.origin),
            "rotationBits": format!("{:016x}", paint.rotation.to_bits()),
            "scaleBits": bits(&paint.scale), "repeat": paint.repeat, "sampling": paint.sampling,
        }),
        (None, None, None, None) => Value::Null,
        _ => panic!("a DrawOp paint must have exactly one kind"),
    }
}

fn draw_snapshot(draw: &DrawOp) -> Value {
    let (pass, z_order, document_order, layer_id) = draw.layer_key;
    json!({
        "nodeId": format!("{:016x}", draw.node_id),
        "nodeKind": draw.kind as u16,
        "layerKey": [json!(pass), json!(z_order), json!(document_order), json!(format!("{layer_id:016x}"))],
        "ancestryKey": draw.ancestry_key.iter().map(|(z, order, id)| json!([z, order, format!("{id:016x}")])).collect::<Vec<_>>(),
        "geometryId": format!("{:016x}", draw.geometry_id),
        "worldMatrixBits": bits(&draw.world_matrix),
        "worldBoundsBits": bits(&draw.bounds),
        "opacityBits": format!("{:016x}", draw.opacity.to_bits()),
        "composite": draw.composite,
        "clipChain": draw.clip_chain.iter().map(|id| format!("{id:016x}")).collect::<Vec<_>>(),
        "isolationChain": draw.isolation_chain.iter().map(|boundary| json!({
            "nodeId": format!("{:016x}", boundary.node_id),
            "opacityBits": format!("{:016x}", boundary.opacity.to_bits()),
            "composite": boundary.composite,
        })).collect::<Vec<_>>(),
        "fill": paint_snapshot(draw.fill_rgba, draw.linear_gradient.as_ref(), draw.radial_gradient.as_ref(), draw.image_pattern.as_ref()),
        "stroke": draw.stroke.as_ref().map(|stroke| json!({
            "widthBits": format!("{:016x}", stroke.width.to_bits()),
            "cap": stroke.cap, "join": stroke.join,
            "miterLimitBits": format!("{:016x}", stroke.miter_limit.to_bits()),
            "dashOffsetBits": format!("{:016x}", stroke.dash_offset.to_bits()),
            "dashBits": bits(&stroke.dash),
            "paint": paint_snapshot(stroke.fill_rgba, stroke.linear_gradient.as_ref(), stroke.radial_gradient.as_ref(), stroke.image_pattern.as_ref()),
        })),
        "image": draw.image.as_ref().map(|image| json!({
            "resourceId": format!("{:016x}", image.resource_id),
            "destinationBits": bits(&image.destination), "sourceBits": bits(&image.source),
            "sampling": image.sampling,
        })),
        "text": draw.text.as_ref().map(|text| json!({
            "originBits": bits(&text.origin),
            "runs": text.runs.iter().map(|run| json!({
                "runId": format!("{:016x}", run.run_id),
                "fontResourceId": format!("{:016x}", run.font_resource_id),
                "faceIndex": run.face_index, "sizeBits": format!("{:016x}", run.size.to_bits()),
                "runOffsetBits": bits(&run.run_offset),
                "glyphs": run.glyphs.iter().map(|glyph| json!({
                    "glyphId": glyph.glyph_id, "originBits": bits(&glyph.origin),
                    "worldOriginBits": bits(&glyph.world_origin),
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })),
    })
}

fn fixture_string<'a>(fixture: &'a toml::Value, field: &str, id: &str) -> &'a str {
    fixture
        .get(field)
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| panic!("{id}: manifest field {field:?} must be a string"))
}

fn fixture_u32(fixture: &toml::Value, field: &str, id: &str) -> u32 {
    let value = fixture
        .get(field)
        .and_then(toml::Value::as_integer)
        .unwrap_or_else(|| panic!("{id}: manifest field {field:?} must be an integer"));
    u32::try_from(value).unwrap_or_else(|_| panic!("{id}: manifest field {field:?} must be a u32"))
}

fn fixture_f64(fixture: &toml::Value, field: &str, id: &str) -> f64 {
    fixture
        .get(field)
        .and_then(|value| {
            value
                .as_float()
                .or_else(|| value.as_integer().map(|v| v as f64))
        })
        .unwrap_or_else(|| panic!("{id}: manifest field {field:?} must be numeric"))
}

#[test]
fn declared_render_raster_fixtures_match_product_output() {
    let render_root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/conformance/render");
    let manifest = load_toml(&render_root.join("manifest.toml"));
    let fixtures = manifest
        .get("fixture")
        .and_then(toml::Value::as_array)
        .expect("Render manifest must declare fixture entries");
    assert!(
        !fixtures.is_empty(),
        "Render manifest must not have an empty fixture set"
    );

    for fixture in fixtures {
        let id = fixture_string(fixture, "id", "Render fixture");
        let source_path = render_root.join(fixture_string(fixture, "source", id));
        let source = fs::read_to_string(&source_path).unwrap_or_else(|error| {
            panic!("{id}: failed to read {}: {error}", source_path.display())
        });
        let document = parse_document(&source)
            .into_result()
            .unwrap_or_else(|errors| panic!("{id}: source parse failed: {errors:?}"));
        let compilation = document
            .canonical_compilation_with_source(
                &source,
                CompileTimeLimits::default(),
                &render_root,
                ResourceLimits::default(),
            )
            .unwrap_or_else(|errors| panic!("{id}: canonical lowering failed: {errors:?}"));
        let bytes = write_from_compilation(&compilation)
            .unwrap_or_else(|error| panic!("{id}: FCBC writing failed: {error}"));
        let render = load_render(&bytes)
            .unwrap_or_else(|error| panic!("{id}: Render loading failed: {error}"));

        let chart_time = fixture_f64(fixture, "chart_time_seconds", id);
        assert!(chart_time.is_finite(), "{id}: chart time must be finite");
        let width = fixture_u32(fixture, "width", id);
        let height = fixture_u32(fixture, "height", id);
        assert!(
            width > 0 && height > 0,
            "{id}: raster dimensions must be positive"
        );
        assert_eq!(fixture_string(fixture, "pixel_format", id), "rgba8", "{id}");
        let color_space = match fixture_string(fixture, "color_space", id) {
            "linear-srgb" => 1,
            "srgb" => 2,
            _ => panic!("{id}: unsupported Render output color space"),
        };
        assert_eq!(
            render.viewport_color_space, color_space,
            "{id}: color space"
        );

        let expected_path = render_root.join(fixture_string(fixture, "raster_expected", id));
        let expected = decode_hex_file(&expected_path);
        let expected_len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .unwrap_or_else(|| panic!("{id}: raster dimensions overflow RGBA8 length"));
        assert_eq!(expected.len(), expected_len, "{id}: raster fixture length");

        let actual = rasterize_solid_rgba8_at(&render, chart_time, width, height)
            .unwrap_or_else(|error| panic!("{id}: product rasterization failed: {error}"));
        assert_eq!(
            actual, expected,
            "{id}: product raster differs from raster_expected"
        );
    }
}

#[test]
fn declared_render_binary_corpus_matches_product_semantics_raster_and_mutations() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/conformance/render");
    let suite = load_toml(&root.join("manifest.toml"));
    let fixtures = suite["binary_fixture"].as_array().unwrap();
    assert!(!fixtures.is_empty());
    for fixture in fixtures {
        let id = fixture_string(fixture, "id", "Render binary fixture");
        let golden = load_toml(&root.join(fixture_string(fixture, "manifest", id)));
        let bytes = decode_hex_file(&root.join(fixture_string(&golden, "path", id)));
        assert_eq!(
            sha256_hex(&bytes),
            fixture_string(&golden, "sha256", id),
            "{id}"
        );
        let render = load_render(&bytes).unwrap_or_else(|error| panic!("{id}: {error}"));
        let vector = load_toml(&root.join(fixture_string(fixture, "vector", id)));
        assert_eq!(fixture_u32(&vector, "schema_version", id), 1);
        assert_eq!(fixture_string(&vector, "id", id), id);
        let expected: Value = serde_json::from_str(
            &fs::read_to_string(root.join(fixture_string(fixture, "semantic_expected", id)))
                .unwrap(),
        )
        .unwrap();
        let frames = vector["frame"].as_array().unwrap();
        assert!(!frames.is_empty());
        assert_eq!(frames.len(), expected["frames"].as_array().unwrap().len());
        // Query in both directions, with an earlier frame after a later one.
        for index in (0..frames.len()).chain((0..frames.len()).rev()) {
            let time_bits = fixture_string(&frames[index], "chart_time_bits", id);
            let time = f64::from_bits(u64::from_str_radix(time_bits, 16).unwrap());
            let draws = evaluate_semantic_draw_list_at(&render, time).unwrap();
            let actual = json!({
                "chartTimeBits": time_bits,
                "drawOps": draws.iter().map(draw_snapshot).collect::<Vec<_>>(),
            });
            assert_eq!(
                actual, expected["frames"][index],
                "{id}: semantic frame {index}"
            );
        }
        let raster = &vector["raster"];
        let time = f64::from_bits(
            u64::from_str_radix(fixture_string(raster, "chart_time_bits", id), 16).unwrap(),
        );
        let width = fixture_u32(raster, "width", id);
        let height = fixture_u32(raster, "height", id);
        assert_eq!(fixture_string(raster, "color_space", id), "srgb");
        assert_eq!(render.viewport_color_space, 2);
        assert_eq!(fixture_string(raster, "pixel_format", id), "rgba8");
        assert_eq!(fixture_u32(raster, "sample_grid", id), 8);
        assert_eq!(fixture_u32(raster, "max_channel_difference", id), 1);
        assert_eq!(
            fixture_u32(raster, "max_different_pixels_per_thousand", id),
            1
        );
        let expected = decode_hex_file(&root.join(fixture_string(fixture, "raster_expected", id)));
        assert_eq!(expected.len(), width as usize * height as usize * 4);
        assert_eq!(
            expected.len(),
            fixture_u32(raster, "decoded_length", id) as usize
        );
        assert_eq!(sha256_hex(&expected), fixture_string(raster, "sha256", id));
        let actual = rasterize_solid_rgba8_at(&render, time, width, height).unwrap();
        assert_eq!(actual.len(), expected.len());
        assert!(
            actual
                .iter()
                .zip(&expected)
                .all(|(a, b)| a.abs_diff(*b) <= 1),
            "{id}: channel tolerance"
        );
        let different_pixels = actual
            .as_chunks::<4>()
            .0
            .iter()
            .zip(expected.as_chunks::<4>().0.iter())
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            different_pixels * 1000 <= expected.len() / 4,
            "{id}: pixel tolerance"
        );

        let mutations = load_toml(&root.join(fixture_string(fixture, "mutations", id)));
        assert_eq!(fixture_u32(&mutations, "schema_version", id), 1);
        assert_eq!(
            fixture_string(&mutations, "base", id),
            fixture_string(&golden, "path", id)
        );
        let cases = mutations["mutation"].as_array().unwrap();
        assert!(!cases.is_empty());
        for case in cases {
            let name = fixture_string(case, "id", id);
            let mut mutated = bytes.clone();
            for patch in case["patch"].as_array().unwrap() {
                let offset = fixture_u32(patch, "offset", name) as usize;
                let replacement = decode_hex(fixture_string(patch, "replace_hex", name));
                mutated[offset..offset + replacement.len()].copy_from_slice(&replacement);
            }
            assert_eq!(
                load_render(&mutated),
                Err(fixture_string(case, "diagnostic", name)),
                "{id}/{name}"
            );
        }
    }
}
