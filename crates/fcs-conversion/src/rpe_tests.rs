use std::collections::BTreeMap;

use serde::Deserialize;

use super::*;
use crate::{ArtifactRole, SourceArtifact, parse_json_document};

const MINIMAL_CHART: &str = r#"{
        "META": {
            "RPEVersion": 150,
            "offset": 12,
            "name": "timing",
            "song": "a.ogg",
            "background": "b.png",
            "metaUnknown": true
        },
        "BPMList": [
            {"startTime": [0, 0, 1], "bpm": 120, "bpmUnknown": 1},
            {"startTime": [4, 0, 1], "bpm": 180}
        ],
        "judgeLineList": [
            {
                "bpmfactor": 2,
                "eventLayers": [
                    null,
                    {
                        "moveXEvents": [
                            {
                                "startTime": [0, 0, 1],
                                "endTime": [1, 0, 1],
                                "start": 0,
                                "end": 100,
                                "easingType": 1,
                                "moveUnknown": 9
                            }
                        ],
                        "speedEvents": [
                            {
                                "startTime": [0, 0, 1],
                                "endTime": [2, 0, 1],
                                "start": 1,
                                "end": 1
                            }
                        ]
                    }
                ],
                "notes": [
                    {
                        "type": 1,
                        "startTime": [1, 0, 1],
                        "endTime": [1, 0, 1],
                        "positionX": 0,
                        "speed": 1,
                        "above": 1,
                        "isFake": 0,
                        "hitsound": "click.ogg"
                    }
                ],
                "father": -1,
                "Texture": "line.png",
                "lineUnknownA": 1,
                "lineUnknownB": 2
            },
            {
                "eventLayers": null,
                "notes": [],
                "rotateWithFather": true
            }
        ],
        "chartTime": 0
    }"#;

fn artifact(bytes: &str) -> SourceArtifact {
    SourceArtifact::new(
        "charts/main.rpe.json",
        ArtifactRole::Chart,
        bytes.as_bytes(),
    )
    .unwrap()
}

#[test]
fn easing_zero_uses_the_reference_linear_alias() {
    assert_eq!(rpe_easing_name(0), Some("linear"));
    assert_eq!(rpe_easing_name(1), Some("linear"));
}

fn parse_minimal() -> RpeSourceDocument {
    let parsed = parse_json_document(SourceFormat::Rpe, &artifact(MINIMAL_CHART)).unwrap();
    parse_rpe_document(&parsed, RpeLimits::default()).unwrap()
}

fn exact(expected: &str) -> ExactRational {
    if expected.contains('/') {
        let (numerator, denominator) = expected.split_once('/').unwrap();
        ExactRational(BigRational::new(
            BigInt::parse_bytes(numerator.as_bytes(), 10).unwrap(),
            BigInt::parse_bytes(denominator.as_bytes(), 10).unwrap(),
        ))
    } else if expected.contains('.') || expected.contains(['e', 'E']) {
        ExactDecimal::parse(expected, DecimalLimits::default())
            .unwrap()
            .exact()
            .clone()
    } else {
        ExactRational(BigRational::new(
            BigInt::parse_bytes(expected.as_bytes(), 10).unwrap(),
            BigInt::one(),
        ))
    }
}

fn beat(a: i64, b: i64, c: i64) -> RpeBeat {
    RpeBeat {
        whole: ExactDecimal::parse(&a.to_string(), DecimalLimits::default()).unwrap(),
        numerator: ExactDecimal::parse(&b.to_string(), DecimalLimits::default()).unwrap(),
        denominator: ExactDecimal::parse(&c.to_string(), DecimalLimits::default()).unwrap(),
    }
}

#[test]
fn parse_retains_identity_version_layers_and_unknown_order() {
    let source = parse_minimal();
    assert_eq!(source.artifact_id().as_str(), "charts/main.rpe.json");
    assert_eq!(
        source.artifact_content_sha256(),
        artifact(MINIMAL_CHART).content_sha256()
    );
    let version = source.meta().rpe_version().unwrap();
    assert!(version.is_number());
    assert_eq!(version.raw_spelling(), "150");
    assert_eq!(source.meta().offset().raw(), "12");
    assert_eq!(source.meta().song(), Some("a.ogg"));
    assert_eq!(source.meta().unknown_fields()[0].key(), "metaUnknown");
    assert_eq!(source.unknown_fields()[0].key(), "chartTime");
    assert!(matches!(
        source.lines()[0].event_layers(),
        RpeEventLayersField::Present(_)
    ));
    if let RpeEventLayersField::Present(slots) = source.lines()[0].event_layers() {
        assert!(matches!(slots[0], RpeEventLayerSlot::Null));
        assert!(matches!(slots[1], RpeEventLayerSlot::Layer(_)));
    }
    assert!(matches!(
        source.lines()[1].event_layers(),
        RpeEventLayersField::Null
    ));
    assert_eq!(source.lines()[0].rotate_with_father(), None);
    assert_eq!(source.lines()[1].rotate_with_father(), Some(true));
    assert_eq!(source.lines()[0].notes()[0].hitsound(), Some("click.ogg"));
    assert_eq!(source.lines()[0].unknown_fields()[0].key(), "lineUnknownA");
    assert_eq!(source.lines()[0].unknown_fields()[1].key(), "lineUnknownB");
}

#[test]
fn string_rpe_version_is_preserved_as_string_evidence() {
    let chart = MINIMAL_CHART.replace("\"RPEVersion\": 150", "\"RPEVersion\": \"170\"");
    let parsed = parse_json_document(SourceFormat::Rpe, &artifact(&chart)).unwrap();
    let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
    let version = source.meta().rpe_version().unwrap();
    assert!(version.is_string());
    assert!(version.raw_spelling().contains("170"));
}

#[test]
fn missing_event_layers_and_sparse_fields_are_observable() {
    let chart = r#"{
            "META": {"offset": 0},
            "BPMList": [{"startTime": [0,0,1], "bpm": 120}],
            "judgeLineList": [{
                "notes": [],
                "eventLayers": [{
                    "moveXEvents": null
                }]
            }]
        }"#;
    let parsed = parse_json_document(SourceFormat::Rpe, &artifact(chart)).unwrap();
    let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
    match source.lines()[0].event_layers() {
        RpeEventLayersField::Present(slots) => match &slots[0] {
            RpeEventLayerSlot::Layer(layer) => {
                assert!(matches!(layer.move_x_events(), RpeOptionalEventList::Null));
                assert!(matches!(
                    layer.speed_events(),
                    RpeOptionalSpeedList::Missing
                ));
            }
            _ => panic!("expected layer object"),
        },
        other => panic!("unexpected layers {other:?}"),
    }
}

#[test]
fn five_profile_bindings_resolve_factor_and_rotate_defaults() {
    let source = parse_minimal();
    let divide = interpret_rpe_timing(
        &source,
        &RpeProfileBinding::community_divide(RpeSpeedMode::LegacyLinear),
    )
    .unwrap();
    assert_eq!(divide.profile().id(), "rpe.community.divide-bpmfactor");
    assert_eq!(divide.lines()[0].bpmfactor(), &exact("2"));
    assert!(!divide.lines()[0].rotate_with_father());
    assert!(!divide.lines()[0].rotate_with_father_was_present());
    assert!(divide.lines()[1].rotate_with_father());
    assert!(divide.lines()[1].rotate_with_father_was_present());

    let multiply = interpret_rpe_timing(
        &source,
        &RpeProfileBinding::docs_example_multiply(RpeSpeedMode::ModernEased),
    )
    .unwrap();
    assert_eq!(multiply.profile().factor_mode(), RpeFactorMode::Multiply);
    assert!(multiply.lines()[0].rotate_with_father());

    let phichain = interpret_rpe_timing(&source, &RpeProfileBinding::phichain_import()).unwrap();
    assert_eq!(phichain.profile().factor_mode(), RpeFactorMode::Ignore);
    assert!(phichain.lines()[0].rotate_with_father());

    let legacy = interpret_rpe_timing(&source, &RpeProfileBinding::phira_legacy_speed()).unwrap();
    assert!(!legacy.lines()[0].rotate_with_father());

    let modern =
        interpret_rpe_timing(&source, &RpeProfileBinding::phira_rpe170_speed(None)).unwrap();
    assert_eq!(modern.rpe_version_era(), Some(RpeVersionEra::Pre170));
}

#[test]
fn profile_parameter_requirements_are_strict() {
    let source = parse_minimal();
    let missing_speed = interpret_rpe_timing(
        &source,
        &RpeProfileBinding {
            profile: RpeProfile::CommunityDivideBpmfactor,
            speed_mode: None,
            rpe_version_era: None,
        },
    )
    .unwrap_err();
    assert_eq!(missing_speed.category(), PROFILE_PARAMETER_INVALID);

    let chart = MINIMAL_CHART.replace("\"RPEVersion\": 150,\n            ", "");
    let parsed = parse_json_document(SourceFormat::Rpe, &artifact(&chart)).unwrap();
    let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
    assert!(source.meta().rpe_version().is_none());
    let missing_era =
        interpret_rpe_timing(&source, &RpeProfileBinding::phira_rpe170_speed(None)).unwrap_err();
    assert_eq!(missing_era.category(), PROFILE_PARAMETER_INVALID);
    let with_era = interpret_rpe_timing(
        &source,
        &RpeProfileBinding::phira_rpe170_speed(Some(RpeVersionEra::AtLeast170)),
    )
    .unwrap();
    assert_eq!(with_era.rpe_version_era(), Some(RpeVersionEra::AtLeast170));
}

#[test]
fn beat_rules_follow_profile_and_reject_invalid_denominators() {
    assert_eq!(
        resolve_beat(&beat(4, 1, 2), RpeProfile::PhiraLegacySpeed, "beat").unwrap(),
        exact("9/2")
    );
    assert_eq!(
        resolve_beat(&beat(4, 0, 0), RpeProfile::PhichainImport, "beat").unwrap(),
        exact("4")
    );
    assert_eq!(
        resolve_beat(&beat(1, 0, 0), RpeProfile::PhiraLegacySpeed, "beat")
            .unwrap_err()
            .category(),
        SOURCE_INVALID
    );
    assert_eq!(
        resolve_beat(&beat(1, 1, 0), RpeProfile::PhichainImport, "beat")
            .unwrap_err()
            .category(),
        SOURCE_INVALID
    );
}

#[test]
fn factor_modes_diverge_exactly_on_same_inputs() {
    let delta = exact("1");
    let bpm = exact("120");
    let factor = exact("2");
    assert_eq!(
        chart_time_delta_seconds(&delta, &bpm, &factor, RpeFactorMode::Divide).unwrap(),
        exact("1")
    );
    assert_eq!(
        chart_time_delta_seconds(&delta, &bpm, &factor, RpeFactorMode::Multiply).unwrap(),
        exact("1/4")
    );
    assert_eq!(
        chart_time_delta_seconds(&delta, &bpm, &factor, RpeFactorMode::Ignore).unwrap(),
        exact("1/2")
    );
}

#[test]
fn interpret_maps_note_and_event_boundaries_through_bpmlist() {
    let source = parse_minimal();
    let divide = interpret_rpe_timing(
        &source,
        &RpeProfileBinding::community_divide(RpeSpeedMode::LegacyLinear),
    )
    .unwrap();
    // beat 1 with bpmfactor=2 and first BPM 120: dt = 1 * 60 * 2 / 120 = 1
    assert_eq!(
        divide.lines()[0].notes()[0]
            .start_time()
            .chart_time_seconds(),
        &exact("1")
    );
    let ignore = interpret_rpe_timing(&source, &RpeProfileBinding::phira_legacy_speed()).unwrap();
    // same beat with ignore factor: dt = 1 * 60 / 120 = 1/2
    assert_eq!(
        ignore.lines()[0].notes()[0]
            .start_time()
            .chart_time_seconds(),
        &exact("1/2")
    );
    let layer = ignore.lines()[0].event_layers()[1].as_ref().unwrap();
    assert_eq!(
        layer.move_x_events()[0].end_time().chart_time_seconds(),
        &exact("1/2")
    );
}

#[test]
fn same_beat_bpm_points_keep_source_order_last_active() {
    let chart = r#"{
            "META": {"offset": 0},
            "BPMList": [
                {"startTime": [0,0,1], "bpm": 60},
                {"startTime": [0,0,1], "bpm": 120}
            ],
            "judgeLineList": [{
                "notes": [{
                    "type": 1,
                    "startTime": [1,0,1],
                    "endTime": [1,0,1],
                    "positionX": 0,
                    "speed": 1
                }]
            }]
        }"#;
    let parsed = parse_json_document(SourceFormat::Rpe, &artifact(chart)).unwrap();
    let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
    let semantic = interpret_rpe_timing(&source, &RpeProfileBinding::phira_legacy_speed()).unwrap();
    assert_eq!(semantic.bpm_points().len(), 2);
    assert_eq!(
        semantic.lines()[0].notes()[0]
            .start_time()
            .chart_time_seconds(),
        &exact("1/2")
    );
}

#[test]
fn limits_and_duplicate_known_fields_fail_strictly() {
    let limits = RpeLimits {
        max_lines: 0,
        ..RpeLimits::default()
    };
    let parsed = parse_json_document(SourceFormat::Rpe, &artifact(MINIMAL_CHART)).unwrap();
    assert_eq!(
        parse_rpe_document(&parsed, limits).unwrap_err().category(),
        SOURCE_INVALID
    );

    let duplicate = r#"{
            "META": {"offset": 0, "offset": 1},
            "BPMList": [{"startTime": [0,0,1], "bpm": 120}],
            "judgeLineList": []
        }"#;
    let parsed = parse_json_document(SourceFormat::Rpe, &artifact(duplicate)).unwrap();
    assert_eq!(
        parse_rpe_document(&parsed, RpeLimits::default())
            .unwrap_err()
            .path(),
        "$.META.offset"
    );
}

#[test]
fn decreasing_bpmlist_is_rejected_without_sorting() {
    let chart = r#"{
            "META": {"offset": 0},
            "BPMList": [
                {"startTime": [2,0,1], "bpm": 120},
                {"startTime": [1,0,1], "bpm": 120}
            ],
            "judgeLineList": []
        }"#;
    let parsed = parse_json_document(SourceFormat::Rpe, &artifact(chart)).unwrap();
    let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
    assert_eq!(
        interpret_rpe_timing(&source, &RpeProfileBinding::phira_legacy_speed())
            .unwrap_err()
            .category(),
        SOURCE_INVALID
    );
}

#[derive(Debug, Deserialize)]
struct MappingCorpus {
    vector: Vec<MappingVector>,
    invalid: Vec<InvalidVector>,
}

#[derive(Debug, Deserialize)]
struct MappingVector {
    id: String,
    rule_id: String,
    source: BTreeMap<String, toml::Value>,
    expected: String,
}

#[derive(Debug, Deserialize)]
struct InvalidVector {
    id: String,
    rule_id: String,
    source: BTreeMap<String, toml::Value>,
    diagnostic: String,
}

#[test]
fn checked_in_rpe_beat_and_bpmfactor_vectors_execute_exactly() {
    let corpus: MappingCorpus = toml::from_str(include_str!(
        "../../../docs/conformance/conversion/mapping-vectors.toml"
    ))
    .unwrap();
    let mut executed = 0;
    for vector in &corpus.vector {
        let actual = match vector.rule_id.as_str() {
            "rpe.beat.abc-strict" => resolve_beat(
                &beat(
                    vector.source["a"].as_integer().unwrap(),
                    vector.source["b"].as_integer().unwrap(),
                    vector.source["c"].as_integer().unwrap(),
                ),
                RpeProfile::PhiraLegacySpeed,
                &vector.id,
            )
            .unwrap(),
            "rpe.beat.abc-zero-zero-integer" => resolve_beat(
                &beat(
                    vector.source["a"].as_integer().unwrap(),
                    vector.source["b"].as_integer().unwrap(),
                    vector.source["c"].as_integer().unwrap(),
                ),
                RpeProfile::PhichainImport,
                &vector.id,
            )
            .unwrap(),
            "rpe.time.bpmfactor-divide" => chart_time_delta_seconds(
                &exact(vector.source["beat_delta"].as_str().unwrap()),
                &exact(vector.source["bpm"].as_str().unwrap()),
                &exact(vector.source["bpmfactor"].as_str().unwrap()),
                RpeFactorMode::Divide,
            )
            .unwrap(),
            "rpe.time.bpmfactor-multiply" => chart_time_delta_seconds(
                &exact(vector.source["beat_delta"].as_str().unwrap()),
                &exact(vector.source["bpm"].as_str().unwrap()),
                &exact(vector.source["bpmfactor"].as_str().unwrap()),
                RpeFactorMode::Multiply,
            )
            .unwrap(),
            "rpe.time.bpmfactor-ignore" => chart_time_delta_seconds(
                &exact(vector.source["beat_delta"].as_str().unwrap()),
                &exact(vector.source["bpm"].as_str().unwrap()),
                &exact(vector.source["bpmfactor"].as_str().unwrap()),
                RpeFactorMode::Ignore,
            )
            .unwrap(),
            _ => continue,
        };
        assert_eq!(actual, exact(&vector.expected), "{}", vector.id);
        executed += 1;
    }
    assert_eq!(executed, 5);

    let mut invalid_executed = 0;
    for vector in &corpus.invalid {
        let error = match vector.rule_id.as_str() {
            "rpe.beat.abc-strict" => resolve_beat(
                &beat(
                    vector.source["a"].as_integer().unwrap(),
                    vector.source["b"].as_integer().unwrap(),
                    vector.source["c"].as_integer().unwrap(),
                ),
                RpeProfile::PhiraLegacySpeed,
                &vector.id,
            )
            .unwrap_err(),
            "rpe.beat.abc-zero-zero-integer" => resolve_beat(
                &beat(
                    vector.source["a"].as_integer().unwrap(),
                    vector.source["b"].as_integer().unwrap(),
                    vector.source["c"].as_integer().unwrap(),
                ),
                RpeProfile::PhichainImport,
                &vector.id,
            )
            .unwrap_err(),
            _ => continue,
        };
        assert_eq!(
            error.category(),
            vector.diagnostic.as_str(),
            "{}",
            vector.id
        );
        invalid_executed += 1;
    }
    assert_eq!(invalid_executed, 2);
}

#[test]
fn i63b_semantics_cover_layers_parent_notes_and_vectors() {
    let source = parse_minimal();
    let phira = interpret_rpe_semantics(&source, &RpeProfileBinding::phira_legacy_speed()).unwrap();
    assert_eq!(phira.layer_policy(), RpeLayerPolicy::Additive);
    assert!(!phira.layer_loss_reported());
    assert_eq!(phira.lines()[0].speed_era(), RpeSpeedEra::LegacyLinear);
    assert_eq!(phira.lines()[0].notes()[0].kind(), RpeNoteKind::Tap);
    assert_eq!(phira.lines()[0].notes()[0].side(), RpeNoteSide::Above);
    assert!(phira.lines()[0].notes()[0].judgment_enabled());

    let phichain = interpret_rpe_semantics(&source, &RpeProfileBinding::phichain_import()).unwrap();
    assert_eq!(phichain.layer_policy(), RpeLayerPolicy::FirstOnly);
    assert_eq!(phichain.lines()[0].retained_layer_count(), 1);
    assert_eq!(phichain.lines()[0].dropped_layer_count(), 0);

    let multi_layer = r#"{
            "META": {"offset": 0, "RPEVersion": 150},
            "BPMList": [{"startTime": [0,0,1], "bpm": 120}],
            "judgeLineList": [{
                "eventLayers": [
                    {"moveXEvents": []},
                    {"moveXEvents": []}
                ],
                "notes": []
            }]
        }"#;
    let parsed = parse_json_document(SourceFormat::Rpe, &artifact(multi_layer)).unwrap();
    let multi = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
    let multi_phichain =
        interpret_rpe_semantics(&multi, &RpeProfileBinding::phichain_import()).unwrap();
    assert!(multi_phichain.layer_loss_reported());
    assert_eq!(multi_phichain.lines()[0].retained_layer_count(), 1);
    assert_eq!(multi_phichain.lines()[0].dropped_layer_count(), 1);

    assert_eq!(scale_speed_4_5(&exact("9/2")).unwrap(), exact("1"));
    assert!(!note_judgment_enabled(Some(&exact("2"))));
    assert!(note_judgment_enabled(Some(&exact("0"))));
    assert_eq!(note_side_from_above(Some(&exact("1"))), RpeNoteSide::Above);
    assert_eq!(note_side_from_above(Some(&exact("0"))), RpeNoteSide::Below);
    assert_eq!(phira_visible_from(&exact("5"), &exact("2")), exact("3"));
    assert_eq!(phira_linear_alpha(256), exact("1"));
    assert_eq!(
        phira_offset_y_logical_px(&exact("10"), &exact("2")),
        exact("24")
    );

    // Parent cycle is rejected without Repair.
    let cyclic = r#"{
            "META": {"offset": 0},
            "BPMList": [{"startTime": [0,0,1], "bpm": 120}],
            "judgeLineList": [
                {"father": 1, "notes": []},
                {"father": 0, "notes": []}
            ]
        }"#;
    let parsed = parse_json_document(SourceFormat::Rpe, &artifact(cyclic)).unwrap();
    let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
    assert_eq!(
        interpret_rpe_semantics(&source, &RpeProfileBinding::phira_legacy_speed())
            .unwrap_err()
            .category(),
        SOURCE_INVALID
    );
}

#[test]
fn checked_in_rpe_note_and_speed_vectors_execute_exactly() {
    let corpus: MappingCorpus = toml::from_str(include_str!(
        "../../../docs/conformance/conversion/mapping-vectors.toml"
    ))
    .unwrap();
    let mut executed = 0;
    for vector in &corpus.vector {
        match vector.rule_id.as_str() {
            "rpe.speed.scale4_5" => {
                let actual =
                    scale_speed_4_5(&exact(vector.source["speed"].as_str().unwrap())).unwrap();
                assert_eq!(actual, exact(&vector.expected), "{}", vector.id);
                executed += 1;
            }
            "rpe.note-fake.nonzero" => {
                let is_fake = exact(&vector.source["is_fake"].as_integer().unwrap().to_string());
                assert!(!note_judgment_enabled(Some(&is_fake)), "{}", vector.id);
                executed += 1;
            }
            "rpe.note-visible-time.phira" => {
                let actual = phira_visible_from(
                    &exact(vector.source["note_chart_time_seconds"].as_str().unwrap()),
                    &exact(vector.source["visible_time_seconds"].as_str().unwrap()),
                );
                assert_eq!(actual, exact(&vector.expected), "{}", vector.id);
                executed += 1;
            }
            "rpe.note-alpha.phira-u16" => {
                let actual = phira_linear_alpha(vector.source["alpha"].as_integer().unwrap());
                assert_eq!(actual, exact(&vector.expected), "{}", vector.id);
                executed += 1;
            }
            "rpe.note-size.phira-uniform" => {
                let size = exact(vector.source["size"].as_str().unwrap());
                assert_eq!(
                    (size.clone(), size),
                    (exact("3/2"), exact("3/2")),
                    "{}",
                    vector.id
                );
                executed += 1;
            }
            "rpe.note-y-offset.phira-speed" => {
                let actual = phira_offset_y_logical_px(
                    &exact(vector.source["y_offset"].as_str().unwrap()),
                    &exact(vector.source["note_speed"].as_str().unwrap()),
                );
                assert_eq!(actual, exact(&vector.expected), "{}", vector.id);
                executed += 1;
            }
            _ => {}
        }
    }
    assert_eq!(executed, 6);
}
