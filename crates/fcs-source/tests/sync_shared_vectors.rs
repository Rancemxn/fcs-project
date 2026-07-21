//! Shared player/converter vectors for the FCS §7.4 sync formula.

use std::fs;
use std::path::PathBuf;

use fcs_model::{AudioOffset, CanonicalPreview, CanonicalSync};
use fcs_source::parse_document;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SharedVectors {
    formula: Vec<FormulaVector>,
    preview: Vec<PreviewVector>,
}

#[derive(Debug, Deserialize)]
struct FormulaVector {
    id: String,
    audio_offset_seconds: f64,
    chart_time_seconds: f64,
    audio_time_seconds: f64,
}

#[derive(Debug, Deserialize)]
struct PreviewVector {
    id: String,
    audio_offset_seconds: f64,
    preview_start_seconds: f64,
    preview_end_seconds: f64,
    contains_chart_times: Vec<f64>,
    excludes_chart_times: Vec<f64>,
}

fn vectors() -> SharedVectors {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/conformance/fcs5/expected/sync-shared-vectors.toml");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    toml::from_str(&text).unwrap_or_else(|error| panic!("invalid shared vectors: {error}"))
}

#[test]
fn shared_formula_vectors_are_bidirectional_for_player_and_converter() {
    for vector in vectors().formula {
        let offset = AudioOffset::new(vector.audio_offset_seconds)
            .unwrap_or_else(|error| panic!("{}: {error}", vector.id));
        let sync = CanonicalSync::new(Some("song".into()), offset, None)
            .unwrap_or_else(|error| panic!("{}: {error}", vector.id));
        assert_eq!(
            sync.audio_time(vector.chart_time_seconds).unwrap(),
            vector.audio_time_seconds,
            "{} forward",
            vector.id
        );
        assert_eq!(
            sync.chart_time(vector.audio_time_seconds).unwrap(),
            vector.chart_time_seconds,
            "{} inverse",
            vector.id
        );
        // Direct model methods must match the offset helpers used by converters.
        assert_eq!(
            offset.audio_time(vector.chart_time_seconds).unwrap(),
            vector.audio_time_seconds,
            "{} offset forward",
            vector.id
        );
        assert_eq!(
            offset.chart_time(vector.audio_time_seconds).unwrap(),
            vector.chart_time_seconds,
            "{} offset inverse",
            vector.id
        );
    }
}

#[test]
fn shared_preview_vectors_are_audio_domain_half_open() {
    for vector in vectors().preview {
        let offset = AudioOffset::new(vector.audio_offset_seconds)
            .unwrap_or_else(|error| panic!("{}: {error}", vector.id));
        let preview =
            CanonicalPreview::new(vector.preview_start_seconds, vector.preview_end_seconds)
                .unwrap_or_else(|| panic!("{}: invalid preview domain", vector.id));
        let sync = CanonicalSync::new(Some("song".into()), offset, Some(preview))
            .unwrap_or_else(|error| panic!("{}: {error}", vector.id));
        for chart_time in vector.contains_chart_times {
            assert!(
                sync.preview_contains_chart_time(chart_time).unwrap(),
                "{} should contain chartTime {chart_time}",
                vector.id
            );
        }
        for chart_time in vector.excludes_chart_times {
            assert!(
                !sync.preview_contains_chart_time(chart_time).unwrap(),
                "{} should exclude chartTime {chart_time}",
                vector.id
            );
        }
    }
}

#[test]
fn metadata_fixture_expected_offset_equation_is_executable() {
    let source = include_str!(
        "../../../docs/conformance/fcs5/source/valid/metadata-credits-resources-sync.fcs"
    );
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/conformance/fcs5/expected/metadata-credits-resources-sync.json"
    ))
    .unwrap();
    let metadata = parse_document(source)
        .into_result()
        .unwrap()
        .canonical_metadata()
        .unwrap();
    let sync = metadata.sync().expect("fixture has sync");
    assert_eq!(
        sync.audio_offset().seconds(),
        expected["audioOffsetSeconds"].as_f64().unwrap()
    );
    let chart = expected["offsetEquationAtChartTime1"]["chartTime"]
        .as_f64()
        .unwrap();
    let audio = expected["offsetEquationAtChartTime1"]["audioTime"]
        .as_f64()
        .unwrap();
    assert_eq!(sync.audio_time(chart).unwrap(), audio);
    assert_eq!(sync.chart_time(audio).unwrap(), chart);
}
