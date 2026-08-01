use super::*;
use std::fs;

use fcs_source::ResourceLimits;
use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;
use tempfile::tempdir;

fn compilation(source: &str) -> CanonicalCompilation {
    let workspace = tempdir().unwrap();
    let document = parse_document(source).into_result().unwrap();
    document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap()
}

/// Compiles `source` and returns the product FCBC bytes.
fn compile(source: &str) -> Vec<u8> {
    let compilation = compilation(source);
    write_from_compilation(&compilation).unwrap()
}

fn assert_runtime_value_bits(actual: crate::RuntimeValue, expected: crate::RuntimeValue) {
    match (actual, expected) {
        (
            crate::RuntimeValue::Scalar {
                ty: actual_ty,
                value: actual,
            },
            crate::RuntimeValue::Scalar {
                ty: expected_ty,
                value: expected,
            },
        ) => {
            assert_eq!(actual_ty, expected_ty);
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
        (crate::RuntimeValue::Color(actual), crate::RuntimeValue::Color(expected)) => {
            for (actual, expected) in actual.into_iter().zip(expected) {
                assert_eq!(actual.to_bits(), expected.to_bits());
            }
        }
        (
            crate::RuntimeValue::Vec2 {
                ty: actual_ty,
                value: actual,
            },
            crate::RuntimeValue::Vec2 {
                ty: expected_ty,
                value: expected,
            },
        ) => {
            assert_eq!(actual_ty, expected_ty);
            for (actual, expected) in actual.into_iter().zip(expected) {
                assert_eq!(actual.to_bits(), expected.to_bits());
            }
        }
        (actual, expected) => assert_eq!(actual, expected),
    }
}

#[test]
fn fidelity_profile_encodes_source_free_section_without_changing_execution() {
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
"#;
    let base = compilation(source);
    let raw_source_value = "authoring source text must not be serialized";
    let distribution = DistributionMetadata::new(
        fcs_model::ProvenanceGraph::new([fcs_model::RestrictedProvenanceFact::new(
            "pgr/note/0",
            Some("chart.json".into()),
            Some(fcs_model::LogicalSourceLocator::new("chart.json").unwrap()),
            Some(raw_source_value.into()),
            Some(0),
            Some(fcs_model::MappingRuleRef::new("pgr.note.position@1.0.0").unwrap()),
            fcs_model::OriginState::Imported,
            Some(fcs_model::SemanticStatus::Mapped),
            [],
        )
        .unwrap()])
        .unwrap(),
        Vec::new(),
        vec![
            fcs_model::InputContentHash::sha256_lower_hex(
                "a".repeat(64),
                Some(fcs_model::LogicalSourceLocator::new("chart.json").unwrap()),
            )
            .unwrap(),
        ],
        CanonicalObject::new(vec![fcs_model::CanonicalObjectEntry::new(
            "producer",
            CanonicalValue::String("fixture".into()),
        )])
        .unwrap(),
    )
    .unwrap()
    .with_semantic_losses([fcs_model::SemanticLoss::new(
        fcs_model::ConversionDomain::Timing,
        fcs_model::SemanticStatus::Preserved,
        fcs_model::SemanticLoss::CAPABILITY_NEGOTIATED,
        None,
    )
    .unwrap()]);
    let compilation =
        CanonicalCompilation::new(base.chart().clone(), base.resources().clone(), distribution);

    let strict = write_from_compilation(&compilation).unwrap();
    let fidelity =
        write_from_compilation_with_profile(&compilation, ContainerProfile::Fidelity).unwrap();
    let container = crate::load_container(&fidelity).unwrap();
    assert_eq!(container.header.profile, ContainerProfile::Fidelity);
    assert!(
        container
            .header
            .feature_flags
            .contains(crate::FeatureFlags::HAS_FIDELITY)
    );
    assert!(container.section_types().contains(&16));
    assert!(
        fidelity
            .windows(fcs_model::SemanticLoss::CAPABILITY_NEGOTIATED.len())
            .any(|window| { window == fcs_model::SemanticLoss::CAPABILITY_NEGOTIATED.as_bytes() })
    );
    assert!(
        !fidelity
            .windows(raw_source_value.len())
            .any(|window| window == raw_source_value.as_bytes())
    );

    let strict_chart = crate::load_chart(&strict).unwrap();
    let fidelity_chart = crate::load_chart(&fidelity).unwrap();
    assert_eq!(fidelity_chart.constants, strict_chart.constants);
    assert_eq!(fidelity_chart.resources, strict_chart.resources);
    assert_eq!(fidelity_chart.extensions, strict_chart.extensions);
    assert_eq!(fidelity_chart.tempo_points, strict_chart.tempo_points);
    assert_eq!(fidelity_chart.lines, strict_chart.lines);
    assert_eq!(fidelity_chart.notes, strict_chart.notes);

    let mut malformed = fidelity.clone();
    let digest_offset = malformed
        .windows(64)
        .position(|window| {
            window == b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        })
        .unwrap();
    malformed[digest_offset] = b'A';
    let strings = container
        .sections
        .iter()
        .find(|section| section.section_type == 1)
        .unwrap();
    let start = strings.offset as usize;
    let end = start + strings.length as usize;
    let checksum = crate::section_crc32_iso_hdlc(&malformed[start..end]);
    let entry = (0..container.header.section_count as usize)
        .map(|index| 128 + index * 40)
        .find(|offset| malformed[*offset..*offset + 4] == 1u32.to_le_bytes())
        .unwrap();
    malformed[entry + 32..entry + 36].copy_from_slice(&checksum.to_le_bytes());
    assert_eq!(crate::load_chart(&malformed), Err("fcbc.invalid-fidelity"));
}

#[test]
fn a_sub_beat_tempo_point_keeps_its_exact_rational_beat() {
    // Beat literals are decimal, so 0.5 and 1.5 reduce to 1/2 and 3/2.
    // Flooring collapsed them onto 0 and 1, so two distinct tempo points
    // could land on the same beat.
    let bytes = compile(
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; 0.5beat -> 180bpm; 1.5beat -> 240bpm; }
lines { line main {} }
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
"#,
    );
    let decoded = crate::load_chart(&bytes).expect("chart must load");
    let beats: Vec<(i64, i64)> = decoded
        .tempo_points
        .iter()
        .map(|point| (point.beat_numerator, point.beat_denominator))
        .collect();
    assert_eq!(beats, vec![(0, 1), (1, 2), (3, 2)]);
}

#[test]
fn a_non_dyadic_tempo_map_round_trips_through_product_load() {
    let bytes = compile(
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 960bpm; 800.1beat -> 60bpm; 800.3beat -> 120bpm; }
lines { line main {} }
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
"#,
    );
    let decoded = crate::load_chart(&bytes).expect("non-dyadic tempo map must load");
    let beats: Vec<(i64, i64)> = decoded
        .tempo_points
        .iter()
        .map(|point| (point.beat_numerator, point.beat_denominator))
        .collect();
    assert_eq!(beats, vec![(0, 1), (8001, 10), (8003, 10)]);
    assert_eq!(
        decoded.tempo_points[1].chart_time,
        (8001.0 * 60.0) / (10.0 * 960.0)
    );
    assert_eq!(
        decoded.tempo_points[2].chart_time,
        decoded.tempo_points[1].chart_time + (20.0 * 60.0) / (100.0 * 60.0)
    );
}

#[test]
fn a_flick_is_written_as_the_section_12_kind() {
    let bytes = compile(
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections { notes { flick { id: "f"; line: @main; gameplay.time: 1s; }; } }
"#,
    );
    let decoded = crate::load_chart(&bytes).expect("chart must load");
    assert_eq!(decoded.notes.len(), 1);
    assert_eq!(
        decoded.notes[0].kind,
        note_kind_ordinal(CanonicalNoteKind::Flick)
    );
    assert_eq!(decoded.notes[0].kind, 3, "section 12 assigns 3 to flick");
}

#[test]
fn note_records_are_sorted_by_numeric_time() {
    let bytes = compile(
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections { notes {
    tap { id: "later"; line: @main; gameplay.time: -1s; };
    tap { id: "earlier"; line: @main; gameplay.time: -2s; };
} }
"#,
    );
    let decoded = crate::load_chart(&bytes).expect("chart must load");
    let times: Vec<f64> = decoded.notes.iter().map(|note| note.time).collect();
    assert_eq!(times, vec![-2.0, -1.0]);
}

#[test]
fn write_from_compilation_round_trips_through_product_load() {
    let workspace = tempdir().unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();
    let bytes = write_from_compilation(&compilation).unwrap();
    let container = crate::load_container(&bytes).expect("compiled FCBC framing must load");
    assert_eq!(container.byte_length, bytes.len());
    assert!(container.sections.len() >= 14);
    assert_eq!(&bytes[..4], b"FCSB");
    let decoded = crate::load_chart(&bytes).expect("compiled FCBC Core chart must load");
    assert_eq!(
        decoded.lines.len(),
        compilation.chart().lines().lines().count()
    );
    assert_eq!(
        decoded.notes.len(),
        compilation.chart().notes().notes().len()
    );
}

#[test]
fn write_from_compilation_preserves_empty_chart_cardinality() {
    let bytes = compile(include_str!(
        "../../../docs/conformance/fcs5/source/valid/minimal-chart.fcs"
    ));
    let decoded = crate::load_chart(&bytes).expect("empty chart must load");

    assert!(decoded.lines.is_empty());
    assert!(decoded.notes.is_empty());
    assert!(decoded.distances.is_empty());
}

#[test]
fn write_from_compilation_preserves_exact_expression_dag() {
    let workspace = tempdir().unwrap();
    let document = parse_document(include_str!(
        "../../../docs/conformance/fcs5/source/valid/exact-expression-dag.fcs"
    ))
    .into_result()
    .expect("exact Expression DAG fixture must parse");
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .expect("exact Expression DAG fixture must compile");
    let table = compilation
        .chart()
        .descriptors()
        .expect("dynamic alpha must produce a descriptor table");
    let root = table
        .roots()
        .iter()
        .find(|root| root.target_path() == "note.presentation.alpha")
        .expect("dynamic alpha root");
    let CanonicalDescriptorKind::Expression(expression) =
        table.descriptor(root.descriptor()).unwrap().kind()
    else {
        panic!("dynamic alpha must remain an Expression DAG");
    };

    let bytes = write_from_compilation(&compilation).expect("native Expression DAG write");
    let decoded = crate::load_chart(&bytes).expect("native Expression DAG load");
    let note = decoded.notes.first().expect("exact-expression Note");
    assert!(matches!(
        &decoded.descriptors[note.property_descriptors[4] as usize].kind,
        crate::DescriptorKind::Expression(_)
    ));
    assert!(decoded.expressions.iter().any(|node| node.opcode == 70));
    assert!(decoded.expressions.iter().any(|node| node.opcode == 50));

    for distance in [50.0, 150.0] {
        let canonical = fcs_runtime::evaluate_expression(
            expression,
            fcs_runtime::ExpressionEnvironment::new(1.0, 0.0, 0.0, distance).unwrap(),
        )
        .unwrap();
        let fcs_model::CanonicalExpressionValue::Float(canonical) = canonical else {
            panic!("alpha expression must return float");
        };
        let encoded = crate::query_descriptor(
            &decoded,
            note.property_descriptors[4],
            1.0,
            crate::EvaluationEnvironment {
                s: 1.0,
                b: 0.0,
                q: 0.0,
                d: distance,
                p: 0.0,
            },
        )
        .expect("encoded alpha query")
        .value;
        assert_runtime_value_bits(
            encoded,
            crate::RuntimeValue::Scalar {
                ty: crate::ValueType::Float,
                value: canonical,
            },
        );
    }
}

#[test]
fn write_from_compilation_preserves_dynamic_color() {
    let workspace = tempdir().unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections {
    notes {
        tap {
            id: "color";
            line: @main;
            gameplay.time: 1s;
            presentation.color: choose {
                when d < 100px => #FF0000;
                else => #00FF0080;
            };
        };
    }
}
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .expect("dynamic color must compile");
    let bytes = write_from_compilation(&compilation).expect("dynamic color write");
    let decoded = crate::load_chart(&bytes).expect("dynamic color load");
    let note = decoded.notes.first().unwrap();

    for (distance, expected) in [
        (50.0, crate::RuntimeValue::Color([1.0, 0.0, 0.0, 1.0])),
        (
            150.0,
            crate::RuntimeValue::Color([0.0, 1.0, 0.0, 128.0 / 255.0]),
        ),
    ] {
        let actual = crate::query_descriptor(
            &decoded,
            note.property_descriptors[8],
            1.0,
            crate::EvaluationEnvironment {
                s: 1.0,
                b: 0.0,
                q: 0.0,
                d: distance,
                p: 0.0,
            },
        )
        .unwrap()
        .value;
        assert_runtime_value_bits(actual, expected);
    }
}

#[test]
fn expression_vec2_constants_use_element_payload_layout() {
    let int = canonical_expression_constant(&CanonicalExpressionValue::Vec2(
        Box::new(CanonicalExpressionValue::Int(-2)),
        Box::new(CanonicalExpressionValue::Int(3)),
    ))
    .unwrap();
    assert_eq!(int.payload.len(), 24);
    assert_eq!(&int.payload[8..16], &(-2_i64).to_le_bytes());
    assert_eq!(&int.payload[16..24], &3_i64.to_le_bytes());

    let beat = canonical_expression_constant(&CanonicalExpressionValue::Vec2(
        Box::new(CanonicalExpressionValue::ExactBeat(
            fcs_model::Beat::new(1, 3).unwrap(),
        )),
        Box::new(CanonicalExpressionValue::ExactBeat(
            fcs_model::Beat::new(2, 3).unwrap(),
        )),
    ))
    .unwrap();
    assert_eq!(beat.payload.len(), 40);
    assert_eq!(
        &beat.payload[8..24],
        &[1, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        &beat.payload[24..40],
        &[2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn write_from_compilation_preserves_note_presentation_and_texture() {
    let workspace = tempdir().unwrap();
    fs::write(workspace.path().join("cover.bin"), b"cover-bytes").unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
resources { image cover { source: "cover.bin"; mediaType: "image/png"; } }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections {
    notes {
        tap {
            id: "styled";
            line: @main;
            gameplay.time: 1s;
            presentation.positionX: 12px;
            presentation.scrollFactor: 0.5;
            presentation.xOffset: -2px;
            presentation.yOffset: 3px;
            presentation.alpha: 0.25;
            presentation.scaleX: 2.0;
            presentation.scaleY: 0.75;
            presentation.rotation: 90deg;
            presentation.color: #FF0000;
            presentation.texture: "cover";
            presentation.visibleFrom: 1beat;
            presentation.visibleUntil: 3beat;
            render.enabled: false;
        };
    }
}
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();
    let canonical_color = compilation.chart().notes().notes()[0]
        .presentation()
        .color();
    let bytes = write_from_compilation(&compilation).unwrap();
    let decoded = crate::load_chart(&bytes).expect("note presentation must load");
    let note = decoded.notes.first().expect("styled Note");
    let evaluate = |descriptor, time| {
        crate::query_descriptor(
            &decoded,
            descriptor,
            time,
            crate::EvaluationEnvironment::at_time(time),
        )
        .expect("Note property evaluation")
        .value
    };
    assert_eq!(note.flags, 0b1);
    assert_runtime_value_bits(
        evaluate(note.property_descriptors[0], 1.0),
        crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Length,
            value: 12.0,
        },
    );
    assert_runtime_value_bits(
        evaluate(note.property_descriptors[1], 1.0),
        crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Float,
            value: 0.5,
        },
    );
    assert_runtime_value_bits(
        evaluate(note.property_descriptors[2], 1.0),
        crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Length,
            value: -2.0,
        },
    );
    assert_runtime_value_bits(
        evaluate(note.property_descriptors[3], 1.0),
        crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Length,
            value: 3.0,
        },
    );
    assert_runtime_value_bits(
        evaluate(note.property_descriptors[4], 1.0),
        crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Float,
            value: 0.25,
        },
    );
    assert_runtime_value_bits(
        evaluate(note.property_descriptors[5], 1.0),
        crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Float,
            value: 2.0,
        },
    );
    assert_runtime_value_bits(
        evaluate(note.property_descriptors[6], 1.0),
        crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Float,
            value: 0.75,
        },
    );
    assert_runtime_value_bits(
        evaluate(note.property_descriptors[7], 1.0),
        crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Angle,
            value: std::f64::consts::FRAC_PI_2,
        },
    );
    assert_runtime_value_bits(
        evaluate(note.property_descriptors[8], 1.0),
        crate::RuntimeValue::Color([
            canonical_color.red(),
            canonical_color.green(),
            canonical_color.blue(),
            canonical_color.alpha(),
        ]),
    );
    assert_eq!(
        evaluate(note.property_descriptors[9], 0.25),
        crate::RuntimeValue::Bool(false)
    );
    assert_eq!(
        evaluate(note.property_descriptors[9], 1.0),
        crate::RuntimeValue::Bool(true)
    );
    assert_eq!(
        evaluate(note.property_descriptors[9], 2.0),
        crate::RuntimeValue::Bool(false)
    );
    assert_eq!(
        note.texture_resource_id,
        stable_id(b"fcs.resource", b"cover")
    );
}

#[test]
fn write_from_compilation_preserves_note_gameplay_and_extensions() {
    let workspace = tempdir().unwrap();
    fs::write(workspace.path().join("hit.bin"), b"exact hit sound").unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
resources {
    audio hit { source: "hit.bin"; mediaType: "audio/ogg"; }
}
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections {
    notes {
        tap {
            id: "rectangle";
            line: @main;
            gameplay.time: 1beat;
            gameplay.side: "below";
            gameplay.judgeShape.kind: "rectangle";
            gameplay.judgeShape.center: vec2(2px, 3px);
            gameplay.judgeShape.halfExtents: vec2(4px, 5px);
            gameplay.soundPolicy: "resource";
            gameplay.soundResource: "hit";
            gameplay.scorePolicy: "none";
        };
        hold {
            id: "circle-hold";
            line: @main;
            gameplay.time: 2beat;
            gameplay.endTime: 4beat;
            gameplay.judgeShape.kind: "circle";
            gameplay.judgeShape.radius: 6px;
            gameplay.soundPolicy: "none";
            gameplay.scorePolicy: "custom";
            gameplay.scoreExtension: "score.ext";
        };
    }
}
extensions {
    extension("score.ext", 1.2.3) required { "mode": "test", }
}
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();

    let bytes = write_from_compilation(&compilation).unwrap();
    let decoded = crate::load_chart(&bytes).expect("Note gameplay FCBC must load");
    assert_eq!(decoded.feature_flags & (1 << 2), 1 << 2);
    assert_eq!(
        decoded.extensions,
        vec![crate::ExtensionRecord {
            namespace: "score.ext".into(),
            version: (1, 2, 3),
            flags: 1,
        }]
    );

    let rectangle = &decoded.notes[0];
    assert_eq!(rectangle.kind, 1);
    assert_eq!(rectangle.side, 2);
    assert_eq!(
        rectangle.judge_shape,
        crate::DecodedJudgeShape::Rectangle {
            center: [2.0, 3.0],
            half_extents: [4.0, 5.0],
        }
    );
    assert_eq!(
        rectangle.sound_policy,
        crate::DecodedNoteSoundPolicy::Resource
    );
    assert_eq!(
        rectangle.sound_resource_id,
        stable_id(b"fcs.resource", b"hit")
    );
    assert_eq!(rectangle.score_policy, crate::DecodedNoteScorePolicy::None);

    let hold = &decoded.notes[1];
    assert_eq!(hold.kind, 2);
    assert_eq!(hold.flags & 0b100, 0b100);
    assert_eq!(hold.time, 1.0);
    assert_eq!(hold.end_time, 2.0);
    assert_eq!(
        hold.judge_shape,
        crate::DecodedJudgeShape::Circle {
            center: [0.0, 0.0],
            radius: 6.0,
        }
    );
    assert_eq!(hold.sound_policy, crate::DecodedNoteSoundPolicy::None);
    assert_eq!(
        hold.score_policy,
        crate::DecodedNoteScorePolicy::Custom("score.ext".into())
    );
}

#[test]
fn write_from_compilation_embeds_exact_resource_data() {
    let workspace = tempdir().unwrap();
    let payload = b"opaque\0resource\xffbytes";
    fs::write(workspace.path().join("payload.bin"), payload).unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
resources {
    binary blob { source: "payload.bin"; mediaType: "application/octet-stream"; }
}
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();

    let bytes = write_from_compilation(&compilation).unwrap();
    let decoded = crate::load_chart(&bytes).expect("compiled resources must load");
    let resource = decoded.resources.first().expect("embedded resource");
    assert_eq!(resource.id, stable_id(b"fcs.resource", b"blob"));
    assert_eq!(resource.kind, 7);
    assert_eq!(resource.media_type, "application/octet-stream");
    assert_eq!(resource.data_offset, 0);
    assert_eq!(resource.data_length, payload.len() as u64);
    let expected_sha256: [u8; 32] = Sha256::digest(payload).into();
    assert_eq!(resource.content_sha256, expected_sha256);
    assert_eq!(resource.bytes.as_ref(), payload);
}

#[test]
fn write_from_compilation_preserves_native_line_record_fields() {
    let workspace = tempdir().unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line root {}
    line child {
        parent: @root;
        floorScale: 96px;
        integrationOrigin: -2s;
        initialFloorPosition: 4.5;
        allowReverseScroll: true;
        zOrder: -3;
        inherit.position: false;
        inherit.rotation: true;
        inherit.scale: false;
        inherit.alpha: true;
        inherit.scroll: true;
    }
}
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();

    let bytes = write_from_compilation(&compilation).unwrap();
    let decoded = crate::load_chart(&bytes).expect("compiled Lines must load");
    let child_id = stable_id(b"fcs.line", b"child");
    let child = decoded
        .lines
        .iter()
        .find(|line| line.id == child_id)
        .expect("child Line");
    assert_eq!(child.parent_id, stable_id(b"fcs.line", b"root"));
    assert_eq!(child.document_order, 1);
    assert_eq!(child.z_order, -3);
    assert_eq!(child.inherit_flags, 0b1_1010);
    assert_eq!(child.line_flags, 1);
    assert_eq!(child.floor_scale, 96.0);
    assert_eq!(child.integration_origin, -2.0);
    assert_eq!(child.initial_floor_position, 4.5);
    assert_eq!(decoded.feature_flags & (1 << 8), 1 << 8);
}

#[test]
fn write_from_compilation_evaluates_exact_line_base_descriptors() {
    let workspace = tempdir().unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        position: vec2(3px, -4px);
        rotation: 90deg;
        scale: vec2(0.5, 2.0);
        alpha: 0.25;
        transformOrigin: vec2(1px, 2px);
        textureAnchor: vec2(0.25, 0.75);
    }
}
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();

    let bytes = write_from_compilation(&compilation).unwrap();
    let decoded = crate::load_chart(&bytes).expect("compiled Line descriptors must load");
    let line = decoded.lines.first().expect("main Line");
    let evaluate = |descriptor| {
        crate::query_descriptor(
            &decoded,
            descriptor,
            7.0,
            crate::EvaluationEnvironment::at_time(7.0),
        )
        .expect("Line descriptor evaluation")
        .value
    };
    assert_runtime_value_bits(
        evaluate(line.position_descriptor),
        crate::RuntimeValue::Vec2 {
            ty: crate::ValueType::Vec2Length,
            value: [3.0, -4.0],
        },
    );
    assert_runtime_value_bits(
        evaluate(line.rotation_descriptor),
        crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Angle,
            value: std::f64::consts::FRAC_PI_2,
        },
    );
    assert_runtime_value_bits(
        evaluate(line.scale_descriptor),
        crate::RuntimeValue::Vec2 {
            ty: crate::ValueType::Vec2Float,
            value: [0.5, 2.0],
        },
    );
    assert_runtime_value_bits(
        evaluate(line.alpha_descriptor),
        crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Float,
            value: 0.25,
        },
    );
    assert_runtime_value_bits(
        decoded.constants[line.transform_origin_constant as usize].clone(),
        crate::RuntimeValue::Vec2 {
            ty: crate::ValueType::Vec2Length,
            value: [1.0, 2.0],
        },
    );
    assert_runtime_value_bits(
        decoded.constants[line.texture_anchor_constant as usize].clone(),
        crate::RuntimeValue::Vec2 {
            ty: crate::ValueType::Vec2Float,
            value: [0.25, 0.75],
        },
    );
}

#[test]
fn write_from_compilation_evaluates_native_line_tracks() {
    let workspace = tempdir().unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        tracks {
            track move -> position: vec2<length> {
                fill: "error";
                extrapolateBefore: "holdBefore";
                extrapolateAfter: "holdAfter";
                segments { [0s, 2s): vec2(0px, 0px) -> vec2(2px, 4px) using "linear"; }
            }
            track turn -> rotation: angle {
                fill: "error";
                extrapolateBefore: "holdBefore";
                extrapolateAfter: "holdAfter";
                segments { [0s, 2s): 0deg -> 180deg using "linear"; }
            }
            track zoom -> scale: vec2<float> {
                fill: "error";
                extrapolateBefore: "holdBefore";
                extrapolateAfter: "holdAfter";
                segments { [0s, 2s): vec2(1.0, 1.0) -> vec2(3.0, 5.0) using "linear"; }
            }
            track fade -> alpha: float {
                fill: "error";
                extrapolateBefore: "holdBefore";
                extrapolateAfter: "holdAfter";
                segments { [0s, 2s): 0.0 -> 1.0 using "linear"; }
            }
        }
    }
}
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();

    let bytes = write_from_compilation(&compilation).unwrap();
    let decoded = crate::load_chart(&bytes).expect("compiled Line Tracks must load");
    let line = decoded.lines.first().expect("main Line");
    let evaluate = |descriptor| {
        crate::query_descriptor(
            &decoded,
            descriptor,
            1.0,
            crate::EvaluationEnvironment::at_time(1.0),
        )
        .expect("Line Track evaluation")
        .value
    };
    assert_runtime_value_bits(
        evaluate(line.position_descriptor),
        crate::RuntimeValue::Vec2 {
            ty: crate::ValueType::Vec2Length,
            value: [1.0, 2.0],
        },
    );
    assert_runtime_value_bits(
        evaluate(line.rotation_descriptor),
        crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Angle,
            value: std::f64::consts::FRAC_PI_2,
        },
    );
    assert_runtime_value_bits(
        evaluate(line.scale_descriptor),
        crate::RuntimeValue::Vec2 {
            ty: crate::ValueType::Vec2Float,
            value: [2.0, 3.0],
        },
    );
    assert_runtime_value_bits(
        evaluate(line.alpha_descriptor),
        crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Float,
            value: 0.5,
        },
    );
}

#[test]
fn write_from_compilation_couples_scroll_speed_track_and_distance() {
    let workspace = tempdir().unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        scrollTempoMap { 0s -> 60bpm; }
        tracks {
            track speed -> scrollSpeed: float {
                fill: "error";
                extrapolateBefore: "holdBefore";
                extrapolateAfter: "holdAfter";
                segments { [0s, 2s): 1.0 -> 3.0 using "linear"; }
            }
        }
    }
}
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();

    let bytes = write_from_compilation(&compilation).unwrap();
    let decoded = crate::load_chart(&bytes).expect("compiled scroll Track must load");
    let line = decoded.lines.first().expect("main Line");
    assert_runtime_value_bits(
        crate::query_descriptor(
            &decoded,
            line.scroll_speed_descriptor,
            1.0,
            crate::EvaluationEnvironment::at_time(1.0),
        )
        .expect("scroll speed evaluation")
        .value,
        crate::RuntimeValue::Scalar {
            ty: crate::ValueType::Float,
            value: 2.0,
        },
    );
    let distance = crate::query_distance(&decoded, line.distance_descriptor, 1.0)
        .expect("scroll distance evaluation");
    assert_eq!(
        distance.classification,
        crate::DistanceClassification::PortableEvaluable
    );
    assert_eq!(distance.floor_position, 1.5);
    assert_eq!(
        decoded.distances[line.distance_descriptor as usize].boundaries,
        [0.0, 2.0]
    );
}

#[test]
fn write_from_compilation_lowers_line_scroll_tempo_maps() {
    let workspace = tempdir().unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; 4beat -> 240bpm; }
lines {
    line explicit {
        scrollTempoMap { 0beat -> 60bpm; 4beat -> 90bpm; 4beat -> 120bpm; }
        tracks {
            track speed -> scrollSpeed: float {
                fill: "error";
                extrapolateBefore: "holdBefore";
                extrapolateAfter: "holdAfter";
                segments { [0s, 2s): 1.0 -> 3.0 using "linear"; }
            }
        }
    }
    line global {}
}
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();

    let bytes = write_from_compilation(&compilation).unwrap();
    let decoded = crate::load_chart(&bytes).expect("compiled scroll tempo maps must load");
    let explicit = decoded
        .lines
        .iter()
        .find(|line| line.id == stable_id(b"fcs.line", b"explicit"))
        .expect("explicit Line");
    let global = decoded
        .lines
        .iter()
        .find(|line| line.id == stable_id(b"fcs.line", b"global"))
        .expect("global Line");
    let bpm = |line: &crate::LineRecord, time| {
        crate::query_descriptor(
            &decoded,
            line.scroll_tempo_descriptor,
            time,
            crate::EvaluationEnvironment::at_time(time),
        )
        .expect("scroll tempo evaluation")
        .value
    };
    let scalar = |value| crate::RuntimeValue::Scalar {
        ty: crate::ValueType::Float,
        value,
    };

    assert_runtime_value_bits(bpm(explicit, 1.0), scalar(60.0));
    assert_runtime_value_bits(bpm(explicit, 2.0), scalar(120.0));
    assert_runtime_value_bits(bpm(global, 1.0), scalar(120.0));
    assert_runtime_value_bits(bpm(global, 2.0), scalar(240.0));
    assert_eq!(
        crate::query_scroll_coordinate(&decoded, explicit.scroll_tempo_descriptor, -1.0),
        Ok(-1.0)
    );
    assert_eq!(
        crate::query_scroll_coordinate(&decoded, explicit.scroll_tempo_descriptor, 3.0),
        Ok(4.0)
    );
    assert_eq!(
        crate::query_scroll_coordinate(&decoded, global.scroll_tempo_descriptor, 3.0),
        Ok(8.0)
    );
    for (line, expected_floor) in [(explicit, 10.0), (global, 8.0)] {
        let distance = crate::query_distance(&decoded, line.distance_descriptor, 3.0)
            .expect("scroll distance evaluation");
        assert_eq!(
            distance.classification,
            crate::DistanceClassification::PortableEvaluable
        );
        assert_eq!(distance.floor_position, expected_floor);
        assert_eq!(
            decoded.distances[line.distance_descriptor as usize].boundaries,
            [0.0, 2.0]
        );
    }
}

#[test]
fn write_from_compilation_evaluates_native_linear_alpha_track() {
    let workspace = tempdir().unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        alpha: 0.25;
        tracks {
            track fade -> alpha: float {
                fill: "error";
                extrapolateBefore: "holdBefore";
                extrapolateAfter: "holdAfter";
                segments { [0s, 2s): 1.0 -> 0.0 using "linear"; }
            }
        }
    }
}
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();

    let bytes = write_from_compilation(&compilation).unwrap();
    let decoded = crate::load_chart(&bytes).expect("compiled alpha Track must load");
    let descriptor = decoded.lines.first().expect("main Line").alpha_descriptor;
    let evaluate = |time| {
        crate::query_descriptor(
            &decoded,
            descriptor,
            time,
            crate::EvaluationEnvironment::at_time(time),
        )
        .expect("alpha Track evaluation")
        .value
    };
    let alpha = |value| crate::RuntimeValue::Scalar {
        ty: crate::ValueType::Float,
        value,
    };
    assert_runtime_value_bits(evaluate(-1.0), alpha(1.0));
    assert_runtime_value_bits(evaluate(1.0), alpha(0.5));
    assert_runtime_value_bits(evaluate(3.0), alpha(0.0));
}

#[test]
fn write_from_compilation_materializes_track_fill_and_extrapolation() {
    let workspace = tempdir().unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        alpha: 0.25;
        tracks {
            track fade -> alpha: float {
                fill: "zero";
                extrapolateBefore: "base";
                extrapolateAfter: "one";
                segments {
                    [0s, 1s): 0.5 -> 0.75 using "linear";
                    [2s, 3s): 0.25 -> 0.5 using "linear";
                }
            }
        }
    }
}
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();

    let bytes = write_from_compilation(&compilation).unwrap();
    let decoded = crate::load_chart(&bytes).expect("filled alpha Track must load");
    let descriptor = decoded.lines.first().expect("main Line").alpha_descriptor;
    let crate::DescriptorKind::Piecewise(pieces) = &decoded.descriptors[descriptor as usize].kind
    else {
        panic!("filled Track root must preserve its Piecewise boundary");
    };
    assert_eq!(pieces.len(), 2);
    assert!(matches!(
        decoded.descriptors[pieces[0].descriptor_index as usize].kind,
        crate::DescriptorKind::Constant(_)
    ));
    assert!(matches!(
        decoded.descriptors[pieces[1].descriptor_index as usize].kind,
        crate::DescriptorKind::SegmentTrack(_)
    ));
    let evaluate = |time| {
        crate::query_descriptor(
            &decoded,
            descriptor,
            time,
            crate::EvaluationEnvironment::at_time(time),
        )
        .expect("filled alpha Track evaluation")
        .value
    };
    let alpha = |value| crate::RuntimeValue::Scalar {
        ty: crate::ValueType::Float,
        value,
    };
    assert_runtime_value_bits(evaluate(-1.0), alpha(0.25));
    assert_runtime_value_bits(evaluate(0.5), alpha(0.625));
    assert_runtime_value_bits(evaluate(1.5), alpha(0.0));
    assert_runtime_value_bits(evaluate(2.5), alpha(0.375));
    assert_runtime_value_bits(evaluate(4.0), alpha(1.0));
}

#[test]
fn write_from_compilation_materializes_track_hold_policies() {
    let workspace = tempdir().unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        tracks {
            track fade -> alpha: float {
                fill: "holdBefore";
                extrapolateBefore: "holdBefore";
                extrapolateAfter: "holdAfter";
                segments {
                    [0s, 1s): 0.0 -> 1.0 using "linear";
                    [2s, 3s): 0.25 -> 0.5 using "linear";
                }
            }
        }
    }
}
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();

    let bytes = write_from_compilation(&compilation).unwrap();
    let decoded = crate::load_chart(&bytes).expect("held alpha Track must load");
    let descriptor = decoded.lines.first().expect("main Line").alpha_descriptor;
    let evaluate = |time| {
        crate::query_descriptor(
            &decoded,
            descriptor,
            time,
            crate::EvaluationEnvironment::at_time(time),
        )
        .expect("held alpha Track evaluation")
        .value
    };
    let alpha = |value| crate::RuntimeValue::Scalar {
        ty: crate::ValueType::Float,
        value,
    };
    assert_runtime_value_bits(evaluate(-1.0), alpha(0.0));
    assert_runtime_value_bits(evaluate(0.5), alpha(0.5));
    assert_runtime_value_bits(evaluate(1.5), alpha(0.25));
    assert_runtime_value_bits(evaluate(2.5), alpha(0.375));
    assert_runtime_value_bits(evaluate(4.0), alpha(0.5));
}

#[test]
fn write_from_compilation_merges_disjoint_replace_tracks() {
    let workspace = tempdir().unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        alpha: 0.5;
        tracks {
            track early -> alpha: float {
                segments { [0s, 1s): 0.25 -> 0.5 using "linear"; }
            }
            track late -> alpha: float {
                segments { [2s, 3s): 0.5 -> 0.75 using "linear"; }
            }
        }
    }
}
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();

    let bytes = write_from_compilation(&compilation).unwrap();
    let decoded = crate::load_chart(&bytes).expect("disjoint replace Tracks must load");
    let descriptor = decoded.lines.first().expect("main Line").alpha_descriptor;
    let evaluate = |time| {
        crate::query_descriptor(
            &decoded,
            descriptor,
            time,
            crate::EvaluationEnvironment::at_time(time),
        )
        .expect("disjoint replace Track evaluation")
        .value
    };
    let alpha = |value| crate::RuntimeValue::Scalar {
        ty: crate::ValueType::Float,
        value,
    };
    assert_runtime_value_bits(evaluate(-1.0), alpha(0.5));
    assert_runtime_value_bits(evaluate(0.5), alpha(0.375));
    assert_runtime_value_bits(evaluate(1.5), alpha(0.5));
    assert_runtime_value_bits(evaluate(2.5), alpha(0.625));
    assert_runtime_value_bits(evaluate(4.0), alpha(0.5));
}

#[test]
fn write_from_compilation_evaluates_native_alpha_track_points() {
    let workspace = tempdir().unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        tracks {
            track fade -> alpha: float {
                fill: "error";
                extrapolateBefore: "holdBefore";
                extrapolateAfter: "holdAfter";
                segments {
                    point 0s: 1.0;
                    [1s, 3s): 0.8 -> 0.0 using "linear";
                    point 3s: 0.0;
                }
            }
        }
    }
}
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();

    let bytes = write_from_compilation(&compilation).unwrap();
    let decoded = crate::load_chart(&bytes).expect("compiled alpha Track points must load");
    let descriptor = decoded.lines.first().expect("main Line").alpha_descriptor;
    let evaluate = |time| {
        crate::query_descriptor(
            &decoded,
            descriptor,
            time,
            crate::EvaluationEnvironment::at_time(time),
        )
        .expect("alpha Track point evaluation")
        .value
    };
    let alpha = |value| crate::RuntimeValue::Scalar {
        ty: crate::ValueType::Float,
        value,
    };
    assert_runtime_value_bits(evaluate(-1.0), alpha(0.8));
    assert_runtime_value_bits(evaluate(0.5), alpha(1.0));
    assert_runtime_value_bits(evaluate(2.0), alpha(0.4));
    assert_runtime_value_bits(evaluate(4.0), alpha(0.0));
}

#[test]
fn write_from_compilation_evaluates_native_alpha_easing_and_bezier() {
    let workspace = tempdir().unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        tracks {
            track fade -> alpha: float {
                fill: "error";
                extrapolateBefore: "holdBefore";
                extrapolateAfter: "holdAfter";
                segments {
                    [0s, 2s): 0.0 -> 1.0 using "easeInQuad";
                    [2s, 4s): 1.0 -> 0.0 using cubicBezier(0.0, 0.0, 1.0, 1.0);
                }
            }
        }
    }
}
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();

    let bytes = write_from_compilation(&compilation).unwrap();
    let decoded = crate::load_chart(&bytes).expect("compiled alpha easing Track must load");
    let descriptor = decoded.lines.first().expect("main Line").alpha_descriptor;
    let evaluate = |time| {
        crate::query_descriptor(
            &decoded,
            descriptor,
            time,
            crate::EvaluationEnvironment::at_time(time),
        )
        .expect("alpha easing Track evaluation")
        .value
    };
    let alpha = |value| crate::RuntimeValue::Scalar {
        ty: crate::ValueType::Float,
        value,
    };
    assert_runtime_value_bits(evaluate(1.0), alpha(0.25));
    assert_runtime_value_bits(evaluate(3.0), alpha(0.5));
}

#[test]
fn write_from_compilation_preserves_all_native_alpha_easing_ids() {
    for easing in EasingId::ALL {
        let workspace = tempdir().unwrap();
        let source = format!(
            r#"#fcs 5.0.0
format {{ profile: chart; }}
tempoMap {{ 0beat -> 120bpm; }}
lines {{
    line main {{
        tracks {{
            track fade -> alpha: float {{
                fill: "error";
                extrapolateBefore: "holdBefore";
                extrapolateAfter: "holdAfter";
                segments {{ [0s, 2s): 0.0 -> 1.0 using "{}"; }}
            }}
        }}
    }}
}}
"#,
            easing.name()
        );
        let document = parse_document(&source).into_result().unwrap();
        let compilation = document
            .canonical_compilation(
                CompileTimeLimits::default(),
                workspace.path(),
                ResourceLimits::default(),
            )
            .unwrap();

        let bytes = write_from_compilation(&compilation).unwrap();
        let decoded = crate::load_chart(&bytes).expect("compiled alpha easing Track must load");
        let descriptor = decoded.lines.first().expect("main Line").alpha_descriptor;
        let actual = crate::query_descriptor(
            &decoded,
            descriptor,
            1.0,
            crate::EvaluationEnvironment::at_time(1.0),
        )
        .expect("alpha easing Track evaluation")
        .value;
        // The shared product runtime proves serialized ID preservation, not independent math.
        assert_runtime_value_bits(
            actual,
            crate::RuntimeValue::Scalar {
                ty: crate::ValueType::Float,
                value: easing.evaluate(0.5).unwrap(),
            },
        );
    }
}
