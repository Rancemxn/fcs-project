use fcs_model::{
    CanonicalChartScrollTempoPoint, CanonicalLine, CanonicalLineBase, CanonicalLineGraph,
    CanonicalLineInherit, CanonicalScrollCoordinate, CanonicalScrollLine, CanonicalScrollSet,
    CanonicalScrollTempo, CanonicalTextualId, CanonicalTime, CanonicalTrack, CanonicalTrackBlend,
    CanonicalTrackFill, CanonicalTrackInterpolation, CanonicalTrackPiece, CanonicalTrackSegment,
    CanonicalTrackSet, CanonicalTrackTarget, CanonicalTrackValue, CanonicalVec2, EntityKind,
    StableId, StableIdRegistry,
};
use fcs_runtime::{evaluate_line_scroll, evaluate_line_transform, evaluate_track_set};
use proptest::{
    prelude::*,
    test_runner::{RngAlgorithm, RngSeed},
};

const PROPERTY_CASES: u32 = 96;
const MAX_GENERATOR_DEPTH: usize = 1;
const MAX_SEGMENTS_PER_TRACK: usize = 1;
const MAX_FRAME_PARTITIONS: usize = 16;
const MAX_QUERIES_PER_CASE: usize = MAX_FRAME_PARTITIONS + 2;
const PORTABLE_ERROR: f64 = 2.328_306_436_538_696_3e-10;

fn deterministic_config() -> ProptestConfig {
    ProptestConfig {
        cases: PROPERTY_CASES,
        max_local_rejects: 256,
        failure_persistence: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(0xF0C5_4901),
        ..ProptestConfig::default()
    }
}

#[derive(Clone, Debug)]
enum InterpolationCase {
    Constant,
    Step,
    Linear,
    Easing(&'static str),
    Bezier([f64; 4]),
}

fn interpolation_strategy() -> impl Strategy<Value = InterpolationCase> {
    prop_oneof![
        Just(InterpolationCase::Constant),
        Just(InterpolationCase::Step),
        Just(InterpolationCase::Linear),
        Just(InterpolationCase::Easing("easeInQuad")),
        Just(InterpolationCase::Easing("easeOutSine")),
        Just(InterpolationCase::Easing("easeInOutCubic")),
        Just(InterpolationCase::Bezier([0.25, 0.1, 0.75, 0.9])),
        Just(InterpolationCase::Bezier([0.1, 1.4, 0.9, -0.2])),
    ]
}

fn scalar_strategy() -> impl Strategy<Value = f64> {
    (-32i16..=32).prop_map(|value| f64::from(value) / 8.0)
}

fn line_id(name: &str) -> StableId {
    StableIdRegistry::new()
        .insert(
            EntityKind::Line,
            CanonicalTextualId::explicit(name).unwrap(),
        )
        .unwrap()
}

fn time(value: f64) -> CanonicalTime {
    CanonicalTime::from_chart_time_seconds(value).unwrap()
}

fn interpolation(case: &InterpolationCase) -> CanonicalTrackInterpolation {
    match case {
        InterpolationCase::Constant | InterpolationCase::Linear => {
            CanonicalTrackInterpolation::Linear
        }
        InterpolationCase::Step => CanonicalTrackInterpolation::Step,
        InterpolationCase::Easing(name) => CanonicalTrackInterpolation::Easing((*name).to_owned()),
        InterpolationCase::Bezier(control) => CanonicalTrackInterpolation::CubicBezier(*control),
    }
}

fn speed_track(
    owner: StableId,
    start_value: f64,
    end_value: f64,
    interpolation_case: &InterpolationCase,
) -> CanonicalTrack {
    assert_eq!(MAX_GENERATOR_DEPTH, 1);
    let (start_value, end_value) = if matches!(interpolation_case, InterpolationCase::Constant) {
        (start_value, start_value)
    } else {
        (start_value, end_value)
    };
    let track = CanonicalTrack::new(
        owner,
        "speed",
        CanonicalTrackTarget::ScrollSpeed,
        CanonicalTrackBlend::Replace,
        0,
        CanonicalTrackFill::HoldAfter,
        CanonicalTrackFill::HoldBefore,
        CanonicalTrackFill::HoldAfter,
        vec![CanonicalTrackPiece::Segment(
            CanonicalTrackSegment::new(
                time(-4.0),
                time(4.0),
                CanonicalTrackValue::Float(start_value),
                CanonicalTrackValue::Float(end_value),
                interpolation(interpolation_case),
                0,
            )
            .unwrap(),
        )],
    )
    .unwrap();
    assert_eq!(track.pieces().len(), MAX_SEGMENTS_PER_TRACK);
    track
}

fn scroll_descriptor(owner: StableId, origin: f64, initial_floor: f64) -> CanonicalScrollLine {
    CanonicalScrollLine::new(
        owner,
        CanonicalScrollCoordinate::new([CanonicalChartScrollTempoPoint::new(0.0, 60.0).unwrap()])
            .unwrap(),
        0.0,
        true,
        1.0,
        origin,
        initial_floor,
    )
    .unwrap()
}

fn scroll_fixture(
    interpolation_case: &InterpolationCase,
    start_value: f64,
    end_value: f64,
    origin: f64,
    initial_floor: f64,
) -> (
    StableId,
    CanonicalLineGraph,
    CanonicalScrollSet,
    CanonicalTrackSet,
) {
    let owner = line_id("property-line");
    let line = CanonicalLine::new(
        owner.clone(),
        None,
        0,
        CanonicalLineBase::identity(),
        CanonicalLineInherit::default(),
        CanonicalScrollTempo::Global,
    )
    .unwrap();
    let graph = CanonicalLineGraph::new([line]).unwrap();
    let scroll = CanonicalScrollSet::new(vec![scroll_descriptor(
        owner.clone(),
        origin,
        initial_floor,
    )])
    .unwrap();
    let tracks = CanonicalTrackSet::new(vec![speed_track(
        owner.clone(),
        start_value,
        end_value,
        interpolation_case,
    )])
    .unwrap();
    (owner, graph, scroll, tracks)
}

fn partitioned_floor(
    owner: &StableId,
    interpolation_case: &InterpolationCase,
    start_value: f64,
    end_value: f64,
    query: f64,
    partitions: usize,
) -> f64 {
    assert!(partitions <= MAX_FRAME_PARTITIONS);
    assert!(partitions + 2 <= MAX_QUERIES_PER_CASE);
    let mut total = 0.0;
    let mut previous = 0.0;
    for index in 1..=partitions {
        let current = query * (index as f64 / partitions as f64);
        let (_, graph, scroll, tracks) =
            scroll_fixture(interpolation_case, start_value, end_value, previous, 0.0);
        let result = evaluate_line_scroll(&graph, &scroll, &tracks, owner, current).unwrap();
        total += result.local_floor();
        previous = current;
    }
    total
}

fn base(
    position: (f64, f64),
    rotation: f64,
    scale: (f64, f64),
    alpha: f64,
    origin: (f64, f64),
) -> CanonicalLineBase {
    CanonicalLineBase::new(
        CanonicalVec2::new(position.0, position.1).unwrap(),
        rotation,
        CanonicalVec2::new(scale.0, scale.1).unwrap(),
        alpha,
        CanonicalVec2::new(origin.0, origin.1).unwrap(),
        CanonicalVec2::new(0.5, 0.5).unwrap(),
        120.0,
        0.0,
        0.0,
        false,
        0,
    )
    .unwrap()
}

proptest! {
    #![proptest_config(deterministic_config())]

    #[test]
    fn track_queries_are_bit_stable_and_declaration_order_independent(
        start in scalar_strategy(),
        end in scalar_strategy(),
        query in -32i16..=32,
    ) {
        let owner = line_id("track-order");
        let query = f64::from(query) / 8.0;
        let replace = CanonicalTrack::new(
            owner.clone(), "replace", CanonicalTrackTarget::Alpha,
            CanonicalTrackBlend::Replace, 0, CanonicalTrackFill::HoldAfter,
            CanonicalTrackFill::HoldBefore, CanonicalTrackFill::HoldAfter,
            vec![CanonicalTrackPiece::Segment(CanonicalTrackSegment::new(
                time(-4.0), time(4.0), CanonicalTrackValue::Float(start),
                CanonicalTrackValue::Float(end), CanonicalTrackInterpolation::Linear, 0,
            ).unwrap())],
        ).unwrap();
        let add = CanonicalTrack::new(
            owner.clone(), "add", CanonicalTrackTarget::Alpha,
            CanonicalTrackBlend::Add, 1, CanonicalTrackFill::Zero,
            CanonicalTrackFill::Zero, CanonicalTrackFill::Zero,
            vec![CanonicalTrackPiece::Point(fcs_model::CanonicalTrackPoint::new(
                time(0.0), CanonicalTrackValue::Float(0.25), 0,
            ).unwrap())],
        ).unwrap();
        let multiply = CanonicalTrack::new(
            owner.clone(), "multiply", CanonicalTrackTarget::Alpha,
            CanonicalTrackBlend::Multiply, 2, CanonicalTrackFill::One,
            CanonicalTrackFill::One, CanonicalTrackFill::One,
            vec![CanonicalTrackPiece::Point(fcs_model::CanonicalTrackPoint::new(
                time(0.0), CanonicalTrackValue::Float(1.0), 0,
            ).unwrap())],
        ).unwrap();
        let forward = CanonicalTrackSet::new(vec![replace.clone(), add.clone(), multiply.clone()]).unwrap();
        let reversed = CanonicalTrackSet::new(vec![multiply, replace, add]).unwrap();
        let first = evaluate_track_set(
            &forward, &owner, CanonicalTrackTarget::Alpha, query,
            CanonicalTrackValue::Float(0.0),
        ).unwrap();
        let second = evaluate_track_set(
            &reversed, &owner, CanonicalTrackTarget::Alpha, query,
            CanonicalTrackValue::Float(0.0),
        ).unwrap();
        prop_assert_eq!(first, second);
        let repeated = evaluate_track_set(
            &forward, &owner, CanonicalTrackTarget::Alpha, query,
            CanonicalTrackValue::Float(0.0),
        ).unwrap();
        prop_assert_eq!(first, repeated);
    }

    #[test]
    fn direct_scroll_seek_is_partition_invariant_within_error_budget(
        start in scalar_strategy(),
        end in scalar_strategy(),
        interpolation_case in interpolation_strategy(),
        query in -32i16..=32,
        partitions in 1usize..=MAX_FRAME_PARTITIONS,
    ) {
        let query = f64::from(query) / 8.0;
        let (owner, graph, scroll, tracks) = scroll_fixture(
            &interpolation_case, start, end, 0.0, 0.0,
        );
        let direct = evaluate_line_scroll(&graph, &scroll, &tracks, &owner, query).unwrap();
        let partitioned = partitioned_floor(
            &owner, &interpolation_case, start, end, query, partitions,
        );
        prop_assert!(
            (direct.local_floor() - partitioned).abs() <= PORTABLE_ERROR * 8.0,
            "direct={} partitioned={} query={} partitions={}",
            direct.local_floor(), partitioned, query, partitions,
        );
        let repeated = evaluate_line_scroll(&graph, &scroll, &tracks, &owner, query).unwrap();
        prop_assert_eq!(direct.local_floor().to_bits(), repeated.local_floor().to_bits());
    }

    #[test]
    fn linear_scroll_matches_independent_integral_bound(
        start in scalar_strategy(),
        end in scalar_strategy(),
        query in 0i16..=32,
    ) {
        let query = f64::from(query) / 8.0;
        let interpolation_case = InterpolationCase::Linear;
        let (owner, graph, scroll, tracks) = scroll_fixture(
            &interpolation_case, start, end, 0.0, 0.0,
        );
        let actual = evaluate_line_scroll(&graph, &scroll, &tracks, &owner, query)
            .unwrap()
            .local_floor();
        let midpoint_speed = (start + end) * 0.5;
        let slope = (end - start) / 8.0;
        let expected = midpoint_speed * query + 0.5 * slope * query * query;
        prop_assert!(
            (actual - expected).abs() <= PORTABLE_ERROR * 8.0,
            "actual={} expected={} query={}", actual, expected, query,
        );
    }

    #[test]
    fn transform_graph_is_declaration_order_independent(
        px in scalar_strategy(),
        py in scalar_strategy(),
        rotation in -16i16..=16,
        sx in 4i16..=16,
        sy in 4i16..=16,
        alpha in 1i16..=8,
        inherit_position in any::<bool>(),
        inherit_rotation in any::<bool>(),
        inherit_scale in any::<bool>(),
        inherit_alpha in any::<bool>(),
    ) {
        let parent_id = line_id("property-parent");
        let child_id = line_id("property-child");
        let parent = CanonicalLine::new(
            parent_id.clone(), None, 0,
            base((px, py), f64::from(rotation) / 8.0,
                (f64::from(sx) / 8.0, f64::from(sy) / 8.0),
                f64::from(alpha) / 8.0, (0.25, -0.5)),
            CanonicalLineInherit::default(), CanonicalScrollTempo::Global,
        ).unwrap();
        let child = CanonicalLine::new(
            child_id.clone(), Some(parent_id), 1,
            base((-py, px), f64::from(rotation) / 16.0,
                (1.0, 1.0), 0.75, (0.0, 0.0)),
            CanonicalLineInherit::new(
                inherit_position, inherit_rotation, inherit_scale, inherit_alpha, false,
            ), CanonicalScrollTempo::Global,
        ).unwrap();
        let first = CanonicalLineGraph::new([parent.clone(), child.clone()]).unwrap();
        let second = CanonicalLineGraph::new([child, parent]).unwrap();
        let tracks = CanonicalTrackSet::new(Vec::new()).unwrap();
        let left = evaluate_line_transform(&first, &tracks, &child_id, 0.0).unwrap();
        let right = evaluate_line_transform(&second, &tracks, &child_id, 0.0).unwrap();
        prop_assert_eq!(left, right);
    }
}

#[test]
fn runtime_error_categories_remain_stable() {
    let owner = line_id("stable-errors");
    let line = CanonicalLine::new(
        owner.clone(),
        None,
        0,
        CanonicalLineBase::identity(),
        CanonicalLineInherit::default(),
        CanonicalScrollTempo::Global,
    )
    .unwrap();
    let graph = CanonicalLineGraph::new([line]).unwrap();
    let scroll = CanonicalScrollSet::new(vec![scroll_descriptor(owner.clone(), 0.0, 0.0)]).unwrap();
    let tracks = CanonicalTrackSet::new(vec![
        CanonicalTrack::new(
            owner.clone(),
            "reverse",
            CanonicalTrackTarget::ScrollSpeed,
            CanonicalTrackBlend::Replace,
            0,
            CanonicalTrackFill::HoldAfter,
            CanonicalTrackFill::HoldBefore,
            CanonicalTrackFill::HoldAfter,
            vec![CanonicalTrackPiece::Segment(
                CanonicalTrackSegment::new(
                    time(-1.0),
                    time(1.0),
                    CanonicalTrackValue::Float(-1.0),
                    CanonicalTrackValue::Float(-1.0),
                    CanonicalTrackInterpolation::Step,
                    0,
                )
                .unwrap(),
            )],
        )
        .unwrap(),
    ])
    .unwrap();
    let without_reverse = CanonicalScrollSet::new(vec![
        CanonicalScrollLine::new(
            owner.clone(),
            CanonicalScrollCoordinate::new([
                CanonicalChartScrollTempoPoint::new(0.0, 60.0).unwrap()
            ])
            .unwrap(),
            0.0,
            false,
            1.0,
            0.0,
            0.0,
        )
        .unwrap(),
    ])
    .unwrap();
    let error = evaluate_line_scroll(&graph, &without_reverse, &tracks, &owner, 0.5)
        .expect_err("negative speed must be rejected when reverse is disabled");
    assert!(matches!(
        error,
        fcs_runtime::ScrollEvaluationError::ReverseNotAllowed { .. }
    ));
    assert_eq!(
        evaluate_line_scroll(&graph, &scroll, &tracks, &owner, f64::NAN),
        Err(fcs_runtime::ScrollEvaluationError::NonFiniteChartTime),
    );
}
