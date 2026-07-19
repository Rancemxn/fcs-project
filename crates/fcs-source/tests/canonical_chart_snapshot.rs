use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;
use serde_json::Value;

#[path = "support/canonical_snapshot.rs"]
mod canonical_snapshot;

use canonical_snapshot::{canonical_snapshot, chart_value};

const DIRECT: &str =
    include_str!("../../../docs/conformance/fcs5/source/valid/canonical-equivalent-direct.fcs");
const TEMPLATE: &str =
    include_str!("../../../docs/conformance/fcs5/source/valid/canonical-equivalent-template.fcs");
const METADATA: &str =
    include_str!("../../../docs/conformance/fcs5/source/valid/metadata-credits-resources-sync.fcs");
const TRACKS: &str =
    include_str!("../../../docs/conformance/fcs5/source/valid/track-boundaries.fcs");
const GOLDEN: &str =
    include_str!("../../../docs/conformance/fcs5/expected/canonical-chart-snapshot.json");

fn canonical(source: &str) -> fcs_model::CanonicalChart {
    parse_document(source)
        .into_result()
        .expect("source should parse")
        .canonical_chart(CompileTimeLimits::default())
        .unwrap_or_else(|diagnostics| panic!("canonical chart lowering failed: {diagnostics:?}"))
}

#[test]
fn direct_and_template_authoring_produce_the_checked_in_canonical_snapshot() {
    let direct = canonical(DIRECT);
    let template = canonical(TEMPLATE);

    let direct_snapshot = canonical_snapshot(&direct);
    let template_snapshot = canonical_snapshot(&template);

    assert_eq!(direct, template);
    assert_eq!(direct_snapshot, template_snapshot);
    assert_eq!(direct_snapshot, GOLDEN);
}

#[test]
fn canonical_snapshot_projects_nonempty_metadata_tracks_and_extensions() {
    let metadata = chart_value(&canonical(METADATA));
    assert_eq!(
        metadata
            .pointer("/metadata/contributors/0/id")
            .and_then(Value::as_str),
        Some("alice")
    );
    assert_eq!(
        metadata
            .pointer("/metadata/credits/0/role")
            .and_then(Value::as_str),
        Some("charter")
    );
    assert_eq!(
        metadata
            .pointer("/metadata/meta/custom/entries/0/key")
            .and_then(Value::as_str),
        Some("suite")
    );
    assert_eq!(
        metadata
            .pointer("/metadata/meta/custom/entries/1/key")
            .and_then(Value::as_str),
        Some("stable")
    );
    assert_eq!(
        metadata
            .pointer("/metadata/resources/0/declaredSha256")
            .and_then(Value::as_str),
        Some("66eb55e69c42345c65021ea9364fc43c61d2151dde67a89dc02362543b289903")
    );
    assert_eq!(
        metadata
            .pointer("/metadata/sync/audioOffsetSeconds")
            .and_then(Value::as_f64),
        Some(0.1)
    );

    let tracks = chart_value(&canonical(TRACKS));
    assert_eq!(
        tracks.pointer("/tracks/0/name").and_then(Value::as_str),
        Some("fade")
    );
    assert_eq!(
        tracks
            .pointer("/tracks/0/pieces/0/kind")
            .and_then(Value::as_str),
        Some("segment")
    );
    assert_eq!(
        tracks
            .pointer("/tracks/0/pieces/0/interpolation/kind")
            .and_then(Value::as_str),
        Some("linear")
    );
    assert_eq!(
        tracks
            .pointer("/tracks/0/pieces/1/kind")
            .and_then(Value::as_str),
        Some("point")
    );

    let extensions = chart_value(&canonical(
        r#"#fcs 5.0.0
format { profile: chart; features: [playable,]; }
extensions { extension("org.test.snapshot", 1.0.0) required { "mode": "test", } }
tempoMap { 0beat -> 120bpm; }
"#,
    ));
    assert_eq!(
        extensions.pointer("/features/0").and_then(Value::as_str),
        Some("playable")
    );
    assert_eq!(
        extensions
            .pointer("/requiredExtensions/0/namespace")
            .and_then(Value::as_str),
        Some("org.test.snapshot")
    );
    assert_eq!(
        extensions
            .pointer("/requiredExtensions/0/version")
            .and_then(Value::as_str),
        Some("1.0.0")
    );
}

#[test]
fn canonical_snapshot_excludes_authoring_and_workspace_state() {
    let snapshot = chart_value(&canonical(TEMPLATE));
    assert_forbidden_keys_absent(&snapshot);

    let text = serde_json::to_string(&snapshot).expect("snapshot value should serialize");
    for forbidden in [
        "#fcs",
        "canonical-equivalent-template.fcs",
        "org.phigros.rpe",
        "preserveRawPayload",
        env!("CARGO_MANIFEST_DIR"),
    ] {
        assert!(
            !text.contains(forbidden),
            "canonical snapshot retained forbidden authoring/workspace value {forbidden:?}"
        );
    }

    let resource_snapshot = chart_value(&canonical(METADATA));
    assert_forbidden_keys_absent(&resource_snapshot);
    let resource_text =
        serde_json::to_string(&resource_snapshot).expect("snapshot value should serialize");
    assert!(
        !resource_text.contains("assets/opaque-resource.bin"),
        "canonical snapshot retained a logical workspace resource source path"
    );
}

fn assert_forbidden_keys_absent(value: &Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                assert!(
                    !matches!(
                        key.as_str(),
                        "sourceText"
                            | "workspacePath"
                            | "span"
                            | "spans"
                            | "template"
                            | "templates"
                            | "generator"
                            | "generators"
                            | "local"
                            | "locals"
                            | "preserve"
                            | "preserveRawPayload"
                            | "payload"
                    ),
                    "canonical snapshot retained forbidden authoring key {key:?}"
                );
                assert_forbidden_keys_absent(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                assert_forbidden_keys_absent(value);
            }
        }
        _ => {}
    }
}
