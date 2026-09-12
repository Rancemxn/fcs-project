//! Exact Expression-DAG construction for Track contributions.
//!
//! [`TrackExpressionBuilder`] appends float/vec2-float expression nodes that
//! mirror `evaluate_segment`/`interpolate` operation for operation: the
//! segment `Sub → Div → Clamp → Easing` progress and the
//! `start + (end - start) * p'` interpolation. Every binary64 operation rounds
//! once in both lanes, so a product query over the resulting DAG and the
//! canonical runtime agree bit for bit. The FCBC native writer and the Render
//! source lowering share this builder so the two encodings cannot diverge.

use fcs_model::{
    CanonicalExpressionDag, CanonicalExpressionError, CanonicalExpressionNode,
    CanonicalExpressionOpcode, CanonicalExpressionType, CanonicalExpressionValue,
    CanonicalTrackInterpolation, CanonicalTrackSegment, CanonicalTrackTarget, CanonicalTrackValue,
};

use crate::easing::EasingId;
use crate::track::TrackContribution;

/// Why a Track contribution cannot be encoded as an exact Expression DAG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackExpressionError {
    /// No Expression opcode computes the correctly rounded Bezier solve.
    CubicBezierUnsupported,
    /// A named easing is not part of the Core easing set.
    UnknownEasing(String),
    /// The contribution's value shape does not match the target's lane.
    ValueShape { target: CanonicalTrackTarget },
}

impl std::fmt::Display for TrackExpressionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CubicBezierUnsupported => {
                write!(
                    formatter,
                    "Track expression segments require step, linear, or Core easing"
                )
            }
            Self::UnknownEasing(name) => write!(formatter, "unknown easing {name}"),
            Self::ValueShape { target } => write!(
                formatter,
                "Track expression for {target:?} target cannot encode the contribution value shape"
            ),
        }
    }
}

impl std::error::Error for TrackExpressionError {}

/// One composed value: a float node for scalar targets, a vec2-float node for
/// Scale.
#[derive(Clone, Copy)]
pub enum TrackExpressionValue {
    Scalar(usize),
    Vec2(usize),
}

/// Appends exact float/vec2-float expression nodes in topological order.
#[derive(Default)]
pub struct TrackExpressionBuilder {
    nodes: Vec<CanonicalExpressionNode>,
}

impl TrackExpressionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, node: CanonicalExpressionNode) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    fn float_constant(&mut self, value: f64) -> usize {
        self.push(CanonicalExpressionNode::new(
            CanonicalExpressionOpcode::Constant,
            CanonicalExpressionType::Float,
            [None, None, None],
            Some(CanonicalExpressionValue::Float(value)),
            0,
        ))
    }

    fn time_constant(&mut self, value: f64) -> usize {
        self.push(CanonicalExpressionNode::new(
            CanonicalExpressionOpcode::Constant,
            CanonicalExpressionType::Time,
            [None, None, None],
            Some(CanonicalExpressionValue::Time(value)),
            0,
        ))
    }

    fn env_s(&mut self) -> usize {
        self.push(CanonicalExpressionNode::new(
            CanonicalExpressionOpcode::EnvS,
            CanonicalExpressionType::Time,
            [None, None, None],
            None,
            0,
        ))
    }

    fn binary(
        &mut self,
        opcode: CanonicalExpressionOpcode,
        left: usize,
        right: usize,
        result: CanonicalExpressionType,
    ) -> usize {
        self.push(CanonicalExpressionNode::new(
            opcode,
            result,
            [Some(left), Some(right), None],
            None,
            0,
        ))
    }

    /// `Clamp(value, 0.0, 1.0)` — mirrors `clamp_progress`.
    fn clamp01(&mut self, value: usize) -> usize {
        let low = self.float_constant(0.0);
        let high = self.float_constant(1.0);
        self.push(CanonicalExpressionNode::new(
            CanonicalExpressionOpcode::Clamp,
            CanonicalExpressionType::Float,
            [Some(value), Some(low), Some(high)],
            None,
            0,
        ))
    }

    fn easing(&mut self, id: u16, progress: usize) -> usize {
        self.push(CanonicalExpressionNode::new(
            CanonicalExpressionOpcode::Easing,
            CanonicalExpressionType::Float,
            [Some(progress), None, None],
            None,
            id as u32,
        ))
    }

    fn vec2(&mut self, x: usize, y: usize) -> usize {
        self.push(CanonicalExpressionNode::new(
            CanonicalExpressionOpcode::Vec2,
            CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Float)),
            [Some(x), Some(y), None],
            None,
            0,
        ))
    }

    fn component(&mut self, value: usize, y: bool) -> usize {
        self.push(CanonicalExpressionNode::new(
            if y {
                CanonicalExpressionOpcode::Vec2Y
            } else {
                CanonicalExpressionOpcode::Vec2X
            },
            CanonicalExpressionType::Float,
            [Some(value), None, None],
            None,
            0,
        ))
    }

    /// A constant contribution: point value, resolved fill, identity, or base.
    pub fn constant_value(
        &mut self,
        value: CanonicalTrackValue,
        target: CanonicalTrackTarget,
    ) -> Result<TrackExpressionValue, TrackExpressionError> {
        match value {
            CanonicalTrackValue::Float(value) => {
                Ok(TrackExpressionValue::Scalar(self.float_constant(value)))
            }
            CanonicalTrackValue::Vec2Float(value) => {
                let x = self.float_constant(value.x());
                let y = self.float_constant(value.y());
                Ok(TrackExpressionValue::Vec2(self.vec2(x, y)))
            }
            CanonicalTrackValue::Angle(_) | CanonicalTrackValue::Vec2Length(_) => {
                Err(TrackExpressionError::ValueShape { target })
            }
        }
    }

    /// One Track contribution at a region's left endpoint.
    pub fn contribution(
        &mut self,
        contribution: TrackContribution<'_>,
        target: CanonicalTrackTarget,
    ) -> Result<TrackExpressionValue, TrackExpressionError> {
        match contribution {
            TrackContribution::Constant(value) => self.constant_value(value, target),
            TrackContribution::Segment(segment) => self.segment(segment, target),
        }
    }

    /// Segment interpolation, mirroring `evaluate_segment` and `interpolate`
    /// operation for operation: `p = (s - start) / (end - start)`, clamped,
    /// eased, then `start + (end - start) * p'` per component.
    fn segment(
        &mut self,
        segment: &CanonicalTrackSegment,
        target: CanonicalTrackTarget,
    ) -> Result<TrackExpressionValue, TrackExpressionError> {
        let interpolation = segment.interpolation();
        if matches!(interpolation, CanonicalTrackInterpolation::CubicBezier(_)) {
            return Err(TrackExpressionError::CubicBezierUnsupported);
        }
        let (start, end) = (
            segment.start().chart_time_seconds(),
            segment.end().chart_time_seconds(),
        );
        let progress = if matches!(interpolation, CanonicalTrackInterpolation::Step) {
            None
        } else {
            let env_s = self.env_s();
            let start_time = self.time_constant(start);
            let end_time = self.time_constant(end);
            // numerator, denominator, and progress each round once, exactly as
            // evaluate_segment does; the two Sub(end,start) uses are identical.
            let numerator = self.binary(
                CanonicalExpressionOpcode::Sub,
                env_s,
                start_time,
                CanonicalExpressionType::Time,
            );
            let denominator = self.binary(
                CanonicalExpressionOpcode::Sub,
                end_time,
                start_time,
                CanonicalExpressionType::Time,
            );
            let division = self.binary(
                CanonicalExpressionOpcode::Div,
                numerator,
                denominator,
                CanonicalExpressionType::Float,
            );
            let clamped = self.clamp01(division);
            Some(match interpolation {
                CanonicalTrackInterpolation::Easing(name) => {
                    let id = EasingId::ALL
                        .into_iter()
                        .find(|easing| easing.name() == name.as_str())
                        .map(EasingId::abi_id)
                        .ok_or_else(|| TrackExpressionError::UnknownEasing(name.clone()))?;
                    self.easing(id, clamped)
                }
                _ => clamped,
            })
        };
        let scalar = |builder: &mut Self,
                      start: CanonicalTrackValue,
                      end: CanonicalTrackValue|
         -> Result<usize, TrackExpressionError> {
            let (CanonicalTrackValue::Float(start), CanonicalTrackValue::Float(end)) = (start, end)
            else {
                return Err(TrackExpressionError::ValueShape { target });
            };
            let start_constant = builder.float_constant(start);
            let Some(progress) = progress else {
                // Step segments hold their start value.
                return Ok(start_constant);
            };
            let end_constant = builder.float_constant(end);
            let delta = builder.binary(
                CanonicalExpressionOpcode::Sub,
                end_constant,
                start_constant,
                CanonicalExpressionType::Float,
            );
            let scaled = builder.binary(
                CanonicalExpressionOpcode::Mul,
                delta,
                progress,
                CanonicalExpressionType::Float,
            );
            Ok(builder.binary(
                CanonicalExpressionOpcode::Add,
                start_constant,
                scaled,
                CanonicalExpressionType::Float,
            ))
        };
        match target {
            CanonicalTrackTarget::Alpha | CanonicalTrackTarget::ScrollSpeed => {
                Ok(TrackExpressionValue::Scalar(scalar(
                    self,
                    segment.start_value(),
                    segment.end_value(),
                )?))
            }
            CanonicalTrackTarget::Scale => {
                let (CanonicalTrackValue::Vec2Float(start), CanonicalTrackValue::Vec2Float(end)) =
                    (segment.start_value(), segment.end_value())
                else {
                    return Err(TrackExpressionError::ValueShape { target });
                };
                let x = scalar(
                    self,
                    CanonicalTrackValue::Float(start.x()),
                    CanonicalTrackValue::Float(end.x()),
                )?;
                let y = scalar(
                    self,
                    CanonicalTrackValue::Float(start.y()),
                    CanonicalTrackValue::Float(end.y()),
                )?;
                Ok(TrackExpressionValue::Vec2(self.vec2(x, y)))
            }
            CanonicalTrackTarget::Position | CanonicalTrackTarget::Rotation => {
                Err(TrackExpressionError::ValueShape { target })
            }
        }
    }

    /// `Add` on both lanes: a single node for floats and same-typed vectors.
    pub fn add(
        &mut self,
        left: TrackExpressionValue,
        right: TrackExpressionValue,
    ) -> TrackExpressionValue {
        match (left, right) {
            (TrackExpressionValue::Scalar(left), TrackExpressionValue::Scalar(right)) => {
                TrackExpressionValue::Scalar(self.binary(
                    CanonicalExpressionOpcode::Add,
                    left,
                    right,
                    CanonicalExpressionType::Float,
                ))
            }
            (TrackExpressionValue::Vec2(left), TrackExpressionValue::Vec2(right)) => {
                TrackExpressionValue::Vec2(self.binary(
                    CanonicalExpressionOpcode::Add,
                    left,
                    right,
                    CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Float)),
                ))
            }
            _ => unreachable!("track expression operands share the target's value shape"),
        }
    }

    /// `Mul` on both lanes: the ABI has no vector row, so vector multiplies
    /// decompose into component Muls, matching `combine_vec` exactly.
    pub fn multiply(
        &mut self,
        left: TrackExpressionValue,
        right: TrackExpressionValue,
    ) -> TrackExpressionValue {
        match (left, right) {
            (TrackExpressionValue::Scalar(left), TrackExpressionValue::Scalar(right)) => {
                TrackExpressionValue::Scalar(self.binary(
                    CanonicalExpressionOpcode::Mul,
                    left,
                    right,
                    CanonicalExpressionType::Float,
                ))
            }
            (TrackExpressionValue::Vec2(left), TrackExpressionValue::Vec2(right)) => {
                let x = self.component(left, false);
                let right_x = self.component(right, false);
                let x = self.binary(
                    CanonicalExpressionOpcode::Mul,
                    x,
                    right_x,
                    CanonicalExpressionType::Float,
                );
                let y = self.component(left, true);
                let right_y = self.component(right, true);
                let y = self.binary(
                    CanonicalExpressionOpcode::Mul,
                    y,
                    right_y,
                    CanonicalExpressionType::Float,
                );
                TrackExpressionValue::Vec2(self.vec2(x, y))
            }
            _ => unreachable!("track expression operands share the target's value shape"),
        }
    }

    pub const fn root(&self, value: TrackExpressionValue) -> usize {
        match value {
            TrackExpressionValue::Scalar(node) | TrackExpressionValue::Vec2(node) => node,
        }
    }

    /// Assembles the appended nodes into one validated DAG rooted at `root`.
    pub fn finish(self, root: usize) -> Result<CanonicalExpressionDag, CanonicalExpressionError> {
        CanonicalExpressionDag::new(self.nodes, root)
    }
}
