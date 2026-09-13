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
use fcs_model::{
    CanonicalCompilation, CanonicalDescriptorKind, CanonicalExpressionValue, CanonicalTrackTarget,
    CanonicalTrackValue, CanonicalVec2,
};
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
    write_from_compilation(&compilation(source)).unwrap()
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

/// A minimal tap Note carrying one dynamic presentation field.
fn tap_source(presentation: &str) -> String {
    format!(
        r#"#fcs 5.0.0
format {{ profile: chart; }}
tempoMap {{ 0beat -> 120bpm; }}
lines {{ line main {{}} }}
collections {{ notes {{ tap {{
    id: "n";
    line: @main;
    gameplay.time: 1s;
    {presentation}
}}; }} }}
"#
    )
}

fn assert_native_matches_canonical(native: RuntimeValue, canonical: CanonicalExpressionValue) {
    match canonical {
        CanonicalExpressionValue::Float(value) => assert_eq!(native, float(value)),
        CanonicalExpressionValue::Time(value) => assert_eq!(
            native,
            RuntimeValue::Scalar {
                ty: ValueType::Time,
                value,
            }
        ),
        CanonicalExpressionValue::Length(value) => assert_eq!(
            native,
            RuntimeValue::Scalar {
                ty: ValueType::Length,
                value,
            }
        ),
        CanonicalExpressionValue::Angle(value) => assert_eq!(
            native,
            RuntimeValue::Scalar {
                ty: ValueType::Angle,
                value,
            }
        ),
        CanonicalExpressionValue::Color(value) => assert_eq!(native, RuntimeValue::Color(value)),
        other => panic!("unexpected canonical comparison value {other:?}"),
    }
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

#[test]
fn native_unit_integer_scaling_executes() {
    // Issue #646: `U,int` / `int,U` multiplication and `U,int` division must
    // execute after a native round trip. At chart time 0.25 s the time cases
    // are hand-derived; the length cases use note distance 1.5 px.
    for (expression, expected) in [
        ("seconds(s * 2)", 0.5),
        ("seconds(2 * s)", 0.5),
        ("seconds(s / 2)", 0.125),
    ] {
        let source = tap_source(&format!("presentation.alpha: {expression};"));
        let decoded = load_chart(&compile(&source)).unwrap();
        assert_eq!(
            evaluate(&decoded, decoded.notes[0].property_descriptors[4], 0.25),
            float(expected),
            "{expression}"
        );
    }
    for (expression, expected) in [
        ("d * 2", 3.0),
        ("2 * d", 3.0),
        ("d / 2", 0.75),
        ("d * -3", -4.5),
    ] {
        let source = tap_source(&format!("presentation.xOffset: {expression};"));
        let decoded = load_chart(&compile(&source)).unwrap();
        let environment = EvaluationEnvironment {
            s: 0.0,
            b: 0.0,
            q: 0.0,
            d: 1.5,
            p: 0.0,
        };
        assert_eq!(
            query_descriptor(
                &decoded,
                decoded.notes[0].property_descriptors[2],
                environment.s,
                environment
            )
            .unwrap()
            .value,
            RuntimeValue::Scalar {
                ty: ValueType::Length,
                value: expected,
            },
            "{expression}"
        );
    }
}

#[test]
fn native_bezier_preserves_exact_start_value() {
    // Issue #647: the segment start must evaluate to exactly the start
    // constant. For these controls every bisection midpoint has positive x,
    // so the old approximate solver returned y(2^-65) = 3 * 2^-65 instead of
    // exactly +0.0.
    let bytes = compile(
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines { line main {
    tracks { track fade -> alpha: float {
        extrapolateBefore: "holdBefore";
        extrapolateAfter: "holdAfter";
        segments {
            [0s, 1s): 0.0 -> 1.0 using cubicBezier(0.0, 1.0, 1.0, 1.0);
        }
    } }
} }
"#,
    );
    let decoded = load_chart(&bytes).unwrap();
    let descriptor = decoded.lines[0].alpha_descriptor;
    assert_eq!(evaluate(&decoded, descriptor, 0.0), float(0.0));
}

#[test]
fn native_bezier_interior_and_overshoot_match_canonical_vectors() {
    // Nontrivial interior and flat-x overshoot controls through native
    // write -> load -> query. Segment domains are half-open, so progress 1.0
    // is unreachable through a Track query; endpoint pinning and the
    // explicit enclosure failures are bound by the unit test over the same
    // shared solver. With start 0.0 and end 1.0 the alpha equals the y
    // progress exactly, and 1.625 is the value the canonical evaluator
    // independently established for cubicBezier(0.5, 2.0, 0.5, 2.0) at
    // x = 0.5.
    let bytes = compile(
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines { line main {
    tracks { track fade -> alpha: float {
        extrapolateBefore: "holdBefore";
        extrapolateAfter: "holdAfter";
        segments {
            [0s, 1s): 0.0 -> 1.0 using cubicBezier(0.0, 0.0, 1.0, 1.0);
            [1s, 3s): 0.0 -> 1.0 using cubicBezier(0.5, 2.0, 0.5, 2.0);
        }
    } }
} }
"#,
    );
    let decoded = load_chart(&bytes).unwrap();
    let descriptor = decoded.lines[0].alpha_descriptor;
    assert_eq!(evaluate(&decoded, descriptor, 0.25), float(0.25));
    assert_eq!(evaluate(&decoded, descriptor, 2.0), float(1.625));
}

#[test]
fn native_integer_vector_predicates_preserve_values() {
    // Issue #648: `vec2<int>` values survive a native write -> load -> query
    // round trip in exact i64 form. Binary64 storage collided 2^53 + 1 with
    // 2^53 (making the first predicate true instead of false) and lost the
    // truncating integer division (making the second false instead of true).
    // Alpha is float, so each predicate drives a `choose`. The canonical
    // evaluator already keeps i64 components, so it must agree with the
    // native result on the same compiled DAG.
    for (predicate, expected) in [
        (
            "choose { when vec2(choose { when s >= 0s => 9007199254740993; else => 0; }, 0) \
             == vec2(9007199254740992, 0) => 1.0; else => 0.0; }",
            0.0,
        ),
        (
            "choose { when (vec2(choose { when s >= 0s => 5; else => 1; }, 7) / 2) \
             == vec2(2, 3) => 1.0; else => 0.0; }",
            1.0,
        ),
    ] {
        let compilation = compilation(&tap_source(&format!("presentation.alpha: {predicate};")));
        let decoded = load_chart(&write_from_compilation(&compilation).unwrap()).unwrap();
        let native = evaluate(&decoded, decoded.notes[0].property_descriptors[4], 0.25);
        assert_eq!(native, float(expected), "{predicate}");

        let table = compilation
            .chart()
            .descriptors()
            .expect("dynamic presentation must produce a descriptor table");
        let root = table
            .roots()
            .iter()
            .find(|root| root.target_path() == "note.presentation.alpha")
            .expect("root for note.presentation.alpha");
        let CanonicalDescriptorKind::Expression(expression) =
            table.descriptor(root.descriptor()).unwrap().kind()
        else {
            panic!("note.presentation.alpha must stay an expression DAG");
        };
        let canonical = fcs_runtime::evaluate_expression(
            expression,
            fcs_runtime::ExpressionEnvironment::new(0.25, 0.5, 0.0, 0.0).unwrap(),
        )
        .unwrap();
        assert_native_matches_canonical(native, canonical);
    }

    // `choose` stays lazy for integer vectors: the unselected branch would
    // overflow i64 and must not be evaluated. The overflow subexpression
    // depends on `s`, so the lowerer retains it instead of folding it away.
    // Runtime expressions have no vector projection syntax yet, so the
    // selected branch is observed through vector equality.
    let lazy = compile(&tap_source(
        "presentation.alpha: choose { \
         when choose { when s < 0s => vec2(9223372036854775807, 0) \
         + vec2(choose { when s >= 0s => 1; else => 0; }, 0); \
         else => vec2(0, 0); } == vec2(0, 0) => 0.0; \
         else => 1.0; };",
    ));
    let decoded = load_chart(&lazy).unwrap();
    assert_eq!(
        evaluate(&decoded, decoded.notes[0].property_descriptors[4], 0.25),
        float(0.0)
    );
}

#[test]
fn native_unit_integer_scaling_matches_canonical_evaluator() {
    // Every permitted U,int / int,U combination across time, beat, length,
    // and angle must agree with the canonical evaluator on the same compiled
    // DAG. Beat has no note slot and no conversion builtin, so it drives a
    // color `choose` predicate; angle drives a `rotation` choose branch
    // (pure-literal subexpressions are retained by the lowerer, not folded).
    #[derive(Clone, Copy)]
    struct ScalingCase<'a> {
        presentation: &'a str,
        slot: usize,
        target_path: &'a str,
        environments: &'a [(f64, f64, f64)],
    }
    let cases = [
        ScalingCase {
            presentation: "presentation.alpha: seconds(s * 2);",
            slot: 4,
            target_path: "note.presentation.alpha",
            environments: &[(0.25, 0.0, 0.0)],
        },
        ScalingCase {
            presentation: "presentation.alpha: seconds(2 * s);",
            slot: 4,
            target_path: "note.presentation.alpha",
            environments: &[(0.25, 0.0, 0.0)],
        },
        ScalingCase {
            presentation: "presentation.alpha: seconds(s / 2);",
            slot: 4,
            target_path: "note.presentation.alpha",
            environments: &[(0.25, 0.0, 0.0)],
        },
        ScalingCase {
            presentation: "presentation.xOffset: d * 2;",
            slot: 2,
            target_path: "note.presentation.xOffset",
            environments: &[(0.0, 0.0, 1.5)],
        },
        ScalingCase {
            presentation: "presentation.xOffset: 2 * d;",
            slot: 2,
            target_path: "note.presentation.xOffset",
            environments: &[(0.0, 0.0, 1.5)],
        },
        ScalingCase {
            presentation: "presentation.xOffset: d / 2;",
            slot: 2,
            target_path: "note.presentation.xOffset",
            environments: &[(0.0, 0.0, 1.5)],
        },
        ScalingCase {
            presentation: "presentation.color: choose { when b * 2 > 1beat => #FF0000; else => #00FF00; };",
            slot: 8,
            target_path: "note.presentation.color",
            environments: &[(0.0, 0.75, 0.0), (0.0, 0.49, 0.0)],
        },
        ScalingCase {
            presentation: "presentation.color: choose { when 2 * b > 1beat => #FF0000; else => #00FF00; };",
            slot: 8,
            target_path: "note.presentation.color",
            environments: &[(0.0, 0.51, 0.0), (0.0, 0.49, 0.0)],
        },
        ScalingCase {
            presentation: "presentation.color: choose { when b / 2 > 1beat => #FF0000; else => #00FF00; };",
            slot: 8,
            target_path: "note.presentation.color",
            environments: &[(0.0, 2.5, 0.0), (0.0, 1.9, 0.0)],
        },
        ScalingCase {
            presentation: "presentation.rotation: choose { when s > 1s => 30deg * 2; else => 90deg; };",
            slot: 7,
            target_path: "note.presentation.rotation",
            environments: &[(2.0, 0.0, 0.0), (0.5, 0.0, 0.0)],
        },
        ScalingCase {
            presentation: "presentation.rotation: choose { when s > 1s => 2 * 30deg; else => 90deg; };",
            slot: 7,
            target_path: "note.presentation.rotation",
            environments: &[(2.0, 0.0, 0.0)],
        },
        ScalingCase {
            presentation: "presentation.rotation: choose { when s > 1s => 30deg / 2; else => 90deg; };",
            slot: 7,
            target_path: "note.presentation.rotation",
            environments: &[(2.0, 0.0, 0.0)],
        },
    ];
    for &ScalingCase {
        presentation,
        slot,
        target_path,
        environments,
    } in &cases
    {
        let compilation = compilation(&tap_source(presentation));
        let decoded = load_chart(&write_from_compilation(&compilation).unwrap()).unwrap();
        let table = compilation
            .chart()
            .descriptors()
            .expect("dynamic presentation must produce a descriptor table");
        let root = table
            .roots()
            .iter()
            .find(|root| root.target_path() == target_path)
            .unwrap_or_else(|| panic!("root for {target_path}"));
        let CanonicalDescriptorKind::Expression(expression) =
            table.descriptor(root.descriptor()).unwrap().kind()
        else {
            panic!("{target_path} must stay an expression DAG");
        };
        for &(s, b, d) in environments {
            let environment = EvaluationEnvironment {
                s,
                b,
                q: 0.0,
                d,
                p: 0.0,
            };
            let native = query_descriptor(
                &decoded,
                decoded.notes[0].property_descriptors[slot],
                s,
                environment,
            )
            .unwrap()
            .value;
            let canonical = fcs_runtime::evaluate_expression(
                expression,
                fcs_runtime::ExpressionEnvironment::new(s, b, 0.0, d).unwrap(),
            )
            .unwrap();
            assert_native_matches_canonical(native, canonical);
        }
    }

    // Hand-derived absolutes for the units the float/length slots cannot
    // reach: beat at 0.75/0.49 straddles the 1beat boundary through `b * 2`,
    // and `30deg * 2` doubles the lexer's `30deg` payload (radians).
    let decoded = load_chart(&compile(&tap_source(
        "presentation.color: choose { when b * 2 > 1beat => #FF0000; else => #00FF00; };",
    )))
    .unwrap();
    for (b, expected) in [(0.75, [1.0, 0.0, 0.0, 1.0]), (0.49, [0.0, 1.0, 0.0, 1.0])] {
        let environment = EvaluationEnvironment {
            s: 0.0,
            b,
            q: 0.0,
            d: 0.0,
            p: 0.0,
        };
        assert_eq!(
            query_descriptor(
                &decoded,
                decoded.notes[0].property_descriptors[8],
                environment.s,
                environment
            )
            .unwrap()
            .value,
            RuntimeValue::Color(expected),
            "beat scaling at b = {b}"
        );
    }
    let decoded = load_chart(&compile(&tap_source(
        "presentation.rotation: choose { when s > 1s => 30deg * 2; else => 90deg; };",
    )))
    .unwrap();
    for (s, expected) in [
        (2.0, 30.0_f64.to_radians() * 2.0),
        (0.5, 90.0_f64.to_radians()),
    ] {
        assert_eq!(
            evaluate(&decoded, decoded.notes[0].property_descriptors[7], s),
            RuntimeValue::Scalar {
                ty: ValueType::Angle,
                value: expected,
            },
            "angle scaling at s = {s}"
        );
    }
}

const UNIT_BEZIER_BLEND_SOURCE: &str = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        position: vec2(2px, 3px);
        rotation: 0.25rad;
        alpha: 0.0;
        tracks {
            track positionCover -> position: vec2<length> {
                priority: 2;
                fill: "base"; extrapolateBefore: "base"; extrapolateAfter: "base";
                segments { [0s, 1s): vec2(4px, -2px) -> vec2(6px, 2px)
                    using cubicBezier(0.5, 2.0, 0.5, 2.0); }
            }
            track positionShift -> position: vec2<length> {
                blend: "add";
                fill: "zero"; extrapolateBefore: "zero"; extrapolateAfter: "zero";
                segments { [0s, 1s): vec2(0.5px, 1px) -> vec2(1.5px, -1px)
                    using cubicBezier(0.5, 2.0, 0.5, 2.0); }
            }
            track positionFactor -> position: vec2<length> {
                blend: "multiply";
                fill: "one"; extrapolateBefore: "one"; extrapolateAfter: "one";
                segments { [0s, 1s): vec2(1.5px, -2px) -> vec2(2.5px, 0.5px) using "linear"; }
            }
            track rotationCover -> rotation: angle {
                priority: 2;
                fill: "base"; extrapolateBefore: "base"; extrapolateAfter: "base";
                segments { [0s, 1s): 0.25rad -> 0.5rad using cubicBezier(0.5, 2.0, 0.5, 2.0); }
            }
            track rotationShift -> rotation: angle {
                blend: "add";
                fill: "zero"; extrapolateBefore: "zero"; extrapolateAfter: "zero";
                segments { [0s, 1s): -0.125rad -> 0.125rad using cubicBezier(0.5, 2.0, 0.5, 2.0); }
            }
            track rotationFactor -> rotation: angle {
                blend: "multiply";
                fill: "one"; extrapolateBefore: "one"; extrapolateAfter: "one";
                segments { [0s, 1s): 1.5rad -> 2.5rad using "linear"; }
            }
            track alphaShift -> alpha: float {
                blend: "add";
                fill: "zero"; extrapolateBefore: "zero"; extrapolateAfter: "zero";
                segments { [0s, 1s): 0.0 -> 1.0 using cubicBezier(0.5, 2.0, 0.5, 2.0); }
            }
        }
    }
}
"#;

fn numeric_bits(value: RuntimeValue) -> Vec<u64> {
    match value {
        RuntimeValue::Scalar { value, .. } => vec![value.to_bits()],
        RuntimeValue::Vec2 { value, .. } => value.map(f64::to_bits).to_vec(),
        value => panic!("unexpected numeric value {value:?}"),
    }
}

#[test]
fn native_unit_and_bezier_blends_match_runtime_and_reference_bits() {
    for controls in [
        "0.5, 2.0, 0.5, 2.0",
        "0.0, 0.0, 1.0, 1.0",
        "1.0, 0.0, 0.0, 1.0",
        "0.25, 0.1, 0.25, 1.0",
    ] {
        let source = UNIT_BEZIER_BLEND_SOURCE.replace("0.5, 2.0, 0.5, 2.0", controls);
        let compilation = compilation(&source);
        let bytes = write_from_compilation(&compilation).unwrap();
        let decoded = load_chart(&bytes).unwrap();
        let reference = fcbc_reference_loader::load(&bytes).unwrap();
        let line = &decoded.lines[0];
        let reference_line = &reference.lines[0];
        let tracks = compilation.chart().tracks();
        let owner = tracks.tracks()[0].owner();
        for (target, base, ty, descriptor, reference_descriptor) in [
            (
                CanonicalTrackTarget::Position,
                CanonicalTrackValue::Vec2Length(CanonicalVec2::new(2.0, 3.0).unwrap()),
                ValueType::Vec2Length,
                line.position_descriptor,
                reference_line.position_descriptor,
            ),
            (
                CanonicalTrackTarget::Rotation,
                CanonicalTrackValue::Angle(0.25),
                ValueType::Angle,
                line.rotation_descriptor,
                reference_line.rotation_descriptor,
            ),
            (
                CanonicalTrackTarget::Alpha,
                CanonicalTrackValue::Float(0.0),
                ValueType::Float,
                line.alpha_descriptor,
                reference_line.alpha_descriptor,
            ),
        ] {
            for time in [
                -1.0,
                -0.0,
                0.0,
                0.0625,
                0.125,
                0.25,
                0.5,
                0.75,
                1.0f64.next_down(),
                1.0,
                2.0,
                0.5,
            ] {
                let expected =
                    fcs_runtime::evaluate_track_set(tracks, owner, target, time, base).unwrap();
                let expected = match expected {
                    CanonicalTrackValue::Float(value) | CanonicalTrackValue::Angle(value) => {
                        vec![value.to_bits()]
                    }
                    CanonicalTrackValue::Vec2Length(value) => {
                        vec![value.x().to_bits(), value.y().to_bits()]
                    }
                    value => panic!("unexpected Track value {value:?}"),
                };
                let actual = evaluate(&decoded, descriptor, time);
                assert_eq!(actual.value_type(), ty);
                assert_eq!(
                    numeric_bits(actual),
                    expected,
                    "{target:?} at {time}: {controls}"
                );

                let reference_value = fcbc_reference_evaluator::query_descriptor(
                    &reference,
                    reference_descriptor,
                    time,
                    fcbc_reference_evaluator::EvaluationEnvironment::at_time(time),
                )
                .unwrap()
                .value;
                assert_eq!(reference_value.value_type() as u8, ty as u8);
                let reference_bits = match reference_value {
                    fcbc_reference_loader::RuntimeValue::Scalar { value, .. } => {
                        vec![value.to_bits()]
                    }
                    fcbc_reference_loader::RuntimeValue::Vec2 { value, .. } => {
                        value.map(f64::to_bits).to_vec()
                    }
                    value => panic!("unexpected reference value {value:?}"),
                };
                assert_eq!(
                    reference_bits, expected,
                    "reference {target:?} at {time}: {controls}"
                );
            }
        }
        if controls == "0.5, 2.0, 0.5, 2.0" {
            // x(1/2)=1/2 and y(1/2)=13/8 exactly. The typed blends below
            // are hand-derived from that progress and the linear factors.
            for (descriptor, expected) in [
                (line.position_descriptor, vec![18.75f64, -1.6875]),
                (line.rotation_descriptor, vec![1.875]),
                (line.alpha_descriptor, vec![1.625]),
            ] {
                assert_eq!(
                    numeric_bits(evaluate(&decoded, descriptor, 0.5)),
                    expected.into_iter().map(f64::to_bits).collect::<Vec<_>>()
                );
            }
        }
    }
}

/// Retains framing and repairs the CRC so mutations reach ABI validation.
fn patch_expression_section(
    bytes: &[u8],
    section_type: u32,
    patch: impl FnOnce(&mut [u8]),
) -> Vec<u8> {
    let container = fcs_fcbc::load_container(bytes).unwrap();
    let (index, section) = container
        .sections
        .iter()
        .enumerate()
        .find(|(_, section)| section.section_type == section_type)
        .unwrap();
    let start = section.offset as usize;
    let end = start + section.length as usize;
    let mut bytes = bytes.to_vec();
    patch(&mut bytes[start..end]);
    let checksum = fcs_fcbc::section_crc32_iso_hdlc(&bytes[start..end]);
    let entry =
        container.header.section_table_offset as usize + index * fcs_fcbc::SECTION_ENTRY_SIZE;
    bytes[entry + 32..entry + 36].copy_from_slice(&checksum.to_le_bytes());
    bytes
}

fn assert_invalid_expression(bytes: &[u8]) {
    assert_eq!(load_chart(bytes).unwrap_err(), "fcbc.invalid-expression");
    assert_eq!(
        fcbc_reference_loader::load(bytes).unwrap_err(),
        "fcbc.invalid-expression"
    );
}

#[test]
fn native_bezier_expression_rejects_invalid_controls_and_signatures() {
    let source = UNIT_BEZIER_BLEND_SOURCE.replace("0.5, 2.0, 0.5, 2.0", "0.25, 0.1, 0.75, 1.0");
    for invalid_x in [-0.25, 1.25] {
        // A legal Scale base supplies an out-of-range x constant without
        // corrupting ConstantPool sort order or its Value encoding.
        let source = source.replace(
            "alpha: 0.0;",
            &format!("alpha: 0.0; scale: vec2({invalid_x}, 0.0);"),
        );
        let bytes = compile(&source);
        let decoded = load_chart(&bytes).unwrap();
        let (index, curve) = decoded
            .expressions
            .iter()
            .enumerate()
            .find(|(_, node)| node.opcode == 64)
            .unwrap();
        let invalid_constant = decoded.constants.iter().position(|value| {
            matches!(value, RuntimeValue::Vec2 { ty: ValueType::Vec2Float, value } if value[0] == invalid_x)
        }).unwrap() as u32;
        for operand in [1, 2] {
            let control_index = curve.operands[operand] as usize;
            let control = &decoded.expressions[control_index];
            assert_eq!(control.opcode, 1);
            assert_invalid_expression(&patch_expression_section(&bytes, 12, |section| {
                let immediate = 4 + 20 * control_index + 16;
                section[immediate..immediate + 4].copy_from_slice(&invalid_constant.to_le_bytes());
            }));

            for component in [0, 1] {
                for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                    assert_invalid_expression(&patch_expression_section(&bytes, 2, |section| {
                        let mut offset = 4;
                        for _ in 0..control.immediate {
                            let length = u32::from_le_bytes(
                                section[offset + 4..offset + 8].try_into().unwrap(),
                            ) as usize;
                            offset += (8 + length).next_multiple_of(8);
                        }
                        assert_eq!(section[offset], 10);
                        assert_eq!(section[offset + 8], 3);
                        let at = offset + 16 + component * 8;
                        section[at..at + 8].copy_from_slice(&invalid.to_le_bytes());
                    }));
                }
            }

            // A computed vec2-float has the right type but is not a Constant.
            assert!(curve.operands[0] < control_index as u32);
            assert_invalid_expression(&patch_expression_section(&bytes, 12, |section| {
                let at = 4 + 20 * control_index;
                section[at..at + 2].copy_from_slice(&80u16.to_le_bytes());
                section[at + 3] = 2;
                section[at + 4..at + 8].copy_from_slice(&curve.operands[0].to_le_bytes());
                section[at + 8..at + 12].copy_from_slice(&curve.operands[0].to_le_bytes());
                section[at + 16..at + 20].copy_from_slice(&0u32.to_le_bytes());
            }));
        }
        for (field, replacement) in [
            (0, 65u16.to_le_bytes().to_vec()),
            (2, vec![ValueType::Angle as u8]),
            (3, vec![2]),
            (4, curve.operands[1].to_le_bytes().to_vec()),
            (16, 1u32.to_le_bytes().to_vec()),
        ] {
            assert_invalid_expression(&patch_expression_section(&bytes, 12, |section| {
                let at = 4 + 20 * index + field;
                section[at..at + replacement.len()].copy_from_slice(&replacement);
            }));
        }
    }
}

#[test]
fn native_unit_mul_rejects_mixed_units_and_wrong_results() {
    let bytes = compile(UNIT_BEZIER_BLEND_SOURCE);
    let decoded = load_chart(&bytes).unwrap();
    for ty in [ValueType::Angle, ValueType::Vec2Length] {
        let (index, _) = decoded
            .expressions
            .iter()
            .enumerate()
            .rfind(|(_, node)| {
                node.opcode == 22
                    && node.result_type == ty
                    && node.operands[..2]
                        .iter()
                        .all(|operand| decoded.expressions[*operand as usize].result_type == ty)
            })
            .unwrap();
        let wrong_operand = decoded.expressions[..index]
            .iter()
            .position(|node| {
                node.result_type
                    == if ty == ValueType::Angle {
                        ValueType::Time
                    } else {
                        ValueType::Vec2Float
                    }
            })
            .unwrap() as u32;
        assert_invalid_expression(&patch_expression_section(&bytes, 12, |section| {
            let at = 4 + 20 * index + 8;
            section[at..at + 4].copy_from_slice(&wrong_operand.to_le_bytes());
        }));
        assert_invalid_expression(&patch_expression_section(&bytes, 12, |section| {
            section[4 + 20 * index + 2] = ValueType::Float as u8;
        }));
    }
}

#[test]
fn native_bezier_blend_propagates_uncertified_solver_errors() {
    let source = UNIT_BEZIER_BLEND_SOURCE.replace("0.5, 2.0, 0.5, 2.0", "0.25, 0.5, 0.75, 5e-324");
    let compilation = compilation(&source);
    let bytes = write_from_compilation(&compilation).unwrap();
    let decoded = load_chart(&bytes).unwrap();
    let reference = fcbc_reference_loader::load(&bytes).unwrap();
    let tracks = compilation.chart().tracks();
    assert!(
        fcs_runtime::evaluate_track_set(
            tracks,
            tracks.tracks()[0].owner(),
            CanonicalTrackTarget::Alpha,
            0.25,
            CanonicalTrackValue::Float(0.0),
        )
        .is_err()
    );
    assert_eq!(
        query_descriptor(
            &decoded,
            decoded.lines[0].alpha_descriptor,
            0.25,
            EvaluationEnvironment::at_time(0.25),
        )
        .unwrap_err(),
        "fcbc.execution-error"
    );
    assert_eq!(
        fcbc_reference_evaluator::query_descriptor(
            &reference,
            reference.lines[0].alpha_descriptor,
            0.25,
            fcbc_reference_evaluator::EvaluationEnvironment::at_time(0.25),
        )
        .unwrap_err(),
        "fcbc.execution-error"
    );
}
