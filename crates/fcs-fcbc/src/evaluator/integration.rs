//! Deterministic direct integration with whole-panel error enclosures.

use super::enclosure::{Bounds, Math, Taylor};
use super::*;
use astro_float::{BigFloat, RoundingMode};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// One budget covers all panels, DAG entries and nested Line-local q queries.
pub const MAX_INTEGRATION_EVALUATIONS: usize = 65_536;
/// Maximum adaptive bisections below one known graph boundary interval.
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
    // A query endpoint can start a different half-open piece. Check its policy
    // even though a point has no integral mass.
    query
        .panel(
            Some((line.scroll_speed_descriptor, line.line_flags & 1 != 0)),
            time,
            time,
            0,
        )?
        .bound
        .ok_or(EXECUTION_ERROR)?;
    let (_, value) = query.integrate(
        Some((line.scroll_speed_descriptor, line.line_flags & 1 != 0)),
        descriptor.integration_origin,
        time,
        descriptor.initial_floor_position,
        ABSOLUTE_ERROR,
    )?;
    Ok(DistanceEvaluation {
        floor_position: value,
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
    let (_, value) = query.integrate(None, 0.0, time, 0.0, COORDINATE_ERROR)?;
    Ok(value)
}

fn tolerance(bound: Bounds, absolute: f64) -> f64 {
    let minimum = if bound.contains(0.0) {
        0.0
    } else {
        bound.lo.abs().min(bound.hi.abs())
    };
    let spacing = if minimum == f64::MAX {
        minimum - minimum.next_down()
    } else {
        minimum.next_up() - minimum
    };
    absolute.max(4.0 * spacing)
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
            let integrand = evaluator.integrand(speed)?;
            integrand.range().finite()?;
            let integral = integrand.integral(start, end);
            if speed.is_some() {
                integral.finite()
            } else {
                (integral / Bounds::point(60.0)).finite()
            }
        });
        let estimate = if bound.is_some() && start != end {
            self.estimate(speed, start, end)?
        } else {
            BigFloat::from_f64(0.0, PRECISION)
        };
        Ok(Panel {
            start,
            end,
            depth,
            bound,
            estimate,
        })
    }

    fn estimate(
        &mut self,
        speed: Option<(u32, bool)>,
        start: f64,
        end: f64,
    ) -> Result<BigFloat, &'static str> {
        // An open degree-three rule supplies the candidate, avoiding endpoint
        // discontinuities. Only the independent whole-panel enclosure certifies it.
        let mut sum = BigFloat::from_f64(0.0, PRECISION);
        for (fraction, weight) in [(0.25, 2.0), (0.5, -1.0), (0.75, 2.0)] {
            self.charge()?;
            let time = start * (1.0 - fraction) + end * fraction;
            let value = PanelEvaluator::new(self, time, time)
                .and_then(|mut evaluator| evaluator.integrand(speed))
                .and_then(|value| value.range().finite())
                .ok_or(EXECUTION_ERROR)?
                .midpoint();
            let term = BigFloat::from_f64(value, PRECISION).mul(
                &BigFloat::from_f64(weight, PRECISION),
                PRECISION,
                RoundingMode::ToEven,
            );
            sum = sum.add(&term, PRECISION, RoundingMode::ToEven);
        }
        let width = BigFloat::from_f64(end, PRECISION).sub(
            &BigFloat::from_f64(start, PRECISION),
            PRECISION,
            RoundingMode::ToEven,
        );
        Ok(sum.mul(&width, PRECISION, RoundingMode::ToEven).div(
            &BigFloat::from_f64(if speed.is_some() { 3.0 } else { 180.0 }, PRECISION),
            PRECISION,
            RoundingMode::ToEven,
        ))
    }

    fn integrate(
        &mut self,
        speed: Option<(u32, bool)>,
        origin: f64,
        time: f64,
        initial: f64,
        absolute: f64,
    ) -> Result<(Bounds, f64), &'static str> {
        if !origin.is_finite() || !time.is_finite() || !initial.is_finite() {
            return Err(EXECUTION_ERROR);
        }
        if time == origin {
            return Ok((Bounds::point(initial), initial));
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
            total.update(&panel, false);
            uncertain += usize::from(panel.bound.is_none());
            heap.push(panel);
            previous = boundary;
            if previous == end {
                break;
            }
        }
        if previous < end {
            let panel = self.panel(speed, previous, end, 0)?;
            total.update(&panel, false);
            uncertain += usize::from(panel.bound.is_none());
            heap.push(panel);
        }
        loop {
            if uncertain == 0 {
                let (bound, value) = total
                    .absolute(initial, reverse, &mut self.math)
                    .ok_or(EXECUTION_ERROR)?;
                let error = (value - bound.lo).max(bound.hi - value).next_up();
                if error <= tolerance(bound, absolute) {
                    return Ok((bound, value));
                }
            }
            let panel = heap.pop().ok_or(EXECUTION_ERROR)?;
            let middle = panel.start.midpoint(panel.end);
            if panel.depth >= MAX_INTEGRATION_DEPTH || middle <= panel.start || middle >= panel.end
            {
                return Err(EXECUTION_ERROR);
            }
            total.update(&panel, true);
            uncertain -= usize::from(panel.bound.is_none());
            for (start, end) in [(panel.start, middle), (middle, panel.end)] {
                let child = self.panel(speed, start, end, panel.depth + 1)?;
                total.update(&child, false);
                uncertain += usize::from(child.bound.is_none());
                heap.push(child);
            }
        }
    }
}

struct Sum {
    lower: BigFloat,
    upper: BigFloat,
    estimate: BigFloat,
}

impl Sum {
    fn new() -> Self {
        Self {
            lower: BigFloat::from_f64(0.0, PRECISION),
            upper: BigFloat::from_f64(0.0, PRECISION),
            estimate: BigFloat::from_f64(0.0, PRECISION),
        }
    }

    fn update(&mut self, panel: &Panel, subtract: bool) {
        let Some(bound) = panel.bound else {
            return;
        };
        let lower = BigFloat::from_f64(bound.lo, PRECISION);
        let upper = BigFloat::from_f64(bound.hi, PRECISION);
        if subtract {
            self.lower = self.lower.sub(&lower, PRECISION, RoundingMode::Down);
            self.upper = self.upper.sub(&upper, PRECISION, RoundingMode::Up);
            self.estimate = self
                .estimate
                .sub(&panel.estimate, PRECISION, RoundingMode::ToEven);
        } else {
            self.lower = self.lower.add(&lower, PRECISION, RoundingMode::Down);
            self.upper = self.upper.add(&upper, PRECISION, RoundingMode::Up);
            self.estimate = self
                .estimate
                .add(&panel.estimate, PRECISION, RoundingMode::ToEven);
        }
    }

    fn absolute(&self, initial: f64, reverse: bool, math: &mut Math) -> Option<(Bounds, f64)> {
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
        let estimate = if reverse {
            initial.sub(&self.estimate, PRECISION, RoundingMode::ToEven)
        } else {
            initial.add(&self.estimate, PRECISION, RoundingMode::ToEven)
        };
        let value = math.float_round(&estimate)?;
        let bound = Bounds {
            lo: math.float_bound(&lower, true)?,
            hi: math.float_bound(&upper, false)?,
        }
        .finite()?;
        Some((bound, if value == 0.0 { 0.0 } else { value }))
    }
}

struct Panel {
    start: f64,
    end: f64,
    depth: usize,
    bound: Option<Bounds>,
    estimate: BigFloat,
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
        let time = if start == end {
            Taylor::constant(start)
        } else {
            Taylor::variable(start, end)
                .rounded()
                .with_bound(Bounds { lo: start, hi: end })
        };
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

    fn integrand(&mut self, speed: Option<(u32, bool)>) -> Option<Taylor> {
        let tempo = self
            .descriptor(self.query.tempo, Taylor::constant(0.0), false, 0)?
            .float()?;
        if tempo.range().finite()?.lo <= 0.0 {
            return None;
        }
        self.tempo = Some(tempo);
        if let Some((root, allow_reverse)) = speed {
            let speed = self
                .descriptor(root, Taylor::constant(0.0), true, 0)?
                .float()?;
            if !allow_reverse && speed.range().finite()?.lo < 0.0 {
                return None;
            }
            // These are separate ABI binary64 nodes, in the specified order.
            (speed * tempo).divide(Taylor::constant(60.0))
        } else {
            // q integrates BPM in real arithmetic, then divides by 60.
            Some(tempo)
        }
    }

    fn coordinate(&mut self) -> Option<Taylor> {
        if let Some(value) = self.coordinate {
            return Some(value);
        }
        let middle = self.start.midpoint(self.end);
        let center = self
            .query
            .integrate(None, 0.0, middle, 0.0, COORDINATE_ERROR)
            .ok()?
            .0;
        let tempo = self.tempo?;
        let mut value = Taylor::enclosed(center)
            .real_add(
                tempo
                    .primitive(self.start, self.end)
                    .scale(Bounds::point(1.0) / Bounds::point(60.0)),
            )
            .rounded();
        // Positive local tempo and q(0)=0 prove the sign independently of
        // coefficient roundoff, including the endpoint q(0) itself.
        if self.start >= 0.0 {
            value = value.with_bound(Bounds {
                lo: 0.0,
                hi: f64::INFINITY,
            });
        } else if self.end <= 0.0 {
            value = value.with_bound(Bounds {
                lo: f64::NEG_INFINITY,
                hi: 0.0,
            });
        }
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
                        && (piece.flags & 0b100 != 0
                            || if self.start == self.end {
                                self.start < piece.end
                                    || (piece.flags & 1 != 0
                                        && self.start.to_bits() == piece.end.to_bits())
                            } else {
                                piece.end >= self.end
                            })
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
                    segment.flags & 1 == 0
                        && segment.start <= self.start
                        && if self.start == self.end {
                            self.start < segment.end
                        } else {
                            segment.end >= self.end
                        }
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
                        let mut branch = self.branch_memo(
                            node.operands[0],
                            node.opcode == 36,
                            memo,
                            (progress, allow_q),
                            depth + 1,
                        )?;
                        let right = self
                            .expression(
                                node.operands[1],
                                progress,
                                allow_q,
                                &mut branch,
                                depth + 1,
                            )?
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
                        let mut branch = self.branch_memo(
                            node.operands[0],
                            true,
                            memo,
                            (progress, allow_q),
                            depth + 1,
                        )?;
                        let left = self.expression(
                            node.operands[1],
                            progress,
                            allow_q,
                            &mut branch,
                            depth + 1,
                        )?;
                        let mut branch = self.branch_memo(
                            node.operands[0],
                            false,
                            memo,
                            (progress, allow_q),
                            depth + 1,
                        )?;
                        let right = self.expression(
                            node.operands[2],
                            progress,
                            allow_q,
                            &mut branch,
                            depth + 1,
                        )?;
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

    fn branch_memo(
        &mut self,
        predicate: u32,
        selected: bool,
        memo: &BTreeMap<u32, Value>,
        environment: (Taylor, bool),
        depth: usize,
    ) -> Option<BTreeMap<u32, Value>> {
        for _ in memo {
            self.query.charge().ok()?;
        }
        let mut branch = memo.clone();
        let mut changed = std::collections::BTreeSet::new();
        self.restrict_predicate(
            predicate,
            selected,
            &mut branch,
            &mut changed,
            environment,
            depth,
        )?;
        // A value computed before narrowing an operand is no longer this
        // branch's cache value. Topological indices make invalidation one pass.
        for index in memo.keys().copied() {
            self.query.charge().ok()?;
            if !changed.contains(&index)
                && self.query.chart.expressions[index as usize]
                    .operands
                    .iter()
                    .any(|operand| changed.contains(operand))
            {
                branch.remove(&index);
                changed.insert(index);
            }
        }
        branch.insert(predicate, Value::truth(Some(selected)));
        Some(branch)
    }

    fn restrict_predicate(
        &mut self,
        index: u32,
        selected: bool,
        memo: &mut BTreeMap<u32, Value>,
        changed: &mut std::collections::BTreeSet<u32>,
        environment: (Taylor, bool),
        depth: usize,
    ) -> Option<()> {
        self.query.charge().ok()?;
        if depth >= MAX_VALIDATOR_DEPTH {
            return None;
        }
        let node = self.query.chart.expressions.get(index as usize)?;
        match node.opcode {
            11 => self.restrict_predicate(
                node.operands[0],
                !selected,
                memo,
                changed,
                environment,
                depth + 1,
            )?,
            36 if selected => {
                self.restrict_predicate(
                    node.operands[0],
                    true,
                    memo,
                    changed,
                    environment,
                    depth + 1,
                )?;
                self.restrict_predicate(
                    node.operands[1],
                    true,
                    memo,
                    changed,
                    environment,
                    depth + 1,
                )?;
            }
            37 if !selected => {
                self.restrict_predicate(
                    node.operands[0],
                    false,
                    memo,
                    changed,
                    environment,
                    depth + 1,
                )?;
                self.restrict_predicate(
                    node.operands[1],
                    false,
                    memo,
                    changed,
                    environment,
                    depth + 1,
                )?;
            }
            30..=35 => {
                let left = self.expression(
                    node.operands[0],
                    environment.0,
                    environment.1,
                    memo,
                    depth + 1,
                )?;
                let right = self.expression(
                    node.operands[1],
                    environment.0,
                    environment.1,
                    memo,
                    depth + 1,
                )?;
                // Integer comparisons must stay in i64; their branch hulls
                // remain conservative without a lossy binary64 constraint.
                if left.integers().is_some() || right.integers().is_some() {
                    return Some(());
                }
                let (Some(left), Some(right)) = (left.float(), right.float()) else {
                    return Some(());
                };
                let (mut a, mut b) = (left.range().finite()?, right.range().finite()?);
                let opcode = if selected {
                    node.opcode
                } else {
                    match node.opcode {
                        30 => 31,
                        31 => 30,
                        32 => 35,
                        33 => 34,
                        34 => 33,
                        _ => 32,
                    }
                };
                match opcode {
                    30 => {
                        a = a.intersect(b);
                        b = a;
                    }
                    31 => return Some(()),
                    32 | 33 => {
                        let lo = a.lo;
                        a.hi = a.hi.min(if opcode == 32 { b.hi.next_down() } else { b.hi });
                        b.lo = b.lo.max(if opcode == 32 { lo.next_up() } else { lo });
                    }
                    34 | 35 => {
                        let hi = a.hi;
                        a.lo = a.lo.max(if opcode == 34 { b.lo.next_up() } else { b.lo });
                        b.hi = b.hi.min(if opcode == 34 { hi.next_down() } else { hi });
                    }
                    _ => return None,
                }
                a.finite()?;
                b.finite()?;
                for (index, value, bound) in
                    [(node.operands[0], left, a), (node.operands[1], right, b)]
                {
                    let value = if value.exact_constant().is_some() {
                        value
                    } else if bound.lo == 0.0 && bound.hi == 0.0 {
                        // Numeric equality accepts both zero signs; it cannot
                        // turn an uncertain zero into a positive-zero constant.
                        value.with_bound(Bounds {
                            lo: 0.0f64.next_down(),
                            hi: 0.0f64.next_up(),
                        })
                    } else {
                        value.with_bound(bound)
                    };
                    memo.insert(index, Value::Float(value));
                    changed.insert(index);
                }
            }
            _ => {}
        }
        Some(())
    }

    fn operation(
        &mut self,
        node: &super::super::loader::ExpressionNode,
        values: &[Value],
    ) -> Option<Value> {
        let float = |index: usize| values.get(index)?.float();
        let truth = Value::truth;
        if node.result_type == ValueType::Int {
            if matches!(node.opcode, 81 | 82) {
                let (lo, hi) = values[0].integer_components()?[usize::from(node.opcode == 82)];
                return Some(Value::integer(lo, hi));
            }
            return integer_operation(node.opcode, values);
        }
        if node.result_type == ValueType::Vec2Int {
            if node.opcode == 80 {
                return Some(Value::IntVector([
                    values[0].integers()?,
                    values[1].integers()?,
                ]));
            }
            let left = values[0].integer_components()?;
            let right = values[1].integer_components()?;
            let apply = |i: usize| {
                integer_operation(
                    node.opcode,
                    &[
                        Value::integer(left[i].0, left[i].1),
                        Value::integer(right[i].0, right[i].1),
                    ],
                )?
                .integers()
            };
            return Some(Value::IntVector([apply(0)?, apply(1)?]));
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
                let tolerance = float(2)?.range().finite()?;
                if tolerance.lo < 0.0 {
                    return None;
                }
                let difference = absolute(float(0)? - float(1)?);
                truth(compare_bounds(difference.range().finite()?, tolerance, 33))
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
    IntVector([(i64, i64); 2]),
    Color([Bounds; 4]),
    UnknownBool,
}

impl Value {
    fn integer(lo: i64, hi: i64) -> Self {
        if lo == hi {
            Self::Exact(RuntimeValue::Int(lo))
        } else {
            Self::Integer { lo, hi }
        }
    }

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

    fn integer_components(&self) -> Option<[(i64, i64); 2]> {
        match self {
            Self::IntVector(values) => Some(*values),
            Self::Exact(RuntimeValue::Vec2Int(values)) => Some(values.map(|value| (value, value))),
            _ => self.integers().map(|value| [value; 2]),
        }
    }

    fn colors(&self) -> Option<[Bounds; 4]> {
        match self {
            Self::Color(values) => Some(*values),
            Self::Exact(RuntimeValue::Color(values)) => Some(values.map(Bounds::point)),
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
            Self::IntVector(values) if values.iter().any(|(lo, hi)| lo > hi) => return None,
            Self::Color(values) => {
                for value in values {
                    value.finite()?;
                }
            }
            _ => {}
        }
        Some(())
    }

    fn hull(self, other: Self) -> Option<Self> {
        if let (Self::Exact(left), Self::Exact(right)) = (&self, &other)
            && left == right
        {
            let mixed_zero = match (left, right) {
                (RuntimeValue::Scalar { value: a, .. }, RuntimeValue::Scalar { value: b, .. }) => {
                    a.to_bits() != b.to_bits()
                }
                (RuntimeValue::Vec2 { value: a, .. }, RuntimeValue::Vec2 { value: b, .. }) => {
                    a.iter().zip(b).any(|(a, b)| a.to_bits() != b.to_bits())
                }
                _ => false,
            };
            if !mixed_zero {
                return Some(self);
            }
        }
        if let (Some(left), Some(right)) = (self.integers(), other.integers()) {
            return Some(Self::integer(left.0.min(right.0), left.1.max(right.1)));
        }
        if let (Some(left), Some(right)) = (self.integer_components(), other.integer_components()) {
            return Some(Self::IntVector(std::array::from_fn(|i| {
                (left[i].0.min(right[i].0), left[i].1.max(right[i].1))
            })));
        }
        if let (Some(left), Some(right)) = (self.colors(), other.colors()) {
            return Some(Self::Color(std::array::from_fn(|i| left[i].hull(right[i]))));
        }
        if let (Some(left), Some(right)) = (self.boolean(), other.boolean()) {
            return Some(Self::truth(if left == right { left } else { None }));
        }
        if let (Some(left), Some(right)) = (self.float(), other.float()) {
            return Some(Self::Float(Taylor::enclosed(signed_hull(
                left.range(),
                right.range(),
            ))));
        }
        let left = self.components()?;
        let right = other.components()?;
        Some(Self::Vector(std::array::from_fn(|i| {
            Taylor::enclosed(signed_hull(left[i].range(), right[i].range()))
        })))
    }
}

fn signed_hull(left: Bounds, right: Bounds) -> Bounds {
    if left.lo == 0.0
        && left.hi == 0.0
        && right.lo == 0.0
        && right.hi == 0.0
        && (left.lo.to_bits() != right.lo.to_bits() || left.hi.to_bits() != right.hi.to_bits())
    {
        Bounds {
            lo: 0.0f64.next_down(),
            hi: 0.0f64.next_up(),
        }
    } else {
        left.hull(right)
    }
}

fn absolute(value: Taylor) -> Taylor {
    if let Some(value) = value.exact_constant() {
        return Taylor::constant(value.abs());
    }
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
    if let (Some(left), Some(right)) = (left.exact_constant(), right.exact_constant()) {
        return Taylor::constant(if maximum {
            left.max(right)
        } else {
            left.min(right)
        });
    }
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
    if let (Some(left), Some(right)) = (left.integer_components(), right.integer_components()) {
        let x = comparison(
            &Value::integer(left[0].0, left[0].1),
            &Value::integer(right[0].0, right[0].1),
            30,
        )?;
        let y = comparison(
            &Value::integer(left[1].0, left[1].1),
            &Value::integer(right[1].0, right[1].1),
            30,
        )?;
        return Some(combine_equal([x, y]).map(|equal| equal == (opcode == 30)));
    }
    if let (Some(left), Some(right)) = (left.colors(), right.colors()) {
        return Some(
            combine_equal(std::array::from_fn::<_, 4, _>(|i| {
                compare_bounds(left[i], right[i], 30)
            }))
            .map(|equal| equal == (opcode == 30)),
        );
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

fn combine_equal<const N: usize>(values: [Option<bool>; N]) -> Option<bool> {
    if values.contains(&Some(false)) {
        Some(false)
    } else if values.iter().all(|value| *value == Some(true)) {
        Some(true)
    } else {
        None
    }
}

fn integer_operation(opcode: u16, values: &[Value]) -> Option<Value> {
    let (a, b) = values[0].integers()?;
    let (lo, hi) = match opcode {
        10 => (b.checked_neg()?, a.checked_neg()?),
        40 if a >= 0 => (a, b),
        40 if b <= 0 => (b.checked_neg()?, a.checked_neg()?),
        40 => (0, a.checked_abs()?.max(b)),
        24 => {
            let (c, d) = values[1].integers()?;
            if (c <= 0 && d >= 0) || (a == i64::MIN && c <= -1 && d >= -1) {
                return None;
            }
            let maximum = c.unsigned_abs().max(d.unsigned_abs()) - 1;
            (
                if a < 0 {
                    -(a.unsigned_abs().min(maximum) as i64)
                } else {
                    0
                },
                if b > 0 {
                    (b as u64).min(maximum) as i64
                } else {
                    0
                },
            )
        }
        25 => {
            let (c, d) = values[1].integers()?;
            let c = u32::try_from(c).ok()?;
            let d = u32::try_from(d).ok()?;
            if a >= -1 && b <= 1 {
                (-1, 1)
            } else {
                if d > 63 {
                    return None;
                }
                let mut lo = i64::MAX;
                let mut hi = i64::MIN;
                for exponent in c..=d {
                    let x = a.checked_pow(exponent)?;
                    let y = b.checked_pow(exponent)?;
                    lo = lo.min(x).min(y);
                    hi = hi.max(x).max(y);
                    if a <= 0 && b >= 0 && exponent != 0 {
                        lo = lo.min(0);
                        hi = hi.max(0);
                    }
                }
                (lo, hi)
            }
        }
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
        _ => return None,
    };
    Some(Value::integer(lo, hi))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Domain, ExpressionNode, PropertyDescriptor, TempoPoint};

    fn node(
        chart: &mut DecodedChart,
        opcode: u16,
        result_type: ValueType,
        operands: &[u32],
    ) -> u32 {
        let index = chart.expressions.len() as u32;
        let mut refs = [u32::MAX; 3];
        refs[..operands.len()].copy_from_slice(operands);
        chart.expressions.push(ExpressionNode {
            opcode,
            result_type,
            operands: refs,
            arity: operands.len() as u8,
            immediate: 0,
        });
        index
    }

    fn constant(chart: &mut DecodedChart, ty: ValueType, value: f64) -> u32 {
        literal(chart, RuntimeValue::Scalar { ty, value })
    }

    fn literal(chart: &mut DecodedChart, value: RuntimeValue) -> u32 {
        let immediate = chart.constants.len() as u32;
        let ty = value.value_type();
        chart.constants.push(value);
        let index = node(chart, 1, ty, &[]);
        chart.expressions[index as usize].immediate = immediate;
        index
    }

    fn bind_speed(chart: &mut DecodedChart, root: u32, boundaries: &[f64]) -> u32 {
        let speed = chart.descriptors.len() as u32;
        chart.descriptors.push(PropertyDescriptor {
            property_type: ValueType::Float,
            domain: Domain {
                start: 0.0,
                end: 0.0,
                unbounded_before: true,
                unbounded_after: true,
            },
            kind: DescriptorKind::Expression(root),
        });
        chart.lines[0].scroll_speed_descriptor = speed;
        chart.lines[0].integration_origin = 0.0;
        chart.lines[0].initial_floor_position = 0.0;
        let index = chart.lines[0].distance_descriptor;
        let distance = &mut chart.distances[index as usize];
        distance.scroll_speed_descriptor = speed;
        distance.integration_origin = 0.0;
        distance.initial_floor_position = 0.0;
        distance.classification = DistanceClassification::PortableEvaluable;
        distance.boundaries = boundaries.to_vec();
        index
    }

    fn chart() -> DecodedChart {
        let mut chart = crate::load_chart(&crate::write_nonempty_execution()).unwrap();
        let constant = chart.constants.len() as u32;
        chart.constants.push(RuntimeValue::Scalar {
            ty: ValueType::Float,
            value: 60.0,
        });
        let tempo = chart.lines[0].scroll_tempo_descriptor as usize;
        chart.descriptors[tempo].kind = DescriptorKind::Constant(constant);
        chart
    }

    #[test]
    fn continuous_predicates_enclose_a_narrow_pulse_and_preserve_short_circuiting() {
        let mut chart = chart();
        let time = node(&mut chart, 2, ValueType::Time, &[]);
        let lower = constant(&mut chart, ValueType::Time, 0.314_159);
        let upper = constant(&mut chart, ValueType::Time, 0.314_160);
        let after = node(&mut chart, 35, ValueType::Bool, &[time, lower]);
        let before = node(&mut chart, 32, ValueType::Bool, &[time, upper]);
        let inside = node(&mut chart, 36, ValueType::Bool, &[after, before]);
        let high = constant(&mut chart, ValueType::Float, 1000.0);
        let base = constant(&mut chart, ValueType::Float, 1.0);
        let speed = node(&mut chart, 70, ValueType::Float, &[inside, high, base]);
        let index = bind_speed(&mut chart, speed, &[0.0]);
        let result = query_distance(&chart, index, 1.0).unwrap();
        let expected = 1.0 + 999.0 * (0.314_160 - 0.314_159);
        assert!((result.floor_position - expected).abs() <= ABSOLUTE_ERROR);
        assert_eq!(chart.distances[index as usize].boundaries, [0.0]);

        let zero = constant(&mut chart, ValueType::Time, 0.0);
        let negative_time = node(&mut chart, 32, ValueType::Bool, &[time, zero]);
        let negative = constant(&mut chart, ValueType::Float, -1.0);
        let invalid = node(&mut chart, 47, ValueType::Float, &[negative]);
        let root = node(
            &mut chart,
            70,
            ValueType::Float,
            &[negative_time, invalid, base],
        );
        let index = bind_speed(&mut chart, root, &[0.0]);
        let result = query_distance(&chart, index, 1.0).unwrap();
        assert!((result.floor_position - 1.0).abs() <= ABSOLUTE_ERROR);
        assert!(!result.visited_nodes.contains(&invalid));
        assert_eq!(query_distance(&chart, index, -1.0), Err(EXECUTION_ERROR));
    }

    #[test]
    fn speed_uses_line_local_q_global_beat_boundaries_and_local_reverse_policy() {
        let mut chart = chart();
        chart.tempo_points = vec![
            TempoPoint {
                beat_numerator: 0,
                beat_denominator: 1,
                chart_time: 0.0,
                bpm: 120.0,
                source_order: 0,
            },
            TempoPoint {
                beat_numerator: 4,
                beat_denominator: 1,
                chart_time: 2.0,
                bpm: 240.0,
                source_order: 1,
            },
        ];
        let first = chart.constants.len() as u32;
        chart
            .constants
            .extend([60.0, 120.0].map(|value| RuntimeValue::Scalar {
                ty: ValueType::Float,
                value,
            }));
        chart.descriptors[chart.lines[0].scroll_tempo_descriptor as usize].kind =
            DescriptorKind::SegmentTrack(vec![
                Segment {
                    start: 0.0,
                    end: 0.0,
                    flags: 1,
                    interpolation: 1,
                    easing: 0,
                    start_constant: first,
                    end_constant: first,
                    bezier: [0.0; 4],
                },
                Segment {
                    start: 1.0,
                    end: 1.0,
                    flags: 1,
                    interpolation: 1,
                    easing: 0,
                    start_constant: first + 1,
                    end_constant: first + 1,
                    bezier: [0.0; 4],
                },
            ]);
        let q = node(&mut chart, 4, ValueType::Float, &[]);
        let beat = node(&mut chart, 3, ValueType::Beat, &[]);
        let one_beat = constant(&mut chart, ValueType::Beat, 1.0);
        let raw_beat = node(&mut chart, 23, ValueType::Float, &[beat, one_beat]);
        let speed = node(&mut chart, 20, ValueType::Float, &[q, raw_beat]);
        let index = bind_speed(&mut chart, speed, &[0.0, 1.0, 2.0]);
        chart.lines[0].line_flags = 1;
        for (time, expected) in [
            (3.0, 31.5),
            (1.0, 1.5),
            (-1.0, 1.5),
            (2.0, 11.5),
            (3.0, 31.5),
        ] {
            let actual = query_distance(&chart, index, time).unwrap();
            assert!(
                (actual.floor_position - expected).abs() <= ABSOLUTE_ERROR,
                "at {time}: {} != {expected}",
                actual.floor_position
            );
        }
        chart.lines[0].line_flags = 0;
        assert!(query_distance(&chart, index, 1.0).is_ok());
        assert_eq!(query_distance(&chart, index, -1.0), Err(EXECUTION_ERROR));
    }

    #[test]
    fn branch_values_retain_integer_vector_color_and_signed_zero_semantics() {
        for (left, right) in [
            (
                RuntimeValue::Int(9_007_199_254_740_993),
                RuntimeValue::Int(9_007_199_254_740_992),
            ),
            (
                RuntimeValue::Vec2Int([9_007_199_254_740_993, -3]),
                RuntimeValue::Vec2Int([9_007_199_254_740_992, -3]),
            ),
            (
                RuntimeValue::Color([0.1, 0.2, 0.3, 1.0]),
                RuntimeValue::Color([0.1, 0.2, 0.4, 1.0]),
            ),
        ] {
            let mut chart = chart();
            let ty = left.value_type();
            let time = node(&mut chart, 2, ValueType::Time, &[]);
            let switch = constant(&mut chart, ValueType::Time, 0.375);
            let predicate = node(&mut chart, 32, ValueType::Bool, &[time, switch]);
            let left = literal(&mut chart, left);
            let right = literal(&mut chart, right);
            let choice = node(&mut chart, 70, ty, &[predicate, left, right]);
            let equal = node(&mut chart, 30, ValueType::Bool, &[choice, left]);
            let high = constant(&mut chart, ValueType::Float, 4.0);
            let low = constant(&mut chart, ValueType::Float, 1.0);
            let root = node(&mut chart, 70, ValueType::Float, &[equal, high, low]);
            let index = bind_speed(&mut chart, root, &[0.0]);
            let result = query_distance(&chart, index, 1.0).unwrap();
            assert!(
                (result.floor_position - 2.125).abs() <= ABSOLUTE_ERROR,
                "{ty:?}"
            );
        }

        let mut chart = chart();
        let time = node(&mut chart, 2, ValueType::Time, &[]);
        let switch = constant(&mut chart, ValueType::Time, 0.375);
        let predicate = node(&mut chart, 32, ValueType::Bool, &[time, switch]);
        let negative_zero = constant(&mut chart, ValueType::Float, -0.0);
        let positive_zero = constant(&mut chart, ValueType::Float, 0.0);
        let choice = node(
            &mut chart,
            70,
            ValueType::Float,
            &[predicate, negative_zero, positive_zero],
        );
        let negative_one = constant(&mut chart, ValueType::Float, -1.0);
        let angle = node(&mut chart, 56, ValueType::Float, &[choice, negative_one]);
        let zero = node(&mut chart, 30, ValueType::Bool, &[choice, positive_zero]);
        let angle = node(
            &mut chart,
            70,
            ValueType::Float,
            &[zero, angle, negative_one],
        );
        let four = constant(&mut chart, ValueType::Float, 4.0);
        let root = node(&mut chart, 20, ValueType::Float, &[angle, four]);
        let index = bind_speed(&mut chart, root, &[0.0]);
        let mut query = Query::new(&chart, chart.lines[0].scroll_tempo_descriptor, &[0.0]).unwrap();
        let range = PanelEvaluator::new(&mut query, 0.0, 1.0)
            .unwrap()
            .expression(root, Taylor::constant(0.0), true, &mut BTreeMap::new(), 0)
            .unwrap()
            .float()
            .unwrap()
            .range();
        assert!(range.contains(4.0 - std::f64::consts::PI));
        assert!(range.contains(4.0 + std::f64::consts::PI));
        let result = query_distance(&chart, index, 1.0).unwrap();
        assert!(
            (result.floor_position - (4.0 + std::f64::consts::PI / 4.0)).abs() <= ABSOLUTE_ERROR
        );

        // A finite boolean result cannot hide overflow in ApproxEq's subtraction.
        let raw_time = node(&mut chart, 62, ValueType::Float, &[time]);
        let maximum = constant(&mut chart, ValueType::Float, f64::MAX);
        let negative_maximum = constant(&mut chart, ValueType::Float, -f64::MAX);
        let large = node(&mut chart, 20, ValueType::Float, &[maximum, raw_time]);
        let approximate = node(
            &mut chart,
            38,
            ValueType::Bool,
            &[large, negative_maximum, four],
        );
        let root = node(
            &mut chart,
            70,
            ValueType::Float,
            &[approximate, four, positive_zero],
        );
        let index = bind_speed(&mut chart, root, &[0.0]);
        assert_eq!(query_distance(&chart, index, 1.0), Err(EXECUTION_ERROR));
    }

    #[test]
    fn conditional_domains_are_checked_only_where_the_branch_is_selected() {
        let mut chart = chart();
        let time = node(&mut chart, 2, ValueType::Time, &[]);
        let half_time = constant(&mut chart, ValueType::Time, 0.5);
        let before = node(&mut chart, 32, ValueType::Bool, &[time, half_time]);
        let raw_time = node(&mut chart, 62, ValueType::Float, &[time]);
        let half = constant(&mut chart, ValueType::Float, 0.5);
        let left = node(&mut chart, 21, ValueType::Float, &[half, raw_time]);
        let right = node(&mut chart, 21, ValueType::Float, &[raw_time, half]);
        let left = node(&mut chart, 47, ValueType::Float, &[left]);
        let right = node(&mut chart, 47, ValueType::Float, &[right]);
        let root = node(&mut chart, 70, ValueType::Float, &[before, left, right]);
        let one = constant(&mut chart, ValueType::Float, 1.0);
        let root = node(&mut chart, 20, ValueType::Float, &[root, one]);
        let index = bind_speed(&mut chart, root, &[0.0]);
        let actual = query_distance(&chart, index, 1.0).unwrap();
        // Integral of 1 + sqrt(abs(t - 1/2)) on [0,1].
        let expected = 1.0 + 2.0f64.sqrt() / 3.0;
        assert!((actual.floor_position - expected).abs() <= ABSOLUTE_ERROR);
    }

    #[test]
    fn every_boundary_and_nested_query_charges_the_same_finite_budget() {
        assert_eq!(
            tolerance(Bounds::point(f64::MAX), ABSOLUTE_ERROR),
            4.0 * (f64::MAX - f64::MAX.next_down())
        );
        let mut chart = chart();
        let q = node(&mut chart, 4, ValueType::Float, &[]);
        let zero = constant(&mut chart, ValueType::Float, 0.0);
        let mut root = q;
        for _ in 0..100 {
            root = node(&mut chart, 20, ValueType::Float, &[root, zero]);
        }
        let boundaries: Vec<_> = (0..=400).map(f64::from).collect();
        let distance = bind_speed(&mut chart, root, &boundaries);
        assert_eq!(
            query_distance(&chart, distance, 400.0),
            Err(EXECUTION_ERROR)
        );
        let line = &chart.lines[0];
        let mut query = Query::new(&chart, line.scroll_tempo_descriptor, &boundaries).unwrap();
        assert_eq!(
            query.integrate(
                Some((line.scroll_speed_descriptor, false)),
                0.0,
                400.0,
                0.0,
                ABSOLUTE_ERROR
            ),
            Err(EXECUTION_ERROR)
        );
        assert_eq!(query.evaluations, MAX_INTEGRATION_EVALUATIONS);
    }
}
