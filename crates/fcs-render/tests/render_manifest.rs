use std::fs;
use std::path::{Path, PathBuf};

use fcs_fcbc::write_from_compilation;
use fcs_render::{load_render, rasterize_solid_rgba8_at};
use fcs_source::ResourceLimits;
use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;

fn load_toml(path: &Path) -> toml::Value {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    toml::from_str(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn decode_hex_file(path: &Path) -> Vec<u8> {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let filtered: String = source
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect();
    assert!(
        filtered.len().is_multiple_of(2),
        "odd hex string length in {path:?}"
    );
    (0..filtered.len())
        .step_by(2)
        .map(|index| {
            let pair = &filtered[index..index + 2];
            u8::from_str_radix(pair, 16)
                .unwrap_or_else(|error| panic!("invalid hex byte {pair:?} in {path:?}: {error}"))
        })
        .collect()
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
        assert!(
            matches!(
                fixture_string(fixture, "color_space", id),
                "linear-srgb" | "srgb"
            ),
            "{id}: unsupported Render output color space"
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
