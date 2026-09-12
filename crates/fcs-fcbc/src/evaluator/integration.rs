//! Deterministic direct integration with whole-panel error enclosures.

use super::enclosure::{Bounds, Math, Taylor};
use super::*;
use astro_float::{BigFloat, RoundingMode};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// One budget covers all panels, DAG entries and nested Line-local q queries.
pub const MAX_INTEGRATION_EVALUATIONS: usize = 65_536;
pub const MAX_INTEGRATION_DEPTH: usize = 64;
const ABSOLUTE_ERROR: f64 = 2.328_306_436_538_696_3e-10;
const COORDINATE_ERROR: f64 = 5.684_341_886_080_802e-14;
const PRECISION: usize = 256;

pub(super) fn distance(
    chart: &DecodedChart,
    line: &super::super::loader::LineRecord,
    descriptor: &super::super::loader::DistanceDescriptor,
    time: f64,
) -> Result<DistanceEvaluation, &'static str> {
    let mut query = Query::new(chart, line.scroll_tempo_descriptor, &descriptor.boundaries)?;
    let bound = query.integrate(
        Some((line.scroll_speed_descriptor, line.line_flags & 1 != 0)),
        descriptor.integration_origin,
        time,
        descriptor.initial_floor_position,
        ABSOLUTE_ERROR,
    )?;
    Ok(DistanceEvaluation {
        floor_position: normalized_midpoint(bound),
        classification: descriptor.classification,
        visited_nodes: query.visited_nodes,
    })
}

pub(super) fn coordinate(chart: &DecodedChart, tempo: u32, time: f64) -> Result<f64, &'static str> {
    if time == 0.0 {
        return Ok(0.0);
    }
    let tempo_times: Vec<_> = chart
        .tempo_points
        .iter()
        .map(|point| point.chart_time)
        .collect();
    let boundaries = super::super::loader::expected_distance_boundaries(
        0.0,
        tempo,
        tempo,
        &chart.descriptors,
        &chart.expressions,
        &tempo_times,
    )
    .map_err(|_| EXECUTION_ERROR)?;
    let mut query = Query::new(chart, tempo, &boundaries)?;
    let bound = query.integrate(None, 0.0, time, 0.0, COORDINATE_ERROR)?;
    Ok(normalized_midpoint(bound))
}

fn normalized_midpoint(bound: Bounds) -> f64 {
    let value = bound.midpoint();
    if value == 0.0 { 0.0 } else { value }
}

fn tolerance(bound: Bounds, absolute: f64) -> f64 {
    let minimum = if bound.contains(0.0) {
        0.0
    } else {
        bound.lo.abs().min(bound.hi.abs())
    };
    absolute.max(4.0 * (minimum.next_up() - minimum))
}

struct Query<'a> {
    chart: &'a DecodedChart,
    tempo: u32,
    boundaries: &'a [f64],
    math: Math,
    evaluations: usize,
    visited_nodes: Vec<u32>,
}

impl<'a> Query<'a> {
    fn new(
        chart: &'a DecodedChart,
        tempo: u32,
        boundaries: &'a [f64],
    ) -> Result<Self, &'static str> {
        Ok(Self {
            chart,
            tempo,
            boundaries,
            math: Math::new().ok_or(EXECUTION_ERROR)?,
            evaluations: 0,
            visited_nodes: Vec::new(),
        })
    }

    fn charge(&mut self) -> Result<(), &'static str> {
        if self.evaluations >= MAX_INTEGRATION_EVALUATIONS {
            return Err(EXECUTION_ERROR);
        }
        self.evaluations += 1;
        Ok(())
    }

    fn panel(
        &mut self,
        speed: Option<(u32, bool)>,
        start: f64,
        end: f64,
        depth: usize,
    ) -> Result<Panel, &'static str> {
        self.charge()?;
        let bound = PanelEvaluator::new(self, start, end).and_then(|mut evaluator| {
            let tempo = evaluator
                .descriptor(evaluator.query.tempo, Taylor::constant(0.0), false, 0)?
                .float()?;
            if tempo.range().finite()?.lo <= 0.0 {
                return None;
            }
            evaluator.tempo = Some(tempo);
            let integrand = if let Some((root, allow_reverse)) = speed {
                let speed = evaluator
                    .descriptor(root, Taylor::constant(0.0), true, 0)?
                    .float()?;
                if !allow_reverse && speed.range().finite()?.lo < 0.0 {
                    return None;
                }
                // These are separate ABI binary64 nodes, in the specified order.
                (speed * tempo).divide(Taylor::constant(60.0))?
            } else {
                // q is the integral of BPM / 60, not a sampled Distance root.
                tempo.scale(Bounds::point(1.0) / Bounds::point(60.0))
            };
            integrand.range().finite()?;
            integrand.integral(start, end).finite()
        });
        if self.evaluations >= MAX_INTEGRATION_EVALUATIONS {
            return Err(EXECUTION_ERROR);
        }
        Ok(Panel {
            start,
            end,
            depth,
            bound,
        })
    }

    fn integrate(
        &mut self,
        speed: Option<(u32, bool)>,
        origin: f64,
        time: f64,
        initial: f64,
        absolute: f64,
    ) -> Result<Bounds, &'static str> {
        if !origin.is_finite() || !time.is_finite() || !initial.is_finite() {
            return Err(EXECUTION_ERROR);
        }
        if time == origin {
            return Ok(Bounds::point(initial));
        }
        let (start, end) = (origin.min(time), origin.max(time));
        let reverse = time < origin;
        let mut heap = BinaryHeap::new();
        let mut total = Sum::new();
        let mut uncertain = 0usize;
        let mut previous = start;
        // Only known graph boundaries are used here. Adaptive subdivisions stay
        // inside this query; they never become FCBC data or frame history.
        for index in 0..=self.boundaries.len() {
            let boundary = self.boundaries.get(index).copied().unwrap_or(end);
            if boundary <= previous || boundary > end {
                continue;
            }
            let panel = self.panel(speed, previous, boundary, 0)?;
            total.update(panel.bound, false);
            uncertain += usize::from(panel.bound.is_none());
            heap.push(panel);
            previous = boundary;
            if previous == end {
                break;
            }
        }
        if previous < end {
            let panel = self.panel(speed, previous, end, 0)?;
            total.update(panel.bound, false);
            uncertain += usize::from(panel.bound.is_none());
            heap.push(panel);
        }
        loop {
            if uncertain == 0 {
                let bound = total
                    .absolute(initial, reverse, &mut self.math)
                    .ok_or(EXECUTION_ERROR)?;
                let midpoint = bound.midpoint();
                let error = (midpoint - bound.lo).max(bound.hi - midpoint).next_up();
                if error <= tolerance(bound, absolute) {
                    return Ok(bound);
                }
            }
            let panel = heap.pop().ok_or(EXECUTION_ERROR)?;
            let middle = panel.start * 0.5 + panel.end * 0.5;
            if panel.depth >= MAX_INTEGRATION_DEPTH || middle <= panel.start || middle >= panel.end
            {
                return Err(EXECUTION_ERROR);
            }
            total.update(panel.bound, true);
            uncertain -= usize::from(panel.bound.is_none());
            for (start, end) in [(panel.start, middle), (middle, panel.end)] {
                let child = self.panel(speed, start, end, panel.depth + 1)?;
                total.update(child.bound, false);
                uncertain += usize::from(child.bound.is_none());
                heap.push(child);
            }
        }
    }
}

struct Sum {
    lower: BigFloat,
    upper: BigFloat,
}

impl Sum {
    fn new() -> Self {
        Self {
            lower: BigFloat::from_f64(0.0, PRECISION),
            upper: BigFloat::from_f64(0.0, PRECISION),
        }
    }

    fn update(&mut self, bound: Option<Bounds>, subtract: bool) {
        let Some(bound) = bound else {
            return;
        };
        let lower = BigFloat::from_f64(bound.lo, PRECISION);
        let upper = BigFloat::from_f64(bound.hi, PRECISION);
        if subtract {
            self.lower = self.lower.sub(&lower, PRECISION, RoundingMode::Down);
            self.upper = self.upper.sub(&upper, PRECISION, RoundingMode::Up);
        } else {
            self.lower = self.lower.add(&lower, PRECISION, RoundingMode::Down);
            self.upper = self.upper.add(&upper, PRECISION, RoundingMode::Up);
        }
    }

    fn absolute(&self, initial: f64, reverse: bool, math: &mut Math) -> Option<Bounds> {
        let initial = BigFloat::from_f64(initial, PRECISION);
        let (lower, upper) = if reverse {
            (
                initial.sub(&self.upper, PRECISION, RoundingMode::Down),
                initial.sub(&self.lower, PRECISION, RoundingMode::Up),
            )
        } else {
            (
                initial.add(&self.lower, PRECISION, RoundingMode::Down),
                initial.add(&self.upper, PRECISION, RoundingMode::Up),
            )
        };
        Bounds {
            lo: math.float_bound(&lower, true)?,
            hi: math.float_bound(&upper, false)?,
        }
        .finite()
    }
}

struct Panel {
    start: f64,
    end: f64,
    depth: usize,
    bound: Option<Bounds>,
}

impl Panel {
    fn error(&self) -> f64 {
        self.bound
            .map_or(f64::INFINITY, |bound| bound.hi - bound.lo)
    }
}
impl PartialEq for Panel {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Panel {}
impl PartialOrd for Panel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Panel {
    fn cmp(&self, other: &Self) -> Ordering {
        self.error()
            .total_cmp(&other.error())
            .then_with(|| other.start.total_cmp(&self.start))
            .then_with(|| other.end.total_cmp(&self.end))
    }
}

struct PanelEvaluator<'a, 'q> {
    query: &'q mut Query<'a>,
    start: f64,
    end: f64,
    time: Taylor,
    beat: Taylor,
    tempo: Option<Taylor>,
    coordinate: Option<Taylor>,
}

impl<'a, 'q> PanelEvaluator<'a, 'q> {
    fn new(query: &'q mut Query<'a>, start: f64, end: f64) -> Option<Self> {
        let time = Taylor::variable(start, end);
        let first = query.chart.tempo_points.first()?;
        let point = query
            .chart
            .tempo_points
            .iter()
            .rfind(|point| point.chart_time <= start)
            .unwrap_or(first);
        let beat = Taylor::constant(point.beat_numerator as f64 / point.beat_denominator as f64)
            + ((time - Taylor::constant(point.chart_time)) * Taylor::constant(point.bpm))
                .divide(Taylor::constant(60.0))?;
        Some(Self {
            query,
            start,
            end,
            time,
            beat,
            tempo: None,
            coordinate: None,
        })
    }

    fn coordinate(&mut self) -> Option<Taylor> {
        if let Some(value) = self.coordinate {
            return Some(value);
        }
        let middle = self.start * 0.5 + self.end * 0.5;
        let center = self
            .query
            .integrate(None, 0.0, middle, 0.0, COORDINATE_ERROR)
            .ok()?;
        let tempo = self.tempo?;
        let value = Taylor::enclosed(center)
            .real_add(
                tempo
                    .primitive(self.start, self.end)
                    .scale(Bounds::point(1.0) / Bounds::point(60.0)),
            )
            .rounded();
        self.coordinate = Some(value);
        Some(value)
    }

    fn descriptor(
        &mut self,
        index: u32,
        progress: Taylor,
        allow_q: bool,
        depth: usize,
    ) -> Option<Value> {
        self.query.charge().ok()?;
        if depth >= MAX_VALIDATOR_DEPTH {
            return None;
        }
        let descriptor = self.query.chart.descriptors.get(index as usize)?;
        if !descriptor.domain.contains(self.start) || !descriptor.domain.contains(self.end) {
            return None;
        }
        match &descriptor.kind {
            DescriptorKind::Constant(index) => Some(Value::Exact(
                self.query.chart.constants.get(*index as usize)?.clone(),
            )),
            DescriptorKind::Expression(root) => {
                self.expression(*root, progress, allow_q, &mut BTreeMap::new(), depth + 1)
            }
            DescriptorKind::Piecewise(pieces) => {
                // Evaluate the open panel and its one-sided limits. A different
                // half-open piece at the right endpoint has zero integral mass.
                let piece = pieces.iter().find(|piece| {
                    (piece.flags & 0b010 != 0 || piece.start <= self.start)
                        && (piece.flags & 0b100 != 0 || piece.end >= self.end)
                })?;
                let progress = match (piece.flags & 0b010 != 0, piece.flags & 0b100 != 0) {
                    (false, false) => (self.time - Taylor::constant(piece.start))
                        .divide(Taylor::constant(piece.end - piece.start))?,
                    (false, true) => Taylor::constant(1.0),
                    _ => Taylor::constant(0.0),
                };
                self.descriptor(piece.descriptor_index, progress, allow_q, depth + 1)
            }
            DescriptorKind::SegmentTrack(segments) => {
                if let Some(segment) = segments.iter().find(|segment| {
                    segment.flags & 1 == 0 && segment.start <= self.start && segment.end >= self.end
                }) {
                    let start = self
                        .query
                        .chart
                        .constants
                        .get(segment.start_constant as usize)?;
                    if segment.interpolation == 1 {
                        return Some(Value::Exact(start.clone()));
                    }
                    let end = self
                        .query
                        .chart
                        .constants
                        .get(segment.end_constant as usize)?;
                    let progress = (self.time - Taylor::constant(segment.start))
                        .divide(Taylor::constant(segment.end - segment.start))?;
                    let progress = match segment.interpolation {
                        2 => progress,
                        3 => self.query.math.easing(segment.easing, progress)?,
                        4 => self.query.math.bezier(segment.bezier, progress)?,
                        _ => return None,
                    };
                    let start = Taylor::constant(scalar_payload(start).ok()?);
                    let end = Taylor::constant(scalar_payload(end).ok()?);
                    Some(Value::Float(start + (end - start) * progress))
                } else {
                    // Known boundaries were split before entry, so this panel
                    // has the same point-hold lifetime throughout its interior.
                    evaluate_segment_track(self.query.chart, segments, self.start)
                        .ok()
                        .map(Value::Exact)
                }
            }
        }
    }

    fn expression(
        &mut self,
        index: u32,
        progress: Taylor,
        allow_q: bool,
        memo: &mut BTreeMap<u32, Value>,
        depth: usize,
    ) -> Option<Value> {
        self.query.charge().ok()?;
        if depth >= MAX_VALIDATOR_DEPTH {
            return None;
        }
        self.query.visited_nodes.push(index);
        if let Some(value) = memo.get(&index) {
            return Some(value.clone());
        }
        let node = self.query.chart.expressions.get(index as usize)?;
        let value = match node.opcode {
            1 => Value::Exact(
                self.query
                    .chart
                    .constants
                    .get(node.immediate as usize)?
                    .clone(),
            ),
            2 => Value::Float(self.time),
            3 => Value::Float(self.beat),
            4 if allow_q => Value::Float(self.coordinate()?),
            4 | 5 => return None,
            6 => Value::Float(progress),
            36 | 37 | 70 => {
                let condition = self
                    .expression(node.operands[0], progress, allow_q, memo, depth + 1)?
                    .boolean()?;
                match (node.opcode, condition) {
                    (36, Some(false)) => Value::Exact(RuntimeValue::Bool(false)),
                    (37, Some(true)) => Value::Exact(RuntimeValue::Bool(true)),
                    (36 | 37, Some(_)) => {
                        self.expression(node.operands[1], progress, allow_q, memo, depth + 1)?
                    }
                    (36 | 37, None) => {
                        let right = self
                            .expression(node.operands[1], progress, allow_q, memo, depth + 1)?
                            .boolean()?;
                        Value::truth(match (node.opcode, right) {
                            (36, Some(false)) => Some(false),
                            (37, Some(true)) => Some(true),
                            _ => None,
                        })
                    }
                    (70, Some(value)) => self.expression(
                        node.operands[if value { 1 } else { 2 }],
                        progress,
                        allow_q,
                        memo,
                        depth + 1,
                    )?,
                    (70, None) => {
                        let left =
                            self.expression(node.operands[1], progress, allow_q, memo, depth + 1)?;
                        let right =
                            self.expression(node.operands[2], progress, allow_q, memo, depth + 1)?;
                        left.hull(right)?
                    }
                    _ => return None,
                }
            }
            _ => {
                let mut operands = Vec::with_capacity(node.arity as usize);
                for operand in node.operands.iter().take(node.arity as usize) {
                    operands.push(self.expression(*operand, progress, allow_q, memo, depth + 1)?);
                }
                if operands
                    .iter()
                    .all(|value| matches!(value, Value::Exact(_)))
                {
                    Value::Exact(
                        evaluate_node_value(
                            self.query.chart,
                            node,
                            EvaluationEnvironment::at_time(0.0),
                            |index| {
                                if let Value::Exact(value) = &operands[index] {
                                    Ok(value.clone())
                                } else {
                                    Err(EXECUTION_ERROR)
                                }
                            },
                        )
                        .ok()?,
                    )
                } else {
                    self.operation(node, &operands)?
                }
            }
        };
        value.finite()?;
        memo.insert(index, value.clone());
        Some(value)
    }

    fn operation(
        &mut self,
        node: &super::super::loader::ExpressionNode,
        values: &[Value],
    ) -> Option<Value> {
        let float = |index: usize| values.get(index)?.float();
        let truth = Value::truth;
        if node.result_type == ValueType::Int {
            return integer_operation(node.opcode, values);
        }
        let result = match node.opcode {
            10 => Value::Float(-float(0)?),
            11 => truth(values[0].boolean()?.map(|value| !value)),
            20..=23 => {
                let apply = |left: Taylor, right: Taylor| match node.opcode {
                    20 => Some(left + right),
                    21 => Some(left - right),
                    22 => Some(left * right),
                    23 => left.divide(right),
                    _ => None,
                };
                if node.result_type.vector_element().is_some() {
                    let left = values[0].components()?;
                    let right = values[1].components()?;
                    Value::Vector([apply(left[0], right[0])?, apply(left[1], right[1])?])
                } else {
                    Value::Float(apply(float(0)?, float(1)?)?)
                }
            }
            25 => Value::Float(self.query.math.power(float(0)?, float(1)?)?),
            30..=35 => truth(comparison(&values[0], &values[1], node.opcode)?),
            38 => {
                let tolerance = float(2)?.range();
                if tolerance.lo < 0.0 {
                    return None;
                }
                let difference = absolute(float(0)? - float(1)?);
                truth(compare_bounds(difference.range(), tolerance, 33))
            }
            40 => Value::Float(absolute(float(0)?)),
            41 | 42 => Value::Float(min_max(float(0)?, float(1)?, node.opcode == 42)),
            43 => {
                let lower = float(1)?;
                let upper = float(2)?;
                if lower.range().hi > upper.range().lo {
                    return None;
                }
                Value::Float(min_max(min_max(float(0)?, lower, true), upper, false))
            }
            44..=55 => Value::Float(self.query.math.unary(node.opcode, float(0)?)?),
            56 => Value::Float(self.query.math.atan2(float(0)?, float(1)?)?),
            60 => Value::Float(self.query.math.easing(node.immediate as u16, float(0)?)?),
            61..=63 => Value::Float(float(0)?),
            80 => Value::Vector([float(0)?, float(1)?]),
            81 | 82 => Value::Float(values[0].components()?[usize::from(node.opcode == 82)]),
            _ => return None,
        };
        Some(result)
    }
}

#[derive(Clone)]
enum Value {
    Exact(RuntimeValue),
    Float(Taylor),
    Vector([Taylor; 2]),
    Integer { lo: i64, hi: i64 },
    UnknownBool,
}

impl Value {
    fn truth(value: Option<bool>) -> Self {
        value.map_or(Self::UnknownBool, |value| {
            Self::Exact(RuntimeValue::Bool(value))
        })
    }

    fn boolean(&self) -> Option<Option<bool>> {
        match self {
            Self::Exact(RuntimeValue::Bool(value)) => Some(Some(*value)),
            Self::UnknownBool => Some(None),
            _ => None,
        }
    }

    fn integers(&self) -> Option<(i64, i64)> {
        match self {
            Self::Exact(RuntimeValue::Int(value)) => Some((*value, *value)),
            Self::Integer { lo, hi } => Some((*lo, *hi)),
            _ => None,
        }
    }

    fn float(&self) -> Option<Taylor> {
        match self {
            Self::Float(value) => Some(*value),
            Self::Exact(RuntimeValue::Scalar { value, .. }) => Some(Taylor::constant(*value)),
            Self::Exact(RuntimeValue::Int(value)) => Some(Taylor::constant(*value as f64)),
            Self::Integer { lo, hi } => Some(Taylor::enclosed(Bounds {
                lo: *lo as f64,
                hi: *hi as f64,
            })),
            _ => None,
        }
    }

    fn components(&self) -> Option<[Taylor; 2]> {
        match self {
            Self::Vector(values) => Some(*values),
            Self::Exact(RuntimeValue::Vec2 { value, .. }) => Some(value.map(Taylor::constant)),
            _ => self.float().map(|value| [value; 2]),
        }
    }

    fn finite(&self) -> Option<()> {
        match self {
            Self::Float(value) => {
                value.range().finite()?;
            }
            Self::Vector(values) => {
                for value in values {
                    value.range().finite()?;
                }
            }
            Self::Integer { lo, hi } if lo > hi => return None,
            _ => {}
        }
        Some(())
    }

    fn hull(self, other: Self) -> Option<Self> {
        if let (Self::Exact(left), Self::Exact(right)) = (&self, &other)
            && left == right
        {
            return Some(self);
        }
        if let (Some(left), Some(right)) = (self.integers(), other.integers()) {
            return Some(Self::Integer {
                lo: left.0.min(right.0),
                hi: left.1.max(right.1),
            });
        }
        if let (Some(left), Some(right)) = (self.boolean(), other.boolean()) {
            return Some(Self::truth(if left == right { left } else { None }));
        }
        if let (Some(left), Some(right)) = (self.float(), other.float()) {
            return Some(Self::Float(Taylor::enclosed(
                left.range().hull(right.range()),
            )));
        }
        let left = self.components()?;
        let right = other.components()?;
        Some(Self::Vector(std::array::from_fn(|i| {
            Taylor::enclosed(left[i].range().hull(right[i].range()))
        })))
    }
}

fn absolute(value: Taylor) -> Taylor {
    let range = value.range();
    if range.lo >= 0.0 {
        value
    } else if range.hi <= 0.0 {
        -value
    } else {
        Taylor::enclosed(Bounds {
            lo: 0.0,
            hi: range.magnitude(),
        })
    }
}

fn min_max(left: Taylor, right: Taylor, maximum: bool) -> Taylor {
    let a = left.range();
    let b = right.range();
    if a.hi <= b.lo {
        return if maximum { right } else { left };
    }
    if b.hi <= a.lo {
        return if maximum { left } else { right };
    }
    Taylor::enclosed(if maximum {
        Bounds {
            lo: a.lo.max(b.lo),
            hi: a.hi.max(b.hi),
        }
    } else {
        Bounds {
            lo: a.lo.min(b.lo),
            hi: a.hi.min(b.hi),
        }
    })
}

fn compare_bounds(left: Bounds, right: Bounds, opcode: u16) -> Option<bool> {
    match opcode {
        30 | 31 => {
            let equal = if left.lo == left.hi && left == right {
                Some(true)
            } else if left.hi < right.lo || right.hi < left.lo {
                Some(false)
            } else {
                None
            };
            equal.map(|equal| if opcode == 30 { equal } else { !equal })
        }
        32 => {
            if left.hi < right.lo {
                Some(true)
            } else if left.lo >= right.hi {
                Some(false)
            } else {
                None
            }
        }
        33 => {
            if left.hi <= right.lo {
                Some(true)
            } else if left.lo > right.hi {
                Some(false)
            } else {
                None
            }
        }
        34 => compare_bounds(right, left, 32),
        35 => compare_bounds(right, left, 33),
        _ => None,
    }
}

fn comparison(left: &Value, right: &Value, opcode: u16) -> Option<Option<bool>> {
    if let (Some((a, b)), Some((c, d))) = (left.integers(), right.integers()) {
        // Keep comparisons in i64, including values beyond binary64's exact range.
        return Some(match opcode {
            30 | 31 => {
                let equal = if a == b && a == c && c == d {
                    Some(true)
                } else if b < c || d < a {
                    Some(false)
                } else {
                    None
                };
                equal.map(|equal| if opcode == 30 { equal } else { !equal })
            }
            32 => {
                if b < c {
                    Some(true)
                } else if a >= d {
                    Some(false)
                } else {
                    None
                }
            }
            33 => {
                if b <= c {
                    Some(true)
                } else if a > d {
                    Some(false)
                } else {
                    None
                }
            }
            34 => {
                if a > d {
                    Some(true)
                } else if b <= c {
                    Some(false)
                } else {
                    None
                }
            }
            35 => {
                if a >= d {
                    Some(true)
                } else if b < c {
                    Some(false)
                } else {
                    None
                }
            }
            _ => return None,
        });
    }
    if let (Some(left), Some(right)) = (left.float(), right.float()) {
        return Some(compare_bounds(left.range(), right.range(), opcode));
    }
    if !matches!(opcode, 30 | 31) {
        return None;
    }
    if let (Some(a), Some(b)) = (left.boolean(), right.boolean()) {
        return Some(a.zip(b).map(|(a, b)| (a == b) == (opcode == 30)));
    }
    let a = left.components()?;
    let b = right.components()?;
    let x = compare_bounds(a[0].range(), b[0].range(), 30);
    let y = compare_bounds(a[1].range(), b[1].range(), 30);
    let equal = if x == Some(false) || y == Some(false) {
        Some(false)
    } else if x == Some(true) && y == Some(true) {
        Some(true)
    } else {
        None
    };
    Some(equal.map(|equal| equal == (opcode == 30)))
}

fn integer_operation(opcode: u16, values: &[Value]) -> Option<Value> {
    let (a, b) = values[0].integers()?;
    let (lo, hi) = match opcode {
        10 => (b.checked_neg()?, a.checked_neg()?),
        40 if a >= 0 => (a, b),
        40 if b <= 0 => (b.checked_neg()?, a.checked_neg()?),
        40 => (0, a.checked_abs()?.max(b)),
        20..=23 | 41..=43 => {
            let (c, d) = values[1].integers()?;
            match opcode {
                20 => (a.checked_add(c)?, b.checked_add(d)?),
                21 => (a.checked_sub(d)?, b.checked_sub(c)?),
                22 | 23 => {
                    if opcode == 23 && c <= 0 && d >= 0 {
                        return None;
                    }
                    let apply = |x: i64, y: i64| {
                        if opcode == 22 {
                            x.checked_mul(y)
                        } else {
                            x.checked_div(y)
                        }
                    };
                    let results = [apply(a, c)?, apply(a, d)?, apply(b, c)?, apply(b, d)?];
                    (*results.iter().min()?, *results.iter().max()?)
                }
                41 => (a.min(c), b.min(d)),
                42 => (a.max(c), b.max(d)),
                43 => {
                    let (e, f) = values[2].integers()?;
                    if d > e {
                        return None;
                    }
                    (a.max(c).min(e), b.max(d).min(f))
                }
                _ => return None,
            }
        }
        // These operations need refinement until branch-dependent integer
        // operands become exact; no float approximation of i64 arithmetic.
        _ => return None,
    };
    Some(if lo == hi {
        Value::Exact(RuntimeValue::Int(lo))
    } else {
        Value::Integer { lo, hi }
    })
}
