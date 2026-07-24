//! Canonical lowering for bounded Core runtime expressions.

use fcs_model::{
    Beat as CanonicalBeat, CanonicalDescriptorDomain, CanonicalDescriptorKind,
    CanonicalDescriptorRoot, CanonicalDescriptorTable, CanonicalExpressionBuilder,
    CanonicalExpressionDag, CanonicalExpressionEnvironment, CanonicalExpressionNode,
    CanonicalExpressionOpcode, CanonicalExpressionType, CanonicalExpressionValue,
    CanonicalPropertyDescriptor,
};

use crate::ast::{
    BinaryOperator, CollectionItem, Document, EntityExpression, ExpandedSourceDocument,
    SourceExpression, SourceLiteral, SourceSpan, Type, TypedValue, UnaryOperator,
};
use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage};

/// Lowers one source runtime expression into the source-free canonical DAG boundary.
pub fn lower_runtime_expression(
    expression: &SourceExpression,
) -> Result<CanonicalExpressionDag, Diagnostic> {
    lower_runtime_expression_with_resolver(expression, |_| None)
}

pub(crate) fn lower_runtime_expression_with_resolver(
    expression: &SourceExpression,
    mut resolve: impl FnMut(&SourceExpression) -> Option<TypedValue>,
) -> Result<CanonicalExpressionDag, Diagnostic> {
    let mut lowerer = Lowerer {
        builder: CanonicalExpressionBuilder::new(),
        allow_progress: false,
        resolve: &mut resolve,
    };
    let root = lowerer.lower(expression)?;
    lowerer.builder.finish(root.index).map_err(|error| {
        Diagnostic::new(
            DiagnosticCode::TYPE_INVALID_OPERATION,
            DiagnosticStage::Canonical,
            error.to_string(),
            expression.span(),
        )
    })
}

impl Document {
    /// Lowers a directly-authored entity field without retaining source AST in the result.
    ///
    /// This bounded entry point is intentionally limited to a collection item and field path;
    /// template/generator expansion and Piece context are owned by later stages.
    pub fn canonical_runtime_expression(
        &self,
        collection_name: &str,
        entity_index: usize,
        field_path: &str,
    ) -> Result<CanonicalExpressionDag, Diagnostic> {
        let collection = self
            .collections
            .iter()
            .find(|collection| collection.collection_name == collection_name)
            .ok_or_else(|| {
                Diagnostic::new(
                    DiagnosticCode::NAME_UNKNOWN,
                    DiagnosticStage::Canonical,
                    format!("unknown collection {collection_name}"),
                    self.format.span,
                )
            })?;
        let item = collection.items.get(entity_index).ok_or_else(|| {
            Diagnostic::new(
                DiagnosticCode::NAME_UNKNOWN,
                DiagnosticStage::Canonical,
                format!("entity index {entity_index} is out of bounds"),
                collection.span,
            )
        })?;
        let constructor = match item {
            CollectionItem::Constructor(constructor) => constructor,
            CollectionItem::Expression(EntityExpression::Constructor(constructor)) => constructor,
            _ => {
                return Err(Diagnostic::new(
                    DiagnosticCode::TYPE_INVALID_OPERATION,
                    DiagnosticStage::Canonical,
                    "runtime expression fixture requires a direct entity constructor",
                    item.span(),
                ));
            }
        };
        let field = constructor
            .fields
            .iter()
            .find(|field| field.path.segments.join(".") == field_path)
            .ok_or_else(|| {
                Diagnostic::new(
                    DiagnosticCode::SCHEMA_UNKNOWN_FIELD,
                    DiagnosticStage::Canonical,
                    format!("entity has no field {field_path}"),
                    constructor.span,
                )
            })?;
        lower_runtime_expression(&field.value)
    }
}

impl ExpandedSourceDocument {
    pub(crate) fn canonical_runtime_descriptors(
        &self,
    ) -> Result<Option<CanonicalDescriptorTable>, Vec<Diagnostic>> {
        let ids = self
            .canonical_note_ids_with_spans()
            .map_err(|(error, span)| {
                vec![Diagnostic::new(
                    DiagnosticCode::NAME_DUPLICATE,
                    DiagnosticStage::Canonical,
                    error.to_string(),
                    span,
                )]
            })?;
        let mut ids = ids.into_iter();
        let mut descriptors = Vec::new();
        let mut roots = Vec::new();
        let mut first_span = None;

        for entity in self
            .collections()
            .flat_map(|collection| collection.entities())
            .filter(|entity| entity.entity_type() == &Type::Note)
        {
            let (id, _) = ids
                .next()
                .expect("canonical Note IDs and expanded Notes have equal cardinality");
            for field in entity.fields() {
                let Some(expression) = field.runtime_expression() else {
                    continue;
                };
                let Some(target_path) = note_runtime_target_path(field.path()) else {
                    return Err(vec![Diagnostic::new(
                        DiagnosticCode::SCHEMA_DYNAMIC_FIELD_FORBIDDEN,
                        DiagnosticStage::Canonical,
                        format!(
                            "field {} cannot depend on a runtime expression",
                            field.path()
                        ),
                        field.span(),
                    )]);
                };
                if field.path() == "presentation.scrollFactor"
                    && expression
                        .required_environment()
                        .contains(&CanonicalExpressionEnvironment::D)
                {
                    return Err(vec![Diagnostic::new(
                        DiagnosticCode::EXPRESSION_ENVIRONMENT_UNAVAILABLE,
                        DiagnosticStage::Canonical,
                        "presentation.scrollFactor cannot depend on d",
                        field.span(),
                    )]);
                }
                first_span.get_or_insert(field.span());
                let descriptor = CanonicalPropertyDescriptor::new(
                    expression.result_type().clone(),
                    CanonicalDescriptorDomain::new(None, None, false)
                        .expect("the unbounded runtime property domain is valid"),
                    CanonicalDescriptorKind::Expression(expression.clone()),
                )
                .map_err(|error| {
                    vec![Diagnostic::new(
                        DiagnosticCode::TYPE_INVALID_OPERATION,
                        DiagnosticStage::Canonical,
                        error.to_string(),
                        field.span(),
                    )]
                })?;
                let descriptor_index = descriptors.len();
                descriptors.push(descriptor);
                roots.push(
                    CanonicalDescriptorRoot::new(target_path, id.value(), descriptor_index)
                        .map_err(|error| {
                            vec![Diagnostic::new(
                                DiagnosticCode::TYPE_INVALID_OPERATION,
                                DiagnosticStage::Canonical,
                                error.to_string(),
                                field.span(),
                            )]
                        })?,
                );
            }
        }

        if descriptors.is_empty() {
            return Ok(None);
        }
        CanonicalDescriptorTable::new(descriptors, roots)
            .map(Some)
            .map_err(|error| {
                vec![Diagnostic::new(
                    DiagnosticCode::TYPE_INVALID_OPERATION,
                    DiagnosticStage::Canonical,
                    error.to_string(),
                    first_span.expect("a nonempty descriptor table has a source span"),
                )]
            })
    }
}

fn note_runtime_target_path(path: &str) -> Option<&'static str> {
    match path {
        "presentation.positionX" => Some("note.presentation.positionX"),
        "presentation.scrollFactor" => Some("note.presentation.scrollFactor"),
        "presentation.xOffset" => Some("note.presentation.xOffset"),
        "presentation.yOffset" => Some("note.presentation.yOffset"),
        "presentation.alpha" => Some("note.presentation.alpha"),
        "presentation.scaleX" => Some("note.presentation.scaleX"),
        "presentation.scaleY" => Some("note.presentation.scaleY"),
        "presentation.rotation" => Some("note.presentation.rotation"),
        "presentation.color" => Some("note.presentation.color"),
        _ => None,
    }
}

#[derive(Debug, Clone)]
struct LoweredNode {
    index: usize,
    ty: CanonicalExpressionType,
}

struct Lowerer<'a, R> {
    builder: CanonicalExpressionBuilder,
    allow_progress: bool,
    resolve: &'a mut R,
}

impl<R> Lowerer<'_, R>
where
    R: FnMut(&SourceExpression) -> Option<TypedValue>,
{
    fn lower(&mut self, expression: &SourceExpression) -> Result<LoweredNode, Diagnostic> {
        match expression {
            SourceExpression::Literal { literal, span } => self.lower_literal(literal, *span),
            SourceExpression::Name { name, span } => self.lower_name(expression, name, *span),
            SourceExpression::Unary {
                operator,
                operand,
                span,
            } => {
                let operand = self.lower(operand)?;
                let (opcode, result_type) = match (operator, &operand.ty) {
                    (UnaryOperator::Negate, ty) if ty.is_numeric() => {
                        (CanonicalExpressionOpcode::Neg, ty.clone())
                    }
                    (UnaryOperator::Not, CanonicalExpressionType::Bool) => (
                        CanonicalExpressionOpcode::Not,
                        CanonicalExpressionType::Bool,
                    ),
                    _ => {
                        return Err(self.error(
                            DiagnosticCode::TYPE_MISMATCH,
                            "invalid runtime unary operand",
                            *span,
                        ));
                    }
                };
                self.insert(
                    opcode,
                    result_type,
                    [Some(operand.index), None, None],
                    *span,
                )
            }
            SourceExpression::Binary {
                left,
                operator,
                right,
                span,
            } => {
                let left = self.lower(left)?;
                let right = self.lower(right)?;
                let result_type =
                    binary_result_type(*operator, &left.ty, &right.ty).ok_or_else(|| {
                        self.error(
                            DiagnosticCode::TYPE_MISMATCH,
                            "invalid runtime binary operands",
                            *span,
                        )
                    })?;
                self.insert(
                    binary_opcode(*operator),
                    result_type,
                    [Some(left.index), Some(right.index), None],
                    *span,
                )
            }
            SourceExpression::Call {
                callee,
                arguments,
                span,
            } => self.lower_call(callee, arguments, *span),
            SourceExpression::Choose {
                arms,
                else_value,
                span,
            } => self.lower_choose(arms, else_value, *span),
            SourceExpression::Vec2 { x, y, span } => {
                let x = self.lower(x)?;
                let y = self.lower(y)?;
                if x.ty != y.ty || !x.ty.is_numeric() {
                    return Err(self.error(
                        DiagnosticCode::TYPE_MISMATCH,
                        "vec2 runtime components must have one numeric type",
                        *span,
                    ));
                }
                self.insert(
                    CanonicalExpressionOpcode::Vec2,
                    CanonicalExpressionType::Vec2(Box::new(x.ty.clone())),
                    [Some(x.index), Some(y.index), None],
                    *span,
                )
            }
            SourceExpression::Reference { span, .. }
            | SourceExpression::Array { span, .. }
            | SourceExpression::Object { span, .. }
            | SourceExpression::FieldAccess { span, .. }
            | SourceExpression::Index { span, .. } => Err(self.error(
                DiagnosticCode::TYPE_INVALID_OPERATION,
                "runtime expressions may only contain scalar/vector Core nodes",
                *span,
            )),
        }
    }

    fn lower_name(
        &mut self,
        expression: &SourceExpression,
        name: &str,
        span: SourceSpan,
    ) -> Result<LoweredNode, Diagnostic> {
        match (self.resolve)(expression) {
            Some(value) => self.lower_typed_value(&value, span),
            None => self.lower_environment(name, span),
        }
    }

    fn lower_typed_value(
        &mut self,
        value: &TypedValue,
        span: SourceSpan,
    ) -> Result<LoweredNode, Diagnostic> {
        let value = canonical_value(value).ok_or_else(|| {
            self.error(
                DiagnosticCode::TYPE_INVALID_OPERATION,
                "compile-time binding is not a Core runtime expression value",
                span,
            )
        })?;
        let result_type = value.value_type();
        self.insert_with_constant(
            CanonicalExpressionOpcode::Constant,
            result_type,
            [None, None, None],
            Some(value),
            span,
        )
    }

    fn lower_literal(
        &mut self,
        literal: &SourceLiteral,
        span: SourceSpan,
    ) -> Result<LoweredNode, Diagnostic> {
        let (value, result_type) = match literal {
            SourceLiteral::Bool(value) => (
                CanonicalExpressionValue::Bool(*value),
                CanonicalExpressionType::Bool,
            ),
            SourceLiteral::Int(value) => (
                CanonicalExpressionValue::Int(*value),
                CanonicalExpressionType::Int,
            ),
            SourceLiteral::IntMagnitude(value) => {
                let value = value.parse::<i64>().map_err(|_| {
                    self.error(
                        DiagnosticCode::NUMERIC_OVERFLOW,
                        "runtime integer literal exceeds i64",
                        span,
                    )
                })?;
                (
                    CanonicalExpressionValue::Int(value),
                    CanonicalExpressionType::Int,
                )
            }
            SourceLiteral::Float(value) => (
                CanonicalExpressionValue::Float(*value),
                CanonicalExpressionType::Float,
            ),
            SourceLiteral::Time(value) => (
                CanonicalExpressionValue::Time(*value),
                CanonicalExpressionType::Time,
            ),
            SourceLiteral::Beat(value) => (
                CanonicalExpressionValue::ExactBeat(
                    CanonicalBeat::new(value.numerator(), value.denominator())
                        .expect("a source Beat is already a valid normalized rational"),
                ),
                CanonicalExpressionType::Beat,
            ),
            SourceLiteral::Length(value) => (
                CanonicalExpressionValue::Length(*value),
                CanonicalExpressionType::Length,
            ),
            SourceLiteral::Angle(value) => (
                CanonicalExpressionValue::Angle(*value),
                CanonicalExpressionType::Angle,
            ),
            SourceLiteral::Color(value) => (
                CanonicalExpressionValue::Color(value.to_linear()),
                CanonicalExpressionType::Color,
            ),
            SourceLiteral::Null | SourceLiteral::String(_) | SourceLiteral::Line(_) => {
                return Err(self.error(
                    DiagnosticCode::TYPE_INVALID_OPERATION,
                    "literal is not a Core runtime expression value",
                    span,
                ));
            }
        };
        if !value.is_finite() {
            return Err(self.error(
                DiagnosticCode::NUMERIC_NON_FINITE,
                "runtime constant must be finite",
                span,
            ));
        }
        self.insert_with_constant(
            CanonicalExpressionOpcode::Constant,
            result_type,
            [None, None, None],
            Some(value),
            span,
        )
    }

    fn lower_environment(
        &mut self,
        name: &str,
        span: SourceSpan,
    ) -> Result<LoweredNode, Diagnostic> {
        let (opcode, result_type) = match name {
            "s" => (
                CanonicalExpressionOpcode::EnvS,
                CanonicalExpressionType::Time,
            ),
            "b" => (
                CanonicalExpressionOpcode::EnvB,
                CanonicalExpressionType::Beat,
            ),
            "q" => (
                CanonicalExpressionOpcode::EnvQ,
                CanonicalExpressionType::Float,
            ),
            "d" => (
                CanonicalExpressionOpcode::EnvD,
                CanonicalExpressionType::Length,
            ),
            "p" if self.allow_progress => (
                CanonicalExpressionOpcode::EnvP,
                CanonicalExpressionType::Float,
            ),
            "p" => {
                return Err(self.error(
                    DiagnosticCode::EXPRESSION_ENVIRONMENT_UNAVAILABLE,
                    "EnvP requires a Piece context",
                    span,
                ));
            }
            _ => {
                return Err(self.error(
                    DiagnosticCode::NAME_UNKNOWN,
                    format!("unknown runtime environment {name}"),
                    span,
                ));
            }
        };
        self.insert(opcode, result_type, [None, None, None], span)
    }

    fn lower_call(
        &mut self,
        callee: &SourceExpression,
        arguments: &[SourceExpression],
        span: SourceSpan,
    ) -> Result<LoweredNode, Diagnostic> {
        let SourceExpression::Name { name, .. } = callee else {
            return Err(self.error(
                DiagnosticCode::TYPE_INVALID_OPERATION,
                "runtime call target must be a Core function name",
                span,
            ));
        };
        let arguments = arguments
            .iter()
            .map(|argument| self.lower(argument))
            .collect::<Result<Vec<_>, _>>()?;
        let (opcode, result_type, expected_types) = match name.as_str() {
            "abs" => (
                CanonicalExpressionOpcode::Abs,
                arguments.first().map(|node| node.ty.clone()),
                1,
            ),
            "min" => (
                CanonicalExpressionOpcode::Min,
                arguments.first().map(|node| node.ty.clone()),
                2,
            ),
            "max" => (
                CanonicalExpressionOpcode::Max,
                arguments.first().map(|node| node.ty.clone()),
                2,
            ),
            "clamp" => (
                CanonicalExpressionOpcode::Clamp,
                arguments.first().map(|node| node.ty.clone()),
                3,
            ),
            "floor" => (
                CanonicalExpressionOpcode::Floor,
                Some(CanonicalExpressionType::Float),
                1,
            ),
            "ceil" => (
                CanonicalExpressionOpcode::Ceil,
                Some(CanonicalExpressionType::Float),
                1,
            ),
            "round" => (
                CanonicalExpressionOpcode::Round,
                Some(CanonicalExpressionType::Float),
                1,
            ),
            "sqrt" => (
                CanonicalExpressionOpcode::Sqrt,
                Some(CanonicalExpressionType::Float),
                1,
            ),
            "exp" => (
                CanonicalExpressionOpcode::Exp,
                Some(CanonicalExpressionType::Float),
                1,
            ),
            "ln" => (
                CanonicalExpressionOpcode::Ln,
                Some(CanonicalExpressionType::Float),
                1,
            ),
            "sin" => (
                CanonicalExpressionOpcode::Sin,
                Some(CanonicalExpressionType::Float),
                1,
            ),
            "cos" => (
                CanonicalExpressionOpcode::Cos,
                Some(CanonicalExpressionType::Float),
                1,
            ),
            "tan" => (
                CanonicalExpressionOpcode::Tan,
                Some(CanonicalExpressionType::Float),
                1,
            ),
            "asin" => (
                CanonicalExpressionOpcode::Asin,
                Some(CanonicalExpressionType::Float),
                1,
            ),
            "acos" => (
                CanonicalExpressionOpcode::Acos,
                Some(CanonicalExpressionType::Float),
                1,
            ),
            "atan" => (
                CanonicalExpressionOpcode::Atan,
                Some(CanonicalExpressionType::Float),
                1,
            ),
            "atan2" => (
                CanonicalExpressionOpcode::Atan2,
                Some(CanonicalExpressionType::Float),
                2,
            ),
            "pow" => (
                CanonicalExpressionOpcode::Pow,
                Some(CanonicalExpressionType::Float),
                2,
            ),
            "approxEq" => (
                CanonicalExpressionOpcode::ApproxEq,
                Some(CanonicalExpressionType::Bool),
                3,
            ),
            "toFloat" => (
                CanonicalExpressionOpcode::ToFloat,
                Some(CanonicalExpressionType::Float),
                1,
            ),
            "seconds" => (
                CanonicalExpressionOpcode::Seconds,
                Some(CanonicalExpressionType::Float),
                1,
            ),
            "radians" => (
                CanonicalExpressionOpcode::Radians,
                Some(CanonicalExpressionType::Float),
                1,
            ),
            _ => {
                return Err(self.error(
                    DiagnosticCode::NAME_UNKNOWN,
                    format!("unknown runtime function {name}"),
                    span,
                ));
            }
        };
        if arguments.len() != expected_types {
            return Err(self.error(
                DiagnosticCode::TYPE_INVALID_OPERATION,
                format!("runtime function {name} has wrong arity"),
                span,
            ));
        }
        let result_type = result_type.ok_or_else(|| {
            self.error(
                DiagnosticCode::TYPE_INVALID_OPERATION,
                format!("runtime function {name} has no typed result"),
                span,
            )
        })?;
        validate_call_types(name, &arguments, &result_type)
            .map_err(|message| self.error(DiagnosticCode::TYPE_MISMATCH, message, span))?;
        let mut operands = [None, None, None];
        for (slot, argument) in arguments.iter().enumerate() {
            operands[slot] = Some(argument.index);
        }
        self.insert(opcode, result_type, operands, span)
    }

    fn lower_choose(
        &mut self,
        arms: &[crate::ast::SourceChooseArm],
        else_value: &SourceExpression,
        span: SourceSpan,
    ) -> Result<LoweredNode, Diagnostic> {
        let mut selected = self.lower(else_value)?;
        for arm in arms.iter().rev() {
            let value = self.lower(&arm.value)?;
            let predicate = self.lower(&arm.condition)?;
            if predicate.ty != CanonicalExpressionType::Bool || value.ty != selected.ty {
                return Err(self.error(
                    DiagnosticCode::TYPE_MISMATCH,
                    "choose predicates must be bool and result types must match",
                    arm.span,
                ));
            }
            selected = self.insert(
                CanonicalExpressionOpcode::Choose,
                value.ty.clone(),
                [
                    Some(predicate.index),
                    Some(value.index),
                    Some(selected.index),
                ],
                span,
            )?;
        }
        Ok(selected)
    }

    fn insert(
        &mut self,
        opcode: CanonicalExpressionOpcode,
        result_type: CanonicalExpressionType,
        operands: [Option<usize>; 3],
        span: SourceSpan,
    ) -> Result<LoweredNode, Diagnostic> {
        self.insert_with_constant(opcode, result_type, operands, None, span)
    }

    fn insert_with_constant(
        &mut self,
        opcode: CanonicalExpressionOpcode,
        result_type: CanonicalExpressionType,
        operands: [Option<usize>; 3],
        constant: Option<CanonicalExpressionValue>,
        span: SourceSpan,
    ) -> Result<LoweredNode, Diagnostic> {
        let node = CanonicalExpressionNode::new(opcode, result_type.clone(), operands, constant, 0);
        let index = self.builder.intern(node).map_err(|error| {
            self.error(
                DiagnosticCode::TYPE_INVALID_OPERATION,
                error.to_string(),
                span,
            )
        })?;
        Ok(LoweredNode {
            index,
            ty: result_type,
        })
    }

    fn error(
        &self,
        code: DiagnosticCode,
        message: impl Into<String>,
        span: SourceSpan,
    ) -> Diagnostic {
        Diagnostic::new(code, DiagnosticStage::Canonical, message, span)
    }
}

fn canonical_value(value: &TypedValue) -> Option<CanonicalExpressionValue> {
    match value {
        TypedValue::Bool(value) => Some(CanonicalExpressionValue::Bool(*value)),
        TypedValue::Int(value) => Some(CanonicalExpressionValue::Int(*value)),
        TypedValue::Float(value) => Some(CanonicalExpressionValue::Float(*value)),
        TypedValue::Time(value) => Some(CanonicalExpressionValue::Time(*value)),
        TypedValue::Beat(value) => Some(CanonicalExpressionValue::ExactBeat(
            CanonicalBeat::new(value.numerator(), value.denominator())
                .expect("a typed Beat is already a valid normalized rational"),
        )),
        TypedValue::Length(value) => Some(CanonicalExpressionValue::Length(*value)),
        TypedValue::Angle(value) => Some(CanonicalExpressionValue::Angle(*value)),
        TypedValue::Color(value) => Some(CanonicalExpressionValue::Color(value.to_linear())),
        TypedValue::Vec2(x, y) => Some(CanonicalExpressionValue::Vec2(
            Box::new(canonical_value(x)?),
            Box::new(canonical_value(y)?),
        )),
        _ => None,
    }
}

fn binary_opcode(operator: BinaryOperator) -> CanonicalExpressionOpcode {
    match operator {
        BinaryOperator::Add => CanonicalExpressionOpcode::Add,
        BinaryOperator::Subtract => CanonicalExpressionOpcode::Sub,
        BinaryOperator::Multiply => CanonicalExpressionOpcode::Mul,
        BinaryOperator::Divide => CanonicalExpressionOpcode::Div,
        BinaryOperator::Remainder => CanonicalExpressionOpcode::Mod,
        BinaryOperator::Power => CanonicalExpressionOpcode::Pow,
        BinaryOperator::Equal => CanonicalExpressionOpcode::Eq,
        BinaryOperator::NotEqual => CanonicalExpressionOpcode::Ne,
        BinaryOperator::LessThan => CanonicalExpressionOpcode::Lt,
        BinaryOperator::LessThanOrEqual => CanonicalExpressionOpcode::Le,
        BinaryOperator::GreaterThan => CanonicalExpressionOpcode::Gt,
        BinaryOperator::GreaterThanOrEqual => CanonicalExpressionOpcode::Ge,
        BinaryOperator::And => CanonicalExpressionOpcode::And,
        BinaryOperator::Or => CanonicalExpressionOpcode::Or,
    }
}

fn binary_result_type(
    operator: BinaryOperator,
    left: &CanonicalExpressionType,
    right: &CanonicalExpressionType,
) -> Option<CanonicalExpressionType> {
    use BinaryOperator as Op;
    match operator {
        Op::And | Op::Or if left == &CanonicalExpressionType::Bool && right == left => {
            Some(CanonicalExpressionType::Bool)
        }
        Op::Equal | Op::NotEqual if left == right => Some(CanonicalExpressionType::Bool),
        Op::LessThan | Op::LessThanOrEqual | Op::GreaterThan | Op::GreaterThanOrEqual
            if left == right && left.is_numeric() =>
        {
            Some(CanonicalExpressionType::Bool)
        }
        Op::Add | Op::Subtract if left == right && (left.is_numeric() || left.is_vector()) => {
            Some(left.clone())
        }
        Op::Multiply
            if left == right
                && matches!(
                    left,
                    CanonicalExpressionType::Int | CanonicalExpressionType::Float
                ) =>
        {
            Some(left.clone())
        }
        Op::Multiply if vector_scalar_result(left, right).is_some() => {
            vector_scalar_result(left, right)
        }
        Op::Multiply if vector_scalar_result(right, left).is_some() => {
            vector_scalar_result(right, left)
        }
        Op::Multiply if is_unit(left) && is_scalar(right) => Some(left.clone()),
        Op::Multiply if is_scalar(left) && is_unit(right) => Some(right.clone()),
        Op::Divide if vector_scalar_result(left, right).is_some() => {
            vector_scalar_result(left, right)
        }
        Op::Divide
            if left == right
                && matches!(
                    left,
                    CanonicalExpressionType::Int | CanonicalExpressionType::Float
                ) =>
        {
            Some(left.clone())
        }
        Op::Divide if is_unit(left) && is_scalar(right) => Some(left.clone()),
        Op::Divide if left == right && is_unit(left) => Some(CanonicalExpressionType::Float),
        Op::Remainder if left == right && left == &CanonicalExpressionType::Int => {
            Some(left.clone())
        }
        Op::Power
            if left == right
                && matches!(
                    left,
                    CanonicalExpressionType::Int | CanonicalExpressionType::Float
                ) =>
        {
            Some(left.clone())
        }
        _ => None,
    }
}

fn is_scalar(value: &CanonicalExpressionType) -> bool {
    matches!(
        value,
        CanonicalExpressionType::Int | CanonicalExpressionType::Float
    )
}

fn vector_scalar_result(
    vector: &CanonicalExpressionType,
    scalar: &CanonicalExpressionType,
) -> Option<CanonicalExpressionType> {
    let CanonicalExpressionType::Vec2(element) = vector else {
        return None;
    };
    if !is_scalar(scalar) {
        return None;
    }
    let result_element = if is_unit(element) || **element == CanonicalExpressionType::Float {
        (**element).clone()
    } else if **element == CanonicalExpressionType::Int && *scalar == CanonicalExpressionType::Float
    {
        CanonicalExpressionType::Float
    } else if **element == CanonicalExpressionType::Int && *scalar == CanonicalExpressionType::Int {
        CanonicalExpressionType::Int
    } else {
        return None;
    };
    Some(CanonicalExpressionType::Vec2(Box::new(result_element)))
}

fn is_unit(value: &CanonicalExpressionType) -> bool {
    matches!(
        value,
        CanonicalExpressionType::Time
            | CanonicalExpressionType::Beat
            | CanonicalExpressionType::Length
            | CanonicalExpressionType::Angle
    )
}

fn validate_call_types(
    name: &str,
    arguments: &[LoweredNode],
    result_type: &CanonicalExpressionType,
) -> Result<(), String> {
    let same = |expected: &CanonicalExpressionType| {
        arguments.iter().all(|argument| &argument.ty == expected)
    };
    match name {
        "abs" => {
            if !arguments[0].ty.is_numeric() || arguments[0].ty.is_vector() {
                return Err("abs requires a scalar numeric value".into());
            }
        }
        "min" | "max" => {
            if arguments[0].ty != arguments[1].ty
                || !arguments[0].ty.is_numeric()
                || arguments[0].ty.is_vector()
            {
                return Err(format!(
                    "{name} requires two values of one scalar numeric type"
                ));
            }
        }
        "clamp" => {
            if !same(&arguments[0].ty)
                || !arguments[0].ty.is_numeric()
                || arguments[0].ty.is_vector()
            {
                return Err("clamp requires three values of one scalar numeric type".into());
            }
        }
        "toFloat" => {
            if arguments[0].ty != CanonicalExpressionType::Int
                || result_type != &CanonicalExpressionType::Float
            {
                return Err("toFloat requires an int".into());
            }
        }
        "seconds" => {
            if arguments[0].ty != CanonicalExpressionType::Time {
                return Err("seconds requires time".into());
            }
        }
        "radians" => {
            if arguments[0].ty != CanonicalExpressionType::Angle {
                return Err("radians requires angle".into());
            }
        }
        "floor" | "ceil" | "round" | "sqrt" | "exp" | "ln" | "sin" | "cos" | "tan" | "asin"
        | "acos" | "atan" => {
            if arguments[0].ty != CanonicalExpressionType::Float {
                return Err(format!("{name} requires float"));
            }
        }
        "pow" | "atan2" => {
            if !same(&CanonicalExpressionType::Float) {
                return Err(format!("{name} requires two floats"));
            }
        }
        "approxEq" if !same(&CanonicalExpressionType::Float) => {
            return Err("approxEq requires three floats".into());
        }
        _ => {}
    }
    Ok(())
}
