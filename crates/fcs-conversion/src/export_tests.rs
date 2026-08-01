use super::*;
use crate::{CapabilityLimit, RpeSpeedMode, SelectionDirection, load_profile_registry};
use fcs_model::{
    CanonicalChart, CanonicalMetadata, CanonicalNote, CanonicalNotePresentation, CanonicalNoteSet,
    CanonicalObject, CanonicalResourceBundle, CanonicalSourceVersion, CanonicalTime,
    CanonicalTrack, CanonicalTrackBlend, CanonicalTrackFill, CanonicalTrackInterpolation,
    CanonicalTrackPiece, CanonicalTrackSegment, CanonicalTrackTarget, CanonicalTrackValue,
    CanonicalValue, DistributionMetadata, InputContentHash, OriginState, ProvenanceGraph,
    RestrictedProvenanceFact,
};
use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pgr_chart(name: &str, profile: PgrProfile) -> CanonicalChart {
    let bytes = fs::read(root().join(format!(
        "docs/conformance/conversion/public-fixtures/sources/{name}"
    )))
    .unwrap();
    let artifact = SourceArtifact::new(name, ArtifactRole::Chart, bytes).unwrap();
    let parsed = parse_json_document(SourceFormat::Pgr, &artifact).unwrap();
    let source = parse_pgr_document(&parsed, PgrLimits::default()).unwrap();
    let floor = ExactDecimal::parse("120", DecimalLimits::default()).unwrap();
    let binding = PgrProfileBinding::new(profile, floor).unwrap();
    let semantic = interpret_pgr(&source, &binding).unwrap();
    lower_pgr_to_canonical(&semantic, &artifact)
        .unwrap()
        .compilation()
        .chart()
        .clone()
}

fn with_first_note_position(chart: &CanonicalChart, position_x: f64) -> CanonicalChart {
    let notes = chart
        .notes()
        .notes()
        .iter()
        .enumerate()
        .map(|(index, note)| {
            let presentation = note.presentation();
            let presentation = CanonicalNotePresentation::new(
                if index == 0 {
                    position_x
                } else {
                    presentation.position_x()
                },
                presentation.scroll_factor(),
                presentation.x_offset(),
                presentation.y_offset(),
                presentation.alpha(),
                presentation.scale_x(),
                presentation.scale_y(),
                presentation.rotation(),
                presentation.color(),
                presentation.texture().map(str::to_owned),
                presentation.render_enabled(),
                presentation.visible_from(),
                presentation.visible_until(),
            )
            .unwrap();
            CanonicalNote::new(
                note.id().clone(),
                note.kind(),
                note.document_order(),
                note.gameplay().clone(),
                presentation,
            )
            .unwrap()
        })
        .collect();
    let notes = CanonicalNoteSet::new(notes).unwrap();
    let mut changed = CanonicalChart::new(
        chart.source_version().clone(),
        chart.profile(),
        chart.features().iter().copied(),
        chart.time_map().clone(),
        chart.metadata().clone(),
        chart.lines().clone(),
        notes,
        chart.tracks().clone(),
        chart.scroll().clone(),
        chart.required_extensions().iter().cloned(),
    );
    if let Some(descriptors) = chart.descriptors() {
        changed = changed.with_descriptors(descriptors.clone());
    }
    changed
}

fn rpe_chart_from_fixture(name: &str, binding: &RpeProfileBinding) -> CanonicalChart {
    let bytes = fs::read(root().join(format!(
        "docs/conformance/conversion/public-fixtures/sources/{name}"
    )))
    .unwrap();
    let artifact = SourceArtifact::new(name, ArtifactRole::Chart, bytes).unwrap();
    let parsed = parse_json_document(SourceFormat::Rpe, &artifact).unwrap();
    let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
    let semantic = interpret_rpe_semantics(&source, binding).unwrap();
    lower_rpe_to_canonical(&semantic, &artifact)
        .unwrap()
        .compilation()
        .chart()
        .clone()
}

fn rpe_chart() -> CanonicalChart {
    rpe_chart_from_fixture(
        "rpe-minimal.rpe.json",
        &RpeProfileBinding::phira_legacy_speed(),
    )
}

fn rpe_parent_chart_without_tracks() -> CanonicalChart {
    let bytes = br#"{
            "META": {"RPEVersion": 150, "offset": 0},
            "BPMList": [{"startTime": [0, 0, 1], "bpm": 120}],
            "judgeLineList": [
                {"eventLayers": [null], "notes": [], "father": -1},
                {"eventLayers": [null], "notes": [], "father": 0, "rotateWithFather": true}
            ]
        }"#;
    let artifact = SourceArtifact::new("parent-only.rpe.json", ArtifactRole::Chart, bytes).unwrap();
    let parsed = parse_json_document(SourceFormat::Rpe, &artifact).unwrap();
    let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
    let semantic =
        interpret_rpe_semantics(&source, &RpeProfileBinding::phira_legacy_speed()).unwrap();
    lower_rpe_to_canonical(&semantic, &artifact)
        .unwrap()
        .compilation()
        .chart()
        .clone()
}

fn rpe_extreme_chart() -> CanonicalChart {
    rpe_chart_from_fixture(
        "rpe-extreme.rpe.json",
        &RpeProfileBinding::phira_legacy_speed(),
    )
}

fn reparse_rpe_chart(bytes: Vec<u8>, binding: &RpeProfileBinding) -> CanonicalChart {
    let artifact = SourceArtifact::new("mutated.rpe.json", ArtifactRole::Chart, bytes).unwrap();
    let parsed = parse_json_document(SourceFormat::Rpe, &artifact).unwrap();
    let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
    let semantic = interpret_rpe_semantics(&source, binding).unwrap();
    lower_rpe_to_canonical(&semantic, &artifact)
        .unwrap()
        .compilation()
        .chart()
        .clone()
}

fn pec_chart() -> CanonicalChart {
    pec_chart_from_fixture("pec-minimal.pec")
}

fn pec_chart_from_fixture(name: &str) -> CanonicalChart {
    let bytes = fs::read(root().join(format!(
        "docs/conformance/conversion/public-fixtures/sources/{name}"
    )))
    .unwrap();
    let artifact = SourceArtifact::new(name, ArtifactRole::Chart, bytes).unwrap();
    let source = parse_pec_document(&artifact, PecLimits::default()).unwrap();
    let floor = ExactDecimal::parse("120", DecimalLimits::default()).unwrap();
    let binding = PecProfileBinding::new(PecProfile::Phira, floor).unwrap();
    let semantic = interpret_pec(&source, &binding).unwrap();
    lower_pec_to_canonical(&semantic, &artifact)
        .unwrap()
        .compilation()
        .chart()
        .clone()
}

fn reparse_pec_chart(bytes: Vec<u8>) -> CanonicalChart {
    let artifact = SourceArtifact::new("mutated.pec", ArtifactRole::Chart, bytes).unwrap();
    let source = parse_pec_document(&artifact, PecLimits::default()).unwrap();
    let floor = ExactDecimal::parse("120", DecimalLimits::default()).unwrap();
    let binding = PecProfileBinding::new(PecProfile::Phira, floor).unwrap();
    let semantic = interpret_pec(&source, &binding).unwrap();
    lower_pec_to_canonical(&semantic, &artifact)
        .unwrap()
        .compilation()
        .chart()
        .clone()
}

fn profile_options(set: CapabilitySet, id: &str, version: &str) -> ExportOptions {
    ExportOptions::semantic(set.descriptor(Some(profile_reference(id, version))))
}

fn filler_report_entries(count: usize) -> Vec<ConversionEntry> {
    (0..count)
        .map(|index| {
            ConversionEntry::new(
                format!("filler/{index:04}"),
                "conversion.tool-rewrite",
                ConversionDomain::Profile,
                ConversionSeverity::Info,
                SemanticStatus::Equivalent,
                ConversionPhase::Export,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                "bounded test entry",
                [],
            )
            .unwrap()
        })
        .collect()
}

fn loss_descriptor(profile: &str, approximation: bool, drop: bool) -> CapabilityDescriptor {
    let base = CapabilitySet::pec_line().descriptor(Some(profile.into()));
    let motion_features = CapabilitySet::pgr_v3()
        .descriptor(None)
        .domain(ConversionDomain::Motion)
        .unwrap()
        .features()
        .to_vec();
    CapabilityDescriptor::new(
        base.format(),
        base.version(),
        base.profile().map(str::to_owned),
        ConversionDomain::ALL
            .map(|domain| {
                if domain == ConversionDomain::Motion {
                    CapabilityDomainDescriptor::new(
                        domain,
                        false,
                        false,
                        approximation,
                        false,
                        drop,
                        None,
                        None,
                    )
                    .with_features(motion_features.clone())
                    .unwrap()
                } else {
                    base.domain(domain).unwrap().clone()
                }
            })
            .into(),
    )
    .unwrap()
}

fn preserve_timing_descriptor() -> CapabilityDescriptor {
    let base = CapabilitySet::pec_line().descriptor(Some("pec.phira@1.0.0".into()));
    CapabilityDescriptor::new(
        base.format(),
        base.version(),
        base.profile().map(str::to_owned),
        base.domains()
            .iter()
            .map(|domain| {
                if domain.domain() == ConversionDomain::Timing {
                    CapabilityDomainDescriptor::new(
                        domain.domain(),
                        false,
                        false,
                        false,
                        true,
                        false,
                        domain.max_entities(),
                        domain.max_bytes(),
                    )
                    .with_features(domain.features().to_vec())
                    .and_then(|descriptor| descriptor.with_limits(domain.limits().iter().cloned()))
                    .unwrap()
                } else {
                    domain.clone()
                }
            })
            .collect(),
    )
    .unwrap()
}

fn preserve_metadata_descriptor() -> CapabilityDescriptor {
    let base = CapabilitySet::pec_line().descriptor(Some("pec.phira@1.0.0".into()));
    CapabilityDescriptor::new(
        base.format(),
        base.version(),
        base.profile().map(str::to_owned),
        base.domains()
            .iter()
            .map(|domain| {
                if domain.domain() == ConversionDomain::Metadata {
                    CapabilityDomainDescriptor::new(
                        domain.domain(),
                        false,
                        false,
                        false,
                        true,
                        false,
                        domain.max_entities(),
                        domain.max_bytes(),
                    )
                    .with_features(domain.features().to_vec())
                    .and_then(|descriptor| descriptor.with_limits(domain.limits().iter().cloned()))
                    .unwrap()
                } else {
                    domain.clone()
                }
            })
            .collect(),
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn descriptor_with_domain(
    set: CapabilitySet,
    profile: &str,
    target: ConversionDomain,
    exact: bool,
    approximation: bool,
    drop: bool,
    max_entities: Option<usize>,
    max_bytes: Option<usize>,
) -> CapabilityDescriptor {
    let base = set.descriptor(Some(profile.into()));
    let domains = base
        .domains()
        .iter()
        .map(|descriptor| {
            if descriptor.domain() == target {
                let features = descriptor.features().to_vec();
                let limits = descriptor.limits().to_vec();
                CapabilityDomainDescriptor::new(
                    target,
                    exact,
                    false,
                    approximation,
                    false,
                    drop,
                    max_entities,
                    max_bytes,
                )
                .with_features(features)
                .and_then(|descriptor| descriptor.with_limits(limits))
                .unwrap()
            } else {
                descriptor.clone()
            }
        })
        .collect();
    CapabilityDescriptor::new(
        base.format(),
        base.version(),
        base.profile().map(str::to_owned),
        domains,
    )
    .unwrap()
}

fn rpe_motion_scroll_drop_descriptor(profile: &str) -> CapabilityDescriptor {
    let base = CapabilitySet::rpe_json().descriptor(Some(profile.into()));
    let domains = base
        .domains()
        .iter()
        .map(|descriptor| {
            if matches!(
                descriptor.domain(),
                ConversionDomain::Motion | ConversionDomain::Scroll
            ) {
                CapabilityDomainDescriptor::new(
                    descriptor.domain(),
                    false,
                    false,
                    false,
                    false,
                    true,
                    descriptor.max_entities(),
                    descriptor.max_bytes(),
                )
                .with_features(if descriptor.domain() == ConversionDomain::Scroll {
                    Vec::new()
                } else {
                    descriptor.features().to_vec()
                })
                .unwrap()
                .with_limits(descriptor.limits().iter().cloned())
                .unwrap()
            } else {
                descriptor.clone()
            }
        })
        .collect();
    CapabilityDescriptor::new(
        base.format(),
        base.version(),
        base.profile().map(str::to_owned),
        domains,
    )
    .unwrap()
}

fn with_source_version(chart: &CanonicalChart, version: &str) -> CanonicalChart {
    let mut changed = CanonicalChart::new(
        CanonicalSourceVersion::new(version).unwrap(),
        chart.profile(),
        chart.features().iter().copied(),
        chart.time_map().clone(),
        chart.metadata().clone(),
        chart.lines().clone(),
        chart.notes().clone(),
        chart.tracks().clone(),
        chart.scroll().clone(),
        chart.required_extensions().iter().cloned(),
    );
    if let Some(descriptors) = chart.descriptors() {
        changed = changed.with_descriptors(descriptors.clone());
    }
    changed
}

fn with_metadata_fact(chart: &CanonicalChart) -> CanonicalChart {
    let mut meta = BTreeMap::new();
    meta.insert(
        "title".into(),
        CanonicalValue::String("dropped title".into()),
    );
    let metadata = CanonicalMetadata::new(
        Some(meta),
        chart.metadata().contributors().clone(),
        chart.metadata().credits().to_vec(),
        chart.metadata().resources().clone(),
        chart.metadata().artwork().cloned(),
        chart.metadata().sync().cloned(),
    );
    let mut changed = CanonicalChart::new(
        chart.source_version().clone(),
        chart.profile(),
        chart.features().iter().copied(),
        chart.time_map().clone(),
        metadata,
        chart.lines().clone(),
        chart.notes().clone(),
        chart.tracks().clone(),
        chart.scroll().clone(),
        chart.required_extensions().iter().cloned(),
    );
    if let Some(descriptors) = chart.descriptors() {
        changed = changed.with_descriptors(descriptors.clone());
    }
    changed
}

fn compilation_with_stale_roundtrip_fact(chart: &CanonicalChart) -> CanonicalCompilation {
    let root = RestrictedProvenanceFact::new(
        "canonical-edit",
        None,
        None,
        Some("old canonical value".into()),
        Some(0),
        None,
        OriginState::Imported,
        Some(SemanticStatus::Mapped),
        std::iter::empty(),
    )
    .unwrap();
    let dependent = RestrictedProvenanceFact::new(
        "source-roundtrip-handle",
        None,
        None,
        Some("stale source representation".into()),
        Some(1),
        None,
        OriginState::Imported,
        Some(SemanticStatus::Preserved),
        ["canonical-edit".into()],
    )
    .unwrap();
    let mut provenance = ProvenanceGraph::new([root, dependent]).unwrap();
    let stale = provenance
        .mark_user_modified_and_stale_dependents("canonical-edit")
        .unwrap();
    assert_eq!(
        stale.into_iter().collect::<Vec<_>>(),
        vec!["source-roundtrip-handle".to_owned()]
    );
    let distribution = DistributionMetadata::new(
        provenance,
        Vec::new(),
        Vec::new(),
        CanonicalObject::new(Vec::new()).unwrap(),
    )
    .unwrap();
    CanonicalCompilation::new(
        chart.clone(),
        CanonicalResourceBundle::new(Vec::new()).unwrap(),
        distribution,
    )
}

#[test]
fn formatter_applies_one_idempotent_text_policy() {
    let source =
        fs::read_to_string(root().join("docs/conformance/fcs5/source/valid/minimal-chart.fcs"))
            .unwrap();
    let noisy = source.replace('\n', "  \r\n");
    let formatted = format_fcs_source(&noisy).unwrap();
    assert_eq!(formatted, format_fcs_source(&formatted).unwrap());
    assert!(!formatted.contains('\r'));
    assert!(formatted.ends_with('\n'));
    assert!(
        !formatted
            .lines()
            .any(|line| line.ends_with(' ') || line.ends_with('\t'))
    );
    assert!(formatted.contains("format {\n    profile: chart;\n}"));
}

#[test]
fn formatter_preserves_comments_and_string_payloads() {
    let source = "#fcs 5.0.0\nformat{profile:fragment;}meta{custom:{\"text\":\"1e2\"};}//keep\n";
    let formatted = format_fcs_source(source).unwrap();
    assert!(formatted.contains("} //keep"));
    assert!(formatted.contains("\"text\": \"1e2\""));
    assert_eq!(formatted, format_fcs_source(&formatted).unwrap());
}

#[test]
fn formatter_applies_deterministic_numeric_policy() {
    let source = "#fcs 5.0.0\nformat { profile: fragment; }\n\
meta { custom: { \"float\": 1e2, \"time\": 1ms, \"length\": 2.0px, \
\"angle\": 0.5rad, \"beat\": 1.25beat }; }";
    let formatted = format_fcs_source(source).unwrap();
    assert!(formatted.contains("100.0"));
    assert!(formatted.contains("0.001s"));
    assert!(formatted.contains("2px"));
    assert!(formatted.contains("0.5rad"));
    assert!(formatted.contains("1.25beat"));
    assert_eq!(formatted, format_fcs_source(&formatted).unwrap());
}

#[test]
fn formatter_numeric_rewrite_preserves_canonical_chart_semantics() {
    let source =
        fs::read_to_string(root().join("docs/conformance/fcs5/source/valid/minimal-chart.fcs"))
            .unwrap()
            .replace("120bpm", "120.00e0bpm");
    let formatted = format_fcs_source(&source).unwrap();
    let canonical = |input: &str| {
        fcs_source::parser::parse_document(input)
            .into_result()
            .unwrap()
            .canonical_chart(fcs_source::elaborator::CompileTimeLimits::default())
            .unwrap()
    };
    assert_eq!(canonical(&source), canonical(&formatted));
    assert!(formatted.contains("120bpm"));
}

#[test]
fn format_fcs_source_rejects_invalid() {
    let error = format_fcs_source("not a chart").unwrap_err();
    assert_eq!(error.category(), "source.invalid");
}

#[test]
fn format_fcs_source_rejects_non_finite_numeric_literals() {
    let error = format_fcs_source(
        "#fcs 5.0.0\nformat { profile: fragment; }\nmeta { custom: { \"x\": 1e999 }; }",
    )
    .unwrap_err();
    assert_eq!(error.category(), "source.invalid");
}

#[test]
fn public_pgr_feature_fixture_roundtrips_through_export() {
    let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
    let options = profile_options(
        CapabilitySet::pgr_v3(),
        PgrProfile::PhiraV3.id(),
        PgrProfile::PhiraV3.version(),
    );
    let outcome = export_pgr_v3_with_options(&chart, &options).unwrap();
    assert!(outcome.comparison().is_equivalent());
    assert_eq!(outcome.report().status(), ConversionStatus::Equivalent);
    assert!(outcome.report().output_hash().is_some());
}

#[test]
fn public_pgr_roundtrip_helper_returns_full_canonical_comparison() {
    let bytes = fs::read(
        root().join("docs/conformance/conversion/public-fixtures/sources/pgr-feature.pgr.json"),
    )
    .unwrap();
    let comparison = roundtrip_pgr_v3_public_bytes(&bytes).unwrap();
    assert!(comparison.is_equivalent());
    assert!(comparison.mismatches().is_empty());
}

#[test]
fn pgr_v1_uses_the_selected_packed_coordinate_profile() {
    let chart = pgr_chart("pgr-minimal.pgr.json", PgrProfile::PhiraV1);
    let options = profile_options(
        CapabilitySet::pgr_v1(),
        PgrProfile::PhiraV1.id(),
        PgrProfile::PhiraV1.version(),
    );
    assert!(
        export_pgr_with_options(&chart, &options)
            .unwrap()
            .comparison()
            .is_equivalent()
    );
}

#[test]
fn rpe_and_pec_export_reparse_compare() {
    let rpe = rpe_chart();
    let rpe_options = profile_options(
        CapabilitySet::rpe_json(),
        RpeProfile::PhiraLegacySpeed.id(),
        RpeProfile::PhiraLegacySpeed.version(),
    );
    let outcome = export_rpe_json_with_options(&rpe, &rpe_options).unwrap();
    assert!(outcome.comparison().is_equivalent());
    let target: Value = serde_json::from_slice(outcome.bytes()).unwrap();
    assert!(
        target["judgeLineList"][0]["eventLayers"][0]["speedEvents"]
            .as_array()
            .is_some_and(|events| !events.is_empty())
    );

    let pec = pec_chart();
    let pec_options = profile_options(
        CapabilitySet::pec_line(),
        PecProfile::Phira.id(),
        PecProfile::Phira.version(),
    );
    assert!(
        export_pec_line_with_options(&pec, &pec_options)
            .unwrap()
            .comparison()
            .is_equivalent()
    );
}

#[test]
fn pec_feature_motion_roundtrip_and_mutation_are_observable() {
    let chart = pec_chart_from_fixture("pec-feature.pec");
    let options = profile_options(
        CapabilitySet::pec_line(),
        PecProfile::Phira.id(),
        PecProfile::Phira.version(),
    );
    let outcome = export_pec_line_with_options(&chart, &options).unwrap();
    assert!(outcome.comparison().is_equivalent());
    let target = String::from_utf8(outcome.bytes().to_vec()).unwrap();
    assert!(target.contains("cp 0"));
    assert!(target.contains("cd 0"));
    let mutated = target.replacen("1024", "1100", 1).into_bytes();
    let actual = reparse_pec_chart(mutated);
    let comparison = crate::comparison::compare_canonical_charts(&chart, &actual);
    assert!(!comparison.is_equivalent());
    assert!(
        comparison
            .mismatches()
            .iter()
            .any(|mismatch| mismatch.domain() == "motion")
    );
}

#[test]
fn rpe_extreme_roundtrip_covers_sparse_motion_and_scroll_layers() {
    let chart = rpe_extreme_chart();
    let profile = RpeProfile::PhiraLegacySpeed;
    let options = profile_options(CapabilitySet::rpe_json(), profile.id(), profile.version());
    let outcome = export_rpe_json_with_options(&chart, &options).unwrap();
    assert!(outcome.comparison().is_equivalent());
    assert!(outcome.comparison().mismatches().is_empty());
    assert_eq!(chart.tracks().tracks().len(), 7);

    let target: Value = serde_json::from_slice(outcome.bytes()).unwrap();
    let layers = target["judgeLineList"][0]["eventLayers"]
        .as_array()
        .expect("RPE eventLayers array");
    assert_eq!(layers.len(), 3);
    assert!(layers[1].is_null());
    assert!(
        layers[0]["moveXEvents"]
            .as_array()
            .is_some_and(|events| { !events.is_empty() })
    );
    assert!(
        layers[2]["moveYEvents"]
            .as_array()
            .is_some_and(|events| { !events.is_empty() })
    );
    assert!(
        layers[2]["speedEvents"]
            .as_array()
            .is_some_and(|events| { !events.is_empty() })
    );
}

#[test]
fn rpe_extreme_motion_mutation_is_not_equivalent() {
    let expected = rpe_extreme_chart();
    let profile = RpeProfile::PhiraLegacySpeed;
    let options = profile_options(CapabilitySet::rpe_json(), profile.id(), profile.version());
    let outcome = export_rpe_json_with_options(&expected, &options).unwrap();
    let mut target: Value = serde_json::from_slice(outcome.bytes()).unwrap();
    target["judgeLineList"][0]["eventLayers"][0]["moveXEvents"][0]["end"] = json!(676);
    let actual = reparse_rpe_chart(
        serde_json::to_vec(&target).unwrap(),
        &RpeProfileBinding::phira_legacy_speed(),
    );
    let comparison = crate::comparison::compare_canonical_charts(&expected, &actual);
    assert!(!comparison.is_equivalent());
    assert!(
        comparison
            .mismatches()
            .iter()
            .any(|mismatch| mismatch.domain() == "motion")
    );
}

#[test]
fn rpe_extreme_speed_mutation_changes_cumulative_scroll_distance() {
    let expected = rpe_extreme_chart();
    let profile = RpeProfile::PhiraLegacySpeed;
    let options = profile_options(CapabilitySet::rpe_json(), profile.id(), profile.version());
    let outcome = export_rpe_json_with_options(&expected, &options).unwrap();
    let mut target: Value = serde_json::from_slice(outcome.bytes()).unwrap();
    target["judgeLineList"][0]["eventLayers"][0]["speedEvents"][0]["end"] = json!(4.25);
    let actual = reparse_rpe_chart(
        serde_json::to_vec(&target).unwrap(),
        &RpeProfileBinding::phira_legacy_speed(),
    );
    let comparison = crate::comparison::compare_canonical_charts(&expected, &actual);
    assert!(!comparison.is_equivalent());
    assert!(
        comparison
            .mismatches()
            .iter()
            .any(|mismatch| mismatch.metric() == "scroll.distance")
    );
}

#[test]
fn rpe_parameterized_target_profiles_reuse_the_typed_binding_for_reparse() {
    let chart = rpe_chart();
    let cases = [
        (
            RpeProfileBinding::community_divide(RpeSpeedMode::LegacyDerivative),
            150,
        ),
        (
            RpeProfileBinding::docs_example_multiply(RpeSpeedMode::ModernEased),
            150,
        ),
        (
            RpeProfileBinding::phira_rpe170_speed(Some(RpeVersionEra::AtLeast170)),
            170,
        ),
    ];
    for (binding, expected_rpe_version) in cases {
        let profile = binding.profile();
        let descriptor = CapabilitySet::rpe_json()
            .descriptor(Some(profile_reference(profile.id(), profile.version())));
        let options = ExportOptions::semantic(descriptor).with_rpe_profile_binding(binding);
        let outcome = export_rpe_json_with_options(&chart, &options).unwrap();
        assert!(outcome.comparison().is_equivalent());
        let target: Value = serde_json::from_slice(outcome.bytes()).unwrap();
        assert_eq!(target["META"]["RPEVersion"], expected_rpe_version);
        assert!(
            target["judgeLineList"][0]["eventLayers"][0]["speedEvents"]
                .as_array()
                .is_some_and(|events| !events.is_empty())
        );
    }
}

#[test]
fn every_registry_target_profile_has_export_reparse_evidence() {
    let registry =
        load_profile_registry(root().join("docs/conformance/conversion/profile-registry.toml"))
            .unwrap();
    let expected = registry
        .profiles()
        .iter()
        .filter(|profile| {
            profile.strict_eligible() && profile.supports_direction(SelectionDirection::Target)
        })
        .map(|profile| profile.as_ref_key())
        .collect::<BTreeSet<_>>();
    let mut covered = BTreeSet::new();

    for (profile, fixture, capabilities, expected_version) in [
        (
            PgrProfile::PhiraV1,
            "pgr-minimal.pgr.json",
            CapabilitySet::pgr_v1(),
            1,
        ),
        (
            PgrProfile::PhiraV3,
            "pgr-feature.pgr.json",
            CapabilitySet::pgr_v3(),
            3,
        ),
    ] {
        let chart = pgr_chart(fixture, profile);
        let options = profile_options(capabilities, profile.id(), profile.version());
        let outcome = export_pgr_with_options(&chart, &options).unwrap();
        assert!(outcome.comparison().is_equivalent());
        let target: Value = serde_json::from_slice(outcome.bytes()).unwrap();
        assert_eq!(target["formatVersion"], expected_version);
        covered.insert(profile_reference(profile.id(), profile.version()));
    }

    let chart = rpe_chart();
    for (binding, expected_version) in [
        (
            RpeProfileBinding::community_divide(RpeSpeedMode::LegacyDerivative),
            150,
        ),
        (
            RpeProfileBinding::docs_example_multiply(RpeSpeedMode::ModernEased),
            150,
        ),
        (RpeProfileBinding::phira_legacy_speed(), 150),
        (
            RpeProfileBinding::phira_rpe170_speed(Some(RpeVersionEra::AtLeast170)),
            170,
        ),
    ] {
        let profile = binding.profile();
        let options = ExportOptions::semantic(
            CapabilitySet::rpe_json()
                .descriptor(Some(profile_reference(profile.id(), profile.version()))),
        )
        .with_rpe_profile_binding(binding);
        let outcome = export_rpe_json_with_options(&chart, &options).unwrap();
        assert!(outcome.comparison().is_equivalent());
        let target: Value = serde_json::from_slice(outcome.bytes()).unwrap();
        assert_eq!(target["META"]["RPEVersion"], expected_version);
        covered.insert(profile_reference(profile.id(), profile.version()));
    }

    let profile = PecProfile::Phira;
    let options = profile_options(CapabilitySet::pec_line(), profile.id(), profile.version());
    let outcome = export_pec_line_with_options(&pec_chart(), &options).unwrap();
    assert!(outcome.comparison().is_equivalent());
    covered.insert(profile_reference(profile.id(), profile.version()));

    assert_eq!(covered, expected);
}

#[test]
fn rpe_export_rejects_unsupported_speed_track_shapes() {
    let chart = rpe_chart();
    let owner = chart.lines().lines().next().unwrap().id().clone();
    let start = CanonicalTime::from_chart_time_seconds(0.0).unwrap();
    let end = CanonicalTime::from_chart_time_seconds(1.0).unwrap();
    let segment = CanonicalTrackSegment::new(
        start,
        end,
        CanonicalTrackValue::Float(1.0),
        CanonicalTrackValue::Float(2.0),
        CanonicalTrackInterpolation::Step,
        0,
    )
    .unwrap();
    let track = CanonicalTrack::new(
        owner,
        "rpe.layer.0.speed",
        CanonicalTrackTarget::ScrollSpeed,
        CanonicalTrackBlend::Add,
        0,
        CanonicalTrackFill::Zero,
        CanonicalTrackFill::Zero,
        CanonicalTrackFill::Zero,
        vec![CanonicalTrackPiece::Segment(segment)],
    )
    .unwrap();
    let error = rpe_track_is_supported(
        &track,
        rpe_track_spec(&track).unwrap(),
        &RpeProfileBinding::phira_legacy_speed(),
    )
    .unwrap_err();
    assert_eq!(error.category(), "conversion.capability-mismatch");
}

#[test]
fn rpe_target_binding_profile_identity_must_match_the_selected_profile() {
    let selected = RpeProfile::CommunityDivideBpmfactor;
    let descriptor = CapabilitySet::rpe_json()
        .descriptor(Some(profile_reference(selected.id(), selected.version())));
    let options = ExportOptions::semantic(descriptor)
        .with_rpe_profile_binding(RpeProfileBinding::docs_example_multiply(
            RpeSpeedMode::LegacyLinear,
        ))
        .with_target_profile(profile_reference(selected.id(), selected.version()));
    let error = export_rpe_json_with_options(&rpe_chart(), &options).unwrap_err();
    assert_eq!(error.category(), "conversion.profile-parameter-invalid");
    assert!(error.message().contains("does not match selected profile"));
}

#[test]
fn strict_profile_choice_is_not_repair() {
    let chart = pec_chart();
    let profile = profile_reference(PecProfile::Phira.id(), PecProfile::Phira.version());
    let descriptor = CapabilitySet::pec_line().descriptor(Some(profile.clone()));
    let options = ExportOptions::strict(descriptor)
        .with_repair_mode(RepairMode::new(true, std::iter::empty()));
    let error = negotiate_export_with_options(&chart, &options).unwrap_err();
    assert_eq!(error.category(), "conversion.target-profile-required");

    let options =
        ExportOptions::strict(CapabilitySet::pec_line().descriptor(None)).with_target_profile("  ");
    let error = negotiate_export_with_options(&chart, &options).unwrap_err();
    assert_eq!(error.category(), "conversion.target-profile-required");

    let options = ExportOptions::strict(CapabilitySet::pec_line().descriptor(Some(profile)))
        .with_target_profile(profile_reference(
            PecProfile::Phira.id(),
            PecProfile::Phira.version(),
        ));
    let outcome = export_pec_line_with_options(&chart, &options).unwrap();
    assert!(outcome.comparison().is_equivalent());
    assert_eq!(outcome.report().status(), ConversionStatus::Equivalent);

    let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
    let profile = profile_reference(PecProfile::Phira.id(), PecProfile::Phira.version());
    let authorization = DropAuthorization::new(
        ["motion.track".into()],
        "explicit strict-loss test authorization",
    )
    .unwrap();
    let options = ExportOptions::strict(loss_descriptor(&profile, false, true))
        .with_target_profile(profile)
        .with_drop(authorization);
    let error = negotiate_export_with_options(&chart, &options).unwrap_err();
    assert_eq!(error.category(), "conversion.capability-mismatch");
    assert!(error.message().contains("strict export cannot preserve"));
    assert!(error.entries().iter().any(|entry| {
        entry.id() == "capability/motion" && entry.semantic_status() == SemanticStatus::Dropped
    }));
}

#[test]
fn preserve_negotiation_fails_without_a_fidelity_or_sidecar_sink() {
    let options = ExportOptions::semantic(preserve_timing_descriptor());

    let error = export_pec_line_with_options(&pec_chart(), &options).unwrap_err();

    assert_eq!(error.category(), "conversion.capability-mismatch");
    assert!(
        error
            .message()
            .contains("structured Fidelity or external sidecar sink")
    );
    assert!(error.entries().iter().any(|entry| {
        entry.id() == "capability/timing" && entry.semantic_status() == SemanticStatus::Preserved
    }));
}

#[test]
fn compilation_export_preserves_into_source_free_fidelity_metadata() {
    let chart = with_metadata_fact(&pec_chart());
    let distribution = DistributionMetadata::new(
        ProvenanceGraph::empty(),
        Vec::new(),
        vec![InputContentHash::sha256_lower_hex("a".repeat(64), None).unwrap()],
        CanonicalObject::new(Vec::new()).unwrap(),
    )
    .unwrap();
    let compilation = CanonicalCompilation::new(
        chart.clone(),
        CanonicalResourceBundle::new(Vec::new()).unwrap(),
        distribution,
    );
    let outcome = export_pec_compilation_with_options(
        &compilation,
        &ExportOptions::semantic(preserve_metadata_descriptor()),
    )
    .unwrap();

    assert_eq!(outcome.report().status(), ConversionStatus::PreservedOnly);
    assert!(outcome.report().entries().iter().any(|entry| {
        entry.id() == "capability/metadata" && entry.semantic_status() == SemanticStatus::Preserved
    }));
    assert!(
        !String::from_utf8(outcome.bytes().to_vec())
            .unwrap()
            .contains("semanticLosses")
    );
    assert!(compilation.distribution().semantic_losses().is_empty());
    let losses = outcome.distribution().unwrap().semantic_losses();
    assert_eq!(losses.len(), 1);
    assert_eq!(losses[0].domain(), ConversionDomain::Metadata);
    assert_eq!(losses[0].status(), SemanticStatus::Preserved);
    assert_eq!(losses[0].category(), SemanticLoss::CAPABILITY_NEGOTIATED);
    assert_eq!(losses[0].entity_id(), None);

    let fidelity_compilation = CanonicalCompilation::new(
        chart.clone(),
        CanonicalResourceBundle::new(Vec::new()).unwrap(),
        outcome.distribution().unwrap().clone(),
    );
    let fidelity = fcs_fcbc::write_from_compilation_with_profile(
        &fidelity_compilation,
        fcs_fcbc::ContainerProfile::Fidelity,
    )
    .unwrap();
    let stripped = fcs_fcbc::write_from_compilation_with_profile(
        &CanonicalCompilation::new(
            chart,
            CanonicalResourceBundle::new(Vec::new()).unwrap(),
            DistributionMetadata::empty(),
        ),
        fcs_fcbc::ContainerProfile::StrictRuntime,
    )
    .unwrap();
    let fidelity_container = fcs_fcbc::load_container(&fidelity).unwrap();
    let stripped_container = fcs_fcbc::load_container(&stripped).unwrap();
    assert_eq!(
        fidelity_container.header.profile,
        fcs_fcbc::ContainerProfile::Fidelity
    );
    assert!(fidelity_container.section_types().contains(&16));
    assert_eq!(
        stripped_container.header.profile,
        fcs_fcbc::ContainerProfile::StrictRuntime
    );
    assert!(!stripped_container.section_types().contains(&16));
    assert!(
        fidelity
            .windows(SemanticLoss::CAPABILITY_NEGOTIATED.len())
            .any(|window| window == SemanticLoss::CAPABILITY_NEGOTIATED.as_bytes())
    );
    let fidelity_chart = fcs_fcbc::load_chart(&fidelity).unwrap();
    let stripped_chart = fcs_fcbc::load_chart(&stripped).unwrap();
    assert_eq!(
        fidelity_chart.document_profile,
        stripped_chart.document_profile
    );
    assert_eq!(fidelity_chart.constants, stripped_chart.constants);
    assert_eq!(fidelity_chart.resources, stripped_chart.resources);
    assert_eq!(fidelity_chart.extensions, stripped_chart.extensions);
    assert_eq!(fidelity_chart.tempo_points, stripped_chart.tempo_points);
    assert_eq!(fidelity_chart.lines, stripped_chart.lines);
    assert_eq!(fidelity_chart.notes, stripped_chart.notes);
    assert_eq!(fidelity_chart.descriptors, stripped_chart.descriptors);
    assert_eq!(fidelity_chart.expressions, stripped_chart.expressions);
    assert_eq!(fidelity_chart.distances, stripped_chart.distances);

    let strict = ExportOptions::strict(preserve_metadata_descriptor())
        .with_target_profile("pec.phira@1.0.0");
    let error = export_pec_compilation_with_options(&compilation, &strict).unwrap_err();
    assert_eq!(error.category(), "conversion.capability-mismatch");
    assert!(error.message().contains("strict export cannot preserve"));
}

#[test]
fn compilation_export_omits_preserved_metadata_from_external_bytes() {
    let chart = with_metadata_fact(&pec_chart());
    let compilation = CanonicalCompilation::new(
        chart,
        CanonicalResourceBundle::new(Vec::new()).unwrap(),
        DistributionMetadata::empty(),
    );
    let outcome = export_pec_compilation_with_options(
        &compilation,
        &ExportOptions::semantic(preserve_metadata_descriptor()),
    )
    .unwrap();

    assert!(outcome.negotiation().preserves(ConversionDomain::Metadata));
    assert_eq!(outcome.report().status(), ConversionStatus::PreservedOnly);
    assert!(
        !String::from_utf8(outcome.bytes().to_vec())
            .unwrap()
            .contains("dropped title")
    );
    assert!(outcome.comparison().is_equivalent());
    assert!(
        outcome
            .distribution()
            .unwrap()
            .semantic_losses()
            .iter()
            .any(|loss| loss.domain() == ConversionDomain::Metadata)
    );
}

#[test]
fn negotiation_rejects_missing_feature_and_entity_limit_before_writing() {
    let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
    let profile = profile_reference(PgrProfile::PhiraV3.id(), PgrProfile::PhiraV3.version());
    let base = CapabilitySet::pgr_v3().descriptor(Some(profile.clone()));
    let domains = base
        .domains()
        .iter()
        .map(|descriptor| {
            let features = descriptor
                .features()
                .iter()
                .filter(|feature| {
                    !(descriptor.domain() == ConversionDomain::Gameplay
                        && feature.axis() == "note.kind"
                        && feature.value() == "hold")
                })
                .cloned()
                .collect::<Vec<_>>();
            CapabilityDomainDescriptor::new(
                descriptor.domain(),
                descriptor.exact(),
                descriptor.equivalent(),
                descriptor.approximation(),
                descriptor.preserve(),
                descriptor.drop(),
                descriptor.max_entities(),
                descriptor.max_bytes(),
            )
            .with_features(features)
            .unwrap()
            .with_limits(descriptor.limits().iter().cloned())
            .unwrap()
        })
        .collect();
    let descriptor = CapabilityDescriptor::new(
        base.format(),
        base.version(),
        base.profile().map(str::to_owned),
        domains,
    )
    .unwrap();
    let error = negotiate_export_with_options(
        &chart,
        &ExportOptions::semantic(descriptor).with_target_profile(profile),
    )
    .unwrap_err();
    assert_eq!(error.category(), "conversion.capability-mismatch");
    assert!(error.entries().iter().any(|entry| {
        entry.field_key() == Some("note.kind=hold") && entry.message().contains("note.kind=hold")
    }));

    let base = CapabilitySet::pgr_v3().descriptor(Some(profile_reference(
        PgrProfile::PhiraV3.id(),
        PgrProfile::PhiraV3.version(),
    )));
    let domains = base
        .domains()
        .iter()
        .map(|descriptor| {
            if descriptor.domain() != ConversionDomain::Gameplay {
                return descriptor.clone();
            }
            CapabilityDomainDescriptor::new(
                descriptor.domain(),
                descriptor.exact(),
                descriptor.equivalent(),
                descriptor.approximation(),
                descriptor.preserve(),
                descriptor.drop(),
                descriptor.max_entities(),
                descriptor.max_bytes(),
            )
            .with_features(descriptor.features().iter().cloned())
            .unwrap()
            .with_limits([crate::CapabilityLimit::new("entity.count", 0.0).unwrap()])
            .unwrap()
        })
        .collect();
    let descriptor = CapabilityDescriptor::new(
        base.format(),
        base.version(),
        base.profile().map(str::to_owned),
        domains,
    )
    .unwrap();
    let error = negotiate_export_with_options(
        &chart,
        &ExportOptions::semantic(descriptor).with_target_profile(profile_reference(
            PgrProfile::PhiraV3.id(),
            PgrProfile::PhiraV3.version(),
        )),
    )
    .unwrap_err();
    assert!(error.entries().iter().any(|entry| {
        entry.domain() == ConversionDomain::Gameplay
            && entry.field_key() == Some("entity.count")
            && entry.message().contains("entity.count")
    }));
}

#[test]
fn authorized_approximation_can_cover_a_missing_feature() {
    let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
    let profile = profile_reference(PgrProfile::PhiraV3.id(), PgrProfile::PhiraV3.version());
    let base = CapabilitySet::pgr_v3().descriptor(Some(profile));
    let domains = base
        .domains()
        .iter()
        .map(|descriptor| {
            if descriptor.domain() != ConversionDomain::Motion {
                return descriptor.clone();
            }
            CapabilityDomainDescriptor::new(
                descriptor.domain(),
                false,
                false,
                true,
                false,
                false,
                descriptor.max_entities(),
                descriptor.max_bytes(),
            )
            .with_features(
                descriptor
                    .features()
                    .iter()
                    .filter(|feature| {
                        !(feature.axis() == "track.interpolation" && feature.value() == "step")
                    })
                    .cloned(),
            )
            .unwrap()
            .with_limits(descriptor.limits().iter().cloned())
            .unwrap()
        })
        .collect();
    let descriptor = CapabilityDescriptor::new(
        base.format(),
        base.version(),
        base.profile().map(str::to_owned),
        domains,
    )
    .unwrap();
    let authorization = ApproximationAuthorization::new(
        ["motion".into()],
        [("motion.track_value".into(), 0.001)],
        1024,
        "linear-segment",
        "1.0.0",
    )
    .unwrap();

    let (plan, entries) = negotiate_export_with_options(
        &chart,
        &ExportOptions::semantic(descriptor).with_approximation(authorization),
    )
    .unwrap();

    assert_eq!(
        plan.action_for(ConversionDomain::Motion),
        Some(NegotiationAction::Bake)
    );
    assert!(entries.iter().any(|entry| {
        entry.category() == "conversion.capability-negotiated"
            && entry.domain() == ConversionDomain::Motion
    }));
}

#[test]
fn line_motion_is_negotiated_for_authorized_drop_without_tracks() {
    let chart = rpe_parent_chart_without_tracks();
    let profile = profile_reference(
        RpeProfile::PhiraLegacySpeed.id(),
        RpeProfile::PhiraLegacySpeed.version(),
    );
    let descriptor = rpe_motion_scroll_drop_descriptor(&profile);
    let authorization = DropAuthorization::new(
        ["motion.line".into(), "scroll.line".into()],
        "explicitly discard target-inexpressible line motion and scroll",
    )
    .unwrap();

    let (plan, _) = negotiate_export_with_options(
        &chart,
        &ExportOptions::semantic(descriptor).with_drop(authorization),
    )
    .unwrap();

    assert_eq!(
        plan.action_for(ConversionDomain::Motion),
        Some(NegotiationAction::Drop)
    );
}

#[test]
fn authorized_motion_drop_neutralizes_rpe_parent_and_inherit_before_write() {
    let chart = rpe_parent_chart_without_tracks();
    assert!(chart.lines().lines().any(|line| line.parent().is_some()));
    assert!(chart.lines().lines().any(|line| line.inherit().rotation()));
    let profile = profile_reference(
        RpeProfile::PhiraLegacySpeed.id(),
        RpeProfile::PhiraLegacySpeed.version(),
    );
    let descriptor = rpe_motion_scroll_drop_descriptor(&profile);
    let authorization = DropAuthorization::new(
        ["motion.line".into(), "scroll.line".into()],
        "explicitly discard target-inexpressible line motion and scroll",
    )
    .unwrap();
    let outcome = export_rpe_json_with_options(
        &chart,
        &ExportOptions::semantic(descriptor).with_drop(authorization),
    )
    .unwrap();

    assert!(outcome.negotiation().drops(ConversionDomain::Motion));
    assert!(outcome.negotiation().drops(ConversionDomain::Scroll));
    let target: Value = serde_json::from_slice(outcome.bytes()).unwrap();
    let lines = target["judgeLineList"].as_array().unwrap();
    assert!(
        lines.iter().all(|line| {
            line["father"] == json!(-1) && line["rotateWithFather"] == json!(false)
        })
    );
}

#[test]
fn typed_event_limit_is_enforced_during_capability_negotiation() {
    let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
    let profile = profile_reference(PgrProfile::PhiraV3.id(), PgrProfile::PhiraV3.version());
    let base = CapabilitySet::pgr_v3().descriptor(Some(profile.clone()));
    let domains = base
        .domains()
        .iter()
        .map(|descriptor| {
            if descriptor.domain() != ConversionDomain::Gameplay {
                return descriptor.clone();
            }
            CapabilityDomainDescriptor::new(
                descriptor.domain(),
                descriptor.exact(),
                descriptor.equivalent(),
                descriptor.approximation(),
                descriptor.preserve(),
                descriptor.drop(),
                descriptor.max_entities(),
                descriptor.max_bytes(),
            )
            .with_features(descriptor.features().iter().cloned())
            .unwrap()
            .with_limits([CapabilityLimit::new("event.count", 0.0).unwrap()])
            .unwrap()
        })
        .collect();
    let descriptor = CapabilityDescriptor::new(
        base.format(),
        base.version(),
        base.profile().map(str::to_owned),
        domains,
    )
    .unwrap();

    let error = negotiate_export_with_options(
        &chart,
        &ExportOptions::semantic(descriptor).with_target_profile(profile),
    )
    .unwrap_err();

    assert_eq!(error.category(), "conversion.capability-mismatch");
    assert!(error.entries().iter().any(|entry| {
        entry.field_key() == Some("event.count") && entry.message().contains("event.count")
    }));
}

#[test]
fn approximation_and_drop_need_independent_typed_authorization() {
    let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
    let profile = profile_reference(PecProfile::Phira.id(), PecProfile::Phira.version());

    let approximation = ExportOptions::semantic(loss_descriptor(&profile, true, false));
    let error = negotiate_export_with_options(&chart, &approximation).unwrap_err();
    assert_eq!(error.category(), "conversion.approximation-not-authorized");
    let authorization = ApproximationAuthorization::new(
        ["motion".into()],
        [("motion.track_value".into(), 0.001)],
        1024,
        "linear-segment",
        "1.0.0",
    )
    .unwrap();
    assert_eq!(authorization.target_domains(), ["motion"]);
    assert_eq!(authorization.error_budgets()["motion.track_value"], 0.001);
    let (plan, _) =
        negotiate_export_with_options(&chart, &approximation.with_approximation(authorization))
            .unwrap();
    assert_eq!(plan.action(), NegotiationAction::Bake);

    let drop = ExportOptions::semantic(loss_descriptor(&profile, false, true));
    let error = negotiate_export_with_options(&chart, &drop).unwrap_err();
    assert_eq!(error.category(), "conversion.drop-not-authorized");
    let authorization =
        DropAuthorization::new(["motion.track".into()], "explicit target loss").unwrap();
    assert_eq!(authorization.target_selectors(), ["motion.track"]);
    assert_eq!(authorization.reason(), "explicit target loss");
    let (plan, _) = negotiate_export_with_options(&chart, &drop.with_drop(authorization)).unwrap();
    assert_eq!(
        plan.action_for(ConversionDomain::Motion),
        Some(NegotiationAction::Drop)
    );
    assert_ne!(
        plan.action_for(ConversionDomain::Gameplay),
        Some(NegotiationAction::Drop)
    );
}

#[test]
fn approximation_segment_limit_is_a_hard_reparse_budget() {
    let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
    let profile = profile_reference(PgrProfile::PhiraV3.id(), PgrProfile::PhiraV3.version());
    let descriptor = descriptor_with_domain(
        CapabilitySet::pgr_v3(),
        &profile,
        ConversionDomain::Presentation,
        false,
        true,
        false,
        None,
        None,
    );
    let authorization = ApproximationAuthorization::new(
        ["presentation".into()],
        [("presentation.value".into(), 0.001)],
        1,
        "linear-segment",
        "1.0.0",
    )
    .unwrap();
    let error = export_pgr_v3_with_options(
        &chart,
        &ExportOptions::semantic(descriptor).with_approximation(authorization),
    )
    .unwrap_err();
    assert_eq!(error.category(), "conversion.approximation-budget-exceeded");
}

#[test]
fn successful_approximation_reports_verified_maximum_and_segment_count() {
    let chart = with_first_note_position(
        &pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3),
        0.1234,
    );
    let profile = profile_reference(PgrProfile::PhiraV3.id(), PgrProfile::PhiraV3.version());
    let descriptor = descriptor_with_domain(
        CapabilitySet::pgr_v3(),
        &profile,
        ConversionDomain::Presentation,
        false,
        true,
        false,
        None,
        None,
    );
    let authorization = ApproximationAuthorization::new(
        ["presentation".into()],
        [("presentation.value".into(), 0.001)],
        1024,
        "linear-segment",
        "1.0.0",
    )
    .unwrap();
    let exact = export_pgr_v3_with_options(
        &chart,
        &profile_options(
            CapabilitySet::pgr_v3(),
            PgrProfile::PhiraV3.id(),
            PgrProfile::PhiraV3.version(),
        ),
    )
    .unwrap();

    let outcome = export_pgr_v3_with_options(
        &chart,
        &ExportOptions::semantic(descriptor).with_approximation(authorization),
    )
    .unwrap();

    assert!(
        outcome
            .negotiation()
            .approximates(ConversionDomain::Presentation)
    );
    assert_ne!(outcome.bytes(), exact.bytes());
    assert_eq!(outcome.report().status(), ConversionStatus::Approximate);
    let verified = outcome
        .comparison()
        .verified_maximum_error("presentation.value")
        .unwrap();
    let verified_sample_count = chart.notes().notes().len() as u64 * 8;
    assert!(verified > 0.0 && verified <= 0.001);
    assert_eq!(
        outcome
            .comparison()
            .verified_sample_count("presentation.value"),
        Some(verified_sample_count)
    );
    let verification = outcome
        .report()
        .entries()
        .iter()
        .find(|entry| entry.id() == "approximation/verified/000000")
        .unwrap();
    assert_eq!(verification.category(), "conversion.approximation-verified");
    assert_eq!(verification.phase(), ConversionPhase::ReparseCompare);
    assert_eq!(verification.semantic_status(), SemanticStatus::Approximated);
    assert_eq!(verification.field_key(), Some("presentation.value"));
    assert_eq!(
        verification.source_value(),
        Some(&CanonicalValue::Float(0.001))
    );
    assert_eq!(
        verification.target_value(),
        Some(&CanonicalValue::Float(verified))
    );
    let metric = verification.error_metric().unwrap();
    assert_eq!(metric.domain(), ConversionDomain::Presentation);
    assert_eq!(metric.metric(), "presentation.value");
    assert_eq!(metric.declared_maximum(), 0.001);
    assert_eq!(metric.verified_maximum(), verified);
    assert_eq!(
        metric.verification_method(),
        "same-profile-canonical-reparse"
    );
    assert_eq!(metric.sample_count(), verified_sample_count);
    assert!(metric.segment_count() > 0);
    assert!(
        metric
            .forced_boundaries()
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    assert_eq!(metric.source_descriptor_hash().len(), 64);
    assert!(
        metric
            .source_descriptor_hash()
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    );
    assert!(verification.message().contains("target segments"));
    assert!(verification.message().contains("linear-segment@1.0.0"));
}

#[test]
fn approximation_error_over_budget_is_reclassified_after_reparse() {
    let chart = with_first_note_position(
        &pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3),
        0.1234,
    );
    let profile = profile_reference(PgrProfile::PhiraV3.id(), PgrProfile::PhiraV3.version());
    let descriptor = descriptor_with_domain(
        CapabilitySet::pgr_v3(),
        &profile,
        ConversionDomain::Presentation,
        false,
        true,
        false,
        None,
        None,
    );
    let authorization = ApproximationAuthorization::new(
        ["presentation".into()],
        [("presentation.value".into(), 0.0001)],
        1024,
        "linear-segment",
        "1.0.0",
    )
    .unwrap();

    let error = export_pgr_v3_with_options(
        &chart,
        &ExportOptions::semantic(descriptor).with_approximation(authorization),
    )
    .unwrap_err();

    assert_eq!(error.category(), "conversion.approximation-budget-exceeded");
    assert!(error.entries().iter().any(|entry| {
        entry.category() == "conversion.approximation-budget-exceeded"
            && entry.id().starts_with("roundtrip/")
    }));
}

#[test]
fn declared_approximation_metric_must_be_exercised_by_comparison() {
    let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
    let profile = profile_reference(PgrProfile::PhiraV3.id(), PgrProfile::PhiraV3.version());
    let descriptor = descriptor_with_domain(
        CapabilitySet::pgr_v3(),
        &profile,
        ConversionDomain::Presentation,
        false,
        true,
        false,
        None,
        None,
    );
    let authorization = ApproximationAuthorization::new(
        ["presentation".into()],
        [("presentation.unmeasured".into(), 0.001)],
        1024,
        "linear-segment",
        "1.0.0",
    )
    .unwrap();

    let error = export_pgr_v3_with_options(
        &chart,
        &ExportOptions::semantic(descriptor).with_approximation(authorization),
    )
    .unwrap_err();

    assert_eq!(error.category(), "conversion.approximation-budget-exceeded");
    assert!(error.message().contains("presentation.unmeasured"));
    assert!(error.entries().iter().any(|entry| {
        entry.field_key() == Some("presentation.unmeasured")
            && entry.category() == "conversion.approximation-budget-exceeded"
    }));
}

#[test]
fn authorized_metadata_drop_is_applied_by_the_writer_and_reported() {
    let chart = with_metadata_fact(&rpe_chart());
    let profile = profile_reference(
        RpeProfile::PhiraLegacySpeed.id(),
        RpeProfile::PhiraLegacySpeed.version(),
    );
    let descriptor = descriptor_with_domain(
        CapabilitySet::rpe_json(),
        &profile,
        ConversionDomain::Metadata,
        false,
        false,
        true,
        None,
        None,
    );
    let authorization = DropAuthorization::new(
        ["metadata.chart.meta".into()],
        "remove target-inexpressible metadata",
    )
    .unwrap();
    let outcome = export_rpe_json_with_options(
        &chart,
        &ExportOptions::semantic(descriptor).with_drop(authorization),
    )
    .unwrap();
    assert!(outcome.negotiation().drops(ConversionDomain::Metadata));
    assert_eq!(outcome.report().status(), ConversionStatus::Approximate);
    assert!(!outcome.comparison().is_equivalent());
    assert_eq!(
        outcome.comparison().unverified_selectors(),
        ["metadata.chart.meta"]
    );
    let applied = outcome
        .report()
        .entries()
        .iter()
        .find(|entry| entry.id() == "roundtrip/unverified/000000")
        .unwrap();
    assert_eq!(applied.category(), "conversion.drop-applied");
    assert_eq!(applied.phase(), ConversionPhase::ReparseCompare);
    assert_eq!(applied.semantic_status(), SemanticStatus::Dropped);
    assert_eq!(applied.field_key(), Some("metadata.chart.meta"));
    assert!(
        outcome
            .report()
            .entries()
            .iter()
            .filter(|entry| entry.category() == "conversion.capability-negotiated")
            .all(|entry| entry.phase() == ConversionPhase::CapabilityNegotiation)
    );
    assert!(
        !outcome
            .report()
            .entries()
            .iter()
            .any(|entry| entry.id() == "roundtrip/equivalent")
    );
    assert_eq!(outcome.report().summary().drop_count(), 2);
    let recorded = outcome.report().drop_authorization().unwrap();
    assert_eq!(recorded.target_selectors(), ["metadata.chart.meta"]);
    assert_eq!(recorded.reason(), "remove target-inexpressible metadata");
}

#[test]
fn successful_export_retains_unused_loss_authorizations_without_lowering_status() {
    let approximation = ApproximationAuthorization::new(
        ["motion".into()],
        [("motion.track_value".into(), 0.001)],
        1024,
        "linear-segment",
        "1.0.0",
    )
    .unwrap();
    let drop = DropAuthorization::new(
        ["metadata.chart.meta".into()],
        "explicit target loss boundary",
    )
    .unwrap();
    let options = profile_options(
        CapabilitySet::rpe_json(),
        RpeProfile::PhiraLegacySpeed.id(),
        RpeProfile::PhiraLegacySpeed.version(),
    )
    .with_approximation(approximation)
    .with_drop(drop);

    let outcome = export_rpe_json_with_options(&rpe_chart(), &options).unwrap();

    assert_eq!(outcome.report().status(), ConversionStatus::Equivalent);
    assert!(
        !outcome
            .report()
            .entries()
            .iter()
            .any(|entry| entry.id() == "roundtrip/equivalent")
    );
    let approximation = outcome.report().approximation_authorization().unwrap();
    assert_eq!(approximation.target_domains(), ["motion"]);
    assert_eq!(approximation.error_budgets()["motion.track_value"], 0.001);
    assert_eq!(approximation.maximum_segments(), 1024);
    assert_eq!(approximation.algorithm_id(), "linear-segment");
    assert_eq!(approximation.algorithm_version(), "1.0.0");
    let drop = outcome.report().drop_authorization().unwrap();
    assert_eq!(drop.target_selectors(), ["metadata.chart.meta"]);
    assert_eq!(drop.reason(), "explicit target loss boundary");
}

#[test]
fn unused_drop_authorization_cannot_mask_a_direct_roundtrip_mismatch() {
    let chart = with_source_version(&rpe_chart(), "5.0.1");
    let options = profile_options(
        CapabilitySet::rpe_json(),
        RpeProfile::PhiraLegacySpeed.id(),
        RpeProfile::PhiraLegacySpeed.version(),
    )
    .with_drop(
        DropAuthorization::new(["metadata.chart.sourceVersion".into()], "not negotiated").unwrap(),
    );
    let first = export_rpe_json_with_options(&chart, &options).unwrap_err();
    let second = export_rpe_json_with_options(&chart, &options).unwrap_err();
    assert_eq!(first.category(), "conversion.roundtrip-mismatch");
    let report = first.report().expect("post-write failure report");
    assert_eq!(report.status(), ConversionStatus::Failed);
    assert!(report.output_hash().is_none());
    assert_eq!(report.conversion_policy(), options.policy);
    assert_eq!(report.repair_mode(), &options.repair_mode);
    assert_eq!(report.entries(), first.entries());
    assert_eq!(
        report.drop_authorization().unwrap().target_selectors(),
        ["metadata.chart.sourceVersion"]
    );
    assert_eq!(
        report.operation_id(),
        second.report().unwrap().operation_id()
    );
}

#[test]
fn roundtrip_policy_rebuilds_from_canonical_when_source_fidelity_is_stale() {
    let chart = rpe_chart();
    let compilation = compilation_with_stale_roundtrip_fact(&chart);
    let mut options = profile_options(
        CapabilitySet::rpe_json(),
        RpeProfile::PhiraLegacySpeed.id(),
        RpeProfile::PhiraLegacySpeed.version(),
    );
    options.policy = ConversionPolicy::Roundtrip;
    let outcome = export_rpe_compilation_with_options(&compilation, &options).unwrap();
    assert!(outcome.comparison().is_equivalent());
    assert_eq!(outcome.report().status(), ConversionStatus::Equivalent);
    assert!(outcome.report().entries().iter().any(|entry| {
        entry.id() == "roundtrip/stale-source-representation"
            && entry.category() == "conversion.tool-rewrite"
    }));
}

#[test]
fn serialized_target_bytes_obey_the_declared_hard_limit() {
    let chart = rpe_chart();
    let profile = profile_reference(
        RpeProfile::PhiraLegacySpeed.id(),
        RpeProfile::PhiraLegacySpeed.version(),
    );
    let descriptor = descriptor_with_domain(
        CapabilitySet::rpe_json(),
        &profile,
        ConversionDomain::Package,
        true,
        false,
        false,
        None,
        Some(1),
    );
    let error =
        export_rpe_json_with_options(&chart, &ExportOptions::semantic(descriptor)).unwrap_err();
    assert_eq!(error.category(), "conversion.capability-mismatch");
    assert!(error.message().contains("byte limit"));
}

#[test]
fn typed_byte_limit_is_reported_after_target_write() {
    let chart = rpe_chart();
    let profile = profile_reference(
        RpeProfile::PhiraLegacySpeed.id(),
        RpeProfile::PhiraLegacySpeed.version(),
    );
    let base = CapabilitySet::rpe_json().descriptor(Some(profile.clone()));
    let domains = base
        .domains()
        .iter()
        .map(|descriptor| {
            if descriptor.domain() != ConversionDomain::Package {
                return descriptor.clone();
            }
            CapabilityDomainDescriptor::new(
                descriptor.domain(),
                descriptor.exact(),
                descriptor.equivalent(),
                descriptor.approximation(),
                descriptor.preserve(),
                descriptor.drop(),
                descriptor.max_entities(),
                descriptor.max_bytes(),
            )
            .with_features(descriptor.features().iter().cloned())
            .unwrap()
            .with_limits([CapabilityLimit::new("byte.count", 1.0).unwrap()])
            .unwrap()
        })
        .collect();
    let descriptor = CapabilityDescriptor::new(
        base.format(),
        base.version(),
        base.profile().map(str::to_owned),
        domains,
    )
    .unwrap();
    let error = export_rpe_json_with_options(
        &chart,
        &ExportOptions::semantic(descriptor).with_target_profile(profile),
    )
    .unwrap_err();

    assert_eq!(error.category(), "conversion.capability-mismatch");
    assert!(error.entries().iter().any(|entry| {
        entry.domain() == ConversionDomain::Package
            && entry.field_key() == Some("byte.count")
            && entry.message().contains("byte.count")
    }));
}

#[test]
fn resource_byte_limit_does_not_measure_the_target_artifact() {
    let chart = rpe_chart();
    assert!(chart.metadata().resources().is_empty());
    let profile = profile_reference(
        RpeProfile::PhiraLegacySpeed.id(),
        RpeProfile::PhiraLegacySpeed.version(),
    );
    let descriptor = descriptor_with_domain(
        CapabilitySet::rpe_json(),
        &profile,
        ConversionDomain::Resource,
        true,
        false,
        false,
        None,
        Some(1),
    );
    let outcome = export_rpe_json_with_options(
        &chart,
        &ExportOptions::semantic(descriptor).with_target_profile(profile),
    )
    .unwrap();

    assert!(outcome.bytes().len() > 1);
}

#[test]
fn report_limit_rejects_export_before_target_output() {
    let expected = rpe_chart();
    let actual = with_source_version(&expected, "5.0.1");
    let options = profile_options(
        CapabilitySet::rpe_json(),
        RpeProfile::PhiraLegacySpeed.id(),
        RpeProfile::PhiraLegacySpeed.version(),
    );
    let (negotiation, _) = negotiate_export_with_options(&expected, &options).unwrap();
    let alignment = EntityAlignment::new(
        expected
            .lines()
            .lines()
            .map(|line| (line.id().clone(), line.id().clone())),
        expected
            .notes()
            .notes()
            .iter()
            .map(|note| (note.id().clone(), note.id().clone())),
    )
    .unwrap();
    let resources = CanonicalResourceBundle::new(Vec::new()).unwrap();
    let error = finish_export(
        "rpe",
        &expected,
        &actual,
        None,
        &resources,
        &alignment,
        &options,
        negotiation,
        filler_report_entries(MAX_REPORT_ENTRIES),
        b"target bytes must not escape".to_vec(),
    )
    .unwrap_err();

    assert_eq!(error.category(), "conversion.report-limit");
    assert!(error.message().contains("maximum 1024"));
    assert!(error.message().contains("observed 1025"));
    assert!(error.entries().len() <= MAX_REPORT_ENTRIES);
}

#[test]
fn unsupported_required_domain_fails_before_target_write() {
    let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
    let profile = profile_reference(PecProfile::Phira.id(), PecProfile::Phira.version());
    let options = ExportOptions::semantic(loss_descriptor(&profile, false, false));
    let error = negotiate_export_with_options(&chart, &options).unwrap_err();
    assert_eq!(error.category(), "conversion.capability-mismatch");
    assert!(
        error
            .entries()
            .iter()
            .any(|entry| entry.semantic_status() == SemanticStatus::Unsupported)
    );
}

#[test]
fn negotiation_report_order_is_deterministic() {
    let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
    let options = profile_options(
        CapabilitySet::pgr_v3(),
        PgrProfile::PhiraV3.id(),
        PgrProfile::PhiraV3.version(),
    );
    let (_, first) = negotiate_export_with_options(&chart, &options).unwrap();
    let (_, second) = negotiate_export_with_options(&chart, &options).unwrap();
    assert_eq!(
        first.iter().map(ConversionEntry::id).collect::<Vec<_>>(),
        second.iter().map(ConversionEntry::id).collect::<Vec<_>>()
    );
    assert!(
        first
            .iter()
            .all(|entry| entry.category() == "conversion.capability-negotiated")
    );
}

#[test]
fn capability_and_report_domains_share_the_section_7_2_inventory() {
    let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
    let profile = profile_reference(PgrProfile::PhiraV3.id(), PgrProfile::PhiraV3.version());
    let descriptor = CapabilitySet::pgr_v3().descriptor(Some(profile.clone()));
    assert_eq!(
        descriptor
            .domains()
            .iter()
            .map(CapabilityDomainDescriptor::domain)
            .collect::<Vec<_>>(),
        ConversionDomain::ALL.to_vec()
    );

    let options = ExportOptions::semantic(descriptor).with_target_profile(profile);
    let (_, entries) = negotiate_export_with_options(&chart, &options).unwrap();
    assert!(entries.iter().any(|entry| {
        entry.id() == "capability/profile" && entry.domain() == ConversionDomain::Profile
    }));
    assert!(entries.iter().all(|entry| !matches!(
        entry.id(),
        "capability/numeric" | "capability/entity" | "capability/limits" | "capability/expression"
    )));
    assert_eq!(
        capability_entity_count(&chart, ConversionDomain::Motion),
        chart.lines().lines().count() + chart.tracks().tracks().len()
    );
}

#[test]
fn unregistered_report_domains_do_not_fall_back_to_profile_or_package() {
    for domain in ["numeric", "entity", "limits", "expression", "unknown"] {
        let error = conversion_domain_from_str(domain).unwrap_err();
        assert_eq!(error.category(), "conversion.internal");
    }
    for domain in ConversionDomain::ALL {
        assert_eq!(conversion_domain_from_str(domain.as_str()).unwrap(), domain);
    }
}

#[test]
fn successful_writer_fails_if_same_profile_reparse_changes_canonical_identity() {
    let chart = with_source_version(&rpe_chart(), "5.0.1");
    let options = profile_options(
        CapabilitySet::rpe_json(),
        RpeProfile::PhiraLegacySpeed.id(),
        RpeProfile::PhiraLegacySpeed.version(),
    );
    let error = export_rpe_json_with_options(&chart, &options).unwrap_err();
    assert_eq!(error.category(), "conversion.roundtrip-mismatch");
    assert!(
        error
            .entries()
            .iter()
            .any(|entry| entry.category() == "conversion.roundtrip-mismatch")
    );
}
