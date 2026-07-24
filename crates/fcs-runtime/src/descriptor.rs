use std::fmt;

use fcs_model::{CanonicalDescriptorKind, CanonicalDescriptorTable, CanonicalExpressionValue};

use crate::{
    ExpressionEnvironment, ExpressionEvaluationError, evaluate_expression,
    expression::runtime_constant,
};

#[derive(Debug, Clone, PartialEq)]
pub enum DescriptorEvaluationError {
    UnknownDescriptor { descriptor: usize },
    OutsideDomain { descriptor: usize },
    MissingPiece { descriptor: usize },
    InvalidProgress { descriptor: usize },
    TypeMismatch { descriptor: usize },
    Expression(ExpressionEvaluationError),
}

impl fmt::Display for DescriptorEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDescriptor { descriptor } => {
                write!(formatter, "descriptor {descriptor} does not exist")
            }
            Self::OutsideDomain { descriptor } => {
                write!(
                    formatter,
                    "descriptor {descriptor} was queried outside its domain"
                )
            }
            Self::MissingPiece { descriptor } => {
                write!(
                    formatter,
                    "descriptor {descriptor} has no Piece for the query"
                )
            }
            Self::InvalidProgress { descriptor } => {
                write!(
                    formatter,
                    "descriptor {descriptor} produced invalid Piece progress"
                )
            }
            Self::TypeMismatch { descriptor } => {
                write!(
                    formatter,
                    "descriptor {descriptor} returned the wrong value type"
                )
            }
            Self::Expression(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DescriptorEvaluationError {}

impl From<ExpressionEvaluationError> for DescriptorEvaluationError {
    fn from(error: ExpressionEvaluationError) -> Self {
        Self::Expression(error)
    }
}

/// Evaluates one canonical descriptor at the chart time carried by `environment`.
pub fn evaluate_descriptor(
    table: &CanonicalDescriptorTable,
    descriptor: usize,
    environment: ExpressionEnvironment,
) -> Result<CanonicalExpressionValue, DescriptorEvaluationError> {
    evaluate_descriptor_inner(table, descriptor, environment)
}

fn evaluate_descriptor_inner(
    table: &CanonicalDescriptorTable,
    descriptor: usize,
    environment: ExpressionEnvironment,
) -> Result<CanonicalExpressionValue, DescriptorEvaluationError> {
    let value = table
        .descriptor(descriptor)
        .ok_or(DescriptorEvaluationError::UnknownDescriptor { descriptor })?;
    let chart_time = environment.chart_time();
    if !value.domain().contains(chart_time) {
        return Err(DescriptorEvaluationError::OutsideDomain { descriptor });
    }
    let result = match value.kind() {
        CanonicalDescriptorKind::Constant(value) => runtime_constant(value.clone()),
        CanonicalDescriptorKind::Expression(expression) => {
            evaluate_expression(expression, environment)?
        }
        CanonicalDescriptorKind::Piecewise(pieces) => {
            let piece = pieces
                .iter()
                .copied()
                .find(|piece| piece.contains(chart_time))
                .ok_or(DescriptorEvaluationError::MissingPiece { descriptor })?;
            let progress = match (piece.start(), piece.end()) {
                (Some(start), Some(end)) => (chart_time - start) / (end - start),
                (None, Some(_)) => 0.0,
                (Some(_), None) => 1.0,
                (None, None) => 0.0,
            };
            if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
                return Err(DescriptorEvaluationError::InvalidProgress { descriptor });
            }
            evaluate_descriptor_inner(
                table,
                piece.descriptor(),
                environment.with_progress(progress)?,
            )?
        }
    };
    if result.value_type() != value.property_type().clone() {
        return Err(DescriptorEvaluationError::TypeMismatch { descriptor });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fcs_model::{
        Beat, CanonicalDescriptorDomain, CanonicalDescriptorRoot, CanonicalExpressionDag,
        CanonicalExpressionNode, CanonicalExpressionOpcode, CanonicalExpressionType,
        CanonicalPiece, CanonicalPropertyDescriptor,
    };

    fn domain(start: Option<f64>, end: Option<f64>, inclusive: bool) -> CanonicalDescriptorDomain {
        CanonicalDescriptorDomain::new(start, end, inclusive).unwrap()
    }

    fn constant(value: f64, domain: CanonicalDescriptorDomain) -> CanonicalPropertyDescriptor {
        CanonicalPropertyDescriptor::new(
            CanonicalExpressionType::Float,
            domain,
            CanonicalDescriptorKind::Constant(CanonicalExpressionValue::Float(value)),
        )
        .unwrap()
    }

    fn root(descriptor: usize) -> CanonicalDescriptorRoot {
        CanonicalDescriptorRoot::new("line.alpha", 1, descriptor).unwrap()
    }

    fn environment(time: f64) -> ExpressionEnvironment {
        ExpressionEnvironment::new(time, 0.0, 0.0, 0.0).unwrap()
    }

    #[test]
    fn constant_descriptor_uses_runtime_constant_values() {
        let beat = CanonicalPropertyDescriptor::new(
            CanonicalExpressionType::Beat,
            domain(None, None, false),
            CanonicalDescriptorKind::Constant(CanonicalExpressionValue::ExactBeat(
                Beat::new(1, 2).unwrap(),
            )),
        )
        .unwrap();
        let vector = CanonicalPropertyDescriptor::new(
            CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Beat)),
            domain(None, None, false),
            CanonicalDescriptorKind::Constant(CanonicalExpressionValue::Vec2(
                Box::new(CanonicalExpressionValue::ExactBeat(
                    Beat::new(1, 2).unwrap(),
                )),
                Box::new(CanonicalExpressionValue::ExactBeat(
                    Beat::new(3, 2).unwrap(),
                )),
            )),
        )
        .unwrap();
        let table = CanonicalDescriptorTable::new(
            vec![beat, vector],
            vec![
                CanonicalDescriptorRoot::new("line.beat", 1, 0).unwrap(),
                CanonicalDescriptorRoot::new("line.position", 1, 1).unwrap(),
            ],
        )
        .unwrap();

        assert_eq!(
            evaluate_descriptor(&table, 0, environment(0.0)).unwrap(),
            CanonicalExpressionValue::Beat(0.5)
        );
        assert_eq!(
            evaluate_descriptor(&table, 1, environment(0.0)).unwrap(),
            CanonicalExpressionValue::Vec2(
                Box::new(CanonicalExpressionValue::Beat(0.5)),
                Box::new(CanonicalExpressionValue::Beat(1.5)),
            )
        );
    }

    #[test]
    fn piecewise_selects_half_open_boundaries_and_final_inclusive_end() {
        let first = constant(1.0, domain(Some(0.0), Some(5.0), false));
        let second = constant(2.0, domain(Some(5.0), Some(10.0), true));
        let parent = CanonicalPropertyDescriptor::new(
            CanonicalExpressionType::Float,
            domain(Some(0.0), Some(10.0), true),
            CanonicalDescriptorKind::Piecewise(vec![
                CanonicalPiece::new(Some(0.0), Some(5.0), false, 0).unwrap(),
                CanonicalPiece::new(Some(5.0), Some(10.0), true, 1).unwrap(),
            ]),
        )
        .unwrap();
        let table =
            CanonicalDescriptorTable::new(vec![first, second, parent], vec![root(2)]).unwrap();
        let descriptor = table.roots()[0].descriptor();
        assert_eq!(
            evaluate_descriptor(&table, descriptor, environment(4.999)).unwrap(),
            CanonicalExpressionValue::Float(1.0)
        );
        assert_eq!(
            evaluate_descriptor(&table, descriptor, environment(5.0)).unwrap(),
            CanonicalExpressionValue::Float(2.0)
        );
        assert_eq!(
            evaluate_descriptor(&table, descriptor, environment(10.0)).unwrap(),
            CanonicalExpressionValue::Float(2.0)
        );
    }

    #[test]
    fn piecewise_rebinds_progress_for_nested_expression() {
        let expression = CanonicalExpressionDag::new(
            vec![CanonicalExpressionNode::new(
                CanonicalExpressionOpcode::EnvP,
                CanonicalExpressionType::Float,
                [None; 3],
                None,
                0,
            )],
            0,
        )
        .unwrap();
        let expression = CanonicalPropertyDescriptor::new(
            CanonicalExpressionType::Float,
            domain(Some(0.0), Some(5.0), false),
            CanonicalDescriptorKind::Expression(expression),
        )
        .unwrap();
        let inner = CanonicalPropertyDescriptor::new(
            CanonicalExpressionType::Float,
            domain(Some(0.0), Some(10.0), true),
            CanonicalDescriptorKind::Piecewise(vec![
                CanonicalPiece::new(Some(0.0), Some(5.0), false, 0).unwrap(),
                CanonicalPiece::new(Some(5.0), Some(10.0), true, 2).unwrap(),
            ]),
        )
        .unwrap();
        let second = constant(9.0, domain(Some(5.0), Some(10.0), true));
        let outer = CanonicalPropertyDescriptor::new(
            CanonicalExpressionType::Float,
            domain(Some(0.0), Some(10.0), true),
            CanonicalDescriptorKind::Piecewise(vec![
                CanonicalPiece::new(Some(0.0), Some(10.0), true, 1).unwrap(),
            ]),
        )
        .unwrap();
        let table =
            CanonicalDescriptorTable::new(vec![expression, inner, second, outer], vec![root(3)])
                .unwrap();
        let descriptor = table.roots()[0].descriptor();
        let CanonicalExpressionValue::Float(value) =
            evaluate_descriptor(&table, descriptor, environment(2.5)).unwrap()
        else {
            panic!("expected scalar expression result");
        };
        assert_eq!(value.to_bits(), 0.5_f64.to_bits());
    }

    #[test]
    fn unbounded_piece_binds_fixed_progress() {
        let expression = CanonicalExpressionDag::new(
            vec![CanonicalExpressionNode::new(
                CanonicalExpressionOpcode::EnvP,
                CanonicalExpressionType::Float,
                [None; 3],
                None,
                0,
            )],
            0,
        )
        .unwrap();
        let before = CanonicalPropertyDescriptor::new(
            CanonicalExpressionType::Float,
            domain(None, Some(0.0), false),
            CanonicalDescriptorKind::Expression(expression.clone()),
        )
        .unwrap();
        let after = CanonicalPropertyDescriptor::new(
            CanonicalExpressionType::Float,
            domain(Some(0.0), None, false),
            CanonicalDescriptorKind::Expression(expression),
        )
        .unwrap();
        let parent = CanonicalPropertyDescriptor::new(
            CanonicalExpressionType::Float,
            domain(None, None, false),
            CanonicalDescriptorKind::Piecewise(vec![
                CanonicalPiece::new(None, Some(0.0), false, 0).unwrap(),
                CanonicalPiece::new(Some(0.0), None, false, 1).unwrap(),
            ]),
        )
        .unwrap();
        let table =
            CanonicalDescriptorTable::new(vec![before, after, parent], vec![root(2)]).unwrap();
        let descriptor = table.roots()[0].descriptor();
        assert_eq!(
            evaluate_descriptor(&table, descriptor, environment(-10.0)).unwrap(),
            CanonicalExpressionValue::Float(0.0)
        );
        assert_eq!(
            evaluate_descriptor(&table, descriptor, environment(10.0)).unwrap(),
            CanonicalExpressionValue::Float(1.0)
        );
    }
}
