use super::fcbc_reference_loader::{
    DecodedChart, DescriptorKind, DistanceClassification, RuntimeValue, Segment, ValueType,
};

const EXECUTION_ERROR: &str = "fcbc.execution-error";

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
    let value = evaluate_descriptor_inner(
        chart,
        descriptor_index,
        time,
        environment,
        &mut visited_nodes,
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
                EvaluationEnvironment::at_time(time),
            )?;
            let tempo = query_descriptor(
                chart,
                line.scroll_tempo_descriptor,
                time,
                EvaluationEnvironment::at_time(time),
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

fn evaluate_descriptor_inner(
    chart: &DecodedChart,
    descriptor_index: u32,
    time: f64,
    environment: EvaluationEnvironment,
    visited_nodes: &mut Vec<u32>,
    depth: usize,
) -> Result<RuntimeValue, &'static str> {
    if depth > chart.descriptors.len() + 1 {
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
            evaluate_descriptor_inner(
                chart,
                piece.descriptor_index,
                time,
                environment,
                visited_nodes,
                depth + 1,
            )?
        }
        DescriptorKind::Expression(root) => {
            evaluate_node(chart, *root, environment, visited_nodes, depth + 1)?
        }
    };
    if value.value_type() != descriptor.property_type {
        return Err(EXECUTION_ERROR);
    }
    Ok(value)
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
    let point = segments
        .iter()
        .rfind(|segment| segment.flags & 1 != 0 && segment.start <= time)
        .or_else(|| segments.first().filter(|segment| segment.flags & 1 != 0))
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
    depth: usize,
) -> Result<RuntimeValue, &'static str> {
    if depth > chart.expressions.len() + 1 {
        return Err(EXECUTION_ERROR);
    }
    let node = chart
        .expressions
        .get(index as usize)
        .ok_or(EXECUTION_ERROR)?;
    visited_nodes.push(index);
    let operand =
        |operand_index: usize, visited: &mut Vec<u32>| -> Result<RuntimeValue, &'static str> {
            evaluate_node(
                chart,
                node.operands[operand_index],
                environment,
                visited,
                depth + 1,
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
        10 => negate(operand(0, visited_nodes)?)?,
        11 => RuntimeValue::Bool(!boolean(&operand(0, visited_nodes)?)?),
        20 => arithmetic(
            operand(0, visited_nodes)?,
            operand(1, visited_nodes)?,
            Arithmetic::Add,
        )?,
        21 => arithmetic(
            operand(0, visited_nodes)?,
            operand(1, visited_nodes)?,
            Arithmetic::Subtract,
        )?,
        22 => arithmetic(
            operand(0, visited_nodes)?,
            operand(1, visited_nodes)?,
            Arithmetic::Multiply,
        )?,
        23 => arithmetic(
            operand(0, visited_nodes)?,
            operand(1, visited_nodes)?,
            Arithmetic::Divide,
        )?,
        24 => {
            let left = integer(&operand(0, visited_nodes)?)?;
            let right = integer(&operand(1, visited_nodes)?)?;
            RuntimeValue::Int(left.checked_rem(right).ok_or(EXECUTION_ERROR)?)
        }
        25 => power(operand(0, visited_nodes)?, operand(1, visited_nodes)?)?,
        30 => RuntimeValue::Bool(values_equal(
            &operand(0, visited_nodes)?,
            &operand(1, visited_nodes)?,
        )?),
        31 => RuntimeValue::Bool(!values_equal(
            &operand(0, visited_nodes)?,
            &operand(1, visited_nodes)?,
        )?),
        32..=35 => compare(
            operand(0, visited_nodes)?,
            operand(1, visited_nodes)?,
            node.opcode,
        )?,
        36 => {
            let left = boolean(&operand(0, visited_nodes)?)?;
            if left {
                RuntimeValue::Bool(boolean(&operand(1, visited_nodes)?)?)
            } else {
                RuntimeValue::Bool(false)
            }
        }
        37 => {
            let left = boolean(&operand(0, visited_nodes)?)?;
            if left {
                RuntimeValue::Bool(true)
            } else {
                RuntimeValue::Bool(boolean(&operand(1, visited_nodes)?)?)
            }
        }
        38 => {
            let left = scalar_payload(&operand(0, visited_nodes)?)?;
            let right = scalar_payload(&operand(1, visited_nodes)?)?;
            let tolerance = scalar_payload(&operand(2, visited_nodes)?)?;
            if tolerance < 0.0 {
                return Err(EXECUTION_ERROR);
            }
            let difference = left - right;
            if !difference.is_finite() {
                return Err(EXECUTION_ERROR);
            }
            RuntimeValue::Bool(difference.abs() <= tolerance)
        }
        40 => absolute(operand(0, visited_nodes)?)?,
        41 | 42 => min_max(
            operand(0, visited_nodes)?,
            operand(1, visited_nodes)?,
            node.opcode == 42,
        )?,
        43 => clamp(
            operand(0, visited_nodes)?,
            operand(1, visited_nodes)?,
            operand(2, visited_nodes)?,
        )?,
        44..=55 => unary_float(operand(0, visited_nodes)?, node.opcode)?,
        56 => {
            let left = scalar_payload(&operand(0, visited_nodes)?)?;
            let right = scalar_payload(&operand(1, visited_nodes)?)?;
            scalar(ValueType::Float, left.atan2(right))?
        }
        60 => {
            let input = scalar_payload(&operand(0, visited_nodes)?)?;
            scalar(ValueType::Float, easing(node.immediate as u16, input)?)?
        }
        61 => {
            let value = integer(&operand(0, visited_nodes)?)? as f64;
            scalar(ValueType::Float, value)?
        }
        62 | 63 => {
            let value = scalar_payload(&operand(0, visited_nodes)?)?;
            scalar(ValueType::Float, value)?
        }
        70 => {
            if boolean(&operand(0, visited_nodes)?)? {
                operand(1, visited_nodes)?
            } else {
                operand(2, visited_nodes)?
            }
        }
        80 => {
            let left = operand(0, visited_nodes)?;
            let right = operand(1, visited_nodes)?;
            make_vec2(left, right, node.result_type)?
        }
        81 | 82 => {
            let vector = operand(0, visited_nodes)?;
            vector_component(vector, node.opcode == 82)?
        }
        _ => return Err(EXECUTION_ERROR),
    };
    if value.value_type() != node.result_type {
        return Err(EXECUTION_ERROR);
    }
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
        (
            RuntimeValue::Vec2 { ty, value },
            RuntimeValue::Scalar {
                ty: scalar_type,
                value: scalar_value,
            },
        ) if matches!(operation, Arithmetic::Multiply | Arithmetic::Divide)
            && matches!(scalar_type, ValueType::Float) =>
        {
            let apply = |component: f64| match operation {
                Arithmetic::Multiply => component * scalar_value,
                Arithmetic::Divide => component / scalar_value,
                _ => unreachable!(),
            };
            vector(ty, [apply(value[0]), apply(value[1])])
        }
        _ => Err(EXECUTION_ERROR),
    }
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
    let left = component_payload(&left, element_type)?;
    let right = component_payload(&right, element_type)?;
    vector(result_type, [left, right])
}

fn vector_component(vector_value: RuntimeValue, use_y: bool) -> Result<RuntimeValue, &'static str> {
    let RuntimeValue::Vec2 { ty, value } = vector_value else {
        return Err(EXECUTION_ERROR);
    };
    let element_type = ty.vector_element().ok_or(EXECUTION_ERROR)?;
    let component = value[usize::from(use_y)];
    if element_type == ValueType::Int {
        if component.fract() != 0.0 || component < i64::MIN as f64 || component > i64::MAX as f64 {
            return Err(EXECUTION_ERROR);
        }
        Ok(RuntimeValue::Int(component as i64))
    } else {
        scalar(element_type, component)
    }
}

fn component_payload(value: &RuntimeValue, expected: ValueType) -> Result<f64, &'static str> {
    match value {
        RuntimeValue::Int(value) if expected == ValueType::Int => Ok(*value as f64),
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
    if !(0.0..=1.0).contains(&progress) {
        return Err(EXECUTION_ERROR);
    }
    let output = match id {
        0 => progress,
        1 => 1.0 - (progress * std::f64::consts::FRAC_PI_2).cos(),
        2 => (progress * std::f64::consts::FRAC_PI_2).sin(),
        3 => (1.0 - (std::f64::consts::PI * progress).cos()) / 2.0,
        4 => progress * progress,
        5 => 1.0 - (1.0 - progress) * (1.0 - progress),
        6 => {
            if progress < 0.5 {
                2.0 * progress * progress
            } else {
                1.0 - (-2.0 * progress + 2.0).powi(2) / 2.0
            }
        }
        _ => return Err(EXECUTION_ERROR),
    };
    if output.is_finite() {
        Ok(output)
    } else {
        Err(EXECUTION_ERROR)
    }
}

fn cubic_bezier_progress(bezier: [f64; 4], progress: f64) -> Result<f64, &'static str> {
    if !(0.0..=1.0).contains(&progress) {
        return Err(EXECUTION_ERROR);
    }
    let [x1, y1, x2, y2] = bezier;
    let sample = |parameter: f64, first: f64, second: f64| {
        let inverse = 1.0 - parameter;
        3.0 * inverse * inverse * parameter * first
            + 3.0 * inverse * parameter * parameter * second
            + parameter * parameter * parameter
    };
    let mut lower = 0.0;
    let mut upper = 1.0;
    for _ in 0..64 {
        let middle = (lower + upper) * 0.5;
        if sample(middle, x1, x2) < progress {
            lower = middle;
        } else {
            upper = middle;
        }
    }
    let result = sample((lower + upper) * 0.5, y1, y2);
    if result.is_finite() {
        Ok(result)
    } else {
        Err(EXECUTION_ERROR)
    }
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
    Err(EXECUTION_ERROR)
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
