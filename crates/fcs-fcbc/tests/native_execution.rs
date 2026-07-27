//! End-to-end Execution ABI oracle over natively written FCBC bytes (Issue #314).
//!
//! Every byte sequence here runs the full product pipeline — real FCS source
//! text through `canonical_compilation`, `write_from_compilation`, and
//! `load_chart` — and the queries assert semantic values derived by hand from
//! the source (120 bpm is 0.5 s per beat, the default scroll speed is 1.0),
//! not values echoed back by the code under test. The load itself is also
//! evidence: the loader revalidates the section 10 tempo consistency against
//! the Core mapping on every native container used here.

#[path = "../../fcs-source/tests/support/fcbc_reference_evaluator.rs"]
mod fcbc_reference_evaluator;
#[path = "../../fcs-source/tests/support/fcbc_reference_loader.rs"]
mod fcbc_reference_loader;

use fcs_fcbc::{
    DistanceClassification, EvaluationEnvironment, RuntimeValue, ValueType, load_chart,
    query_descriptor, query_distance, query_scroll_coordinate, write_from_compilation,
};
use fcs_source::ResourceLimits;
use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;
use tempfile::tempdir;

/// Compiles `source` and returns the product FCBC bytes.
fn compile(source: &str) -> Vec<u8> {
    let workspace = tempdir().unwrap();
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();
    write_from_compilation(&compilation).unwrap()
}

fn float(value: f64) -> RuntimeValue {
    RuntimeValue::Scalar {
        ty: ValueType::Float,
        value,
    }
}

fn evaluate(chart: &fcs_fcbc::DecodedChart, descriptor: u32, time: f64) -> RuntimeValue {
    query_descriptor(
        chart,
        descriptor,
        time,
        EvaluationEnvironment::at_time(time),
    )
    .expect("descriptor query")
    .value
}

#[test]
fn a_native_hold_is_executable_through_the_abi() {
    // 120 bpm is exactly 0.5 s per beat, so 2beat/4beat are the exact chart
    // times 1.0 s and 2.0 s, and the visible window [1beat, 3beat) is
    // [0.5 s, 1.5 s).
    let bytes = compile(
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections {
    notes {
        hold {
            id: "hold";
            line: @main;
            gameplay.time: 2beat;
            gameplay.endTime: 4beat;
            presentation.alpha: 0.25;
            presentation.visibleFrom: 1beat;
            presentation.visibleUntil: 3beat;
        };
    }
}
"#,
    );
    let decoded = load_chart(&bytes).expect("native Hold chart must load");
    let hold = decoded.notes.first().expect("Hold note");
    assert_eq!(hold.kind, 2, "section 12 assigns 2 to hold");
    assert_eq!(hold.flags & 0b100, 0b100, "hold must carry hasEndTime");
    assert_eq!(hold.time, 1.0);
    assert_eq!(hold.end_time, 2.0);

    // Property queries at the head and the tail of the hold.
    for time in [1.0, 2.0] {
        assert_eq!(
            evaluate(&decoded, hold.property_descriptors[4], time),
            float(0.25)
        );
    }
    // The visibility window is evaluated, not stored: before, inside, after.
    for (time, visible) in [(0.25, false), (1.0, true), (1.75, false)] {
        assert_eq!(
            evaluate(&decoded, hold.property_descriptors[9], time),
            RuntimeValue::Bool(visible),
            "visibility at {time}"
        );
    }

    // Distance execution across the hold: the default scroll speed is 1.0 and
    // the scroll tempo is the constant global 120 bpm — an analytic distance —
    // so the floor position is 120 / 60 * t = 2t. The head-to-tail travel pins
    // the hold length in scroll space through `query_distance`, not through
    // the record fields.
    let line = decoded.lines.first().expect("main Line");
    let head = query_distance(&decoded, line.distance_descriptor, hold.time)
        .expect("distance at hold head");
    let tail = query_distance(&decoded, line.distance_descriptor, hold.end_time)
        .expect("distance at hold tail");
    assert_eq!(
        head.classification,
        DistanceClassification::PortableAnalytic
    );
    assert_eq!(
        tail.classification,
        DistanceClassification::PortableAnalytic
    );
    assert_eq!(head.floor_position, 2.0);
    assert_eq!(tail.floor_position, 4.0);
    assert_eq!(tail.floor_position - head.floor_position, 2.0);

    // Run the same compilation-derived bytes through the independent test oracle.
    let reference = fcbc_reference_loader::load(&bytes).expect("independent native FCBC load");
    let reference_line = reference.lines.first().expect("independent main Line");
    let reference_hold = reference.notes.first().expect("independent Hold note");
    assert_eq!(
        fcbc_reference_evaluator::query_descriptor(
            &reference,
            reference_hold.property_descriptors[4],
            1.0,
            fcbc_reference_evaluator::EvaluationEnvironment::at_time(1.0),
        )
        .expect("independent Hold alpha query")
        .value,
        fcbc_reference_loader::RuntimeValue::Scalar {
            ty: fcbc_reference_loader::ValueType::Float,
            value: 0.25,
        }
    );
    assert_eq!(
        fcbc_reference_evaluator::query_scroll_coordinate(
            &reference,
            reference_line.scroll_tempo_descriptor,
            reference_hold.time,
        )
        .expect("independent Hold-head scroll coordinate"),
        2.0
    );
    assert_eq!(
        fcbc_reference_evaluator::query_distance(
            &reference,
            reference_line.distance_descriptor,
            reference_hold.end_time,
        )
        .expect("independent Hold tail distance")
        .floor_position,
        4.0
    );
}

#[test]
fn a_native_sub_beat_tempo_map_survives_revalidation_and_executes() {
    // 0.5beat and 1.5beat reduce to the exact rationals 1/2 and 3/2. By hand:
    // chartTime(1/2) = 0.5 * 60/120 = 0.25 s, and
    // chartTime(3/2) = 0.25 + 1.0 * 60/180 = 0.25 + 1/3 s. `load_chart`
    // recomputes both from the Core mapping and rejects beyond 2 ULP, so the
    // successful load is itself section 10 evidence for native bytes.
    let bytes = compile(
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; 0.5beat -> 180bpm; 1.5beat -> 240bpm; }
lines { line main {} }
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1beat; }; } }
"#,
    );
    let decoded = load_chart(&bytes).expect("sub-beat tempo map must survive revalidation");
    let beats: Vec<(i64, i64)> = decoded
        .tempo_points
        .iter()
        .map(|point| (point.beat_numerator, point.beat_denominator))
        .collect();
    assert_eq!(beats, vec![(0, 1), (1, 2), (3, 2)]);
    let bpm: Vec<f64> = decoded.tempo_points.iter().map(|point| point.bpm).collect();
    assert_eq!(bpm, vec![120.0, 180.0, 240.0]);
    assert_eq!(decoded.tempo_points[0].chart_time, 0.0);
    assert_eq!(decoded.tempo_points[1].chart_time, 0.25);
    let third_time = decoded.tempo_points[2].chart_time;
    assert!((third_time - (0.25 + 1.0 / 3.0)).abs() <= 1e-12);

    // The tap at 1beat sits mid-segment: 0.25 + 0.5 * 60/180 = 0.25 + 1/6 s.
    let tap = decoded.notes.first().expect("tap note");
    assert!((tap.time - (0.25 + 1.0 / 6.0)).abs() <= 1e-12);

    // Execute the tempo descriptor: bpm per segment, and the scroll coordinate
    // (integrated beats) at the exact segment boundaries.
    let line = decoded.lines.first().expect("main Line");
    assert_eq!(
        evaluate(&decoded, line.scroll_tempo_descriptor, 0.1),
        float(120.0)
    );
    assert_eq!(
        evaluate(&decoded, line.scroll_tempo_descriptor, 0.3),
        float(180.0)
    );
    assert_eq!(
        evaluate(&decoded, line.scroll_tempo_descriptor, 1.0),
        float(240.0)
    );
    let half_beat = query_scroll_coordinate(&decoded, line.scroll_tempo_descriptor, 0.25)
        .expect("scroll coordinate at 1/2 beat");
    assert!((half_beat - 0.5).abs() <= 1e-12);
    let three_half_beat =
        query_scroll_coordinate(&decoded, line.scroll_tempo_descriptor, third_time)
            .expect("scroll coordinate at 3/2 beat");
    assert!((three_half_beat - 1.5).abs() <= 1e-9);

    // Distance at the first sub-beat boundary: speed 1.0, so the floor equals
    // the integrated beats, 0.5.
    let distance = query_distance(&decoded, line.distance_descriptor, 0.25)
        .expect("distance at the 1/2-beat boundary");
    assert_eq!(
        distance.classification,
        DistanceClassification::PortableEvaluable
    );
    assert!((distance.floor_position - 0.5).abs() <= 1e-12);
}

#[test]
fn deterministic_random_tempo_maps_round_trip_through_product_load() {
    fn next(seed: &mut u64) -> u64 {
        *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        *seed
    }

    let mut seed = 0xF0C5_0340_u64;
    for _ in 0..12 {
        let first_milli = 100 + next(&mut seed) % 900_000;
        let second_milli = first_milli + 1 + next(&mut seed) % 900_000;
        let first_bpm = 240.0 + (next(&mut seed) % 960) as f64;
        let second_bpm = 30.0 + (next(&mut seed) % 240) as f64;
        let third_bpm = 60.0 + (next(&mut seed) % 600) as f64;
        let source = format!(
            r#"#fcs 5.0.0
format {{ profile: chart; }}
tempoMap {{ 0beat -> {first_bpm}bpm; {first_whole}.{first_fraction:03}beat -> {second_bpm}bpm; {second_whole}.{second_fraction:03}beat -> {third_bpm}bpm; }}
lines {{ line main {{}} }}
collections {{ notes {{ tap {{ id: "tap"; line: @main; gameplay.time: 1s; }}; }} }}
"#,
            first_whole = first_milli / 1000,
            first_fraction = first_milli % 1000,
            second_whole = second_milli / 1000,
            second_fraction = second_milli % 1000,
        );
        let decoded = load_chart(&compile(&source)).expect("generated tempo map must load");
        assert_eq!(decoded.tempo_points.len(), 3);
    }
}
