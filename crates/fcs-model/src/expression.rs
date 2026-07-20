use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalExpressionType {
    Bool,
    Int,
    Float,
    Time,
    Beat,
    Length,
    Angle,
    Vec2(Box<Self>),
}

impl CanonicalExpressionType {
    pub fn vector(element: Self) -> Option<Self> {
        matches!(
            &element,
            Self::Int | Self::Float | Self::Time | Self::Beat | Self::Length | Self::Angle
        )
        .then_some(Self::Vec2(Box::new(element)))
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::Int | Self::Float | Self::Time | Self::Beat | Self::Length | Self::Angle
        )
    }

    pub fn is_vector(&self) -> bool {
        matches!(self, Self::Vec2(_))
    }

    fn is_scalar(&self) -> bool {
        matches!(self, Self::Int | Self::Float)
    }

    fn is_unit(&self) -> bool {
        matches!(self, Self::Time | Self::Beat | Self::Length | Self::Angle)
    }

    fn vector_scalar_result(&self, scalar: &Self) -> Option<Self> {
        let Self::Vec2(element) = self else {
            return None;
        };
        if !scalar.is_scalar() {
            return None;
        }
        let result_element = if element.is_unit() || **element == Self::Float {
            (**element).clone()
        } else if **element == Self::Int && *scalar == Self::Float {
            Self::Float
        } else if **element == Self::Int && *scalar == Self::Int {
            Self::Int
        } else {
            return None;
        };
        Some(Self::Vec2(Box::new(result_element)))
    }

    fn is_valid(&self) -> bool {
        match self {
            Self::Vec2(element) => element.is_numeric(),
            _ => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalExpressionValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Time(f64),
    Beat(f64),
    Length(f64),
    Angle(f64),
    Vec2(Box<Self>, Box<Self>),
}

impl CanonicalExpressionValue {
    pub fn value_type(&self) -> CanonicalExpressionType {
        match self {
            Self::Bool(_) => CanonicalExpressionType::Bool,
            Self::Int(_) => CanonicalExpressionType::Int,
            Self::Float(_) => CanonicalExpressionType::Float,
            Self::Time(_) => CanonicalExpressionType::Time,
            Self::Beat(_) => CanonicalExpressionType::Beat,
            Self::Length(_) => CanonicalExpressionType::Length,
            Self::Angle(_) => CanonicalExpressionType::Angle,
            Self::Vec2(x, y) => {
                let x_type = x.value_type();
                if x_type == y.value_type() {
                    CanonicalExpressionType::Vec2(Box::new(x_type))
                } else {
                    CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Float))
                }
            }
        }
    }

    pub fn is_finite(&self) -> bool {
        match self {
            Self::Bool(_) | Self::Int(_) => true,
            Self::Float(value)
            | Self::Time(value)
            | Self::Beat(value)
            | Self::Length(value)
            | Self::Angle(value) => value.is_finite(),
            Self::Vec2(x, y) => x.is_finite() && y.is_finite(),
        }
    }

    fn is_valid(&self) -> bool {
        match self {
            Self::Vec2(x, y) => {
                x.is_valid()
                    && y.is_valid()
                    && x.value_type() == y.value_type()
                    && x.value_type().is_numeric()
            }
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalExpressionEnvironment {
    S,
    B,
    Q,
    D,
    P,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalExpressionOpcode {
    Constant,
    EnvS,
    EnvB,
    EnvQ,
    EnvD,
    EnvP,
    Neg,
    Not,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    ApproxEq,
    Abs,
    Min,
    Max,
    Clamp,
    Floor,
    Ceil,
    Round,
    Sqrt,
    Exp,
    Ln,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Easing,
    ToFloat,
    Seconds,
    Radians,
    Choose,
    Vec2,
    Vec2X,
    Vec2Y,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalExpressionNode {
    opcode: CanonicalExpressionOpcode,
    result_type: CanonicalExpressionType,
    operands: [Option<usize>; 3],
    constant: Option<CanonicalExpressionValue>,
    immediate: u32,
}

impl CanonicalExpressionNode {
    pub fn new(
        opcode: CanonicalExpressionOpcode,
        result_type: CanonicalExpressionType,
        operands: [Option<usize>; 3],
        constant: Option<CanonicalExpressionValue>,
        immediate: u32,
    ) -> Self {
        Self {
            opcode,
            result_type,
            operands,
            constant,
            immediate,
        }
    }

    pub fn opcode(&self) -> CanonicalExpressionOpcode {
        self.opcode
    }

    pub fn result_type(&self) -> &CanonicalExpressionType {
        &self.result_type
    }

    pub fn operands(&self) -> &[Option<usize>; 3] {
        &self.operands
    }

    pub fn constant(&self) -> Option<&CanonicalExpressionValue> {
        self.constant.as_ref()
    }

    pub const fn immediate(&self) -> u32 {
        self.immediate
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalExpressionDag {
    nodes: Vec<CanonicalExpressionNode>,
    root: usize,
}

impl CanonicalExpressionDag {
    pub fn new(
        nodes: Vec<CanonicalExpressionNode>,
        root: usize,
    ) -> Result<Self, CanonicalExpressionError> {
        if nodes.is_empty() {
            return Err(CanonicalExpressionError::EmptyGraph);
        }
        if root >= nodes.len() {
            return Err(CanonicalExpressionError::RootOutOfBounds);
        }
        let graph = Self { nodes, root };
        graph.validate()?;
        Ok(graph)
    }

    pub fn nodes(&self) -> &[CanonicalExpressionNode] {
        &self.nodes
    }

    pub const fn root(&self) -> usize {
        self.root
    }

    pub fn result_type(&self) -> &CanonicalExpressionType {
        self.nodes[self.root].result_type()
    }

    pub fn required_environment(&self) -> Vec<CanonicalExpressionEnvironment> {
        let mut values = Vec::new();
        for node in &self.nodes {
            let environment = match node.opcode {
                CanonicalExpressionOpcode::EnvS => Some(CanonicalExpressionEnvironment::S),
                CanonicalExpressionOpcode::EnvB => Some(CanonicalExpressionEnvironment::B),
                CanonicalExpressionOpcode::EnvQ => Some(CanonicalExpressionEnvironment::Q),
                CanonicalExpressionOpcode::EnvD => Some(CanonicalExpressionEnvironment::D),
                CanonicalExpressionOpcode::EnvP => Some(CanonicalExpressionEnvironment::P),
                _ => None,
            };
            if let Some(environment) = environment
                && !values.contains(&environment)
            {
                values.push(environment);
            }
        }
        values
    }

    fn validate(&self) -> Result<(), CanonicalExpressionError> {
        for (index, node) in self.nodes.iter().enumerate() {
            for operand in node.operands.iter().flatten() {
                if *operand >= index {
                    return Err(CanonicalExpressionError::NonTopologicalOperand {
                        node: index,
                        operand: *operand,
                    });
                }
            }
            validate_node(index, node, &self.nodes)?;
        }
        let mut reachable = vec![false; self.nodes.len()];
        mark_reachable(self.root, &self.nodes, &mut reachable);
        if let Some(node) = reachable.iter().position(|reachable| !reachable) {
            return Err(CanonicalExpressionError::UnreachableNode { node });
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct CanonicalExpressionBuilder {
    nodes: Vec<CanonicalExpressionNode>,
}

impl CanonicalExpressionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(
        &mut self,
        node: CanonicalExpressionNode,
    ) -> Result<usize, CanonicalExpressionError> {
        let index = self.nodes.len();
        for operand in node.operands.iter().flatten() {
            if *operand >= index {
                return Err(CanonicalExpressionError::NonTopologicalOperand {
                    node: index,
                    operand: *operand,
                });
            }
        }
        validate_node(index, &node, &self.nodes)?;
        if let Some(existing) = self
            .nodes
            .iter()
            .enumerate()
            .find_map(|(existing, candidate)| {
                structural_node_equal(existing, candidate, &node, &self.nodes).then_some(existing)
            })
        {
            return Ok(existing);
        }
        self.nodes.push(node);
        Ok(index)
    }

    pub fn finish(self, root: usize) -> Result<CanonicalExpressionDag, CanonicalExpressionError> {
        CanonicalExpressionDag::new(self.nodes, root)
    }

    pub fn nodes(&self) -> &[CanonicalExpressionNode] {
        &self.nodes
    }
}

fn structural_node_equal(
    left_index: usize,
    left: &CanonicalExpressionNode,
    right: &CanonicalExpressionNode,
    nodes: &[CanonicalExpressionNode],
) -> bool {
    if left.opcode != right.opcode
        || left.result_type != right.result_type
        || left.immediate != right.immediate
        || !expression_value_equal(left.constant.as_ref(), right.constant.as_ref())
    {
        return false;
    }
    left.operands
        .iter()
        .zip(right.operands.iter())
        .all(|(left, right)| match (left, right) {
            (None, None) => true,
            (Some(left), Some(right)) => {
                *left < nodes.len()
                    && *right < nodes.len()
                    && structural_node_equal(*left, &nodes[*left], &nodes[*right], nodes)
            }
            _ => false,
        })
        && left_index < nodes.len()
}

fn expression_value_equal(
    left: Option<&CanonicalExpressionValue>,
    right: Option<&CanonicalExpressionValue>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => match (left, right) {
            (CanonicalExpressionValue::Bool(left), CanonicalExpressionValue::Bool(right)) => {
                left == right
            }
            (CanonicalExpressionValue::Int(left), CanonicalExpressionValue::Int(right)) => {
                left == right
            }
            (left, right) => match (expression_float(left), expression_float(right)) {
                (Some(left), Some(right)) => left.to_bits() == right.to_bits(),
                _ => match (left, right) {
                    (
                        CanonicalExpressionValue::Vec2(left_x, left_y),
                        CanonicalExpressionValue::Vec2(right_x, right_y),
                    ) => {
                        expression_value_equal(Some(left_x), Some(right_x))
                            && expression_value_equal(Some(left_y), Some(right_y))
                    }
                    _ => false,
                },
            },
        },
        _ => false,
    }
}

fn expression_float(value: &CanonicalExpressionValue) -> Option<f64> {
    match value {
        CanonicalExpressionValue::Float(value)
        | CanonicalExpressionValue::Time(value)
        | CanonicalExpressionValue::Beat(value)
        | CanonicalExpressionValue::Length(value)
        | CanonicalExpressionValue::Angle(value) => Some(*value),
        _ => None,
    }
}

fn mark_reachable(index: usize, nodes: &[CanonicalExpressionNode], reachable: &mut [bool]) {
    if reachable[index] {
        return;
    }
    reachable[index] = true;
    for operand in nodes[index].operands.iter().flatten() {
        mark_reachable(*operand, nodes, reachable);
    }
}

fn validate_node(
    index: usize,
    node: &CanonicalExpressionNode,
    nodes: &[CanonicalExpressionNode],
) -> Result<(), CanonicalExpressionError> {
    let arity = node
        .operands
        .iter()
        .filter(|operand| operand.is_some())
        .count();
    let expected_arity = match node.opcode {
        CanonicalExpressionOpcode::Constant
        | CanonicalExpressionOpcode::EnvS
        | CanonicalExpressionOpcode::EnvB
        | CanonicalExpressionOpcode::EnvQ
        | CanonicalExpressionOpcode::EnvD
        | CanonicalExpressionOpcode::EnvP => 0,
        CanonicalExpressionOpcode::Neg
        | CanonicalExpressionOpcode::Not
        | CanonicalExpressionOpcode::Abs
        | CanonicalExpressionOpcode::Floor
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
        | CanonicalExpressionOpcode::Atan
        | CanonicalExpressionOpcode::Easing
        | CanonicalExpressionOpcode::ToFloat
        | CanonicalExpressionOpcode::Seconds
        | CanonicalExpressionOpcode::Radians
        | CanonicalExpressionOpcode::Vec2X
        | CanonicalExpressionOpcode::Vec2Y => 1,
        CanonicalExpressionOpcode::Add
        | CanonicalExpressionOpcode::Sub
        | CanonicalExpressionOpcode::Mul
        | CanonicalExpressionOpcode::Div
        | CanonicalExpressionOpcode::Mod
        | CanonicalExpressionOpcode::Pow
        | CanonicalExpressionOpcode::Eq
        | CanonicalExpressionOpcode::Ne
        | CanonicalExpressionOpcode::Lt
        | CanonicalExpressionOpcode::Le
        | CanonicalExpressionOpcode::Gt
        | CanonicalExpressionOpcode::Ge
        | CanonicalExpressionOpcode::And
        | CanonicalExpressionOpcode::Or
        | CanonicalExpressionOpcode::Min
        | CanonicalExpressionOpcode::Max
        | CanonicalExpressionOpcode::Atan2
        | CanonicalExpressionOpcode::Vec2 => 2,
        CanonicalExpressionOpcode::ApproxEq
        | CanonicalExpressionOpcode::Clamp
        | CanonicalExpressionOpcode::Choose => 3,
    };
    if arity != expected_arity {
        return Err(CanonicalExpressionError::Arity {
            node: index,
            expected: expected_arity,
            actual: arity,
        });
    }
    if !node.result_type.is_valid() {
        return Err(CanonicalExpressionError::OperandType { node: index });
    }
    if node.opcode == CanonicalExpressionOpcode::Constant {
        let Some(constant) = node.constant.as_ref() else {
            return Err(CanonicalExpressionError::MissingConstant { node: index });
        };
        if constant.value_type() != node.result_type
            || !constant.is_valid()
            || !constant.is_finite()
        {
            return Err(CanonicalExpressionError::ConstantType { node: index });
        }
    } else if node.constant.is_some() {
        return Err(CanonicalExpressionError::UnexpectedConstant { node: index });
    }
    if node.opcode != CanonicalExpressionOpcode::Easing && node.immediate != 0 {
        return Err(CanonicalExpressionError::UnexpectedImmediate { node: index });
    }
    if node.opcode == CanonicalExpressionOpcode::Easing && node.immediate > 30 {
        return Err(CanonicalExpressionError::EasingId { node: index });
    }
    let operand_type = |slot: usize| {
        node.operands[slot]
            .and_then(|operand| nodes.get(operand))
            .map(|operand| &operand.result_type)
            .ok_or(CanonicalExpressionError::OperandType { node: index })
    };
    let same = |left: usize, right: usize| {
        operand_type(left)
            .and_then(|left_type| operand_type(right).map(|right_type| left_type == right_type))
    };
    let scalar_numeric = |slot: usize| operand_type(slot).map(CanonicalExpressionType::is_numeric);
    let valid_vector =
        |slot: usize| operand_type(slot).map(|operand| operand.is_valid() && operand.is_vector());
    let valid = match node.opcode {
        CanonicalExpressionOpcode::Constant => true,
        CanonicalExpressionOpcode::EnvS => node.result_type == CanonicalExpressionType::Time,
        CanonicalExpressionOpcode::EnvB => node.result_type == CanonicalExpressionType::Beat,
        CanonicalExpressionOpcode::EnvQ | CanonicalExpressionOpcode::EnvP => {
            node.result_type == CanonicalExpressionType::Float
        }
        CanonicalExpressionOpcode::EnvD => node.result_type == CanonicalExpressionType::Length,
        CanonicalExpressionOpcode::Neg | CanonicalExpressionOpcode::Abs => {
            scalar_numeric(0)? && operand_type(0)? == &node.result_type
        }
        CanonicalExpressionOpcode::Not => {
            operand_type(0)? == &CanonicalExpressionType::Bool
                && node.result_type == CanonicalExpressionType::Bool
        }
        CanonicalExpressionOpcode::Add | CanonicalExpressionOpcode::Sub => {
            same(0, 1)?
                && (scalar_numeric(0)? || (valid_vector(0)? && node.result_type.is_valid()))
                && operand_type(0)? == &node.result_type
        }
        CanonicalExpressionOpcode::Mul => {
            let left = operand_type(0)?;
            let right = operand_type(1)?;
            if left == right && left.is_scalar() {
                node.result_type == *left
            } else if let Some(expected) = left.vector_scalar_result(right) {
                node.result_type == expected
            } else if let Some(expected) = right.vector_scalar_result(left) {
                node.result_type == expected
            } else if left.is_unit() && right.is_scalar() {
                node.result_type == *left
            } else if right.is_unit() && left.is_scalar() {
                node.result_type == *right
            } else {
                false
            }
        }
        CanonicalExpressionOpcode::Div => {
            let left = operand_type(0)?;
            let right = operand_type(1)?;
            if left == right && left.is_unit() {
                node.result_type == CanonicalExpressionType::Float
            } else if let Some(expected) = left.vector_scalar_result(right) {
                node.result_type == expected
            } else if left.is_unit() && right.is_scalar() {
                node.result_type == *left
            } else {
                left == right && left.is_scalar() && node.result_type == *left
            }
        }
        CanonicalExpressionOpcode::Mod => {
            same(0, 1)?
                && operand_type(0)? == &CanonicalExpressionType::Int
                && node.result_type == CanonicalExpressionType::Int
        }
        CanonicalExpressionOpcode::Pow => {
            same(0, 1)? && operand_type(0)? == &node.result_type && operand_type(0)?.is_scalar()
        }
        CanonicalExpressionOpcode::Eq | CanonicalExpressionOpcode::Ne => {
            same(0, 1)?
                && operand_type(0)?.is_valid()
                && node.result_type == CanonicalExpressionType::Bool
        }
        CanonicalExpressionOpcode::Lt
        | CanonicalExpressionOpcode::Le
        | CanonicalExpressionOpcode::Gt
        | CanonicalExpressionOpcode::Ge => {
            same(0, 1)? && scalar_numeric(0)? && node.result_type == CanonicalExpressionType::Bool
        }
        CanonicalExpressionOpcode::And | CanonicalExpressionOpcode::Or => {
            operand_type(0)? == &CanonicalExpressionType::Bool
                && operand_type(1)? == &CanonicalExpressionType::Bool
                && node.result_type == CanonicalExpressionType::Bool
        }
        CanonicalExpressionOpcode::ApproxEq => {
            operand_type(0)? == &CanonicalExpressionType::Float
                && operand_type(1)? == &CanonicalExpressionType::Float
                && operand_type(2)? == &CanonicalExpressionType::Float
                && node.result_type == CanonicalExpressionType::Bool
        }
        CanonicalExpressionOpcode::Min | CanonicalExpressionOpcode::Max => {
            same(0, 1)? && scalar_numeric(0)? && operand_type(0)? == &node.result_type
        }
        CanonicalExpressionOpcode::Clamp => {
            same(0, 1)?
                && same(0, 2)?
                && scalar_numeric(0)?
                && operand_type(0)? == &node.result_type
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
        | CanonicalExpressionOpcode::Atan
        | CanonicalExpressionOpcode::Easing => {
            operand_type(0)? == &CanonicalExpressionType::Float
                && node.result_type == CanonicalExpressionType::Float
        }
        CanonicalExpressionOpcode::Atan2 => {
            operand_type(0)? == &CanonicalExpressionType::Float
                && operand_type(1)? == &CanonicalExpressionType::Float
                && node.result_type == CanonicalExpressionType::Float
        }
        CanonicalExpressionOpcode::ToFloat => {
            operand_type(0)? == &CanonicalExpressionType::Int
                && node.result_type == CanonicalExpressionType::Float
        }
        CanonicalExpressionOpcode::Seconds => {
            operand_type(0)? == &CanonicalExpressionType::Time
                && node.result_type == CanonicalExpressionType::Float
        }
        CanonicalExpressionOpcode::Radians => {
            operand_type(0)? == &CanonicalExpressionType::Angle
                && node.result_type == CanonicalExpressionType::Float
        }
        CanonicalExpressionOpcode::Choose => {
            operand_type(0)? == &CanonicalExpressionType::Bool
                && operand_type(1)? == &node.result_type
                && operand_type(2)? == &node.result_type
        }
        CanonicalExpressionOpcode::Vec2 => {
            same(0, 1)?
                && scalar_numeric(0)?
                && node.result_type
                    == CanonicalExpressionType::Vec2(Box::new(operand_type(0)?.clone()))
        }
        CanonicalExpressionOpcode::Vec2X | CanonicalExpressionOpcode::Vec2Y => {
            let operand = operand_type(0)?;
            let CanonicalExpressionType::Vec2(element) = operand else {
                return Err(CanonicalExpressionError::OperandType { node: index });
            };
            element.is_numeric() && node.result_type == **element
        }
    };
    if valid {
        Ok(())
    } else {
        Err(CanonicalExpressionError::OperandType { node: index })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalExpressionError {
    EmptyGraph,
    RootOutOfBounds,
    NonTopologicalOperand {
        node: usize,
        operand: usize,
    },
    UnreachableNode {
        node: usize,
    },
    Arity {
        node: usize,
        expected: usize,
        actual: usize,
    },
    MissingConstant {
        node: usize,
    },
    ConstantType {
        node: usize,
    },
    UnexpectedConstant {
        node: usize,
    },
    UnexpectedImmediate {
        node: usize,
    },
    EasingId {
        node: usize,
    },
    OperandType {
        node: usize,
    },
}

impl fmt::Display for CanonicalExpressionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyGraph => formatter.write_str("expression graph must not be empty"),
            Self::RootOutOfBounds => formatter.write_str("expression root is out of bounds"),
            Self::NonTopologicalOperand { node, operand } => {
                write!(
                    formatter,
                    "expression node {node} references non-topological node {operand}"
                )
            }
            Self::UnreachableNode { node } => {
                write!(formatter, "expression node {node} is unreachable")
            }
            Self::Arity {
                node,
                expected,
                actual,
            } => {
                write!(
                    formatter,
                    "expression node {node} has arity {actual}, expected {expected}"
                )
            }
            Self::MissingConstant { node } => {
                write!(formatter, "expression constant node {node} has no value")
            }
            Self::ConstantType { node } => write!(
                formatter,
                "expression constant node {node} has an invalid value"
            ),
            Self::UnexpectedConstant { node } => write!(
                formatter,
                "expression node {node} has an unexpected constant"
            ),
            Self::UnexpectedImmediate { node } => write!(
                formatter,
                "expression node {node} has an unexpected immediate"
            ),
            Self::EasingId { node } => {
                write!(formatter, "expression node {node} has an unknown easing ID")
            }
            Self::OperandType { node } => write!(
                formatter,
                "expression node {node} has an invalid operand type"
            ),
        }
    }
}

impl std::error::Error for CanonicalExpressionError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(
        opcode: CanonicalExpressionOpcode,
        result_type: CanonicalExpressionType,
        operands: [Option<usize>; 3],
    ) -> CanonicalExpressionNode {
        CanonicalExpressionNode::new(opcode, result_type, operands, None, 0)
    }

    fn constant(value: CanonicalExpressionValue) -> CanonicalExpressionNode {
        CanonicalExpressionNode::new(
            CanonicalExpressionOpcode::Constant,
            value.value_type(),
            [None; 3],
            Some(value),
            0,
        )
    }

    #[test]
    fn validation_rejects_non_topological_and_unreachable_nodes() {
        let non_topological = CanonicalExpressionDag::new(
            vec![node(
                CanonicalExpressionOpcode::Neg,
                CanonicalExpressionType::Float,
                [Some(1), None, None],
            )],
            0,
        )
        .expect_err("forward operand must be rejected");
        assert!(matches!(
            non_topological,
            CanonicalExpressionError::NonTopologicalOperand { .. }
        ));

        let unreachable = CanonicalExpressionDag::new(
            vec![
                constant(CanonicalExpressionValue::Float(1.0)),
                constant(CanonicalExpressionValue::Float(2.0)),
            ],
            0,
        )
        .expect_err("unreachable node must be rejected");
        assert!(matches!(
            unreachable,
            CanonicalExpressionError::UnreachableNode { node: 1 }
        ));
    }

    #[test]
    fn validation_rejects_malformed_types_and_vectors() {
        let invalid_add = CanonicalExpressionDag::new(
            vec![
                constant(CanonicalExpressionValue::Bool(true)),
                constant(CanonicalExpressionValue::Bool(false)),
                node(
                    CanonicalExpressionOpcode::Add,
                    CanonicalExpressionType::Bool,
                    [Some(0), Some(1), None],
                ),
            ],
            2,
        )
        .expect_err("boolean addition must be rejected");
        assert!(matches!(
            invalid_add,
            CanonicalExpressionError::OperandType { node: 2 }
        ));

        let invalid_vector = CanonicalExpressionDag::new(
            vec![CanonicalExpressionNode::new(
                CanonicalExpressionOpcode::Constant,
                CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Float)),
                [None; 3],
                Some(CanonicalExpressionValue::Vec2(
                    Box::new(CanonicalExpressionValue::Int(1)),
                    Box::new(CanonicalExpressionValue::Float(2.0)),
                )),
                0,
            )],
            0,
        )
        .expect_err("mixed vector elements must be rejected");
        assert!(matches!(
            invalid_vector,
            CanonicalExpressionError::ConstantType { node: 0 }
        ));
    }

    #[test]
    fn validation_accepts_vector_arithmetic_and_projects_environments() {
        let graph = CanonicalExpressionDag::new(
            vec![
                node(
                    CanonicalExpressionOpcode::EnvD,
                    CanonicalExpressionType::Length,
                    [None; 3],
                ),
                constant(CanonicalExpressionValue::Length(1.0)),
                node(
                    CanonicalExpressionOpcode::Vec2,
                    CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Length)),
                    [Some(0), Some(1), None],
                ),
                constant(CanonicalExpressionValue::Float(2.0)),
                node(
                    CanonicalExpressionOpcode::Mul,
                    CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Length)),
                    [Some(2), Some(3), None],
                ),
            ],
            4,
        )
        .expect("valid vector graph");
        assert_eq!(
            graph.required_environment(),
            vec![CanonicalExpressionEnvironment::D]
        );
    }
}
