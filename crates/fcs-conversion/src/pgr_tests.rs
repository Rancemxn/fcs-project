use std::collections::BTreeMap;

use serde::Deserialize;

use super::*;
use crate::{ArtifactRole, SourceArtifact, parse_json_document};

const V1_CHART: &str = r#"{
        "formatVersion": 1,
        "offset": 0.125,
        "rootUnknownA": 1,
        "rootUnknownB": 2,
        "judgeLineList": [
            {
                "bpm": 120,
                "judgeLineMoveEvents": [
                    {"startTime": 0, "endTime": 32, "start": 440260, "end": 440500, "moveUnknown": 1}
                ],
                "judgeLineRotateEvents": [
                    {"startTime": 0, "endTime": 32, "start": 0, "end": 90}
                ],
                "judgeLineDisappearEvents": [
                    {"startTime": 0, "endTime": 32, "start": 0, "end": 1}
                ],
                "speedEvents": [
                    {"startTime": 0, "endTime": 64, "value": 2, "floorPosition": 0}
                ],
                "notesAbove": [
                    {"type": 3, "time": 32, "holdTime": 32, "positionX": 1, "speed": 4, "floorPosition": 1, "noteUnknownA": 1, "noteUnknownB": 2}
                ],
                "notesBelow": [
                    {"type": 1, "time": 0, "holdTime": 0, "positionX": -1, "speed": 2, "floorPosition": 0}
                ],
                "lineUnknown": true
            },
            {
                "bpm": 60,
                "judgeLineMoveEvents": [
                    {"startTime": 0, "endTime": 32, "start": 440260, "end": 440260}
                ],
                "judgeLineRotateEvents": [],
                "judgeLineDisappearEvents": [],
                "speedEvents": [
                    {"startTime": 0, "endTime": 64, "value": 1, "floorPosition": 0}
                ],
                "notesAbove": [],
                "notesBelow": []
            }
        ]
    }"#;

const V3_CHART: &str = r#"{
        "formatVersion": 3,
        "offset": 0,
        "judgeLineList": [{
            "bpm": 120,
            "judgeLineMoveEvents": [
                {"startTime": 0, "endTime": 32, "start": 0.75, "end": 0.5, "start2": 0.25, "end2": 0.5}
            ],
            "judgeLineRotateEvents": [],
            "judgeLineDisappearEvents": [],
            "speedEvents": [{"startTime": 0, "endTime": 32, "value": 1, "floorPosition": 0}],
            "notesAbove": [],
            "notesBelow": []
        }]
    }"#;

fn decimal(raw: &str) -> ExactDecimal {
    ExactDecimal::parse(raw, DecimalLimits::default()).unwrap()
}

fn typed(json: &str, limits: PgrLimits) -> Result<PgrSourceDocument, PgrError> {
    let artifact =
        SourceArtifact::new("chart.json", ArtifactRole::Chart, json.as_bytes().to_vec()).unwrap();
    let document = parse_json_document(SourceFormat::Pgr, &artifact).unwrap();
    parse_pgr_document(&document, limits)
}

fn binding(profile: PgrProfile) -> PgrProfileBinding {
    PgrProfileBinding::new(profile, decimal("120")).unwrap()
}

fn exact(expected: &str) -> ExactRational {
    let (numerator, denominator) = expected.split_once('/').unwrap_or((expected, "1"));
    ExactRational(BigRational::new(
        BigInt::parse_bytes(numerator.as_bytes(), 10).unwrap(),
        BigInt::parse_bytes(denominator.as_bytes(), 10).unwrap(),
    ))
}

#[test]
fn typed_parser_preserves_exact_values_and_unknown_order_and_rejects_known_duplicates() {
    let source = typed(V1_CHART, PgrLimits::default()).unwrap();
    assert_eq!(source.format_version(), PgrFormatVersion::V1);
    assert_eq!(source.offset().raw(), "0.125");
    assert_eq!(
        source
            .unknown_fields()
            .iter()
            .map(LosslessJsonMember::key)
            .collect::<Vec<_>>(),
        ["rootUnknownA", "rootUnknownB"]
    );
    assert_eq!(source.lines()[0].unknown_fields()[0].key(), "lineUnknown");
    assert_eq!(
        source.lines()[0].move_events()[0].unknown_fields()[0].key(),
        "moveUnknown"
    );
    assert_eq!(
        source.lines()[0].notes_above()[0]
            .unknown_fields()
            .iter()
            .map(LosslessJsonMember::key)
            .collect::<Vec<_>>(),
        ["noteUnknownA", "noteUnknownB"]
    );

    let duplicate = V1_CHART.replacen(r#""offset": 0.125"#, r#""offset": 0.125, "offset": 0"#, 1);
    let error = typed(&duplicate, PgrLimits::default()).unwrap_err();
    assert_eq!(error.category(), SOURCE_INVALID);
    assert_eq!(error.path(), "$.offset");

    let wrong_shape = r#"{"formatVersion":1,"offset":0,"judgeLineList":{}}"#;
    assert_eq!(
        typed(wrong_shape, PgrLimits::default())
            .unwrap_err()
            .category(),
        SOURCE_INVALID
    );
}

#[test]
fn four_explicit_profiles_expose_timing_coordinate_note_and_hold_differences() {
    let source = typed(V1_CHART, PgrLimits::default()).unwrap();
    let phira = interpret_pgr(&source, &binding(PgrProfile::PhiraV1)).unwrap();
    let phichain = interpret_pgr(&source, &binding(PgrProfile::PhichainImportV1)).unwrap();

    assert_eq!(phira.audio_offset_seconds(), &exact("1/8"));
    assert_eq!(phira.lines()[0].move_events()[0].end_x_px(), &exact("0"));
    assert_eq!(
        phichain.lines()[0].move_events()[0].end_x_px(),
        &exact("24/11")
    );
    assert_eq!(phira.lines()[0].move_events()[0].start_y_px(), &exact("0"));
    assert_eq!(
        phichain.lines()[0].move_events()[0].start_y_px(),
        &exact("-540/53")
    );
    assert_eq!(
        phira.lines()[0].notes_above()[0].position_x_px(),
        &exact("108")
    );
    assert_eq!(
        phichain.lines()[0].notes_above()[0].position_x_px(),
        &exact("320/3")
    );
    assert_eq!(
        phira.lines()[0].notes_above()[0].scroll_factor(),
        &exact("1")
    );
    assert_eq!(
        phichain.lines()[0].notes_above()[0].scroll_factor(),
        &exact("2")
    );
    assert_eq!(
        phira.lines()[0].notes_above()[0].hold_tail_distance_px(),
        Some(&exact("120"))
    );
    assert_eq!(
        phira.lines()[0].rotate_events()[0].end_value(),
        &exact("-1/2")
    );
    assert_eq!(
        phira.lines()[0].speed_events()[0].distance_end_px(),
        &exact("240")
    );
    assert_eq!(
        phira.lines()[1].move_events()[0]
            .end_time()
            .chart_time_seconds(),
        &exact("1")
    );
    assert_eq!(
        phichain.lines()[1].move_events()[0]
            .end_time()
            .chart_time_seconds(),
        &exact("1/2")
    );
    assert!(PgrProfile::PhiraV1.strict_eligible());
    assert!(!PgrProfile::PhichainImportV1.strict_eligible());
}

#[test]
fn v3_profiles_require_split_normalized_coordinates_and_matching_versions() {
    let source = typed(V3_CHART, PgrLimits::default()).unwrap();
    let semantic = interpret_pgr(&source, &binding(PgrProfile::PhiraV3)).unwrap();
    let phichain = interpret_pgr(&source, &binding(PgrProfile::PhichainImportV3)).unwrap();
    let movement = &semantic.lines()[0].move_events()[0];
    assert_eq!(movement.start_x_px(), &exact("480"));
    assert_eq!(movement.start_y_px(), &exact("-270"));
    assert_eq!(phichain.profile(), PgrProfile::PhichainImportV3);
    assert_eq!(
        interpret_pgr(&source, &binding(PgrProfile::PhiraV1))
            .unwrap_err()
            .category(),
        PROFILE_NOT_APPLICABLE
    );

    let missing_y = V3_CHART.replacen(r#", "start2": 0.25, "end2": 0.5"#, "", 1);
    assert_eq!(
        typed(&missing_y, PgrLimits::default())
            .unwrap_err()
            .category(),
        SOURCE_INVALID
    );
    let out_of_range = V3_CHART.replacen(r#""start": 0.75"#, r#""start": 1.1"#, 1);
    assert_eq!(
        interpret_pgr(
            &typed(&out_of_range, PgrLimits::default()).unwrap(),
            &binding(PgrProfile::PhiraV3)
        )
        .unwrap_err()
        .category(),
        SOURCE_INVALID
    );
}

#[test]
fn semantic_validation_rejects_invalid_intervals_speed_hold_coordinates_alpha_and_caches() {
    let cases = [
            (V1_CHART.replacen(r#""bpm": 120"#, r#""bpm": 0"#, 1), SOURCE_INVALID),
            (
                V1_CHART.replacen(
                    r#""startTime": 0, "endTime": 32, "start": 440260"#,
                    r#""startTime": 33, "endTime": 32, "start": 440260"#,
                    1,
                ),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(
                    r#"{"startTime": 0, "endTime": 32, "start": 440260, "end": 440500, "moveUnknown": 1}"#,
                    r#"{"startTime": 0, "endTime": 32, "start": 440260, "end": 440500}, {"startTime": 16, "endTime": 48, "start": 440260, "end": 440260}"#,
                    1,
                ),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(r#""end": 1}"#, r#""end": 2}"#, 1),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(
                    r#"{"startTime": 0, "endTime": 64, "value": 2, "floorPosition": 0}"#,
                    "",
                    1,
                ),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(
                    r#"{"startTime": 0, "endTime": 64, "value": 2, "floorPosition": 0}"#,
                    r#"{"startTime": 0, "endTime": 16, "value": 2, "floorPosition": 0}, {"startTime": 32, "endTime": 64, "value": 2}"#,
                    1,
                ),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(r#""value": 2, "floorPosition": 0"#, r#""value": -1, "floorPosition": 0"#, 1),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(r#""holdTime": 32"#, r#""holdTime": 0"#, 1),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(r#""start": 440260"#, r#""start": -1"#, 1),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(r#""start": 440260"#, r#""start": 440999"#, 1),
                SOURCE_INVALID,
            ),
            (
                V1_CHART.replacen(r#""speed": 4, "floorPosition": 1"#, r#""speed": 4, "floorPosition": 2"#, 1),
                DISTANCE_MISMATCH,
            ),
        ];
    for (json, category) in cases {
        let source = typed(&json, PgrLimits::default()).unwrap();
        let error = interpret_pgr(&source, &binding(PgrProfile::PhiraV1)).unwrap_err();
        assert_eq!(error.category(), category, "{error}");
    }
}

#[test]
fn decimal_entity_and_profile_parameter_limits_cover_their_boundaries() {
    let source = typed(V1_CHART, PgrLimits::default()).unwrap();
    assert_eq!(source.lines().len(), 2);
    let line_limit = PgrLimits {
        max_lines: 1,
        ..PgrLimits::default()
    };
    assert_eq!(
        typed(V1_CHART, line_limit).unwrap_err().category(),
        SOURCE_INVALID
    );
    let event_limit = PgrLimits {
        max_events_per_line: 0,
        ..PgrLimits::default()
    };
    assert_eq!(
        typed(V1_CHART, event_limit).unwrap_err().category(),
        SOURCE_INVALID
    );
    let note_limit = PgrLimits {
        max_notes_per_line: 0,
        ..PgrLimits::default()
    };
    assert_eq!(
        typed(V1_CHART, note_limit).unwrap_err().category(),
        SOURCE_INVALID
    );

    for raw in ["0", "-1", "1e4096"] {
        assert_eq!(
            PgrProfileBinding::new(PgrProfile::PhiraV1, decimal(raw))
                .unwrap_err()
                .category(),
            PROFILE_PARAMETER_INVALID
        );
    }
}

#[derive(Deserialize)]
struct MappingCorpus {
    vector: Vec<MappingVector>,
}

#[derive(Deserialize)]
struct MappingVector {
    id: String,
    rule_id: String,
    source: BTreeMap<String, toml::Value>,
    expected: String,
}

fn source_decimal<'a>(vector: &'a MappingVector, key: &str) -> &'a str {
    vector.source[key].as_str().unwrap()
}

#[test]
fn checked_in_pgr_source_semantic_mapping_vectors_execute_exactly() {
    let corpus: MappingCorpus = toml::from_str(include_str!(
        "../../../docs/conformance/conversion/mapping-vectors.toml"
    ))
    .unwrap();
    let mut executed = 0;
    for vector in &corpus.vector {
        let actual = match vector.rule_id.as_str() {
            "pgr.time.source-line-beat-t32" => {
                ExactRational(decimal(source_decimal(vector, "T")).exact().value() / integer(32))
            }
            "pgr.time.per-line-bpm" => {
                semantic_time_value(
                    decimal(source_decimal(vector, "T")).exact().value(),
                    decimal(source_decimal(vector, "current_line_bpm")).exact(),
                )
                .chart_time_seconds
            }
            "pgr.time.first-line-bpm" => {
                semantic_time_value(
                    decimal(source_decimal(vector, "T")).exact().value(),
                    decimal(source_decimal(vector, "first_line_bpm")).exact(),
                )
                .chart_time_seconds
            }
            "pgr.note-x.unit108" => map_note_x(
                decimal(source_decimal(vector, "x")).exact(),
                PgrProfile::PhiraV1,
            ),
            "pgr.note-x.unit320_3" => map_note_x(
                decimal(source_decimal(vector, "x")).exact(),
                PgrProfile::PhichainImportV1,
            ),
            "pgr.line-x.normalized" => ExactRational(
                (decimal(source_decimal(vector, "x")).exact().value() - half()) * integer(1920),
            ),
            "pgr.line-y.normalized" => ExactRational(
                (decimal(source_decimal(vector, "y")).exact().value() - half()) * integer(1080),
            ),
            "pgr.v1-move-x.trunc1000-div880" => {
                map_v1_point(
                    decimal(source_decimal(vector, "packed")).exact(),
                    PgrProfile::PhiraV1,
                    &vector.id,
                )
                .unwrap()
                .0
            }
            "pgr.v1-move-x.round1000-div880" => {
                map_v1_point(
                    decimal(source_decimal(vector, "packed")).exact(),
                    PgrProfile::PhichainImportV1,
                    &vector.id,
                )
                .unwrap()
                .0
            }
            "pgr.v1-move-y.mod1000-div520" => {
                map_v1_point(
                    decimal(source_decimal(vector, "packed")).exact(),
                    PgrProfile::PhiraV1,
                    &vector.id,
                )
                .unwrap()
                .1
            }
            "pgr.v1-move-y.mod1000-div530" => {
                map_v1_point(
                    decimal(source_decimal(vector, "packed")).exact(),
                    PgrProfile::PhichainImportV1,
                    &vector.id,
                )
                .unwrap()
                .1
            }
            "pgr.offset.seconds" => decimal(source_decimal(vector, "offset_seconds"))
                .exact()
                .clone(),
            _ => continue,
        };
        assert_eq!(actual, exact(&vector.expected), "{}", vector.id);
        executed += 1;
    }
    assert_eq!(executed, 12);
}
