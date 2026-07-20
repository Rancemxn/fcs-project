use std::fmt;

use fcs_model::{
    CanonicalExpressionDag, CanonicalExpressionEnvironment, CanonicalExpressionNode,
    CanonicalExpressionOpcode, CanonicalExpressionValue,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExpressionEnvironment {
    chart_time: f64,
    chart_beat: f64,
    line_scroll_q: f64,
    note_distance: f64,
    progress: Option<f64>,
}

impl ExpressionEnvironment {
    pub fn new(
        chart_time: f64,
        chart_beat: f64,
        line_scroll_q: f64,
        note_distance: f64,
    ) -> Result<Self, ExpressionEvaluationError> {
        let environment = Self {
            chart_time,
            chart_beat,
            line_scroll_q,
            note_distance,
            progress: None,
        };
        environment.validate()
    }

    pub fn with_progress(mut self, progress: f64) -> Result<Self, ExpressionEvaluationError> {
        if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
            return Err(ExpressionEvaluationError::InvalidEnvironment {
                environment: CanonicalExpressionEnvironment::P,
            });
        }
        self.progress = Some(progress);
        Ok(self)
    }

    pub const fn chart_time(self) -> f64 {
        self.chart_time
    }

    pub const fn chart_beat(self) -> f64 {
        self.chart_beat
    }

    pub const fn line_scroll_q(self) -> f64 {
        self.line_scroll_q
    }

    pub const fn note_distance(self) -> f64 {
        self.note_distance
    }

    pub const fn progress(self) -> Option<f64> {
        self.progress
    }

    fn validate(self) -> Result<Self, ExpressionEvaluationError> {
        for (environment, value) in [
            (CanonicalExpressionEnvironment::S, self.chart_time),
            (CanonicalExpressionEnvironment::B, self.chart_beat),
            (CanonicalExpressionEnvironment::Q, self.line_scroll_q),
            (CanonicalExpressionEnvironment::D, self.note_distance),
        ] {
            if !value.is_finite() {
                return Err(ExpressionEvaluationError::InvalidEnvironment { environment });
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionEvaluationError {
    InvalidEnvironment {
        environment: CanonicalExpressionEnvironment,
    },
    EnvironmentUnavailable {
        node: usize,
        environment: CanonicalExpressionEnvironment,
    },
    TypeMismatch {
        node: usize,
        opcode: CanonicalExpressionOpcode,
    },
    InvalidOperation {
        node: usize,
        opcode: CanonicalExpressionOpcode,
    },
    DivisionByZero {
        node: usize,
    },
    IntegerOverflow {
        node: usize,
    },
    Domain {
        node: usize,
    },
    NonFiniteResult {
        node: usize,
    },
}

impl fmt::Display for ExpressionEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEnvironment { environment } => {
                write!(
                    formatter,
                    "expression environment {environment:?} is invalid"
                )
            }
            Self::EnvironmentUnavailable { node, environment } => {
                write!(
                    formatter,
                    "expression node {node} requires unavailable environment {environment:?}"
                )
            }
            Self::TypeMismatch { node, opcode } => {
                write!(
                    formatter,
                    "expression node {node} has a type mismatch for {opcode:?}"
                )
            }
            Self::InvalidOperation { node, opcode } => {
                write!(
                    formatter,
                    "expression node {node} has an invalid operation {opcode:?}"
                )
            }
            Self::DivisionByZero { node } => {
                write!(formatter, "expression node {node} divides by zero")
            }
            Self::IntegerOverflow { node } => {
                write!(formatter, "expression node {node} overflows i64")
            }
            Self::Domain { node } => {
                write!(formatter, "expression node {node} has an invalid domain")
            }
            Self::NonFiniteResult { node } => write!(
                formatter,
                "expression node {node} produced a non-finite result"
            ),
        }
    }
}

impl std::error::Error for ExpressionEvaluationError {}

pub fn evaluate_expression(
    expression: &CanonicalExpressionDag,
    environment: ExpressionEnvironment,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    let mut values = vec![None; expression.nodes().len()];
    evaluate_node(expression, expression.root(), environment, &mut values)
}

fn evaluate_node(
    expression: &CanonicalExpressionDag,
    index: usize,
    environment: ExpressionEnvironment,
    values: &mut [Option<Result<CanonicalExpressionValue, ExpressionEvaluationError>>],
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    if let Some(value) = &values[index] {
        return value.clone();
    }
    let node = &expression.nodes()[index];
    let result = evaluate_node_inner(expression, index, node, environment, values);
    values[index] = Some(result.clone());
    result
}

fn evaluate_node_inner(
    expression: &CanonicalExpressionDag,
    index: usize,
    node: &CanonicalExpressionNode,
    environment: ExpressionEnvironment,
    values: &mut [Option<Result<CanonicalExpressionValue, ExpressionEvaluationError>>],
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    let opcode = node.opcode();
    let operands = node.operands();
    let mut operand = |slot: usize| -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
        let Some(operand) = operands[slot] else {
            return Err(ExpressionEvaluationError::TypeMismatch {
                node: index,
                opcode,
            });
        };
        evaluate_node(expression, operand, environment, values)
    };
    let result = match opcode {
        CanonicalExpressionOpcode::Constant => {
            node.constant()
                .cloned()
                .ok_or(ExpressionEvaluationError::TypeMismatch {
                    node: index,
                    opcode,
                })
        }
        CanonicalExpressionOpcode::EnvS => {
            Ok(CanonicalExpressionValue::Time(environment.chart_time()))
        }
        CanonicalExpressionOpcode::EnvB => {
            Ok(CanonicalExpressionValue::Beat(environment.chart_beat()))
        }
        CanonicalExpressionOpcode::EnvQ => {
            Ok(CanonicalExpressionValue::Float(environment.line_scroll_q()))
        }
        CanonicalExpressionOpcode::EnvD => Ok(CanonicalExpressionValue::Length(
            environment.note_distance(),
        )),
        CanonicalExpressionOpcode::EnvP => environment
            .progress()
            .map(CanonicalExpressionValue::Float)
            .ok_or(ExpressionEvaluationError::EnvironmentUnavailable {
                node: index,
                environment: CanonicalExpressionEnvironment::P,
            }),
        CanonicalExpressionOpcode::Neg => unary_numeric(
            index,
            opcode,
            operand(0),
            |value| {
                value
                    .checked_neg()
                    .ok_or(ExpressionEvaluationError::IntegerOverflow { node: index })
            },
            |value| Ok(-value),
        ),
        CanonicalExpressionOpcode::Not => match operand(0)? {
            CanonicalExpressionValue::Bool(value) => Ok(CanonicalExpressionValue::Bool(!value)),
            _ => Err(ExpressionEvaluationError::TypeMismatch {
                node: index,
                opcode,
            }),
        },
        CanonicalExpressionOpcode::Add => {
            binary_same(index, opcode, operand(0), operand(1), add_values)
        }
        CanonicalExpressionOpcode::Sub => {
            binary_same(index, opcode, operand(0), operand(1), sub_values)
        }
        CanonicalExpressionOpcode::Mul => binary_mul(index, opcode, operand(0), operand(1)),
        CanonicalExpressionOpcode::Div => binary_div(index, opcode, operand(0), operand(1)),
        CanonicalExpressionOpcode::Mod => match (operand(0)?, operand(1)?) {
            (CanonicalExpressionValue::Int(left), CanonicalExpressionValue::Int(right)) => {
                if right == 0 {
                    Err(ExpressionEvaluationError::DivisionByZero { node: index })
                } else {
                    left.checked_rem(right)
                        .map(CanonicalExpressionValue::Int)
                        .ok_or(ExpressionEvaluationError::IntegerOverflow { node: index })
                }
            }
            _ => Err(ExpressionEvaluationError::TypeMismatch {
                node: index,
                opcode,
            }),
        },
        CanonicalExpressionOpcode::Pow => pow_values(index, opcode, operand(0)?, operand(1)?),
        CanonicalExpressionOpcode::Eq | CanonicalExpressionOpcode::Ne => {
            let left = operand(0)?;
            let right = operand(1)?;
            if left.value_type() != right.value_type() {
                Err(ExpressionEvaluationError::TypeMismatch {
                    node: index,
                    opcode,
                })
            } else {
                let equal = equal_values(&left, &right);
                Ok(CanonicalExpressionValue::Bool(
                    if opcode == CanonicalExpressionOpcode::Eq {
                        equal
                    } else {
                        !equal
                    },
                ))
            }
        }
        CanonicalExpressionOpcode::Lt
        | CanonicalExpressionOpcode::Le
        | CanonicalExpressionOpcode::Gt
        | CanonicalExpressionOpcode::Ge => compare_values(index, opcode, operand(0)?, operand(1)?),
        CanonicalExpressionOpcode::And => match operand(0)? {
            CanonicalExpressionValue::Bool(false) => Ok(CanonicalExpressionValue::Bool(false)),
            CanonicalExpressionValue::Bool(true) => match operand(1)? {
                CanonicalExpressionValue::Bool(value) => Ok(CanonicalExpressionValue::Bool(value)),
                _ => Err(ExpressionEvaluationError::TypeMismatch {
                    node: index,
                    opcode,
                }),
            },
            _ => Err(ExpressionEvaluationError::TypeMismatch {
                node: index,
                opcode,
            }),
        },
        CanonicalExpressionOpcode::Or => match operand(0)? {
            CanonicalExpressionValue::Bool(true) => Ok(CanonicalExpressionValue::Bool(true)),
            CanonicalExpressionValue::Bool(false) => match operand(1)? {
                CanonicalExpressionValue::Bool(value) => Ok(CanonicalExpressionValue::Bool(value)),
                _ => Err(ExpressionEvaluationError::TypeMismatch {
                    node: index,
                    opcode,
                }),
            },
            _ => Err(ExpressionEvaluationError::TypeMismatch {
                node: index,
                opcode,
            }),
        },
        CanonicalExpressionOpcode::ApproxEq => {
            let left = expect_float(index, opcode, operand(0)?)?;
            let right = expect_float(index, opcode, operand(1)?)?;
            let tolerance = expect_float(index, opcode, operand(2)?)?;
            if tolerance < 0.0 {
                Err(ExpressionEvaluationError::Domain { node: index })
            } else {
                let difference = finite_value(index, left - right)?;
                let distance = finite_value(index, difference.abs())?;
                Ok(CanonicalExpressionValue::Bool(distance <= tolerance))
            }
        }
        CanonicalExpressionOpcode::Abs => unary_numeric(
            index,
            opcode,
            operand(0),
            |value| {
                value
                    .checked_abs()
                    .ok_or(ExpressionEvaluationError::IntegerOverflow { node: index })
            },
            |value| Ok(value.abs()),
        ),
        CanonicalExpressionOpcode::Min => {
            binary_same(index, opcode, operand(0), operand(1), min_values)
        }
        CanonicalExpressionOpcode::Max => {
            binary_same(index, opcode, operand(0), operand(1), max_values)
        }
        CanonicalExpressionOpcode::Clamp => {
            let value = operand(0)?;
            let low = operand(1)?;
            let high = operand(2)?;
            clamp_value(index, opcode, value, low, high)
        }
        CanonicalExpressionOpcode::Floor
        | CanonicalExpressionOpcode::Ceil
        | CanonicalExpressionOpcode::Round
        | CanonicalExpressionOpcode::Sqrt
        | CanonicalExpressionOpcode::Exp
        | CanonicalExpressionOpcode::Ln
        | CanonicalExpressionOpcode::Sin
        | CanonicalExpressionOpcode::Cos
        | CanonicalExpressionOpcode::Tan
        | CanonicalExpressionOpcode::Asin
        | CanonicalExpressionOpcode::Acos
        | CanonicalExpressionOpcode::Atan => {
            let value = expect_float(index, opcode, operand(0)?)?;
            unary_math(index, opcode, value)
        }
        CanonicalExpressionOpcode::Atan2 => {
            let y = expect_float(index, opcode, operand(0)?)?;
            let x = expect_float(index, opcode, operand(1)?)?;
            finite_value(index, y.atan2(x)).map(CanonicalExpressionValue::Float)
        }
        CanonicalExpressionOpcode::Easing => {
            let value = expect_float(index, opcode, operand(0)?)?;
            crate::EasingId::try_from(u16::try_from(node.immediate()).map_err(|_| {
                ExpressionEvaluationError::InvalidOperation {
                    node: index,
                    opcode,
                }
            })?)
            .map_err(|_| ExpressionEvaluationError::InvalidOperation {
                node: index,
                opcode,
            })?
            .evaluate(value)
            .map(CanonicalExpressionValue::Float)
            .map_err(|_| ExpressionEvaluationError::NonFiniteResult { node: index })
        }
        CanonicalExpressionOpcode::ToFloat => match operand(0)? {
            CanonicalExpressionValue::Int(value) => {
                finite_value(index, value as f64).map(CanonicalExpressionValue::Float)
            }
            _ => Err(ExpressionEvaluationError::TypeMismatch {
                node: index,
                opcode,
            }),
        },
        CanonicalExpressionOpcode::Seconds => match operand(0)? {
            CanonicalExpressionValue::Time(value) => {
                finite_value(index, value).map(CanonicalExpressionValue::Float)
            }
            _ => Err(ExpressionEvaluationError::TypeMismatch {
                node: index,
                opcode,
            }),
        },
        CanonicalExpressionOpcode::Radians => match operand(0)? {
            CanonicalExpressionValue::Angle(value) => {
                finite_value(index, value).map(CanonicalExpressionValue::Float)
            }
            _ => Err(ExpressionEvaluationError::TypeMismatch {
                node: index,
                opcode,
            }),
        },
        CanonicalExpressionOpcode::Choose => match operand(0)? {
            CanonicalExpressionValue::Bool(true) => operand(1),
            CanonicalExpressionValue::Bool(false) => operand(2),
            _ => Err(ExpressionEvaluationError::TypeMismatch {
                node: index,
                opcode,
            }),
        },
        CanonicalExpressionOpcode::Vec2 => {
            let left = operand(0)?;
            let right = operand(1)?;
            if left.value_type() != right.value_type() || !left.value_type().is_numeric() {
                Err(ExpressionEvaluationError::TypeMismatch {
                    node: index,
                    opcode,
                })
            } else {
                Ok(CanonicalExpressionValue::Vec2(
                    Box::new(left),
                    Box::new(right),
                ))
            }
        }
        CanonicalExpressionOpcode::Vec2X => match operand(0)? {
            CanonicalExpressionValue::Vec2(left, _) => Ok(*left),
            _ => Err(ExpressionEvaluationError::TypeMismatch {
                node: index,
                opcode,
            }),
        },
        CanonicalExpressionOpcode::Vec2Y => match operand(0)? {
            CanonicalExpressionValue::Vec2(_, right) => Ok(*right),
            _ => Err(ExpressionEvaluationError::TypeMismatch {
                node: index,
                opcode,
            }),
        },
    }?;
    if result.value_type() != *node.result_type() || !result.is_finite() {
        return Err(ExpressionEvaluationError::NonFiniteResult { node: index });
    }
    Ok(result)
}

fn unary_numeric(
    index: usize,
    opcode: CanonicalExpressionOpcode,
    value: Result<CanonicalExpressionValue, ExpressionEvaluationError>,
    int_operation: impl FnOnce(i64) -> Result<i64, ExpressionEvaluationError>,
    float_operation: impl FnOnce(f64) -> Result<f64, ExpressionEvaluationError>,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    match value? {
        CanonicalExpressionValue::Int(value) => {
            int_operation(value).map(CanonicalExpressionValue::Int)
        }
        CanonicalExpressionValue::Float(value) => {
            float_operation(value).map(CanonicalExpressionValue::Float)
        }
        CanonicalExpressionValue::Time(value) => {
            float_operation(value).map(CanonicalExpressionValue::Time)
        }
        CanonicalExpressionValue::Beat(value) => {
            float_operation(value).map(CanonicalExpressionValue::Beat)
        }
        CanonicalExpressionValue::Length(value) => {
            float_operation(value).map(CanonicalExpressionValue::Length)
        }
        CanonicalExpressionValue::Angle(value) => {
            float_operation(value).map(CanonicalExpressionValue::Angle)
        }
        _ => Err(ExpressionEvaluationError::TypeMismatch {
            node: index,
            opcode,
        }),
    }
}

fn binary_same(
    index: usize,
    opcode: CanonicalExpressionOpcode,
    left: Result<CanonicalExpressionValue, ExpressionEvaluationError>,
    right: Result<CanonicalExpressionValue, ExpressionEvaluationError>,
    operation: impl FnOnce(
        usize,
        CanonicalExpressionValue,
        CanonicalExpressionValue,
    ) -> Result<CanonicalExpressionValue, ExpressionEvaluationError>,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    let left = left?;
    let right = right?;
    if left.value_type() != right.value_type() {
        return Err(ExpressionEvaluationError::TypeMismatch {
            node: index,
            opcode,
        });
    }
    operation(index, left, right)
}

fn add_values(
    index: usize,
    left: CanonicalExpressionValue,
    right: CanonicalExpressionValue,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    match (left, right) {
        (CanonicalExpressionValue::Int(left), CanonicalExpressionValue::Int(right)) => left
            .checked_add(right)
            .map(CanonicalExpressionValue::Int)
            .ok_or(ExpressionEvaluationError::IntegerOverflow { node: index }),
        (
            CanonicalExpressionValue::Vec2(left_x, left_y),
            CanonicalExpressionValue::Vec2(right_x, right_y),
        ) => vector_pair(
            index,
            CanonicalExpressionOpcode::Add,
            *left_x,
            *left_y,
            *right_x,
            *right_y,
            add_values,
        ),
        (left, right) => map_float_pair(
            index,
            left,
            right,
            |left, right| left + right,
            |left, right| map_unit_pair(left, right, |left, right| left + right),
        ),
    }
}

fn sub_values(
    index: usize,
    left: CanonicalExpressionValue,
    right: CanonicalExpressionValue,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    match (left, right) {
        (CanonicalExpressionValue::Int(left), CanonicalExpressionValue::Int(right)) => left
            .checked_sub(right)
            .map(CanonicalExpressionValue::Int)
            .ok_or(ExpressionEvaluationError::IntegerOverflow { node: index }),
        (
            CanonicalExpressionValue::Vec2(left_x, left_y),
            CanonicalExpressionValue::Vec2(right_x, right_y),
        ) => vector_pair(
            index,
            CanonicalExpressionOpcode::Sub,
            *left_x,
            *left_y,
            *right_x,
            *right_y,
            sub_values,
        ),
        (left, right) => map_float_pair(
            index,
            left,
            right,
            |left, right| left - right,
            |left, right| map_unit_pair(left, right, |left, right| left - right),
        ),
    }
}

fn binary_mul(
    index: usize,
    opcode: CanonicalExpressionOpcode,
    left: Result<CanonicalExpressionValue, ExpressionEvaluationError>,
    right: Result<CanonicalExpressionValue, ExpressionEvaluationError>,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    let left = left?;
    let right = right?;
    match (left, right) {
        (CanonicalExpressionValue::Int(left), CanonicalExpressionValue::Int(right)) => left
            .checked_mul(right)
            .map(CanonicalExpressionValue::Int)
            .ok_or(ExpressionEvaluationError::IntegerOverflow { node: index }),
        (CanonicalExpressionValue::Vec2(x, y), scalar) => {
            scale_vector(index, opcode, *x, *y, scalar, false)
        }
        (scalar, CanonicalExpressionValue::Vec2(x, y)) => {
            scale_vector(index, opcode, *x, *y, scalar, false)
        }
        (left, right) => {
            scalar_unit_operation(index, opcode, left, right, |left, right| left * right)
        }
    }
}

fn binary_div(
    index: usize,
    opcode: CanonicalExpressionOpcode,
    left: Result<CanonicalExpressionValue, ExpressionEvaluationError>,
    right: Result<CanonicalExpressionValue, ExpressionEvaluationError>,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    let left = left?;
    let right = right?;
    if matches!(right, CanonicalExpressionValue::Int(0))
        || matches!(right, CanonicalExpressionValue::Float(value) if value == 0.0)
        || matches!(right, CanonicalExpressionValue::Time(value) if value == 0.0)
        || matches!(right, CanonicalExpressionValue::Beat(value) if value == 0.0)
        || matches!(right, CanonicalExpressionValue::Length(value) if value == 0.0)
        || matches!(right, CanonicalExpressionValue::Angle(value) if value == 0.0)
    {
        return Err(ExpressionEvaluationError::DivisionByZero { node: index });
    }
    match (left, right) {
        (CanonicalExpressionValue::Vec2(x, y), scalar) => {
            scale_vector(index, opcode, *x, *y, scalar, true)
        }
        (CanonicalExpressionValue::Int(left), CanonicalExpressionValue::Int(right)) => left
            .checked_div(right)
            .map(CanonicalExpressionValue::Int)
            .ok_or(ExpressionEvaluationError::IntegerOverflow { node: index }),
        (left, right) => scalar_unit_div(index, opcode, left, right),
    }
}

fn vector_pair(
    index: usize,
    opcode: CanonicalExpressionOpcode,
    left_x: CanonicalExpressionValue,
    left_y: CanonicalExpressionValue,
    right_x: CanonicalExpressionValue,
    right_y: CanonicalExpressionValue,
    operation: fn(
        usize,
        CanonicalExpressionValue,
        CanonicalExpressionValue,
    ) -> Result<CanonicalExpressionValue, ExpressionEvaluationError>,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    let x = operation(index, left_x, right_x)?;
    let y = operation(index, left_y, right_y)?;
    if x.value_type() != y.value_type() {
        return Err(ExpressionEvaluationError::TypeMismatch {
            node: index,
            opcode,
        });
    }
    Ok(CanonicalExpressionValue::Vec2(Box::new(x), Box::new(y)))
}

fn scale_vector(
    index: usize,
    opcode: CanonicalExpressionOpcode,
    x: CanonicalExpressionValue,
    y: CanonicalExpressionValue,
    scalar: CanonicalExpressionValue,
    divide: bool,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    let x = scale_value(index, opcode, x, scalar.clone(), divide)?;
    let y = scale_value(index, opcode, y, scalar, divide)?;
    if x.value_type() != y.value_type() {
        return Err(ExpressionEvaluationError::TypeMismatch {
            node: index,
            opcode,
        });
    }
    Ok(CanonicalExpressionValue::Vec2(Box::new(x), Box::new(y)))
}

fn scale_value(
    index: usize,
    opcode: CanonicalExpressionOpcode,
    value: CanonicalExpressionValue,
    scalar: CanonicalExpressionValue,
    divide: bool,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    let operation = |left: f64, right: f64| if divide { left / right } else { left * right };
    match (value, scalar) {
        (CanonicalExpressionValue::Int(left), CanonicalExpressionValue::Int(right)) => {
            if divide {
                left.checked_div(right)
                    .map(CanonicalExpressionValue::Int)
                    .ok_or(ExpressionEvaluationError::IntegerOverflow { node: index })
            } else {
                left.checked_mul(right)
                    .map(CanonicalExpressionValue::Int)
                    .ok_or(ExpressionEvaluationError::IntegerOverflow { node: index })
            }
        }
        (CanonicalExpressionValue::Int(left), CanonicalExpressionValue::Float(right)) => {
            finite_value(index, operation(left as f64, right)).map(CanonicalExpressionValue::Float)
        }
        (CanonicalExpressionValue::Float(left), CanonicalExpressionValue::Int(right)) => {
            finite_value(index, operation(left, right as f64)).map(CanonicalExpressionValue::Float)
        }
        (CanonicalExpressionValue::Float(left), CanonicalExpressionValue::Float(right)) => {
            finite_value(index, operation(left, right)).map(CanonicalExpressionValue::Float)
        }
        (value, CanonicalExpressionValue::Int(right)) => {
            map_unit_scalar(index, opcode, value, right as f64, operation)
        }
        (value, CanonicalExpressionValue::Float(right)) => {
            map_unit_scalar(index, opcode, value, right, operation)
        }
        _ => Err(ExpressionEvaluationError::TypeMismatch {
            node: index,
            opcode,
        }),
    }
}

fn pow_values(
    index: usize,
    opcode: CanonicalExpressionOpcode,
    left: CanonicalExpressionValue,
    right: CanonicalExpressionValue,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    match (left, right) {
        (CanonicalExpressionValue::Int(base), CanonicalExpressionValue::Int(exponent)) => {
            if exponent < 0 {
                return Err(ExpressionEvaluationError::Domain { node: index });
            }
            let exponent = u32::try_from(exponent)
                .map_err(|_| ExpressionEvaluationError::IntegerOverflow { node: index })?;
            base.checked_pow(exponent)
                .map(CanonicalExpressionValue::Int)
                .ok_or(ExpressionEvaluationError::IntegerOverflow { node: index })
        }
        (CanonicalExpressionValue::Float(base), CanonicalExpressionValue::Float(exponent)) => {
            if base == 0.0 {
                if exponent < 0.0 {
                    return Err(ExpressionEvaluationError::Domain { node: index });
                }
                let result = if exponent == 0.0 {
                    1.0
                } else if base.is_sign_negative() && is_odd_binary64_integer(exponent) {
                    -0.0
                } else {
                    0.0
                };
                return Ok(CanonicalExpressionValue::Float(result));
            }
            if base < 0.0 && exponent.fract() != 0.0 {
                return Err(ExpressionEvaluationError::Domain { node: index });
            }
            let magnitude = base.abs().powf(exponent);
            let result = if base.is_sign_negative() && is_odd_binary64_integer(exponent) {
                -magnitude
            } else {
                magnitude
            };
            finite_value(index, result).map(CanonicalExpressionValue::Float)
        }
        _ => Err(ExpressionEvaluationError::TypeMismatch {
            node: index,
            opcode,
        }),
    }
}

fn is_odd_binary64_integer(value: f64) -> bool {
    value.fract() == 0.0
        && value.abs() < 9_007_199_254_740_992.0
        && (value as i64).rem_euclid(2) == 1
}

fn map_float_pair(
    index: usize,
    left: CanonicalExpressionValue,
    right: CanonicalExpressionValue,
    float_operation: impl FnOnce(f64, f64) -> f64,
    unit_operation: impl FnOnce(
        CanonicalExpressionValue,
        CanonicalExpressionValue,
    ) -> Result<CanonicalExpressionValue, ExpressionEvaluationError>,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    match (left, right) {
        (CanonicalExpressionValue::Float(left), CanonicalExpressionValue::Float(right)) => {
            finite_value(index, float_operation(left, right)).map(CanonicalExpressionValue::Float)
        }
        (left, right) => unit_operation(left, right),
    }
}

fn map_unit_pair(
    left: CanonicalExpressionValue,
    right: CanonicalExpressionValue,
    operation: impl FnOnce(f64, f64) -> f64,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    match (left, right) {
        (CanonicalExpressionValue::Time(left), CanonicalExpressionValue::Time(right)) => {
            Ok(CanonicalExpressionValue::Time(operation(left, right)))
        }
        (CanonicalExpressionValue::Beat(left), CanonicalExpressionValue::Beat(right)) => {
            Ok(CanonicalExpressionValue::Beat(operation(left, right)))
        }
        (CanonicalExpressionValue::Length(left), CanonicalExpressionValue::Length(right)) => {
            Ok(CanonicalExpressionValue::Length(operation(left, right)))
        }
        (CanonicalExpressionValue::Angle(left), CanonicalExpressionValue::Angle(right)) => {
            Ok(CanonicalExpressionValue::Angle(operation(left, right)))
        }
        _ => Err(ExpressionEvaluationError::TypeMismatch {
            node: usize::MAX,
            opcode: CanonicalExpressionOpcode::Add,
        }),
    }
}

fn scalar_unit_operation(
    index: usize,
    opcode: CanonicalExpressionOpcode,
    left: CanonicalExpressionValue,
    right: CanonicalExpressionValue,
    operation: impl FnOnce(f64, f64) -> f64,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    match (left, right) {
        (CanonicalExpressionValue::Float(left), CanonicalExpressionValue::Float(right)) => {
            finite_value(index, operation(left, right)).map(CanonicalExpressionValue::Float)
        }
        (CanonicalExpressionValue::Int(left), CanonicalExpressionValue::Int(right)) => {
            finite_value(index, operation(left as f64, right as f64))
                .map(CanonicalExpressionValue::Float)
        }
        (CanonicalExpressionValue::Float(left), CanonicalExpressionValue::Int(right)) => {
            finite_value(index, operation(left, right as f64)).map(CanonicalExpressionValue::Float)
        }
        (CanonicalExpressionValue::Int(left), CanonicalExpressionValue::Float(right)) => {
            finite_value(index, operation(left as f64, right)).map(CanonicalExpressionValue::Float)
        }
        (CanonicalExpressionValue::Float(left), right) => {
            map_unit_scalar(index, opcode, right, left, operation)
        }
        (CanonicalExpressionValue::Int(left), right) => {
            map_unit_scalar(index, opcode, right, left as f64, operation)
        }
        (left, CanonicalExpressionValue::Float(right)) => {
            map_unit_scalar(index, opcode, left, right, operation)
        }
        (left, CanonicalExpressionValue::Int(right)) => {
            map_unit_scalar(index, opcode, left, right as f64, operation)
        }
        _ => Err(ExpressionEvaluationError::TypeMismatch {
            node: index,
            opcode,
        }),
    }
}

fn scalar_unit_div(
    index: usize,
    opcode: CanonicalExpressionOpcode,
    left: CanonicalExpressionValue,
    right: CanonicalExpressionValue,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    match (left, right) {
        (CanonicalExpressionValue::Float(left), CanonicalExpressionValue::Float(right)) => {
            finite_value(index, left / right).map(CanonicalExpressionValue::Float)
        }
        (CanonicalExpressionValue::Time(left), CanonicalExpressionValue::Time(right))
        | (CanonicalExpressionValue::Beat(left), CanonicalExpressionValue::Beat(right))
        | (CanonicalExpressionValue::Length(left), CanonicalExpressionValue::Length(right))
        | (CanonicalExpressionValue::Angle(left), CanonicalExpressionValue::Angle(right)) => {
            finite_value(index, left / right).map(CanonicalExpressionValue::Float)
        }
        (CanonicalExpressionValue::Time(left), CanonicalExpressionValue::Float(right)) => {
            finite_value(index, left / right).map(CanonicalExpressionValue::Time)
        }
        (CanonicalExpressionValue::Beat(left), CanonicalExpressionValue::Float(right)) => {
            finite_value(index, left / right).map(CanonicalExpressionValue::Beat)
        }
        (CanonicalExpressionValue::Length(left), CanonicalExpressionValue::Float(right)) => {
            finite_value(index, left / right).map(CanonicalExpressionValue::Length)
        }
        (CanonicalExpressionValue::Angle(left), CanonicalExpressionValue::Float(right)) => {
            finite_value(index, left / right).map(CanonicalExpressionValue::Angle)
        }
        _ => Err(ExpressionEvaluationError::TypeMismatch {
            node: index,
            opcode,
        }),
    }
}

fn map_unit_scalar(
    index: usize,
    opcode: CanonicalExpressionOpcode,
    value: CanonicalExpressionValue,
    scalar: f64,
    operation: impl FnOnce(f64, f64) -> f64,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    let value = match value {
        CanonicalExpressionValue::Time(value) => {
            CanonicalExpressionValue::Time(operation(value, scalar))
        }
        CanonicalExpressionValue::Beat(value) => {
            CanonicalExpressionValue::Beat(operation(value, scalar))
        }
        CanonicalExpressionValue::Length(value) => {
            CanonicalExpressionValue::Length(operation(value, scalar))
        }
        CanonicalExpressionValue::Angle(value) => {
            CanonicalExpressionValue::Angle(operation(value, scalar))
        }
        _ => {
            return Err(ExpressionEvaluationError::TypeMismatch {
                node: index,
                opcode,
            });
        }
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ExpressionEvaluationError::NonFiniteResult { node: index })
    }
}

fn expect_float(
    node: usize,
    opcode: CanonicalExpressionOpcode,
    value: CanonicalExpressionValue,
) -> Result<f64, ExpressionEvaluationError> {
    match value {
        CanonicalExpressionValue::Float(value) => Ok(value),
        _ => Err(ExpressionEvaluationError::TypeMismatch { node, opcode }),
    }
}

fn finite_value(node: usize, value: f64) -> Result<f64, ExpressionEvaluationError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(ExpressionEvaluationError::NonFiniteResult { node })
}

fn unary_math(
    node: usize,
    opcode: CanonicalExpressionOpcode,
    value: f64,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    let result = match opcode {
        CanonicalExpressionOpcode::Floor => value.floor(),
        CanonicalExpressionOpcode::Ceil => value.ceil(),
        CanonicalExpressionOpcode::Round => value.round_ties_even(),
        CanonicalExpressionOpcode::Sqrt if value >= 0.0 => value.sqrt(),
        CanonicalExpressionOpcode::Exp => value.exp(),
        CanonicalExpressionOpcode::Ln if value > 0.0 => value.ln(),
        CanonicalExpressionOpcode::Sin => value.sin(),
        CanonicalExpressionOpcode::Cos => value.cos(),
        CanonicalExpressionOpcode::Tan => value.tan(),
        CanonicalExpressionOpcode::Asin if (-1.0..=1.0).contains(&value) => value.asin(),
        CanonicalExpressionOpcode::Acos if (-1.0..=1.0).contains(&value) => value.acos(),
        CanonicalExpressionOpcode::Atan => value.atan(),
        CanonicalExpressionOpcode::Sqrt
        | CanonicalExpressionOpcode::Ln
        | CanonicalExpressionOpcode::Asin
        | CanonicalExpressionOpcode::Acos => {
            return Err(ExpressionEvaluationError::Domain { node });
        }
        _ => return Err(ExpressionEvaluationError::InvalidOperation { node, opcode }),
    };
    finite_value(node, result).map(CanonicalExpressionValue::Float)
}

fn equal_values(left: &CanonicalExpressionValue, right: &CanonicalExpressionValue) -> bool {
    match (left, right) {
        (CanonicalExpressionValue::Bool(left), CanonicalExpressionValue::Bool(right)) => {
            left == right
        }
        (CanonicalExpressionValue::Int(left), CanonicalExpressionValue::Int(right)) => {
            left == right
        }
        (CanonicalExpressionValue::Float(left), CanonicalExpressionValue::Float(right))
        | (CanonicalExpressionValue::Time(left), CanonicalExpressionValue::Time(right))
        | (CanonicalExpressionValue::Beat(left), CanonicalExpressionValue::Beat(right))
        | (CanonicalExpressionValue::Length(left), CanonicalExpressionValue::Length(right))
        | (CanonicalExpressionValue::Angle(left), CanonicalExpressionValue::Angle(right)) => {
            left == right
        }
        (
            CanonicalExpressionValue::Vec2(left_x, left_y),
            CanonicalExpressionValue::Vec2(right_x, right_y),
        ) => equal_values(left_x, right_x) && equal_values(left_y, right_y),
        _ => false,
    }
}

fn compare_values(
    node: usize,
    opcode: CanonicalExpressionOpcode,
    left: CanonicalExpressionValue,
    right: CanonicalExpressionValue,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    let result = match (left, right) {
        (CanonicalExpressionValue::Int(left), CanonicalExpressionValue::Int(right)) => {
            compare_order(opcode, left, right)
        }
        (CanonicalExpressionValue::Float(left), CanonicalExpressionValue::Float(right))
        | (CanonicalExpressionValue::Time(left), CanonicalExpressionValue::Time(right))
        | (CanonicalExpressionValue::Beat(left), CanonicalExpressionValue::Beat(right))
        | (CanonicalExpressionValue::Length(left), CanonicalExpressionValue::Length(right))
        | (CanonicalExpressionValue::Angle(left), CanonicalExpressionValue::Angle(right)) => {
            compare_order(opcode, left, right)
        }
        _ => return Err(ExpressionEvaluationError::TypeMismatch { node, opcode }),
    };
    Ok(CanonicalExpressionValue::Bool(result))
}

fn compare_order<T: PartialOrd>(opcode: CanonicalExpressionOpcode, left: T, right: T) -> bool {
    match opcode {
        CanonicalExpressionOpcode::Lt => left < right,
        CanonicalExpressionOpcode::Le => left <= right,
        CanonicalExpressionOpcode::Gt => left > right,
        CanonicalExpressionOpcode::Ge => left >= right,
        _ => false,
    }
}

fn min_values(
    node: usize,
    left: CanonicalExpressionValue,
    right: CanonicalExpressionValue,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    select_min_max(node, left, right, false)
}

fn max_values(
    node: usize,
    left: CanonicalExpressionValue,
    right: CanonicalExpressionValue,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    select_min_max(node, left, right, true)
}

fn select_min_max(
    node: usize,
    left: CanonicalExpressionValue,
    right: CanonicalExpressionValue,
    maximum: bool,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    match (left, right) {
        (CanonicalExpressionValue::Int(left), CanonicalExpressionValue::Int(right)) => Ok(
            CanonicalExpressionValue::Int(if (left < right) == maximum {
                right
            } else {
                left
            }),
        ),
        (CanonicalExpressionValue::Float(left), CanonicalExpressionValue::Float(right)) => Ok(
            CanonicalExpressionValue::Float(select_float(left, right, maximum)),
        ),
        (CanonicalExpressionValue::Time(left), CanonicalExpressionValue::Time(right)) => Ok(
            CanonicalExpressionValue::Time(select_float(left, right, maximum)),
        ),
        (CanonicalExpressionValue::Beat(left), CanonicalExpressionValue::Beat(right)) => Ok(
            CanonicalExpressionValue::Beat(select_float(left, right, maximum)),
        ),
        (CanonicalExpressionValue::Length(left), CanonicalExpressionValue::Length(right)) => Ok(
            CanonicalExpressionValue::Length(select_float(left, right, maximum)),
        ),
        (CanonicalExpressionValue::Angle(left), CanonicalExpressionValue::Angle(right)) => Ok(
            CanonicalExpressionValue::Angle(select_float(left, right, maximum)),
        ),
        _ => Err(ExpressionEvaluationError::TypeMismatch {
            node,
            opcode: if maximum {
                CanonicalExpressionOpcode::Max
            } else {
                CanonicalExpressionOpcode::Min
            },
        }),
    }
}

fn select_float(left: f64, right: f64, maximum: bool) -> f64 {
    if left == right {
        return left;
    }
    if (left < right) == maximum {
        right
    } else {
        left
    }
}

fn clamp_value(
    node: usize,
    opcode: CanonicalExpressionOpcode,
    value: CanonicalExpressionValue,
    low: CanonicalExpressionValue,
    high: CanonicalExpressionValue,
) -> Result<CanonicalExpressionValue, ExpressionEvaluationError> {
    if value.value_type() != low.value_type() || value.value_type() != high.value_type() {
        return Err(ExpressionEvaluationError::TypeMismatch { node, opcode });
    }
    match (value, low, high) {
        (
            CanonicalExpressionValue::Int(value),
            CanonicalExpressionValue::Int(low),
            CanonicalExpressionValue::Int(high),
        ) => {
            if low > high {
                return Err(ExpressionEvaluationError::Domain { node });
            }
            Ok(CanonicalExpressionValue::Int(value.max(low).min(high)))
        }
        (
            CanonicalExpressionValue::Float(value),
            CanonicalExpressionValue::Float(low),
            CanonicalExpressionValue::Float(high),
        ) => {
            if low > high {
                return Err(ExpressionEvaluationError::Domain { node });
            }
            Ok(CanonicalExpressionValue::Float(if value < low {
                low
            } else if value > high {
                high
            } else {
                value
            }))
        }
        (
            CanonicalExpressionValue::Time(value),
            CanonicalExpressionValue::Time(low),
            CanonicalExpressionValue::Time(high),
        ) => clamp_float_value(node, value, low, high).map(CanonicalExpressionValue::Time),
        (
            CanonicalExpressionValue::Beat(value),
            CanonicalExpressionValue::Beat(low),
            CanonicalExpressionValue::Beat(high),
        ) => clamp_float_value(node, value, low, high).map(CanonicalExpressionValue::Beat),
        (
            CanonicalExpressionValue::Length(value),
            CanonicalExpressionValue::Length(low),
            CanonicalExpressionValue::Length(high),
        ) => clamp_float_value(node, value, low, high).map(CanonicalExpressionValue::Length),
        (
            CanonicalExpressionValue::Angle(value),
            CanonicalExpressionValue::Angle(low),
            CanonicalExpressionValue::Angle(high),
        ) => clamp_float_value(node, value, low, high).map(CanonicalExpressionValue::Angle),
        _ => Err(ExpressionEvaluationError::TypeMismatch { node, opcode }),
    }
}

fn clamp_float_value(
    node: usize,
    value: f64,
    low: f64,
    high: f64,
) -> Result<f64, ExpressionEvaluationError> {
    if low > high {
        return Err(ExpressionEvaluationError::Domain { node });
    }
    Ok(if value < low {
        low
    } else if value > high {
        high
    } else {
        value
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fcs_model::{
        CanonicalExpressionBuilder, CanonicalExpressionNode, CanonicalExpressionType,
        CanonicalExpressionValue,
    };

    fn node(
        opcode: CanonicalExpressionOpcode,
        result_type: CanonicalExpressionType,
        operands: [Option<usize>; 3],
    ) -> CanonicalExpressionNode {
        CanonicalExpressionNode::new(opcode, result_type, operands, None, 0)
    }

    fn constant(value: CanonicalExpressionValue) -> CanonicalExpressionNode {
        let result_type = value.value_type();
        CanonicalExpressionNode::new(
            CanonicalExpressionOpcode::Constant,
            result_type,
            [None; 3],
            Some(value),
            0,
        )
    }

    fn exact_fixture_expression() -> CanonicalExpressionDag {
        let nodes = vec![
            node(
                CanonicalExpressionOpcode::EnvD,
                CanonicalExpressionType::Length,
                [None; 3],
            ),
            constant(CanonicalExpressionValue::Length(100.0)),
            node(
                CanonicalExpressionOpcode::Lt,
                CanonicalExpressionType::Bool,
                [Some(0), Some(1), None],
            ),
            node(
                CanonicalExpressionOpcode::EnvS,
                CanonicalExpressionType::Time,
                [None; 3],
            ),
            constant(CanonicalExpressionValue::Time(1.0)),
            node(
                CanonicalExpressionOpcode::Div,
                CanonicalExpressionType::Float,
                [Some(3), Some(4), None],
            ),
            node(
                CanonicalExpressionOpcode::Sin,
                CanonicalExpressionType::Float,
                [Some(5), None, None],
            ),
            constant(CanonicalExpressionValue::Float(0.5)),
            node(
                CanonicalExpressionOpcode::Mul,
                CanonicalExpressionType::Float,
                [Some(7), Some(6), None],
            ),
            node(
                CanonicalExpressionOpcode::Add,
                CanonicalExpressionType::Float,
                [Some(7), Some(8), None],
            ),
            constant(CanonicalExpressionValue::Float(0.25)),
            node(
                CanonicalExpressionOpcode::Choose,
                CanonicalExpressionType::Float,
                [Some(2), Some(9), Some(10)],
            ),
        ];
        CanonicalExpressionDag::new(nodes, 11).expect("fixture DAG must validate")
    }

    #[test]
    fn exact_expression_fixture_evaluates_required_environment_and_choose() {
        let expression = exact_fixture_expression();
        assert_eq!(
            expression.required_environment(),
            vec![
                CanonicalExpressionEnvironment::D,
                CanonicalExpressionEnvironment::S
            ]
        );
        let environment = ExpressionEnvironment::new(1.0, 2.0, 0.0, 50.0).unwrap();
        let value = evaluate_expression(&expression, environment).unwrap();
        let CanonicalExpressionValue::Float(value) = value else {
            panic!("expected float result");
        };
        assert_eq!(value.to_bits(), (0.5 + 0.5 * 1.0_f64.sin()).to_bits());
    }

    #[test]
    fn choose_does_not_evaluate_unselected_invalid_branch() {
        let expression = CanonicalExpressionDag::new(
            vec![
                constant(CanonicalExpressionValue::Bool(false)),
                constant(CanonicalExpressionValue::Float(-1.0)),
                node(
                    CanonicalExpressionOpcode::Sqrt,
                    CanonicalExpressionType::Float,
                    [Some(1), None, None],
                ),
                constant(CanonicalExpressionValue::Float(0.25)),
                node(
                    CanonicalExpressionOpcode::Choose,
                    CanonicalExpressionType::Float,
                    [Some(0), Some(2), Some(3)],
                ),
            ],
            4,
        )
        .unwrap();
        let environment = ExpressionEnvironment::new(0.0, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(
            evaluate_expression(&expression, environment).unwrap(),
            CanonicalExpressionValue::Float(0.25)
        );
    }

    #[test]
    fn progress_requires_a_piece_context() {
        let expression = CanonicalExpressionDag::new(
            vec![node(
                CanonicalExpressionOpcode::EnvP,
                CanonicalExpressionType::Float,
                [None; 3],
            )],
            0,
        )
        .unwrap();
        let environment = ExpressionEnvironment::new(0.0, 0.0, 0.0, 0.0).unwrap();
        assert!(matches!(
            evaluate_expression(&expression, environment),
            Err(ExpressionEvaluationError::EnvironmentUnavailable {
                environment: CanonicalExpressionEnvironment::P,
                ..
            })
        ));
    }

    #[test]
    fn builder_interns_identical_constant_nodes() {
        let mut builder = CanonicalExpressionBuilder::new();
        let first = builder
            .intern(constant(CanonicalExpressionValue::Float(1.0)))
            .unwrap();
        let second = builder
            .intern(constant(CanonicalExpressionValue::Float(1.0)))
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(builder.nodes().len(), 1);
    }

    #[test]
    fn vector_arithmetic_and_unit_extrema_are_evaluated_exactly() {
        let expression = CanonicalExpressionDag::new(
            vec![
                constant(CanonicalExpressionValue::Vec2(
                    Box::new(CanonicalExpressionValue::Float(1.0)),
                    Box::new(CanonicalExpressionValue::Float(2.0)),
                )),
                constant(CanonicalExpressionValue::Vec2(
                    Box::new(CanonicalExpressionValue::Float(3.0)),
                    Box::new(CanonicalExpressionValue::Float(4.0)),
                )),
                node(
                    CanonicalExpressionOpcode::Add,
                    CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Float)),
                    [Some(0), Some(1), None],
                ),
                constant(CanonicalExpressionValue::Float(2.0)),
                node(
                    CanonicalExpressionOpcode::Mul,
                    CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Float)),
                    [Some(2), Some(3), None],
                ),
            ],
            4,
        )
        .unwrap();
        let environment = ExpressionEnvironment::new(0.0, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(
            evaluate_expression(&expression, environment).unwrap(),
            CanonicalExpressionValue::Vec2(
                Box::new(CanonicalExpressionValue::Float(8.0)),
                Box::new(CanonicalExpressionValue::Float(12.0)),
            )
        );

        let extrema = CanonicalExpressionDag::new(
            vec![
                constant(CanonicalExpressionValue::Time(2.0)),
                constant(CanonicalExpressionValue::Time(1.0)),
                node(
                    CanonicalExpressionOpcode::Min,
                    CanonicalExpressionType::Time,
                    [Some(0), Some(1), None],
                ),
            ],
            2,
        )
        .unwrap();
        assert_eq!(
            evaluate_expression(&extrema, environment).unwrap(),
            CanonicalExpressionValue::Time(1.0)
        );
    }

    #[test]
    fn and_or_short_circuit_unavailable_piece_environment() {
        let and_expression = CanonicalExpressionDag::new(
            vec![
                constant(CanonicalExpressionValue::Bool(false)),
                node(
                    CanonicalExpressionOpcode::EnvP,
                    CanonicalExpressionType::Float,
                    [None; 3],
                ),
                constant(CanonicalExpressionValue::Float(1.0)),
                node(
                    CanonicalExpressionOpcode::Eq,
                    CanonicalExpressionType::Bool,
                    [Some(1), Some(2), None],
                ),
                node(
                    CanonicalExpressionOpcode::And,
                    CanonicalExpressionType::Bool,
                    [Some(0), Some(3), None],
                ),
            ],
            4,
        )
        .unwrap();
        let or_expression = CanonicalExpressionDag::new(
            vec![
                constant(CanonicalExpressionValue::Bool(true)),
                node(
                    CanonicalExpressionOpcode::EnvP,
                    CanonicalExpressionType::Float,
                    [None; 3],
                ),
                constant(CanonicalExpressionValue::Float(1.0)),
                node(
                    CanonicalExpressionOpcode::Eq,
                    CanonicalExpressionType::Bool,
                    [Some(1), Some(2), None],
                ),
                node(
                    CanonicalExpressionOpcode::Or,
                    CanonicalExpressionType::Bool,
                    [Some(0), Some(3), None],
                ),
            ],
            4,
        )
        .unwrap();
        let environment = ExpressionEnvironment::new(0.0, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(
            evaluate_expression(&and_expression, environment).unwrap(),
            CanonicalExpressionValue::Bool(false)
        );
        assert_eq!(
            evaluate_expression(&or_expression, environment).unwrap(),
            CanonicalExpressionValue::Bool(true)
        );
    }

    #[test]
    fn arithmetic_errors_and_signed_zero_are_structured() {
        let division = CanonicalExpressionDag::new(
            vec![
                constant(CanonicalExpressionValue::Float(1.0)),
                constant(CanonicalExpressionValue::Float(0.0)),
                node(
                    CanonicalExpressionOpcode::Div,
                    CanonicalExpressionType::Float,
                    [Some(0), Some(1), None],
                ),
            ],
            2,
        )
        .unwrap();
        let square_root = CanonicalExpressionDag::new(
            vec![
                constant(CanonicalExpressionValue::Float(-1.0)),
                node(
                    CanonicalExpressionOpcode::Sqrt,
                    CanonicalExpressionType::Float,
                    [Some(0), None, None],
                ),
            ],
            1,
        )
        .unwrap();
        let signed_zero = CanonicalExpressionDag::new(
            vec![
                constant(CanonicalExpressionValue::Float(0.0)),
                node(
                    CanonicalExpressionOpcode::Neg,
                    CanonicalExpressionType::Float,
                    [Some(0), None, None],
                ),
            ],
            1,
        )
        .unwrap();
        let environment = ExpressionEnvironment::new(0.0, 0.0, 0.0, 0.0).unwrap();
        assert!(matches!(
            evaluate_expression(&division, environment),
            Err(ExpressionEvaluationError::DivisionByZero { node: 2 })
        ));
        assert!(matches!(
            evaluate_expression(&square_root, environment),
            Err(ExpressionEvaluationError::Domain { node: 1 })
        ));
        let CanonicalExpressionValue::Float(value) =
            evaluate_expression(&signed_zero, environment).unwrap()
        else {
            panic!("expected float signed zero");
        };
        assert_eq!(value.to_bits(), (-0.0_f64).to_bits());
    }

    #[test]
    fn approx_eq_rejects_non_finite_intermediate_subtraction() {
        let expression = CanonicalExpressionDag::new(
            vec![
                constant(CanonicalExpressionValue::Float(1e308)),
                constant(CanonicalExpressionValue::Float(-1e308)),
                constant(CanonicalExpressionValue::Float(0.0)),
                node(
                    CanonicalExpressionOpcode::ApproxEq,
                    CanonicalExpressionType::Bool,
                    [Some(0), Some(1), Some(2)],
                ),
            ],
            3,
        )
        .unwrap();
        let environment = ExpressionEnvironment::new(0.0, 0.0, 0.0, 0.0).unwrap();
        assert!(matches!(
            evaluate_expression(&expression, environment),
            Err(ExpressionEvaluationError::NonFiniteResult { node: 3 })
        ));
    }
}
