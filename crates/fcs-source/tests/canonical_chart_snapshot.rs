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
