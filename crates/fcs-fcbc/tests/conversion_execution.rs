//! End-to-end Execution ABI coverage over conversion-produced charts (Issue #666).
//!
//! Every public PGR/RPE/PEC fixture runs the full product import pipeline —
//! source artifact through conversion lowering to a `CanonicalCompilation`,
//! then `write_from_compilation` and `load_chart` — and the shared queries
//! (distance, scroll coordinate, note presentation alpha) are compared
//! bit for bit against the independent reference loader/evaluator. This
//! exercises `write_from_compilation` with canonical charts produced by
//! Conversion, not only FCS-authored charts.

#[path = "../../fcs-source/tests/support/fcbc_reference_evaluator.rs"]
mod fcbc_reference_evaluator;
#[path = "../../fcs-source/tests/support/fcbc_reference_loader.rs"]
mod fcbc_reference_loader;

use std::path::PathBuf;

use fcs_conversion::{
    ArtifactRole, DecimalLimits, ExactDecimal, PecLimits, PecProfile, PecProfileBinding, PgrLimits,
    PgrProfile, PgrProfileBinding, RpeLimits, RpeProfileBinding, SourceArtifact, SourceFormat,
    interpret_pec, interpret_pgr, interpret_rpe_semantics, lower_pec_to_canonical,
    lower_pgr_to_canonical, lower_rpe_to_canonical, parse_json_document, parse_pec_document,
    parse_pgr_document, parse_rpe_document,
};
use fcs_fcbc::{
    DistanceClassification, EvaluationEnvironment, RuntimeValue, ValueType, load_chart,
    query_descriptor, query_distance, query_scroll_coordinate, write_from_compilation,
};
use fcs_model::{CanonicalCompilation, ConversionStatus};

fn sources_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/conformance/conversion/public-fixtures/sources")
}

fn decimal(raw: &str) -> ExactDecimal {
    ExactDecimal::parse(raw, DecimalLimits::default()).unwrap()
}

fn read_artifact(file: &str) -> SourceArtifact {
    let bytes = std::fs::read(sources_root().join(file)).unwrap();
    SourceArtifact::new(file, ArtifactRole::Chart, bytes).unwrap()
}

fn import_pgr(file: &str, profile: PgrProfile, floor_scale_px: &str) -> CanonicalCompilation {
    let artifact = read_artifact(file);
    let binding = PgrProfileBinding::new(profile, decimal(floor_scale_px)).unwrap();
    let parsed = parse_json_document(SourceFormat::Pgr, &artifact).unwrap();
    let source = parse_pgr_document(&parsed, PgrLimits::default()).unwrap();
    let semantic = interpret_pgr(&source, &binding).unwrap();
    let (compilation, report) = lower_pgr_to_canonical(&semantic, &artifact)
        .unwrap()
        .into_parts();
    assert_eq!(report.status(), ConversionStatus::Equivalent);
    compilation
}

fn import_rpe(file: &str) -> CanonicalCompilation {
    let artifact = read_artifact(file);
    let binding = RpeProfileBinding::phira_legacy_speed();
    let parsed = parse_json_document(SourceFormat::Rpe, &artifact).unwrap();
    let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
    let semantic = interpret_rpe_semantics(&source, &binding).unwrap();
    let (compilation, report) = lower_rpe_to_canonical(&semantic, &artifact)
        .unwrap()
        .into_parts();
    assert_eq!(report.status(), ConversionStatus::Equivalent);
    compilation
}

fn import_pec(file: &str, floor_scale_px: &str) -> CanonicalCompilation {
    let artifact = read_artifact(file);
    let binding = PecProfileBinding::new(PecProfile::Phira, decimal(floor_scale_px)).unwrap();
    let source = parse_pec_document(&artifact, PecLimits::default()).unwrap();
    let semantic = interpret_pec(&source, &binding).unwrap();
    let (compilation, report) = lower_pec_to_canonical(&semantic, &artifact)
        .unwrap()
        .into_parts();
    assert_eq!(report.status(), ConversionStatus::Equivalent);
    compilation
}

/// Writes, loads, and cross-checks one conversion-produced compilation
/// against the independent reference implementation. `expected_lines` and
/// `expected_notes` come from the public fixture manifest.
fn exercise(
    id: &str,
    compilation: &CanonicalCompilation,
    expected_lines: usize,
    expected_notes: usize,
) {
    let bytes = write_from_compilation(compilation).unwrap();
    let chart = load_chart(&bytes).unwrap();
    let reference = fcbc_reference_loader::load(&bytes).unwrap();

    assert_eq!(chart.lines.len(), expected_lines, "{id}: line count");
    assert_eq!(chart.notes.len(), expected_notes, "{id}: note count");
    assert_eq!(
        reference.lines.len(),
        expected_lines,
        "{id}: reference line count"
    );
    assert_eq!(
        reference.notes.len(),
        expected_notes,
        "{id}: reference note count"
    );

    // Query times: the chart origin, every note head/tail, and each line's
    // integration origin (always inside the distance domain).
    let mut times = vec![0.0f64];
    for note in &chart.notes {
        times.push(note.time);
        if note.end_time > note.time {
            times.push(note.end_time);
        }
    }
    for line in &chart.lines {
        times.push(line.integration_origin);
    }

    for line in &chart.lines {
        let reference_line = reference
            .lines
            .iter()
            .find(|candidate| candidate.id == line.id)
            .unwrap_or_else(|| panic!("{id}: reference line {} missing", line.id));
        let mut distance_successes = 0usize;
        for time in &times {
            let product = query_distance(&chart, line.distance_descriptor, *time);
            let expected = fcbc_reference_evaluator::query_distance(
                &reference,
                reference_line.distance_descriptor,
                *time,
            );
            match (product, expected) {
                (Ok(product), Ok(expected)) => {
                    assert_eq!(
                        product.floor_position.to_bits(),
                        expected.floor_position.to_bits(),
                        "{id}: distance bits for line {} at {time}",
                        line.id
                    );
                    let same_classification = matches!(
                        (product.classification, expected.classification),
                        (
                            DistanceClassification::PortableAnalytic,
                            fcbc_reference_loader::DistanceClassification::PortableAnalytic
                        ) | (
                            DistanceClassification::PortableEvaluable,
                            fcbc_reference_loader::DistanceClassification::PortableEvaluable
                        )
                    );
                    assert!(
                        same_classification,
                        "{id}: distance classification for line {} at {time}: {:?} vs {:?}",
                        line.id, product.classification, expected.classification
                    );
                    distance_successes += 1;
                }
                (Err(_), Err(_)) => {}
                (product, expected) => panic!(
                    "{id}: distance disagreement for line {} at {time}: product={product:?} reference={expected:?}",
                    line.id
                ),
            }

            let product = query_scroll_coordinate(&chart, line.scroll_tempo_descriptor, *time);
            let expected = fcbc_reference_evaluator::query_scroll_coordinate(
                &reference,
                reference_line.scroll_tempo_descriptor,
                *time,
            );
            match (product, expected) {
                (Ok(product), Ok(expected)) => assert_eq!(
                    product.to_bits(),
                    expected.to_bits(),
                    "{id}: scroll coordinate bits for line {} at {time}",
                    line.id
                ),
                (Err(_), Err(_)) => {}
                (product, expected) => panic!(
                    "{id}: scroll disagreement for line {} at {time}: product={product:?} reference={expected:?}",
                    line.id
                ),
            }
        }
        assert!(
            distance_successes >= 1,
            "{id}: line {} never returned a distance",
            line.id
        );
    }

    // The presentation alpha descriptor of every note must evaluate to a
    // finite float at the note head, identically in both evaluators.
    for note in &chart.notes {
        let reference_note = reference
            .notes
            .iter()
            .find(|candidate| candidate.id == note.id)
            .unwrap_or_else(|| panic!("{id}: reference note {} missing", note.id));
        let product = query_descriptor(
            &chart,
            note.property_descriptors[4],
            note.time,
            EvaluationEnvironment::at_time(note.time),
        )
        .unwrap_or_else(|error| panic!("{id}: note {} alpha descriptor failed: {error}", note.id));
        let expected = fcbc_reference_evaluator::query_descriptor(
            &reference,
            reference_note.property_descriptors[4],
            note.time,
            fcbc_reference_evaluator::EvaluationEnvironment::at_time(note.time),
        )
        .unwrap();
        match (product.value, expected.value) {
            (
                RuntimeValue::Scalar {
                    ty: ValueType::Float,
                    value,
                },
                fcbc_reference_loader::RuntimeValue::Scalar {
                    ty: fcbc_reference_loader::ValueType::Float,
                    value: reference_value,
                },
            ) => {
                assert_eq!(
                    value.to_bits(),
                    reference_value.to_bits(),
                    "{id}: note {} alpha bits",
                    note.id
                );
                assert!(value.is_finite(), "{id}: note {} alpha not finite", note.id);
            }
            (product_value, expected_value) => panic!(
                "{id}: note {} alpha disagreement: product={product_value:?} reference={expected_value:?}",
                note.id
            ),
        }
    }
}

#[test]
fn pgr_minimal_fixture_runs_the_full_product_pipeline() {
    exercise(
        "pgr-minimal",
        &import_pgr("pgr-minimal.pgr.json", PgrProfile::PhiraV1, "120"),
        1,
        2,
    );
}

#[test]
fn pgr_feature_fixture_runs_the_full_product_pipeline() {
    exercise(
        "pgr-feature",
        &import_pgr("pgr-feature.pgr.json", PgrProfile::PhiraV3, "120"),
        2,
        4,
    );
}

#[test]
fn rpe_minimal_fixture_runs_the_full_product_pipeline() {
    exercise("rpe-minimal", &import_rpe("rpe-minimal.rpe.json"), 2, 2);
}

#[test]
fn rpe_extreme_fixture_runs_the_full_product_pipeline() {
    exercise("rpe-extreme", &import_rpe("rpe-extreme.rpe.json"), 2, 5);
}

#[test]
fn pec_minimal_fixture_runs_the_full_product_pipeline() {
    exercise("pec-minimal", &import_pec("pec-minimal.pec", "100"), 1, 2);
}

#[test]
fn pec_feature_fixture_runs_the_full_product_pipeline() {
    exercise("pec-feature", &import_pec("pec-feature.pec", "100"), 1, 5);
}
