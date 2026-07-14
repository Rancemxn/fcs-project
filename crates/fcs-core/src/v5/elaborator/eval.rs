//! Pure expression type checking and evaluation.

use std::collections::BTreeMap;

use crate::ast::{
    Beat, BinaryOperator, ConstDeclaration, Definition, DefinitionsBlock, FunctionDeclaration,
    FunctionStatement, SourceExpression, SourceLiteral, SourceSpan, Type, TypedValue,
    UnaryOperator,
};

use super::scope::{Binding, Scope};
use super::{CompileTimeLimits, Diagnostic};

pub(super) fn check_and_evaluate(
    definitions: &DefinitionsBlock,
    limits: CompileTimeLimits,
) -> Result<(), Diagnostic> {
    let mut constants = BTreeMap::new();
    let mut functions = BTreeMap::new();
    let builtin_span = SourceSpan::new(0, 0);
    let mut global_names = BTreeMap::from([
        ("pi".to_owned(), builtin_span),
        ("sin".to_owned(), builtin_span),
        ("cos".to_owned(), builtin_span),
        ("toFloat".to_owned(), builtin_span),
        ("approxEq".to_owned(), builtin_span),
    ]);
    for definition in &definitions.declarations {
        let (name, span) = match definition {
            Definition::Const(declaration) => (&declaration.name, declaration.name_span),
            Definition::Function(declaration) => (&declaration.name, declaration.name_span),
        };
        if let Some(previous_span) = global_names.insert(name.clone(), span) {
            return Err(Diagnostic::ShadowedBinding {
                name: name.clone(),
                span,
                previous_span,
            });
        }
        match definition {
            Definition::Const(declaration) => {
                constants.insert(declaration.name.clone(), declaration);
            }
            Definition::Function(declaration) => {
                functions.insert(declaration.name.clone(), declaration);
            }
        }
    }

    let mut root = Scope::root();
    root.declare(
        "pi".to_owned(),
        Binding {
            ty: Type::Float,
            value: Some(TypedValue::Float(std::f64::consts::PI)),
            span: SourceSpan::new(0, 0),
        },
    )?;
    for builtin in ["sin", "cos", "toFloat", "approxEq"] {
        root.reserve(builtin.to_owned(), builtin_span);
    }
    for declaration in functions.values() {
        root.reserve(declaration.name.clone(), declaration.name_span);
    }
    for declaration in constants.values() {
        root.declare(
            declaration.name.clone(),
            Binding {
                ty: declaration.ty.clone(),
                value: None,
                span: declaration.name_span,
            },
        )?;
    }

    for declaration in constants.values() {
        let actual = infer_expression(&declaration.initializer, &root, &functions)?;
        require_type(&declaration.ty, &actual, declaration.initializer.span())?;
    }
    for declaration in functions.values() {
        check_function(declaration, &root, &functions)?;
    }

    let mut values = BTreeMap::new();
    let mut budget = Budget::new(limits);
    for name in constants.keys() {
        evaluate_const(
            name,
            &constants,
            &functions,
            &mut values,
            &root,
            &mut budget,
        )?;
    }
    Ok(())
}

/// Evaluate one source expression in the same compile-time environment used by definitions.
/// Entity expansion supplies template parameters through `bindings`.
pub(super) fn evaluate_with_bindings(
    expression: &SourceExpression,
    definitions: Option<&DefinitionsBlock>,
    bindings: &BTreeMap<String, TypedValue>,
    limits: CompileTimeLimits,
) -> Result<TypedValue, Diagnostic> {
    let mut constants = BTreeMap::new();
    let mut functions = BTreeMap::new();
    let builtin_span = SourceSpan::new(0, 0);
    let mut root = Scope::root();
    root.declare(
        "pi".to_owned(),
        Binding {
            ty: Type::Float,
            value: Some(TypedValue::Float(std::f64::consts::PI)),
            span: builtin_span,
        },
    )?;
    for builtin in ["sin", "cos", "toFloat", "approxEq"] {
        root.reserve(builtin.to_owned(), builtin_span);
    }
    if let Some(definitions) = definitions {
        for definition in &definitions.declarations {
            match definition {
                Definition::Const(declaration) => {
                    constants.insert(declaration.name.clone(), declaration);
                }
                Definition::Function(declaration) => {
                    functions.insert(declaration.name.clone(), declaration);
                }
            }
        }
        for declaration in functions.values() {
            root.reserve(declaration.name.clone(), declaration.name_span);
        }
        for declaration in constants.values() {
            root.declare(
                declaration.name.clone(),
                Binding {
                    ty: declaration.ty.clone(),
                    value: None,
                    span: declaration.name_span,
                },
            )?;
        }
    }
    for (name, value) in bindings {
        root.declare(
            name.clone(),
            Binding {
                ty: value.ty(),
                value: Some(value.clone()),
                span: expression.span(),
            },
        )?;
    }
    let mut values = BTreeMap::new();
    let mut budget = Budget::new(limits);
    for name in constants.keys() {
        evaluate_const(
            name,
            &constants,
            &functions,
            &mut values,
            &root,
            &mut budget,
        )?;
    }
    let actual = infer_expression(expression, &root, &functions)?;
    let value = evaluate_expression(
        expression,
        &root,
        &constants,
        &functions,
        &mut values,
        &mut budget,
    )?;
    require_type(&actual, &value.ty(), expression.span())?;
    Ok(value)
}

fn check_function(
    declaration: &FunctionDeclaration,
    root: &Scope,
    functions: &BTreeMap<String, &FunctionDeclaration>,
) -> Result<(), Diagnostic> {
    let mut scope = root.child();
    for parameter in &declaration.parameters {
        scope.declare(
            parameter.name.clone(),
            Binding {
                ty: parameter.ty.clone(),
                value: None,
                span: parameter.name_span,
            },
        )?;
    }
    if !check_block(
        &declaration.body,
        &scope,
        &declaration.return_type,
        functions,
    )? {
        return Err(Diagnostic::MissingReturn {
            function: declaration.name.clone(),
            span: declaration.span,
        });
    }
    Ok(())
}

fn check_block(
    statements: &[FunctionStatement],
    initial_scope: &Scope,
    return_type: &Type,
    functions: &BTreeMap<String, &FunctionDeclaration>,
) -> Result<bool, Diagnostic> {
    let mut scope = initial_scope.clone();
    for statement in statements {
        match statement {
            FunctionStatement::Let(statement) => {
                let actual = infer_expression(&statement.initializer, &scope, functions)?;
                require_type(&statement.ty, &actual, statement.initializer.span())?;
                scope.declare(
                    statement.name.clone(),
                    Binding {
                        ty: statement.ty.clone(),
                        value: None,
                        span: statement.name_span,
                    },
                )?;
            }
            FunctionStatement::Return(statement) => {
                let actual = infer_expression(&statement.value, &scope, functions)?;
                require_type(return_type, &actual, statement.value.span())?;
                return Ok(true);
            }
            FunctionStatement::If(statement) => {
                let condition_type = infer_expression(&statement.condition, &scope, functions)?;
                require_type(&Type::Bool, &condition_type, statement.condition.span())?;
                let then_returns = check_block(
                    &statement.then_branch,
                    &scope.child(),
                    return_type,
                    functions,
                )?;
                let else_returns = check_block(
                    &statement.else_branch,
                    &scope.child(),
                    return_type,
                    functions,
                )?;
                if then_returns && else_returns {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn infer_expression(
    expression: &SourceExpression,
    scope: &Scope,
    functions: &BTreeMap<String, &FunctionDeclaration>,
) -> Result<Type, Diagnostic> {
    match expression {
        SourceExpression::Literal { literal, .. } => Ok(literal_type(literal)),
        SourceExpression::Name { name, span } => scope
            .lookup(name)
            .map(|binding| binding.ty.clone())
            .ok_or_else(|| Diagnostic::UnknownName {
                name: name.clone(),
                span: *span,
            }),
        SourceExpression::Unary {
            operator,
            operand,
            span,
        } => {
            let operand_type = infer_expression(operand, scope, functions)?;
            match (operator, &operand_type) {
                (UnaryOperator::Not, Type::Bool) => Ok(Type::Bool),
                (UnaryOperator::Negate, ty) if is_numeric_or_unit(ty) => Ok(operand_type),
                _ => Err(Diagnostic::InvalidOperation {
                    message: "invalid unary operand",
                    span: *span,
                }),
            }
        }
        SourceExpression::Binary {
            left,
            operator,
            right,
            span,
        } => {
            let left_type = infer_expression(left, scope, functions)?;
            let right_type = infer_expression(right, scope, functions)?;
            infer_binary(*operator, &left_type, &right_type).ok_or(Diagnostic::InvalidOperation {
                message: "invalid binary operands",
                span: *span,
            })
        }
        SourceExpression::Call {
            callee,
            arguments,
            span,
        } => {
            let SourceExpression::Name { name, .. } = callee.as_ref() else {
                return Err(Diagnostic::InvalidOperation {
                    message: "call target must be a pure function name",
                    span: *span,
                });
            };
            let (parameters, return_type) =
                signature(name, functions).ok_or_else(|| Diagnostic::UnknownName {
                    name: name.clone(),
                    span: callee.span(),
                })?;
            if arguments.len() != parameters.len() {
                return Err(Diagnostic::WrongArity {
                    callee: name.clone(),
                    expected: parameters.len(),
                    actual: arguments.len(),
                    span: *span,
                });
            }
            for (argument, expected) in arguments.iter().zip(parameters) {
                let actual = infer_expression(argument, scope, functions)?;
                require_type(&expected, &actual, argument.span())?;
            }
            Ok(return_type)
        }
        SourceExpression::FieldAccess { base, field, span } => {
            let Type::Vec2(element) = infer_expression(base, scope, functions)? else {
                return Err(Diagnostic::InvalidOperation {
                    message: "field access requires vec2",
                    span: *span,
                });
            };
            if matches!(field.as_str(), "x" | "y") {
                Ok(*element)
            } else {
                Err(Diagnostic::InvalidOperation {
                    message: "unknown vec2 field",
                    span: *span,
                })
            }
        }
        SourceExpression::Vec2 { x, y, span } => {
            let x_type = infer_expression(x, scope, functions)?;
            let y_type = infer_expression(y, scope, functions)?;
            require_type(&x_type, &y_type, *span)?;
            if !is_scalar_value_type(&x_type) {
                return Err(Diagnostic::InvalidOperation {
                    message: "invalid vec2 element type",
                    span: *span,
                });
            }
            Ok(Type::Vec2(Box::new(x_type)))
        }
    }
}

fn signature(
    name: &str,
    functions: &BTreeMap<String, &FunctionDeclaration>,
) -> Option<(Vec<Type>, Type)> {
    let builtin = match name {
        "sin" | "cos" => Some((vec![Type::Float], Type::Float)),
        "toFloat" => Some((vec![Type::Int], Type::Float)),
        "approxEq" => Some((vec![Type::Float, Type::Float, Type::Float], Type::Bool)),
        _ => None,
    };
    builtin.or_else(|| {
        functions.get(name).map(|function| {
            (
                function
                    .parameters
                    .iter()
                    .map(|parameter| parameter.ty.clone())
                    .collect(),
                function.return_type.clone(),
            )
        })
    })
}

fn infer_binary(operator: BinaryOperator, left: &Type, right: &Type) -> Option<Type> {
    use BinaryOperator as Op;
    match operator {
        Op::And | Op::Or if left == &Type::Bool && right == &Type::Bool => Some(Type::Bool),
        Op::Equal | Op::NotEqual if left == right && is_equality_type(left) => Some(Type::Bool),
        Op::LessThan | Op::LessThanOrEqual | Op::GreaterThan | Op::GreaterThanOrEqual
            if left == right && is_numeric_or_unit(left) =>
        {
            Some(Type::Bool)
        }
        Op::Add | Op::Subtract if left == right && is_numeric_or_unit(left) => Some(left.clone()),
        Op::Multiply if left == right && matches!(left, Type::Int | Type::Float) => {
            Some(left.clone())
        }
        Op::Multiply if is_unit(left) && is_scalar(right) => Some(left.clone()),
        Op::Multiply if is_scalar(left) && is_unit(right) => Some(right.clone()),
        Op::Divide if left == right && matches!(left, Type::Int | Type::Float) => {
            Some(left.clone())
        }
        Op::Divide if is_unit(left) && is_scalar(right) => Some(left.clone()),
        Op::Divide if left == right && is_unit(left) => Some(Type::Float),
        Op::Remainder if left == right && matches!(left, Type::Int | Type::Float) => {
            Some(left.clone())
        }
        _ => None,
    }
}

fn require_type(expected: &Type, actual: &Type, span: SourceSpan) -> Result<(), Diagnostic> {
    if expected == actual {
        Ok(())
    } else {
        Err(Diagnostic::TypeMismatch {
            expected: expected.clone(),
            actual: actual.clone(),
            span,
        })
    }
}

fn literal_type(literal: &SourceLiteral) -> Type {
    match literal {
        SourceLiteral::Bool(_) => Type::Bool,
        SourceLiteral::Int(_) => Type::Int,
        SourceLiteral::Float(_) => Type::Float,
        SourceLiteral::String(_) => Type::String,
        SourceLiteral::Time(_) => Type::Time,
        SourceLiteral::Beat(_) => Type::Beat,
        SourceLiteral::Length(_) => Type::Length,
        SourceLiteral::Angle(_) => Type::Angle,
        SourceLiteral::Color(_) => Type::Color,
    }
}

fn is_scalar(ty: &Type) -> bool {
    matches!(ty, Type::Int | Type::Float)
}

fn is_unit(ty: &Type) -> bool {
    matches!(ty, Type::Time | Type::Beat | Type::Length | Type::Angle)
}

fn is_numeric_or_unit(ty: &Type) -> bool {
    is_scalar(ty) || is_unit(ty)
}

fn is_scalar_value_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Bool
            | Type::Int
            | Type::Float
            | Type::String
            | Type::Time
            | Type::Beat
            | Type::Length
            | Type::Angle
            | Type::Color
    )
}

fn is_equality_type(ty: &Type) -> bool {
    is_scalar_value_type(ty) || matches!(ty, Type::Vec2(_))
}

fn evaluate_const(
    name: &str,
    constants: &BTreeMap<String, &ConstDeclaration>,
    functions: &BTreeMap<String, &FunctionDeclaration>,
    values: &mut BTreeMap<String, TypedValue>,
    root: &Scope,
    budget: &mut Budget,
) -> Result<TypedValue, Diagnostic> {
    if let Some(value) = values.get(name) {
        return Ok(value.clone());
    }
    let declaration = constants[name];
    let value = evaluate_expression(
        &declaration.initializer,
        root,
        constants,
        functions,
        values,
        budget,
    )?;
    values.insert(name.to_owned(), value.clone());
    Ok(value)
}

fn evaluate_expression(
    expression: &SourceExpression,
    scope: &Scope,
    constants: &BTreeMap<String, &ConstDeclaration>,
    functions: &BTreeMap<String, &FunctionDeclaration>,
    const_values: &mut BTreeMap<String, TypedValue>,
    budget: &mut Budget,
) -> Result<TypedValue, Diagnostic> {
    budget.node(expression.span())?;
    match expression {
        SourceExpression::Literal { literal, .. } => Ok(literal_value(literal)),
        SourceExpression::Name { name, span } => {
            if let Some(value) = scope.lookup(name).and_then(|binding| binding.value.clone()) {
                Ok(value)
            } else if constants.contains_key(name) {
                evaluate_const(name, constants, functions, const_values, scope, budget)
            } else {
                Err(Diagnostic::UnknownName {
                    name: name.clone(),
                    span: *span,
                })
            }
        }
        SourceExpression::Unary {
            operator,
            operand,
            span,
        } => {
            budget.operation(*span)?;
            let value =
                evaluate_expression(operand, scope, constants, functions, const_values, budget)?;
            evaluate_unary(*operator, value, *span)
        }
        SourceExpression::Binary {
            left,
            operator,
            right,
            span,
        } => {
            budget.operation(*span)?;
            let left =
                evaluate_expression(left, scope, constants, functions, const_values, budget)?;
            let right =
                evaluate_expression(right, scope, constants, functions, const_values, budget)?;
            evaluate_binary(left, *operator, right, *span)
        }
        SourceExpression::Call {
            callee,
            arguments,
            span,
        } => {
            budget.operation(*span)?;
            let SourceExpression::Name { name, .. } = callee.as_ref() else {
                return Err(Diagnostic::InvalidOperation {
                    message: "call target must be a pure function name",
                    span: *span,
                });
            };
            let arguments = arguments
                .iter()
                .map(|argument| {
                    evaluate_expression(argument, scope, constants, functions, const_values, budget)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(value) = evaluate_builtin(name, &arguments, *span)? {
                return Ok(value);
            }
            let function = functions.get(name).ok_or_else(|| Diagnostic::UnknownName {
                name: name.clone(),
                span: callee.span(),
            })?;
            evaluate_function(
                function,
                arguments,
                scope,
                constants,
                functions,
                const_values,
                budget,
            )
        }
        SourceExpression::FieldAccess { base, field, span } => {
            let value =
                evaluate_expression(base, scope, constants, functions, const_values, budget)?;
            let TypedValue::Vec2(x, y) = value else {
                return Err(Diagnostic::InvalidOperation {
                    message: "field access requires vec2",
                    span: *span,
                });
            };
            match field.as_str() {
                "x" => Ok(*x),
                "y" => Ok(*y),
                _ => Err(Diagnostic::InvalidOperation {
                    message: "unknown vec2 field",
                    span: *span,
                }),
            }
        }
        SourceExpression::Vec2 { x, y, span } => {
            let x = evaluate_expression(x, scope, constants, functions, const_values, budget)?;
            let y = evaluate_expression(y, scope, constants, functions, const_values, budget)?;
            TypedValue::vec2(x, y).ok_or(Diagnostic::InvalidOperation {
                message: "vec2 components have different types",
                span: *span,
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_function(
    function: &FunctionDeclaration,
    arguments: Vec<TypedValue>,
    root: &Scope,
    constants: &BTreeMap<String, &ConstDeclaration>,
    functions: &BTreeMap<String, &FunctionDeclaration>,
    const_values: &mut BTreeMap<String, TypedValue>,
    budget: &mut Budget,
) -> Result<TypedValue, Diagnostic> {
    let mut scope = root.child();
    for (parameter, value) in function.parameters.iter().zip(arguments) {
        scope.declare(
            parameter.name.clone(),
            Binding {
                ty: parameter.ty.clone(),
                value: Some(value),
                span: parameter.name_span,
            },
        )?;
    }
    evaluate_block(
        &function.body,
        &scope,
        constants,
        functions,
        const_values,
        budget,
    )?
    .ok_or_else(|| Diagnostic::MissingReturn {
        function: function.name.clone(),
        span: function.span,
    })
}

fn evaluate_block(
    statements: &[FunctionStatement],
    initial_scope: &Scope,
    constants: &BTreeMap<String, &ConstDeclaration>,
    functions: &BTreeMap<String, &FunctionDeclaration>,
    const_values: &mut BTreeMap<String, TypedValue>,
    budget: &mut Budget,
) -> Result<Option<TypedValue>, Diagnostic> {
    let mut scope = initial_scope.clone();
    for statement in statements {
        match statement {
            FunctionStatement::Let(statement) => {
                let value = evaluate_expression(
                    &statement.initializer,
                    &scope,
                    constants,
                    functions,
                    const_values,
                    budget,
                )?;
                scope.declare(
                    statement.name.clone(),
                    Binding {
                        ty: statement.ty.clone(),
                        value: Some(value),
                        span: statement.name_span,
                    },
                )?;
            }
            FunctionStatement::Return(statement) => {
                return evaluate_expression(
                    &statement.value,
                    &scope,
                    constants,
                    functions,
                    const_values,
                    budget,
                )
                .map(Some);
            }
            FunctionStatement::If(statement) => {
                let condition = evaluate_expression(
                    &statement.condition,
                    &scope,
                    constants,
                    functions,
                    const_values,
                    budget,
                )?;
                let TypedValue::Bool(condition) = condition else {
                    return Err(Diagnostic::InvalidOperation {
                        message: "if condition must be bool",
                        span: statement.condition.span(),
                    });
                };
                let branch = if condition {
                    &statement.then_branch
                } else {
                    &statement.else_branch
                };
                if let Some(value) = evaluate_block(
                    branch,
                    &scope.child(),
                    constants,
                    functions,
                    const_values,
                    budget,
                )? {
                    return Ok(Some(value));
                }
            }
        }
    }
    Ok(None)
}

fn literal_value(literal: &SourceLiteral) -> TypedValue {
    match literal {
        SourceLiteral::Bool(value) => TypedValue::Bool(*value),
        SourceLiteral::Int(value) => TypedValue::Int(*value),
        SourceLiteral::Float(value) => TypedValue::Float(*value),
        SourceLiteral::String(value) => TypedValue::String(value.clone()),
        SourceLiteral::Time(value) => TypedValue::Time(*value),
        SourceLiteral::Beat(value) => TypedValue::Beat(*value),
        SourceLiteral::Length(value) => TypedValue::Length(*value),
        SourceLiteral::Angle(value) => TypedValue::Angle(*value),
        SourceLiteral::Color(value) => TypedValue::Color(*value),
    }
}

fn evaluate_unary(
    operator: UnaryOperator,
    value: TypedValue,
    span: SourceSpan,
) -> Result<TypedValue, Diagnostic> {
    let invalid = || Diagnostic::InvalidOperation {
        message: "invalid unary value",
        span,
    };
    match (operator, value) {
        (UnaryOperator::Not, TypedValue::Bool(value)) => Ok(TypedValue::Bool(!value)),
        (UnaryOperator::Negate, TypedValue::Int(value)) => {
            value.checked_neg().map(TypedValue::Int).ok_or_else(invalid)
        }
        (UnaryOperator::Negate, TypedValue::Float(value)) => {
            finite(-value, span).map(TypedValue::Float)
        }
        (UnaryOperator::Negate, TypedValue::Time(value)) => {
            finite(-value, span).map(TypedValue::Time)
        }
        (UnaryOperator::Negate, TypedValue::Length(value)) => {
            finite(-value, span).map(TypedValue::Length)
        }
        (UnaryOperator::Negate, TypedValue::Angle(value)) => {
            finite(-value, span).map(TypedValue::Angle)
        }
        (UnaryOperator::Negate, TypedValue::Beat(value)) => Beat::new(
            value.numerator().checked_neg().ok_or_else(invalid)?,
            value.denominator(),
        )
        .map(TypedValue::Beat)
        .map_err(|_| invalid()),
        _ => Err(invalid()),
    }
}

fn evaluate_binary(
    left: TypedValue,
    operator: BinaryOperator,
    right: TypedValue,
    span: SourceSpan,
) -> Result<TypedValue, Diagnostic> {
    use BinaryOperator as Op;
    match operator {
        Op::Equal | Op::NotEqual => {
            let equal = left == right;
            Ok(TypedValue::Bool(if operator == Op::Equal {
                equal
            } else {
                !equal
            }))
        }
        Op::And | Op::Or => match (left, right) {
            (TypedValue::Bool(left), TypedValue::Bool(right)) => {
                Ok(TypedValue::Bool(if operator == Op::And {
                    left && right
                } else {
                    left || right
                }))
            }
            _ => invalid_binary(span),
        },
        Op::LessThan | Op::LessThanOrEqual | Op::GreaterThan | Op::GreaterThanOrEqual => {
            compare_values(left, operator, right, span)
        }
        _ => arithmetic(left, operator, right, span),
    }
}

fn arithmetic(
    left: TypedValue,
    operator: BinaryOperator,
    right: TypedValue,
    span: SourceSpan,
) -> Result<TypedValue, Diagnostic> {
    use BinaryOperator as Op;
    match (left, right) {
        (TypedValue::Int(left), TypedValue::Int(right)) => {
            let value = match operator {
                Op::Add => left.checked_add(right),
                Op::Subtract => left.checked_sub(right),
                Op::Multiply => left.checked_mul(right),
                Op::Divide => left.checked_div(right),
                Op::Remainder => left.checked_rem(right),
                _ => None,
            };
            value
                .map(TypedValue::Int)
                .ok_or(Diagnostic::InvalidOperation {
                    message: "integer arithmetic failed",
                    span,
                })
        }
        (TypedValue::Float(left), TypedValue::Float(right)) => {
            float_arithmetic(left, operator, right, span).map(TypedValue::Float)
        }
        (TypedValue::Time(left), TypedValue::Time(right)) => unit_pair(left, operator, right, span)
            .map(|value| match value {
                UnitResult::Unit(value) => TypedValue::Time(value),
                UnitResult::Ratio(value) => TypedValue::Float(value),
            }),
        (TypedValue::Length(left), TypedValue::Length(right)) => {
            unit_pair(left, operator, right, span).map(|value| match value {
                UnitResult::Unit(value) => TypedValue::Length(value),
                UnitResult::Ratio(value) => TypedValue::Float(value),
            })
        }
        (TypedValue::Angle(left), TypedValue::Angle(right)) => {
            unit_pair(left, operator, right, span).map(|value| match value {
                UnitResult::Unit(value) => TypedValue::Angle(value),
                UnitResult::Ratio(value) => TypedValue::Float(value),
            })
        }
        (TypedValue::Beat(left), TypedValue::Beat(right)) => beat_pair(left, operator, right, span),
        (unit, TypedValue::Int(scalar)) => scale_unit(unit, operator, scalar as f64, span),
        (unit, TypedValue::Float(scalar)) => scale_unit(unit, operator, scalar, span),
        (TypedValue::Int(scalar), unit) if operator == Op::Multiply => {
            scale_unit(unit, operator, scalar as f64, span)
        }
        (TypedValue::Float(scalar), unit) if operator == Op::Multiply => {
            scale_unit(unit, operator, scalar, span)
        }
        _ => invalid_binary(span),
    }
}

enum UnitResult {
    Unit(f64),
    Ratio(f64),
}

fn unit_pair(
    left: f64,
    operator: BinaryOperator,
    right: f64,
    span: SourceSpan,
) -> Result<UnitResult, Diagnostic> {
    use BinaryOperator as Op;
    match operator {
        Op::Add => finite(left + right, span).map(UnitResult::Unit),
        Op::Subtract => finite(left - right, span).map(UnitResult::Unit),
        Op::Divide => finite_divide(left, right, span).map(UnitResult::Ratio),
        _ => Err(Diagnostic::InvalidOperation {
            message: "invalid unit arithmetic",
            span,
        }),
    }
}

fn beat_pair(
    left: Beat,
    operator: BinaryOperator,
    right: Beat,
    span: SourceSpan,
) -> Result<TypedValue, Diagnostic> {
    use BinaryOperator as Op;
    match operator {
        Op::Add => left.checked_add(right).map(TypedValue::Beat).map_err(|_| {
            Diagnostic::InvalidOperation {
                message: "beat arithmetic failed",
                span,
            }
        }),
        Op::Subtract => {
            let numerator =
                right
                    .numerator()
                    .checked_neg()
                    .ok_or(Diagnostic::InvalidOperation {
                        message: "beat arithmetic failed",
                        span,
                    })?;
            let negated = Beat::new(numerator, right.denominator()).map_err(|_| {
                Diagnostic::InvalidOperation {
                    message: "beat arithmetic failed",
                    span,
                }
            })?;
            left.checked_add(negated)
                .map(TypedValue::Beat)
                .map_err(|_| Diagnostic::InvalidOperation {
                    message: "beat arithmetic failed",
                    span,
                })
        }
        Op::Divide if right.numerator() != 0 => finite_divide(
            left.numerator() as f64 / left.denominator() as f64,
            right.numerator() as f64 / right.denominator() as f64,
            span,
        )
        .map(TypedValue::Float),
        _ => invalid_binary(span),
    }
}

fn scale_unit(
    unit: TypedValue,
    operator: BinaryOperator,
    scalar: f64,
    span: SourceSpan,
) -> Result<TypedValue, Diagnostic> {
    use BinaryOperator as Op;
    let scale = |value| match operator {
        Op::Multiply => finite(value * scalar, span),
        Op::Divide => finite_divide(value, scalar, span),
        _ => invalid_binary(span).and_then(|_: TypedValue| unreachable!()),
    };
    match unit {
        TypedValue::Time(value) => scale(value).map(TypedValue::Time),
        TypedValue::Length(value) => scale(value).map(TypedValue::Length),
        TypedValue::Angle(value) => scale(value).map(TypedValue::Angle),
        TypedValue::Beat(value) => {
            if !scalar.is_finite() || scalar.fract() != 0.0 {
                return Err(Diagnostic::InvalidOperation {
                    message: "beat scaling requires an integer scalar",
                    span,
                });
            }
            let scalar = scalar as i64;
            let (numerator, denominator) = match operator {
                Op::Multiply => (
                    value.numerator().checked_mul(scalar),
                    Some(value.denominator()),
                ),
                Op::Divide if scalar != 0 => (
                    Some(value.numerator()),
                    value.denominator().checked_mul(scalar),
                ),
                _ => (None, None),
            };
            Beat::new(
                numerator.ok_or(Diagnostic::InvalidOperation {
                    message: "beat arithmetic failed",
                    span,
                })?,
                denominator.ok_or(Diagnostic::InvalidOperation {
                    message: "beat arithmetic failed",
                    span,
                })?,
            )
            .map(TypedValue::Beat)
            .map_err(|_| Diagnostic::InvalidOperation {
                message: "beat arithmetic failed",
                span,
            })
        }
        _ => invalid_binary(span),
    }
}

fn float_arithmetic(
    left: f64,
    operator: BinaryOperator,
    right: f64,
    span: SourceSpan,
) -> Result<f64, Diagnostic> {
    use BinaryOperator as Op;
    match operator {
        Op::Add => finite(left + right, span),
        Op::Subtract => finite(left - right, span),
        Op::Multiply => finite(left * right, span),
        Op::Divide => finite_divide(left, right, span),
        Op::Remainder if right != 0.0 => finite(left % right, span),
        _ => Err(Diagnostic::InvalidOperation {
            message: "floating-point arithmetic failed",
            span,
        }),
    }
}

fn compare_values(
    left: TypedValue,
    operator: BinaryOperator,
    right: TypedValue,
    span: SourceSpan,
) -> Result<TypedValue, Diagnostic> {
    let ordering = match (left, right) {
        (TypedValue::Int(left), TypedValue::Int(right)) => left.partial_cmp(&right),
        (TypedValue::Float(left), TypedValue::Float(right))
        | (TypedValue::Time(left), TypedValue::Time(right))
        | (TypedValue::Length(left), TypedValue::Length(right))
        | (TypedValue::Angle(left), TypedValue::Angle(right)) => left.partial_cmp(&right),
        (TypedValue::Beat(left), TypedValue::Beat(right)) => left.partial_cmp(&right),
        _ => None,
    }
    .ok_or(Diagnostic::InvalidOperation {
        message: "values cannot be compared",
        span,
    })?;
    use BinaryOperator as Op;
    Ok(TypedValue::Bool(match operator {
        Op::LessThan => ordering.is_lt(),
        Op::LessThanOrEqual => ordering.is_le(),
        Op::GreaterThan => ordering.is_gt(),
        Op::GreaterThanOrEqual => ordering.is_ge(),
        _ => return invalid_binary(span),
    }))
}

fn evaluate_builtin(
    name: &str,
    arguments: &[TypedValue],
    span: SourceSpan,
) -> Result<Option<TypedValue>, Diagnostic> {
    let value = match (name, arguments) {
        ("sin", [TypedValue::Float(value)]) => TypedValue::Float(finite(value.sin(), span)?),
        ("cos", [TypedValue::Float(value)]) => TypedValue::Float(finite(value.cos(), span)?),
        ("toFloat", [TypedValue::Int(value)]) => TypedValue::Float(*value as f64),
        (
            "approxEq",
            [
                TypedValue::Float(left),
                TypedValue::Float(right),
                TypedValue::Float(tolerance),
            ],
        ) => TypedValue::Bool(*tolerance >= 0.0 && (left - right).abs() <= *tolerance),
        ("sin" | "cos" | "toFloat" | "approxEq", _) => {
            return Err(Diagnostic::InvalidOperation {
                message: "invalid builtin arguments",
                span,
            });
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

fn finite(value: f64, span: SourceSpan) -> Result<f64, Diagnostic> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(Diagnostic::InvalidOperation {
            message: "non-finite compile-time arithmetic",
            span,
        })
}

fn finite_divide(left: f64, right: f64, span: SourceSpan) -> Result<f64, Diagnostic> {
    if right == 0.0 {
        return Err(Diagnostic::InvalidOperation {
            message: "division by zero",
            span,
        });
    }
    finite(left / right, span)
}

fn invalid_binary<T>(span: SourceSpan) -> Result<T, Diagnostic> {
    Err(Diagnostic::InvalidOperation {
        message: "invalid binary values",
        span,
    })
}

struct Budget {
    limits: CompileTimeLimits,
    operations: usize,
    expression_nodes: usize,
}

impl Budget {
    const fn new(limits: CompileTimeLimits) -> Self {
        Self {
            limits,
            operations: 0,
            expression_nodes: 0,
        }
    }

    fn node(&mut self, span: SourceSpan) -> Result<(), Diagnostic> {
        self.expression_nodes = self.expression_nodes.saturating_add(1);
        if self.expression_nodes > self.limits.max_expression_nodes {
            Err(Diagnostic::LimitExceeded {
                limit: "max_expression_nodes",
                span,
            })
        } else {
            Ok(())
        }
    }

    fn operation(&mut self, span: SourceSpan) -> Result<(), Diagnostic> {
        self.operations = self.operations.saturating_add(1);
        if self.operations > self.limits.max_compile_time_operations {
            Err(Diagnostic::LimitExceeded {
                limit: "max_compile_time_operations",
                span,
            })
        } else {
            Ok(())
        }
    }
}
