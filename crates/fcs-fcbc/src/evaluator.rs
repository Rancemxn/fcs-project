use super::loader::{
    DecodedChart, DescriptorKind, DistanceClassification, MAX_VALIDATOR_DEPTH, RuntimeValue,
    Segment, ValueType, is_unit_scalar,
};
use fcs_runtime::{evaluate_cubic_bezier_progress, evaluate_easing};
use std::collections::BTreeMap;

const EXECUTION_ERROR: &str = "fcbc.execution-error";

/// Per-query value cache for shared expression subgraphs, keyed by the node
/// index and the exact environment bits. Evaluation is pure, so a cached
/// value is bit-identical to recomputation; the trace still records every
/// recursive entry, including memo hits.
type ExpressionMemo = BTreeMap<([u64; 5], u32), RuntimeValue>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvaluationEnvironment {
    pub s: f64,
    pub b: f64,
    pub q: f64,
    pub d: f64,
    pub p: f64,
}

impl EvaluationEnvironment {
    pub fn at_time(s: f64) -> Self {
        Self {
            s,
            b: s,
            q: 0.0,
            d: 0.0,
            p: 0.0,
        }
    }

    pub fn at_chart_time(chart: &DecodedChart, s: f64) -> Result<Self, &'static str> {
        Ok(Self {
            s,
            b: chart_beat_at_time(chart, s)?,
            q: 0.0,
            d: 0.0,
            p: 0.0,
        })
    }
}

pub fn chart_beat_at_time(chart: &DecodedChart, chart_time: f64) -> Result<f64, &'static str> {
    if !chart_time.is_finite() {
        return Err(EXECUTION_ERROR);
    }
    let first = chart.tempo_points.first().ok_or(EXECUTION_ERROR)?;
    let point = chart
        .tempo_points
        .iter()
        .rfind(|point| point.chart_time <= chart_time)
        .unwrap_or(first);
    let beat = (point.beat_numerator as f64 / point.beat_denominator as f64)
        + ((chart_time - point.chart_time) * point.bpm) / 60.0;
    beat.is_finite().then_some(beat).ok_or(EXECUTION_ERROR)
}

#[derive(Clone, Debug, PartialEq)]
pub struct DescriptorEvaluation {
    pub value: RuntimeValue,
    /// Expression node indices in recursive entry order. A short-circuited branch is absent.
    pub visited_nodes: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DistanceEvaluation {
    pub floor_position: f64,
    pub classification: DistanceClassification,
    pub visited_nodes: Vec<u32>,
}

/// Queries one exact descriptor without frame history or a sampled cache.
pub fn query_descriptor(
    chart: &DecodedChart,
    descriptor_index: u32,
    time: f64,
    environment: EvaluationEnvironment,
) -> Result<DescriptorEvaluation, &'static str> {
    if environment.s.to_bits() != time.to_bits() {
        return Err(EXECUTION_ERROR);
    }
    let mut visited_nodes = Vec::new();
    let mut memo = ExpressionMemo::new();
    let value = evaluate_descriptor_inner(
        chart,
        descriptor_index,
        time,
        environment,
        &mut visited_nodes,
        &mut memo,
        0,
    )?;
    Ok(DescriptorEvaluation {
        value,
        visited_nodes,
    })
}

/// Direct-seek distance query. Both classifications are evaluated from the requested time and
/// the record's integration origin; no result depends on an earlier call.
pub fn query_distance(
    chart: &DecodedChart,
    distance_index: u32,
    time: f64,
) -> Result<DistanceEvaluation, &'static str> {
    let distance = chart
        .distances
        .get(distance_index as usize)
        .ok_or(EXECUTION_ERROR)?;
    if !distance.domain.contains(time) {
        return Err(EXECUTION_ERROR);
    }
    if time == distance.integration_origin {
        return Ok(DistanceEvaluation {
            floor_position: distance.initial_floor_position,
            classification: distance.classification,
            visited_nodes: Vec::new(),
        });
    }
    match distance.classification {
        DistanceClassification::PortableAnalytic => {
            let line = chart
                .lines
                .iter()
                .find(|line| line.id == distance.line_id)
                .ok_or(EXECUTION_ERROR)?;
            let speed = query_descriptor(
                chart,
                line.scroll_speed_descriptor,
                time,
                EvaluationEnvironment::at_chart_time(chart, time)?,
            )?;
            let tempo = query_descriptor(
                chart,
                line.scroll_tempo_descriptor,
                time,
                EvaluationEnvironment::at_chart_time(chart, time)?,
            )?;
            let integrand = scalar_payload(&speed.value)? * scalar_payload(&tempo.value)? / 60.0;
            let mut floor_position =
                distance.initial_floor_position + integrand * (time - distance.integration_origin);
            if !floor_position.is_finite() {
                return Err(EXECUTION_ERROR);
            }
            if floor_position == 0.0 {
                floor_position = 0.0;
            }
            let mut visited_nodes = speed.visited_nodes;
            visited_nodes.extend(tempo.visited_nodes);
            Ok(DistanceEvaluation {
                floor_position,
                classification: distance.classification,
                visited_nodes,
            })
        }
        DistanceClassification::PortableEvaluable => {
            let line = chart
                .lines
                .iter()
                .find(|line| line.id == distance.line_id)
                .ok_or(EXECUTION_ERROR)?;
            let integral = integrate_scroll_product(
                chart,
                line.scroll_speed_descriptor,
                line.scroll_tempo_descriptor,
                distance.integration_origin,
                time,
            )?;
            let mut floor_position = distance.initial_floor_position + integral;
            if !floor_position.is_finite() {
                return Err(EXECUTION_ERROR);
            }
            if floor_position == 0.0 {
                floor_position = 0.0;
            }
            Ok(DistanceEvaluation {
                floor_position,
                classification: distance.classification,
                visited_nodes: Vec::new(),
            })
        }
    }
}

/// Direct-seek Line scroll coordinate with the Core anchor `q(0s) = 0`.
pub fn query_scroll_coordinate(
    chart: &DecodedChart,
    scroll_tempo_descriptor: u32,
    time: f64,
) -> Result<f64, &'static str> {
    if !time.is_finite() {
        return Err(EXECUTION_ERROR);
    }
    let integral = integrate_descriptor(chart, scroll_tempo_descriptor, 0.0, time, 0)?;
    let coordinate = integral / 60.0;
    coordinate
        .is_finite()
        .then_some(coordinate)
        .ok_or(EXECUTION_ERROR)
}

fn evaluate_descriptor_inner(
    chart: &DecodedChart,
    descriptor_index: u32,
    time: f64,
    environment: EvaluationEnvironment,
    visited_nodes: &mut Vec<u32>,
    memo: &mut ExpressionMemo,
    depth: usize,
) -> Result<RuntimeValue, &'static str> {
    if depth > MAX_VALIDATOR_DEPTH || depth > chart.descriptors.len() {
        return Err(EXECUTION_ERROR);
    }
    let descriptor = chart
        .descriptors
        .get(descriptor_index as usize)
        .ok_or(EXECUTION_ERROR)?;
    if !descriptor.domain.contains(time) {
        return Err(EXECUTION_ERROR);
    }
    let value = match &descriptor.kind {
        DescriptorKind::Constant(index) => chart
            .constants
            .get(*index as usize)
            .cloned()
            .ok_or(EXECUTION_ERROR)?,
        DescriptorKind::SegmentTrack(segments) => evaluate_segment_track(chart, segments, time)?,
        DescriptorKind::Piecewise(pieces) => {
            let piece = pieces
                .iter()
                .find(|piece| {
                    (piece.flags & 0b010 != 0 || piece.start <= time)
                        && (piece.flags & 0b100 != 0
                            || time < piece.end
                            || (piece.flags & 1 != 0 && time.to_bits() == piece.end.to_bits()))
                })
                .ok_or(EXECUTION_ERROR)?;
            let mut piece_environment = environment;
            piece_environment.p = piece_progress(piece, time)?;
            evaluate_descriptor_inner(
                chart,
                piece.descriptor_index,
                time,
                piece_environment,
                visited_nodes,
                memo,
                depth + 1,
            )?
        }
        DescriptorKind::Expression(root) => {
            let remaining_depth = (MAX_VALIDATOR_DEPTH - depth).min(chart.expressions.len());
            evaluate_node(
                chart,
                *root,
                environment,
                visited_nodes,
                memo,
                remaining_depth,
            )?
        }
    };
    if value.value_type() != descriptor.property_type {
        return Err(EXECUTION_ERROR);
    }
    Ok(value)
}

fn piece_progress(piece: &super::loader::Piece, time: f64) -> Result<f64, &'static str> {
    let unbounded_before = piece.flags & 0b010 != 0;
    let unbounded_after = piece.flags & 0b100 != 0;
    let progress = match (unbounded_before, unbounded_after) {
        (false, false) => (time - piece.start) / (piece.end - piece.start),
        (true, false) => 0.0,
        (false, true) => 1.0,
        (true, true) => 0.0,
    };
    (progress.is_finite() && (0.0..=1.0).contains(&progress))
        .then_some(progress)
        .ok_or(EXECUTION_ERROR)
}

fn evaluate_segment_track(
    chart: &DecodedChart,
    segments: &[Segment],
    time: f64,
) -> Result<RuntimeValue, &'static str> {
    if let Some(segment) = segments
        .iter()
        .find(|segment| segment.flags & 1 == 0 && segment.start <= time && time < segment.end)
    {
        let start = chart
            .constants
            .get(segment.start_constant as usize)
            .ok_or(EXECUTION_ERROR)?;
        let end = chart
            .constants
            .get(segment.end_constant as usize)
            .ok_or(EXECUTION_ERROR)?;
        return interpolate_segment(start, end, segment, time);
    }
    // A point holds from its start until the next entry begins. The last such
    // point is the hold candidate, but an ordinary segment that began at or
    // after it (and at or before `time`) terminated its lifetime, so the hold
    // cannot be revived across an interval the ordinary left uncovered.
    let mut held: Option<&Segment> = None;
    for segment in segments {
        if segment.flags & 1 != 0 {
            if segment.start <= time {
                held = Some(segment);
            }
        } else if held.is_some() && segment.start <= time {
            held = None;
        }
    }
    let point = held
        .or_else(|| {
            let first = segments.first()?;
            (first.flags & 1 != 0 && first.start > time).then_some(first)
        })
        .ok_or(EXECUTION_ERROR)?;
    chart
        .constants
        .get(point.start_constant as usize)
        .cloned()
        .ok_or(EXECUTION_ERROR)
}

fn interpolate_segment(
    start: &RuntimeValue,
    end: &RuntimeValue,
    segment: &Segment,
    time: f64,
) -> Result<RuntimeValue, &'static str> {
    if segment.interpolation == 1 {
        return Ok(start.clone());
    }
    let raw_progress = (time - segment.start) / (segment.end - segment.start);
    let progress = match segment.interpolation {
        2 => raw_progress,
        3 => easing(segment.easing, raw_progress)?,
        4 => cubic_bezier_progress(segment.bezier, raw_progress)?,
        _ => return Err(EXECUTION_ERROR),
    };
    interpolate_value(start, end, progress)
}

fn evaluate_node(
    chart: &DecodedChart,
    index: u32,
    environment: EvaluationEnvironment,
    visited_nodes: &mut Vec<u32>,
    memo: &mut ExpressionMemo,
    remaining_depth: usize,
) -> Result<RuntimeValue, &'static str> {
    if remaining_depth == 0 {
        return Err(EXECUTION_ERROR);
    }
    let node = chart
        .expressions
        .get(index as usize)
        .ok_or(EXECUTION_ERROR)?;
    visited_nodes.push(index);
    // The push happens before the memo lookup so the trace keeps recording
    // recursive entry order; only the recomputation is skipped on a hit.
    let environment_key = [
        environment.s.to_bits(),
        environment.b.to_bits(),
        environment.q.to_bits(),
        environment.d.to_bits(),
        environment.p.to_bits(),
    ];
    if let Some(value) = memo.get(&(environment_key, index)) {
        return Ok(value.clone());
    }
    let operand = |operand_index: usize,
                   visited: &mut Vec<u32>,
                   memo: &mut ExpressionMemo|
     -> Result<RuntimeValue, &'static str> {
        evaluate_node(
            chart,
            node.operands[operand_index],
            environment,
            visited,
            memo,
            remaining_depth - 1,
        )
    };

    let value = match node.opcode {
        1 => chart
            .constants
            .get(node.immediate as usize)
            .cloned()
            .ok_or(EXECUTION_ERROR)?,
        2 => scalar(ValueType::Time, environment.s)?,
        3 => scalar(ValueType::Beat, environment.b)?,
        4 => scalar(ValueType::Float, environment.q)?,
        5 => scalar(ValueType::Length, environment.d)?,
        6 => scalar(ValueType::Float, environment.p)?,
        10 => negate(operand(0, visited_nodes, memo)?)?,
        11 => RuntimeValue::Bool(!boolean(&operand(0, visited_nodes, memo)?)?),
        20 => arithmetic(
            operand(0, visited_nodes, memo)?,
            operand(1, visited_nodes, memo)?,
            Arithmetic::Add,
        )?,
        21 => arithmetic(
            operand(0, visited_nodes, memo)?,
            operand(1, visited_nodes, memo)?,
            Arithmetic::Subtract,
        )?,
        22 => arithmetic(
            operand(0, visited_nodes, memo)?,
            operand(1, visited_nodes, memo)?,
            Arithmetic::Multiply,
        )?,
        23 => arithmetic(
            operand(0, visited_nodes, memo)?,
            operand(1, visited_nodes, memo)?,
            Arithmetic::Divide,
        )?,
        24 => {
            let left = integer(&operand(0, visited_nodes, memo)?)?;
            let right = integer(&operand(1, visited_nodes, memo)?)?;
            RuntimeValue::Int(left.checked_rem(right).ok_or(EXECUTION_ERROR)?)
        }
        25 => power(
            operand(0, visited_nodes, memo)?,
            operand(1, visited_nodes, memo)?,
        )?,
        30 => RuntimeValue::Bool(values_equal(
            &operand(0, visited_nodes, memo)?,
            &operand(1, visited_nodes, memo)?,
        )?),
        31 => RuntimeValue::Bool(!values_equal(
            &operand(0, visited_nodes, memo)?,
            &operand(1, visited_nodes, memo)?,
        )?),
        32..=35 => compare(
            operand(0, visited_nodes, memo)?,
            operand(1, visited_nodes, memo)?,
            node.opcode,
        )?,
        36 => {
            let left = boolean(&operand(0, visited_nodes, memo)?)?;
            if left {
                RuntimeValue::Bool(boolean(&operand(1, visited_nodes, memo)?)?)
            } else {
                RuntimeValue::Bool(false)
            }
        }
        37 => {
            let left = boolean(&operand(0, visited_nodes, memo)?)?;
            if left {
                RuntimeValue::Bool(true)
            } else {
                RuntimeValue::Bool(boolean(&operand(1, visited_nodes, memo)?)?)
            }
        }
        38 => {
            let left = scalar_payload(&operand(0, visited_nodes, memo)?)?;
            let right = scalar_payload(&operand(1, visited_nodes, memo)?)?;
            let tolerance = scalar_payload(&operand(2, visited_nodes, memo)?)?;
            if tolerance < 0.0 {
                return Err(EXECUTION_ERROR);
            }
            let difference = left - right;
            if !difference.is_finite() {
                return Err(EXECUTION_ERROR);
            }
            RuntimeValue::Bool(difference.abs() <= tolerance)
        }
        40 => absolute(operand(0, visited_nodes, memo)?)?,
        41 | 42 => min_max(
            operand(0, visited_nodes, memo)?,
            operand(1, visited_nodes, memo)?,
            node.opcode == 42,
        )?,
        43 => clamp(
            operand(0, visited_nodes, memo)?,
            operand(1, visited_nodes, memo)?,
            operand(2, visited_nodes, memo)?,
        )?,
        44..=55 => unary_float(operand(0, visited_nodes, memo)?, node.opcode)?,
        56 => {
            let left = scalar_payload(&operand(0, visited_nodes, memo)?)?;
            let right = scalar_payload(&operand(1, visited_nodes, memo)?)?;
            scalar(ValueType::Float, left.atan2(right))?
        }
        60 => {
            let input = scalar_payload(&operand(0, visited_nodes, memo)?)?;
            scalar(ValueType::Float, easing(node.immediate as u16, input)?)?
        }
        61 => {
            let value = integer(&operand(0, visited_nodes, memo)?)? as f64;
            scalar(ValueType::Float, value)?
        }
        62 | 63 => {
            let value = scalar_payload(&operand(0, visited_nodes, memo)?)?;
            scalar(ValueType::Float, value)?
        }
        70 => {
            if boolean(&operand(0, visited_nodes, memo)?)? {
                operand(1, visited_nodes, memo)?
            } else {
                operand(2, visited_nodes, memo)?
            }
        }
        80 => {
            let left = operand(0, visited_nodes, memo)?;
            let right = operand(1, visited_nodes, memo)?;
            make_vec2(left, right, node.result_type)?
        }
        81 | 82 => {
            let vector = operand(0, visited_nodes, memo)?;
            vector_component(vector, node.opcode == 82)?
        }
        _ => return Err(EXECUTION_ERROR),
    };
    if value.value_type() != node.result_type {
        return Err(EXECUTION_ERROR);
    }
    // Only successful values are cached; an erroring node re-runs (and
    // re-errors) on every occurrence, exactly as it did before the memo.
    memo.insert((environment_key, index), value.clone());
    Ok(value)
}

#[derive(Clone, Copy)]
enum Arithmetic {
    Add,
    Subtract,
    Multiply,
    Divide,
}

fn arithmetic(
    left: RuntimeValue,
    right: RuntimeValue,
    operation: Arithmetic,
) -> Result<RuntimeValue, &'static str> {
    match (left, right) {
        (RuntimeValue::Int(left), RuntimeValue::Int(right)) => {
            let result = match operation {
                Arithmetic::Add => left.checked_add(right),
                Arithmetic::Subtract => left.checked_sub(right),
                Arithmetic::Multiply => left.checked_mul(right),
                Arithmetic::Divide => left.checked_div(right),
            }
            .ok_or(EXECUTION_ERROR)?;
            Ok(RuntimeValue::Int(result))
        }
        (
            RuntimeValue::Scalar {
                ty: left_type,
                value: left,
            },
            RuntimeValue::Scalar {
                ty: right_type,
                value: right,
            },
        ) => {
            let (result_type, result) = match operation {
                Arithmetic::Add if left_type == right_type => (left_type, left + right),
                Arithmetic::Subtract if left_type == right_type => (left_type, left - right),
                Arithmetic::Multiply
                    if left_type == right_type && left_type == ValueType::Float =>
                {
                    (ValueType::Float, left * right)
                }
                Arithmetic::Multiply if right_type == ValueType::Float => (left_type, left * right),
                Arithmetic::Multiply if left_type == ValueType::Float => (right_type, left * right),
                Arithmetic::Divide if right_type == ValueType::Float => (left_type, left / right),
                Arithmetic::Divide if left_type == right_type => (ValueType::Float, left / right),
                _ => return Err(EXECUTION_ERROR),
            };
            scalar(result_type, result)
        }
        // Execution ABI section 14: `U,int` and `int,U` multiplication plus
        // `U,int` division, with the integer rounded to binary64 first.
        (RuntimeValue::Scalar { ty, value }, RuntimeValue::Int(right)) if is_unit_scalar(ty) => {
            match operation {
                Arithmetic::Multiply => scalar(ty, value * right as f64),
                Arithmetic::Divide => scalar(ty, value / right as f64),
                _ => Err(EXECUTION_ERROR),
            }
        }
        (RuntimeValue::Int(left), RuntimeValue::Scalar { ty, value })
            if is_unit_scalar(ty) && matches!(operation, Arithmetic::Multiply) =>
        {
            scalar(ty, left as f64 * value)
        }
        (
            RuntimeValue::Vec2 {
                ty,
                value: [left_x, left_y],
            },
            RuntimeValue::Vec2 {
                ty: right_type,
                value: [right_x, right_y],
            },
        ) if ty == right_type && matches!(operation, Arithmetic::Add | Arithmetic::Subtract) => {
            let apply = |left: f64, right: f64| match operation {
                Arithmetic::Add => left + right,
                Arithmetic::Subtract => left - right,
                _ => unreachable!(),
            };
            vector(ty, [apply(left_x, right_x), apply(left_y, right_y)])
        }
        // Execution ABI section 14: integer vectors stay in checked i64 form;
        // overflow, division by zero, and `i64::MIN / -1` are execution
        // errors, and division truncates toward zero.
        (RuntimeValue::Vec2Int([left_x, left_y]), RuntimeValue::Vec2Int([right_x, right_y]))
            if matches!(operation, Arithmetic::Add | Arithmetic::Subtract) =>
        {
            let apply = |left: i64, right: i64| match operation {
                Arithmetic::Add => left.checked_add(right),
                Arithmetic::Subtract => left.checked_sub(right),
                _ => unreachable!(),
            };
            Ok(RuntimeValue::Vec2Int([
                apply(left_x, right_x).ok_or(EXECUTION_ERROR)?,
                apply(left_y, right_y).ok_or(EXECUTION_ERROR)?,
            ]))
        }
        (RuntimeValue::Vec2Int(value), RuntimeValue::Int(scalar))
            if matches!(operation, Arithmetic::Multiply | Arithmetic::Divide) =>
        {
            scale_vector_int(value, scalar, operation)
        }
        (RuntimeValue::Int(scalar), RuntimeValue::Vec2Int(value))
            if matches!(operation, Arithmetic::Multiply) =>
        {
            scale_vector_int(value, scalar, operation)
        }
        (RuntimeValue::Vec2 { ty, value }, scalar)
            if matches!(operation, Arithmetic::Multiply | Arithmetic::Divide) =>
        {
            scale_vector(ty, value, scalar, operation)
        }
        (scalar, RuntimeValue::Vec2 { ty, value }) if matches!(operation, Arithmetic::Multiply) => {
            scale_vector(ty, value, scalar, operation)
        }
        _ => Err(EXECUTION_ERROR),
    }
}

fn scale_vector(
    ty: ValueType,
    value: [f64; 2],
    scalar: RuntimeValue,
    operation: Arithmetic,
) -> Result<RuntimeValue, &'static str> {
    let scalar = match scalar {
        RuntimeValue::Int(value) => value as f64,
        RuntimeValue::Scalar {
            ty: ValueType::Float,
            value,
        } => value,
        _ => return Err(EXECUTION_ERROR),
    };
    let apply = |component: f64| match operation {
        Arithmetic::Multiply => component * scalar,
        Arithmetic::Divide => component / scalar,
        _ => unreachable!(),
    };
    vector(ty, [apply(value[0]), apply(value[1])])
}

fn scale_vector_int(
    value: [i64; 2],
    scalar: i64,
    operation: Arithmetic,
) -> Result<RuntimeValue, &'static str> {
    let apply = |component: i64| match operation {
        Arithmetic::Multiply => component.checked_mul(scalar),
        Arithmetic::Divide => component.checked_div(scalar),
        _ => unreachable!(),
    };
    Ok(RuntimeValue::Vec2Int([
        apply(value[0]).ok_or(EXECUTION_ERROR)?,
        apply(value[1]).ok_or(EXECUTION_ERROR)?,
    ]))
}

fn negate(value: RuntimeValue) -> Result<RuntimeValue, &'static str> {
    match value {
        RuntimeValue::Int(value) => Ok(RuntimeValue::Int(
            value.checked_neg().ok_or(EXECUTION_ERROR)?,
        )),
        RuntimeValue::Scalar { ty, value } => scalar(ty, -value),
        _ => Err(EXECUTION_ERROR),
    }
}

fn power(left: RuntimeValue, right: RuntimeValue) -> Result<RuntimeValue, &'static str> {
    match (left, right) {
        (RuntimeValue::Int(base), RuntimeValue::Int(exponent)) => {
            let exponent = u32::try_from(exponent).map_err(|_| EXECUTION_ERROR)?;
            Ok(RuntimeValue::Int(
                base.checked_pow(exponent).ok_or(EXECUTION_ERROR)?,
            ))
        }
        (
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: base,
            },
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: exponent,
            },
        ) => scalar(ValueType::Float, base.powf(exponent)),
        _ => Err(EXECUTION_ERROR),
    }
}

fn compare(
    left: RuntimeValue,
    right: RuntimeValue,
    opcode: u16,
) -> Result<RuntimeValue, &'static str> {
    let ordering = match (&left, &right) {
        (RuntimeValue::Int(left), RuntimeValue::Int(right)) => left.partial_cmp(right),
        (
            RuntimeValue::Scalar {
                ty: left_type,
                value: left,
            },
            RuntimeValue::Scalar {
                ty: right_type,
                value: right,
            },
        ) if left_type == right_type => left.partial_cmp(right),
        _ => return Err(EXECUTION_ERROR),
    }
    .ok_or(EXECUTION_ERROR)?;
    let result = match opcode {
        32 => ordering.is_lt(),
        33 => ordering.is_le(),
        34 => ordering.is_gt(),
        35 => ordering.is_ge(),
        _ => return Err(EXECUTION_ERROR),
    };
    Ok(RuntimeValue::Bool(result))
}

fn values_equal(left: &RuntimeValue, right: &RuntimeValue) -> Result<bool, &'static str> {
    match (left, right) {
        (RuntimeValue::Bool(left), RuntimeValue::Bool(right)) => Ok(left == right),
        (RuntimeValue::Int(left), RuntimeValue::Int(right)) => Ok(left == right),
        (
            RuntimeValue::Scalar {
                ty: left_type,
                value: left,
            },
            RuntimeValue::Scalar {
                ty: right_type,
                value: right,
            },
        ) if left_type == right_type => Ok(left == right),
        (RuntimeValue::Color(left), RuntimeValue::Color(right)) => Ok(left == right),
        (
            RuntimeValue::Vec2 {
                ty: left_type,
                value: left,
            },
            RuntimeValue::Vec2 {
                ty: right_type,
                value: right,
            },
        ) if left_type == right_type => Ok(left == right),
        (RuntimeValue::Vec2Int(left), RuntimeValue::Vec2Int(right)) => Ok(left == right),
        _ => Err(EXECUTION_ERROR),
    }
}

fn absolute(value: RuntimeValue) -> Result<RuntimeValue, &'static str> {
    match value {
        RuntimeValue::Int(value) => Ok(RuntimeValue::Int(
            value.checked_abs().ok_or(EXECUTION_ERROR)?,
        )),
        RuntimeValue::Scalar { ty, value } => scalar(ty, value.abs()),
        _ => Err(EXECUTION_ERROR),
    }
}

fn min_max(
    left: RuntimeValue,
    right: RuntimeValue,
    maximum: bool,
) -> Result<RuntimeValue, &'static str> {
    match (left, right) {
        (RuntimeValue::Int(left), RuntimeValue::Int(right)) => Ok(RuntimeValue::Int(if maximum {
            left.max(right)
        } else {
            left.min(right)
        })),
        (
            RuntimeValue::Scalar {
                ty: left_type,
                value: left,
            },
            RuntimeValue::Scalar {
                ty: right_type,
                value: right,
            },
        ) if left_type == right_type => scalar(
            left_type,
            if maximum {
                left.max(right)
            } else {
                left.min(right)
            },
        ),
        _ => Err(EXECUTION_ERROR),
    }
}

fn clamp(
    value: RuntimeValue,
    lower: RuntimeValue,
    upper: RuntimeValue,
) -> Result<RuntimeValue, &'static str> {
    match (value, lower, upper) {
        (RuntimeValue::Int(value), RuntimeValue::Int(lower), RuntimeValue::Int(upper))
            if lower <= upper =>
        {
            Ok(RuntimeValue::Int(value.clamp(lower, upper)))
        }
        (
            RuntimeValue::Scalar { ty, value },
            RuntimeValue::Scalar {
                ty: lower_type,
                value: lower,
            },
            RuntimeValue::Scalar {
                ty: upper_type,
                value: upper,
            },
        ) if ty == lower_type && ty == upper_type && lower <= upper => {
            scalar(ty, value.clamp(lower, upper))
        }
        _ => Err(EXECUTION_ERROR),
    }
}

fn unary_float(value: RuntimeValue, opcode: u16) -> Result<RuntimeValue, &'static str> {
    let input = match value {
        RuntimeValue::Scalar {
            ty: ValueType::Float,
            value,
        } => value,
        _ => return Err(EXECUTION_ERROR),
    };
    let output = match opcode {
        44 => input.floor(),
        45 => input.ceil(),
        46 => input.round_ties_even(),
        47 => input.sqrt(),
        48 => input.exp(),
        49 => input.ln(),
        50 => input.sin(),
        51 => input.cos(),
        52 => input.tan(),
        53 => input.asin(),
        54 => input.acos(),
        55 => input.atan(),
        _ => return Err(EXECUTION_ERROR),
    };
    scalar(ValueType::Float, output)
}

fn make_vec2(
    left: RuntimeValue,
    right: RuntimeValue,
    result_type: ValueType,
) -> Result<RuntimeValue, &'static str> {
    let element_type = result_type.vector_element().ok_or(EXECUTION_ERROR)?;
    if element_type == ValueType::Int {
        let (RuntimeValue::Int(x), RuntimeValue::Int(y)) = (&left, &right) else {
            return Err(EXECUTION_ERROR);
        };
        return Ok(RuntimeValue::Vec2Int([*x, *y]));
    }
    let left = component_payload(&left, element_type)?;
    let right = component_payload(&right, element_type)?;
    vector(result_type, [left, right])
}

fn vector_component(vector_value: RuntimeValue, use_y: bool) -> Result<RuntimeValue, &'static str> {
    match vector_value {
        RuntimeValue::Vec2 { ty, value } => {
            let element_type = ty.vector_element().ok_or(EXECUTION_ERROR)?;
            scalar(element_type, value[usize::from(use_y)])
        }
        RuntimeValue::Vec2Int(value) => Ok(RuntimeValue::Int(value[usize::from(use_y)])),
        _ => Err(EXECUTION_ERROR),
    }
}

fn component_payload(value: &RuntimeValue, expected: ValueType) -> Result<f64, &'static str> {
    match value {
        RuntimeValue::Scalar { ty, value } if *ty == expected => Ok(*value),
        _ => Err(EXECUTION_ERROR),
    }
}

fn scalar(ty: ValueType, value: f64) -> Result<RuntimeValue, &'static str> {
    if !value.is_finite() {
        return Err(EXECUTION_ERROR);
    }
    Ok(RuntimeValue::Scalar { ty, value })
}

fn vector(ty: ValueType, value: [f64; 2]) -> Result<RuntimeValue, &'static str> {
    if value.iter().any(|component| !component.is_finite()) {
        return Err(EXECUTION_ERROR);
    }
    Ok(RuntimeValue::Vec2 { ty, value })
}

fn boolean(value: &RuntimeValue) -> Result<bool, &'static str> {
    if let RuntimeValue::Bool(value) = value {
        Ok(*value)
    } else {
        Err(EXECUTION_ERROR)
    }
}

fn integer(value: &RuntimeValue) -> Result<i64, &'static str> {
    if let RuntimeValue::Int(value) = value {
        Ok(*value)
    } else {
        Err(EXECUTION_ERROR)
    }
}

fn scalar_payload(value: &RuntimeValue) -> Result<f64, &'static str> {
    if let RuntimeValue::Scalar { value, .. } = value {
        Ok(*value)
    } else {
        Err(EXECUTION_ERROR)
    }
}

fn interpolate_value(
    start: &RuntimeValue,
    end: &RuntimeValue,
    progress: f64,
) -> Result<RuntimeValue, &'static str> {
    match (start, end) {
        (
            RuntimeValue::Scalar {
                ty: start_type,
                value: start,
            },
            RuntimeValue::Scalar {
                ty: end_type,
                value: end,
            },
        ) if start_type == end_type => scalar(*start_type, start + (end - start) * progress),
        (RuntimeValue::Color(start), RuntimeValue::Color(end)) => {
            let mut result = [0.0; 4];
            for index in 0..4 {
                result[index] = start[index] + (end[index] - start[index]) * progress;
                if !result[index].is_finite() {
                    return Err(EXECUTION_ERROR);
                }
            }
            Ok(RuntimeValue::Color(result))
        }
        (
            RuntimeValue::Vec2 {
                ty: start_type,
                value: start,
            },
            RuntimeValue::Vec2 {
                ty: end_type,
                value: end,
            },
        ) if start_type == end_type => vector(
            *start_type,
            [
                start[0] + (end[0] - start[0]) * progress,
                start[1] + (end[1] - start[1]) * progress,
            ],
        ),
        _ => Err(EXECUTION_ERROR),
    }
}

fn easing(id: u16, progress: f64) -> Result<f64, &'static str> {
    evaluate_easing(id, progress).map_err(|_| EXECUTION_ERROR)
}

fn cubic_bezier_progress(bezier: [f64; 4], progress: f64) -> Result<f64, &'static str> {
    if !(0.0..=1.0).contains(&progress) {
        return Err(EXECUTION_ERROR);
    }
    evaluate_cubic_bezier_progress(bezier, progress).map_err(|_| EXECUTION_ERROR)
}

fn integrate_descriptor(
    chart: &DecodedChart,
    descriptor_index: u32,
    start: f64,
    end: f64,
    depth: usize,
) -> Result<f64, &'static str> {
    if depth > chart.descriptors.len() + 1 {
        return Err(EXECUTION_ERROR);
    }
    let descriptor = chart
        .descriptors
        .get(descriptor_index as usize)
        .ok_or(EXECUTION_ERROR)?;
    if !descriptor.domain.contains(start) || !descriptor.domain.contains(end) {
        return Err(EXECUTION_ERROR);
    }
    if start.to_bits() == end.to_bits() {
        return Ok(0.0);
    }
    if end < start {
        return Ok(-integrate_descriptor(
            chart,
            descriptor_index,
            end,
            start,
            depth + 1,
        )?);
    }
    let result = match &descriptor.kind {
        DescriptorKind::Constant(index) => {
            scalar_payload(
                chart
                    .constants
                    .get(*index as usize)
                    .ok_or(EXECUTION_ERROR)?,
            )? * (end - start)
        }
        DescriptorKind::SegmentTrack(segments) => {
            integrate_segment_track(chart, segments, start, end)?
        }
        DescriptorKind::Piecewise(pieces) => {
            let mut total = 0.0;
            let mut cursor = start;
            for piece in pieces {
                let interpreted_start = if piece.flags & 0b010 != 0 {
                    f64::NEG_INFINITY
                } else {
                    piece.start
                };
                let interpreted_end = if piece.flags & 0b100 != 0 {
                    f64::INFINITY
                } else {
                    piece.end
                };
                let piece_start = cursor.max(interpreted_start);
                let piece_end = end.min(interpreted_end);
                if piece_start < piece_end {
                    total += integrate_descriptor(
                        chart,
                        piece.descriptor_index,
                        piece_start,
                        piece_end,
                        depth + 1,
                    )?;
                    cursor = piece_end;
                }
                if cursor >= end {
                    break;
                }
            }
            if cursor < end {
                return Err(EXECUTION_ERROR);
            }
            total
        }
        DescriptorKind::Expression(_) => return Err(EXECUTION_ERROR),
    };
    if result.is_finite() {
        Ok(result)
    } else {
        Err(EXECUTION_ERROR)
    }
}

fn integrate_segment_track(
    chart: &DecodedChart,
    segments: &[Segment],
    start: f64,
    end: f64,
) -> Result<f64, &'static str> {
    let mut breakpoints = vec![start, end];
    for segment in segments {
        if start < segment.start && segment.start < end {
            breakpoints.push(segment.start);
        }
        if segment.flags & 1 == 0 && start < segment.end && segment.end < end {
            breakpoints.push(segment.end);
        }
    }
    breakpoints.sort_by(f64::total_cmp);
    breakpoints.dedup_by(|left, right| left.to_bits() == right.to_bits());
    let mut total = 0.0;
    for interval in breakpoints.windows(2) {
        let interval_start = interval[0];
        let interval_end = interval[1];
        let midpoint = interval_start + (interval_end - interval_start) * 0.5;
        if let Some(segment) = segments.iter().find(|segment| {
            segment.flags & 1 == 0 && segment.start <= midpoint && midpoint < segment.end
        }) {
            let start_value = scalar_payload(
                chart
                    .constants
                    .get(segment.start_constant as usize)
                    .ok_or(EXECUTION_ERROR)?,
            )?;
            let end_value = scalar_payload(
                chart
                    .constants
                    .get(segment.end_constant as usize)
                    .ok_or(EXECUTION_ERROR)?,
            )?;
            let area = match segment.interpolation {
                1 => start_value * (interval_end - interval_start),
                2 => {
                    let duration = segment.end - segment.start;
                    let slope = (end_value - start_value) / duration;
                    let local_start = interval_start - segment.start;
                    let local_end = interval_end - segment.start;
                    start_value * (interval_end - interval_start)
                        + slope * (local_end * local_end - local_start * local_start) * 0.5
                }
                _ => return Err(EXECUTION_ERROR),
            };
            total += area;
        } else {
            let point = segments
                .iter()
                .rfind(|segment| segment.flags & 1 != 0 && segment.start <= midpoint)
                .or_else(|| segments.first().filter(|segment| segment.flags & 1 != 0))
                .ok_or(EXECUTION_ERROR)?;
            let value = scalar_payload(
                chart
                    .constants
                    .get(point.start_constant as usize)
                    .ok_or(EXECUTION_ERROR)?,
            )?;
            total += value * (interval_end - interval_start);
        }
    }
    Ok(total)
}

fn integrate_scroll_product(
    chart: &DecodedChart,
    speed_descriptor: u32,
    tempo_descriptor: u32,
    start: f64,
    end: f64,
) -> Result<f64, &'static str> {
    if start.to_bits() == end.to_bits() {
        return Ok(0.0);
    }
    if end < start {
        return Ok(-integrate_scroll_product(
            chart,
            speed_descriptor,
            tempo_descriptor,
            end,
            start,
        )?);
    }
    if let Some(tempo) = constant_descriptor_scalar(chart, tempo_descriptor)? {
        let speed_integral = integrate_descriptor(chart, speed_descriptor, start, end, 0)?;
        let result = speed_integral * tempo / 60.0;
        return result.is_finite().then_some(result).ok_or(EXECUTION_ERROR);
    }
    if let Some(speed) = constant_descriptor_scalar(chart, speed_descriptor)? {
        let tempo_integral = integrate_descriptor(chart, tempo_descriptor, start, end, 0)?;
        let result = tempo_integral * speed / 60.0;
        return result.is_finite().then_some(result).ok_or(EXECUTION_ERROR);
    }

    let descriptor = chart
        .descriptors
        .get(tempo_descriptor as usize)
        .ok_or(EXECUTION_ERROR)?;
    let DescriptorKind::SegmentTrack(segments) = &descriptor.kind else {
        return Err(EXECUTION_ERROR);
    };
    if segments
        .iter()
        .any(|segment| segment.flags & 1 == 0 && segment.interpolation != 1)
    {
        return Err(EXECUTION_ERROR);
    }
    let mut breakpoints = vec![start, end];
    for segment in segments {
        if start < segment.start && segment.start < end {
            breakpoints.push(segment.start);
        }
        if segment.flags & 1 == 0 && start < segment.end && segment.end < end {
            breakpoints.push(segment.end);
        }
    }
    breakpoints.sort_by(f64::total_cmp);
    breakpoints.dedup_by(|left, right| left.to_bits() == right.to_bits());
    let mut total = 0.0;
    for interval in breakpoints.windows(2) {
        let interval_start = interval[0];
        let interval_end = interval[1];
        let midpoint = interval_start + (interval_end - interval_start) * 0.5;
        let tempo = scalar_payload(
            &query_descriptor(
                chart,
                tempo_descriptor,
                midpoint,
                EvaluationEnvironment::at_chart_time(chart, midpoint)?,
            )?
            .value,
        )?;
        if !tempo.is_finite() || tempo <= 0.0 {
            return Err(EXECUTION_ERROR);
        }
        total += integrate_descriptor(chart, speed_descriptor, interval_start, interval_end, 0)?
            * tempo
            / 60.0;
    }
    total.is_finite().then_some(total).ok_or(EXECUTION_ERROR)
}

fn constant_descriptor_scalar(
    chart: &DecodedChart,
    descriptor_index: u32,
) -> Result<Option<f64>, &'static str> {
    let descriptor = chart
        .descriptors
        .get(descriptor_index as usize)
        .ok_or(EXECUTION_ERROR)?;
    let DescriptorKind::Constant(index) = &descriptor.kind else {
        return Ok(None);
    };
    Ok(Some(scalar_payload(
        chart
            .constants
            .get(*index as usize)
            .ok_or(EXECUTION_ERROR)?,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{DescriptorKind, Domain, ExpressionNode, PropertyDescriptor, Segment};

    fn unbounded() -> Domain {
        Domain {
            start: 0.0,
            end: 0.0,
            unbounded_before: true,
            unbounded_after: true,
        }
    }

    #[test]
    fn vec2_int_expression_scales_by_int_operand() {
        let mut chart = crate::load_chart(&crate::write_nonempty_execution()).unwrap();
        let vector_constant = chart.constants.len() as u32;
        chart.constants.push(RuntimeValue::Vec2Int([1, 2]));
        let factor_constant = chart.constants.len() as u32;
        chart.constants.push(RuntimeValue::Int(3));

        let vector_node = chart.expressions.len() as u32;
        chart.expressions.push(ExpressionNode {
            opcode: 1,
            result_type: ValueType::Vec2Int,
            operands: [u32::MAX; 3],
            arity: 0,
            immediate: vector_constant,
        });
        let factor_node = chart.expressions.len() as u32;
        chart.expressions.push(ExpressionNode {
            opcode: 1,
            result_type: ValueType::Int,
            operands: [u32::MAX; 3],
            arity: 0,
            immediate: factor_constant,
        });

        for operands in [
            [vector_node, factor_node, u32::MAX],
            [factor_node, vector_node, u32::MAX],
        ] {
            let root = chart.expressions.len() as u32;
            chart.expressions.push(ExpressionNode {
                opcode: 22,
                result_type: ValueType::Vec2Int,
                operands,
                arity: 2,
                immediate: 0,
            });
            let descriptor = chart.descriptors.len() as u32;
            chart.descriptors.push(PropertyDescriptor {
                property_type: ValueType::Vec2Int,
                domain: unbounded(),
                kind: DescriptorKind::Expression(root),
            });

            assert_eq!(
                query_descriptor(&chart, descriptor, 0.0, EvaluationEnvironment::at_time(0.0))
                    .unwrap()
                    .value,
                RuntimeValue::Vec2Int([3, 6])
            );
        }
    }

    #[test]
    fn vec2_int_arithmetic_stays_checked_i64() {
        // Issue #648: integer vectors keep exact i64 components through
        // construction, arithmetic, comparison, and projection. Adjacent
        // integers above 2^53 stay distinct, division truncates toward zero,
        // and overflow, division by zero, and `i64::MIN / -1` are execution
        // errors instead of silent binary64 approximations.
        let mut chart = crate::load_chart(&crate::write_nonempty_execution()).unwrap();
        let push_constant = |chart: &mut DecodedChart, value: RuntimeValue| {
            let immediate = chart.constants.len() as u32;
            chart.constants.push(value.clone());
            let node = chart.expressions.len() as u32;
            chart.expressions.push(ExpressionNode {
                opcode: 1,
                result_type: value.value_type(),
                operands: [u32::MAX; 3],
                arity: 0,
                immediate,
            });
            node
        };
        let push_node =
            |chart: &mut DecodedChart, opcode: u16, result_type: ValueType, operands: [u32; 3]| {
                let node = chart.expressions.len() as u32;
                chart.expressions.push(ExpressionNode {
                    opcode,
                    result_type,
                    operands,
                    arity: operands
                        .iter()
                        .filter(|operand| **operand != u32::MAX)
                        .count() as u8,
                    immediate: 0,
                });
                node
            };
        let query =
            |chart: &mut DecodedChart, opcode: u16, result_type: ValueType, operands: [u32; 3]| {
                let root = push_node(chart, opcode, result_type, operands);
                let descriptor = chart.descriptors.len() as u32;
                chart.descriptors.push(PropertyDescriptor {
                    property_type: result_type,
                    domain: unbounded(),
                    kind: DescriptorKind::Expression(root),
                });
                query_descriptor(chart, descriptor, 0.0, EvaluationEnvironment::at_time(0.0))
                    .map(|evaluation| evaluation.value)
            };

        // Adjacent integers above 2^53 stay distinct: binary64 storage
        // collided them and made this equality true.
        let adjacent = push_constant(&mut chart, RuntimeValue::Vec2Int([(1 << 53) + 1, 0]));
        let neighbor = push_constant(&mut chart, RuntimeValue::Vec2Int([1 << 53, 0]));
        assert_eq!(
            query(
                &mut chart,
                30,
                ValueType::Bool,
                [adjacent, neighbor, u32::MAX]
            )
            .unwrap(),
            RuntimeValue::Bool(false),
            "(2^53 + 1, 0) != (2^53, 0)"
        );
        assert_eq!(
            query(
                &mut chart,
                30,
                ValueType::Bool,
                [adjacent, adjacent, u32::MAX]
            )
            .unwrap(),
            RuntimeValue::Bool(true)
        );

        // Signed i64 endpoints survive constants and component projection.
        let endpoints = push_constant(&mut chart, RuntimeValue::Vec2Int([i64::MAX, i64::MIN]));
        assert_eq!(
            query(
                &mut chart,
                81,
                ValueType::Int,
                [endpoints, u32::MAX, u32::MAX]
            )
            .unwrap(),
            RuntimeValue::Int(i64::MAX)
        );
        assert_eq!(
            query(
                &mut chart,
                82,
                ValueType::Int,
                [endpoints, u32::MAX, u32::MAX]
            )
            .unwrap(),
            RuntimeValue::Int(i64::MIN)
        );

        // Division truncates toward zero in both signs: (5, 7) / 2 = (2, 3)
        // and (-5, -5) / 2 = (-2, -2), with the vectors constructed from Int
        // operands first.
        let five = push_constant(&mut chart, RuntimeValue::Int(5));
        let neg_five = push_constant(&mut chart, RuntimeValue::Int(-5));
        let seven = push_constant(&mut chart, RuntimeValue::Int(7));
        let two = push_constant(&mut chart, RuntimeValue::Int(2));
        let zero = push_constant(&mut chart, RuntimeValue::Int(0));
        let minus_one = push_constant(&mut chart, RuntimeValue::Int(-1));
        let vector = push_node(&mut chart, 80, ValueType::Vec2Int, [five, seven, u32::MAX]);
        assert_eq!(
            query(&mut chart, 23, ValueType::Vec2Int, [vector, two, u32::MAX]).unwrap(),
            RuntimeValue::Vec2Int([2, 3]),
            "(5, 7) / 2 truncates toward zero"
        );
        let negative = push_node(
            &mut chart,
            80,
            ValueType::Vec2Int,
            [neg_five, neg_five, u32::MAX],
        );
        assert_eq!(
            query(
                &mut chart,
                23,
                ValueType::Vec2Int,
                [negative, two, u32::MAX]
            )
            .unwrap(),
            RuntimeValue::Vec2Int([-2, -2]),
            "(-5, -5) / 2 truncates toward zero"
        );

        // Add and Mul overflow, division by zero, and i64::MIN / -1 are
        // execution errors.
        let max_vector = push_constant(&mut chart, RuntimeValue::Vec2Int([i64::MAX, i64::MAX]));
        let one_vector = push_constant(&mut chart, RuntimeValue::Vec2Int([1, 1]));
        assert!(
            query(
                &mut chart,
                20,
                ValueType::Vec2Int,
                [max_vector, one_vector, u32::MAX]
            )
            .is_err(),
            "i64::MAX + 1 must overflow"
        );
        assert!(
            query(
                &mut chart,
                22,
                ValueType::Vec2Int,
                [max_vector, two, u32::MAX]
            )
            .is_err(),
            "i64::MAX * 2 must overflow"
        );
        assert!(
            query(&mut chart, 23, ValueType::Vec2Int, [vector, zero, u32::MAX]).is_err(),
            "division by integer zero must be an execution error"
        );
        let min_vector = push_constant(&mut chart, RuntimeValue::Vec2Int([i64::MIN, i64::MIN]));
        assert!(
            query(
                &mut chart,
                23,
                ValueType::Vec2Int,
                [min_vector, minus_one, u32::MAX]
            )
            .is_err(),
            "i64::MIN / -1 must be an execution error"
        );

        // int / vec2-int is not an ABI combination, and a float component
        // cannot construct an integer vector.
        assert!(
            query(&mut chart, 23, ValueType::Vec2Int, [two, vector, u32::MAX]).is_err(),
            "2 / (5, 7) must be an execution error"
        );
        let float_two = push_constant(
            &mut chart,
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 2.0,
            },
        );
        assert!(
            query(
                &mut chart,
                80,
                ValueType::Vec2Int,
                [float_two, five, u32::MAX]
            )
            .is_err(),
            "vec2(float, int) must be an execution error"
        );
    }

    #[test]
    fn unit_scalar_integer_mul_div_executes_abi_combinations() {
        let mut chart = crate::load_chart(&crate::write_nonempty_execution()).unwrap();
        let push_constant = |chart: &mut DecodedChart, value: RuntimeValue| {
            let immediate = chart.constants.len() as u32;
            chart.constants.push(value.clone());
            let node = chart.expressions.len() as u32;
            chart.expressions.push(ExpressionNode {
                opcode: 1,
                result_type: value.value_type(),
                operands: [u32::MAX; 3],
                arity: 0,
                immediate,
            });
            node
        };
        let query = |chart: &mut DecodedChart,
                     opcode: u16,
                     result_type: ValueType,
                     left: u32,
                     right: u32| {
            let root = chart.expressions.len() as u32;
            chart.expressions.push(ExpressionNode {
                opcode,
                result_type,
                operands: [left, right, u32::MAX],
                arity: 2,
                immediate: 0,
            });
            let descriptor = chart.descriptors.len() as u32;
            chart.descriptors.push(PropertyDescriptor {
                property_type: result_type,
                domain: unbounded(),
                kind: DescriptorKind::Expression(root),
            });
            query_descriptor(chart, descriptor, 0.0, EvaluationEnvironment::at_time(0.0))
                .map(|evaluation| evaluation.value)
        };

        let two = push_constant(&mut chart, RuntimeValue::Int(2));
        let zero = push_constant(&mut chart, RuntimeValue::Int(0));
        let three = push_constant(&mut chart, RuntimeValue::Int(3));
        let max = push_constant(&mut chart, RuntimeValue::Int(i64::MAX));
        for ty in [
            ValueType::Time,
            ValueType::Beat,
            ValueType::Length,
            ValueType::Angle,
        ] {
            let unit = push_constant(&mut chart, RuntimeValue::Scalar { ty, value: 0.75 });
            let one = push_constant(&mut chart, RuntimeValue::Scalar { ty, value: 1.0 });
            let huge = push_constant(&mut chart, RuntimeValue::Scalar { ty, value: 1e300 });
            let expected = |value: f64| RuntimeValue::Scalar { ty, value };

            assert_eq!(
                query(&mut chart, 22, ty, unit, two).unwrap(),
                expected(1.5),
                "{ty:?} * 2"
            );
            assert_eq!(
                query(&mut chart, 22, ty, two, unit).unwrap(),
                expected(1.5),
                "2 * {ty:?}"
            );
            assert_eq!(
                query(&mut chart, 23, ty, unit, two).unwrap(),
                expected(0.375),
                "{ty:?} / 2"
            );
            // i64::MAX is 2^63 - 1, nearer to 2^63 than to 2^63 - 1024, so
            // rounding to nearest lands on 9223372036854775808.0.
            assert_eq!(
                query(&mut chart, 22, ty, one, max).unwrap(),
                expected(9223372036854775808.0),
                "{ty:?} * i64::MAX"
            );
            // 2^53 + 1 is exactly halfway between 2^53 and 2^53 + 2, so
            // ties-to-even selects the even neighbor.
            let tie = push_constant(&mut chart, RuntimeValue::Int((1 << 53) + 1));
            assert_eq!(
                query(&mut chart, 22, ty, one, tie).unwrap(),
                expected(9007199254740992.0),
                "{ty:?} * 2^53 + 1"
            );
            assert!(
                query(&mut chart, 23, ty, unit, zero).is_err(),
                "{ty:?} / 0 must be an execution error"
            );
            assert!(
                query(&mut chart, 22, ty, huge, max).is_err(),
                "{ty:?} * i64::MAX with a huge unit must be an execution error"
            );
            // int,U division is not an ABI combination.
            assert!(
                query(&mut chart, 23, ty, three, unit).is_err(),
                "3 / {ty:?} must be an execution error"
            );
        }
        // float,int multiplication stays rejected in both directions.
        let float_two = push_constant(
            &mut chart,
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 2.0,
            },
        );
        assert!(query(&mut chart, 22, ValueType::Float, float_two, three).is_err());
        assert!(query(&mut chart, 22, ValueType::Float, three, float_two).is_err());
    }

    #[test]
    fn chart_time_environment_uses_global_chart_beat() {
        let mut chart = crate::load_chart(&crate::write_nonempty_execution()).unwrap();
        chart.tempo_points[0].bpm = 120.0;

        let environment = EvaluationEnvironment::at_chart_time(&chart, 1.0).unwrap();

        assert_eq!(environment.s, 1.0);
        assert_eq!(environment.b, 2.0);
    }

    #[test]
    fn cubic_bezier_progress_matches_canonical_vectors() {
        // The native evaluator delegates to the canonical correctly rounded
        // solver, so the canonical test vectors must hold here too: endpoint
        // pinning, identity controls, the flat-x overshoot, and the explicit
        // failures for invalid controls and unavailable enclosures.
        assert_eq!(cubic_bezier_progress([0.25, 2.0, 0.75, -1.0], 0.0), Ok(0.0));
        assert_eq!(cubic_bezier_progress([0.25, 2.0, 0.75, -1.0], 1.0), Ok(1.0));
        assert_eq!(cubic_bezier_progress([0.0, 0.0, 1.0, 1.0], 0.25), Ok(0.25));
        assert_eq!(cubic_bezier_progress([0.5, 2.0, 0.5, 2.0], 0.5), Ok(1.625));
        assert_eq!(
            cubic_bezier_progress([-0.25, 0.0, 0.75, 1.0], 0.5),
            Err(EXECUTION_ERROR)
        );
        assert_eq!(
            cubic_bezier_progress([0.25, 0.5, 0.75, f64::from_bits(1)], 0.25),
            Err(EXECUTION_ERROR)
        );
        // The native wrapper keeps its own progress range check: the shared
        // solver only rejects non-finite progress, so out-of-range and NaN
        // progress must be rejected before the delegation.
        for progress in [1.5, -0.2, f64::NAN] {
            assert_eq!(
                cubic_bezier_progress([0.25, 2.0, 0.75, -1.0], progress),
                Err(EXECUTION_ERROR)
            );
        }
    }

    #[test]
    fn shared_expression_subgraphs_evaluate_once_per_environment() {
        // Node i adds node i - 1 twice, so tree-shaped re-evaluation visits
        // the leaf about 2^26 times; the memo evaluates each node once per
        // environment while the entry-order trace still records every visit,
        // memo hits included.
        let mut chart = crate::load_chart(&crate::write_nonempty_execution()).unwrap();
        let one = chart.constants.len() as u32;
        chart.constants.push(RuntimeValue::Scalar {
            ty: ValueType::Float,
            value: 1.0,
        });

        let leaf = chart.expressions.len() as u32;
        chart.expressions.push(ExpressionNode {
            opcode: 1,
            result_type: ValueType::Float,
            operands: [u32::MAX; 3],
            arity: 0,
            immediate: one,
        });
        for level in 1..=26u32 {
            chart.expressions.push(ExpressionNode {
                opcode: 20,
                result_type: ValueType::Float,
                operands: [leaf + level - 1, leaf + level - 1, u32::MAX],
                arity: 2,
                immediate: 0,
            });
        }
        let root = chart.expressions.len() as u32 - 1;
        let descriptor = chart.descriptors.len() as u32;
        chart.descriptors.push(PropertyDescriptor {
            property_type: ValueType::Float,
            domain: unbounded(),
            kind: DescriptorKind::Expression(root),
        });

        let evaluation =
            query_descriptor(&chart, descriptor, 0.0, EvaluationEnvironment::at_time(0.0)).unwrap();
        assert_eq!(
            evaluation.value,
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 67_108_864.0
            }
        );
        let expected: Vec<u32> = (leaf..=leaf + 26).rev().chain(leaf..leaf + 26).collect();
        assert_eq!(evaluation.visited_nodes, expected);
    }

    #[test]
    fn a_point_cannot_be_revived_across_an_uncovered_interval() {
        // point(0)=1, ordinary [0,1): 1 -> 0.8, point(3)=0.5. The ordinary
        // begins at the point's own time, so the point's lifetime ends there
        // and [1,3) is uncovered: querying it must be an execution error,
        // not the old point's value.
        let mut chart = crate::load_chart(&crate::write_nonempty_execution()).unwrap();
        let base = chart.constants.len() as u32;
        chart.constants.extend([
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 1.0,
            },
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 0.8,
            },
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 0.5,
            },
        ]);
        let segments = vec![
            Segment {
                start: 0.0,
                end: 0.0,
                interpolation: 1,
                easing: 0,
                flags: 1,
                start_constant: base,
                end_constant: base,
                bezier: [0.0; 4],
            },
            Segment {
                start: 0.0,
                end: 1.0,
                interpolation: 2,
                easing: 0,
                flags: 0,
                start_constant: base,
                end_constant: base + 1,
                bezier: [0.0; 4],
            },
            Segment {
                start: 3.0,
                end: 3.0,
                interpolation: 1,
                easing: 0,
                flags: 1,
                start_constant: base + 2,
                end_constant: base + 2,
                bezier: [0.0; 4],
            },
        ];
        let descriptor = chart.descriptors.len() as u32;
        chart.descriptors.push(PropertyDescriptor {
            property_type: ValueType::Float,
            domain: unbounded(),
            kind: DescriptorKind::SegmentTrack(segments),
        });
        let query = |time: f64| {
            query_descriptor(
                &chart,
                descriptor,
                time,
                EvaluationEnvironment::at_time(time),
            )
        };
        assert_eq!(
            query(0.5).unwrap().value,
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 0.9,
            }
        );
        assert_eq!(query(2.0).unwrap_err(), "fcbc.execution-error");
        assert_eq!(
            query(4.0).unwrap().value,
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 0.5,
            }
        );
        assert_eq!(
            query(-1.0).unwrap().value,
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 1.0,
            }
        );
    }
}
