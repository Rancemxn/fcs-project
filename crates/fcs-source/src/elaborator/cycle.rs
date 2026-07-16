//! Deterministic dependency-cycle detection.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{
    Definition, DefinitionsBlock, EntityExpression, FunctionStatement, SourceExpression,
    SourceSpan, TemplateStatement,
};

use super::ElaboratorError as Diagnostic;

pub(super) fn reject_cycles(definitions: &DefinitionsBlock) -> Result<(), Diagnostic> {
    let const_names: BTreeSet<_> = definitions
        .declarations
        .iter()
        .filter_map(|definition| match definition {
            Definition::Const(declaration) => Some(declaration.name.clone()),
            Definition::Function(_) | Definition::Template(_) => None,
        })
        .collect();
    let function_names: BTreeSet<_> = definitions
        .declarations
        .iter()
        .filter_map(|definition| match definition {
            Definition::Function(declaration) => Some(declaration.name.clone()),
            Definition::Const(_) | Definition::Template(_) => None,
        })
        .collect();
    let template_names: BTreeSet<_> = definitions
        .declarations
        .iter()
        .filter_map(|definition| match definition {
            Definition::Template(declaration) => Some(declaration.name.clone()),
            Definition::Const(_) | Definition::Function(_) => None,
        })
        .collect();

    let mut const_graph = BTreeMap::new();
    let mut const_spans = BTreeMap::new();
    let mut function_graph = BTreeMap::new();
    let mut function_spans = BTreeMap::new();
    let mut template_graph = BTreeMap::new();
    let mut template_spans = BTreeMap::new();
    for definition in &definitions.declarations {
        match definition {
            Definition::Const(declaration) => {
                let mut dependencies = BTreeSet::new();
                collect_const_names(&declaration.initializer, &const_names, &mut dependencies);
                const_graph.insert(declaration.name.clone(), dependencies);
                const_spans.insert(declaration.name.clone(), declaration.name_span);
            }
            Definition::Function(declaration) => {
                let mut dependencies = BTreeSet::new();
                collect_function_calls_in_block(
                    &declaration.body,
                    &function_names,
                    &mut dependencies,
                );
                function_graph.insert(declaration.name.clone(), dependencies);
                function_spans.insert(declaration.name.clone(), declaration.name_span);
            }
            Definition::Template(declaration) => {
                let mut dependencies = BTreeSet::new();
                collect_template_calls_in_block(
                    &declaration.body,
                    &template_names,
                    &mut dependencies,
                );
                template_graph.insert(declaration.name.clone(), dependencies);
                template_spans.insert(declaration.name.clone(), declaration.name_span);
            }
        }
    }

    if let Some(chain) = find_cycle(&const_graph) {
        return Err(Diagnostic::RecursiveConst {
            span: const_spans[&chain[0]],
            chain,
        });
    }
    if let Some(chain) = find_cycle(&function_graph) {
        return Err(Diagnostic::RecursiveFunction {
            span: function_spans[&chain[0]],
            chain,
        });
    }
    if let Some(chain) = find_cycle(&template_graph) {
        return Err(Diagnostic::RecursiveTemplate {
            span: template_spans[&chain[0]],
            chain,
        });
    }
    Ok(())
}

fn collect_template_calls_in_block(
    statements: &[TemplateStatement],
    names: &BTreeSet<String>,
    output: &mut BTreeSet<String>,
) {
    for statement in statements {
        match statement {
            TemplateStatement::Let(statement) => {
                collect_template_calls(&statement.initializer, names, output);
            }
            TemplateStatement::If(statement) => {
                collect_template_calls(&statement.condition, names, output);
                collect_template_calls_in_block(&statement.then_branch, names, output);
                collect_template_calls_in_block(&statement.else_branch, names, output);
            }
            TemplateStatement::Return(statement) => {
                collect_template_calls_in_entity(&statement.value, names, output);
            }
        }
    }
}

fn collect_template_calls_in_entity(
    expression: &EntityExpression,
    names: &BTreeSet<String>,
    output: &mut BTreeSet<String>,
) {
    match expression {
        EntityExpression::Constructor(constructor) => {
            for field in &constructor.fields {
                collect_template_calls(&field.value, names, output);
            }
        }
        EntityExpression::SourceConstructor(constructor) => {
            for field in &constructor.fields {
                collect_template_calls(&field.value, names, output);
            }
        }
        EntityExpression::Source(expression) => collect_template_calls(expression, names, output),
        EntityExpression::With(expression) => {
            collect_template_calls_in_entity(&expression.base, names, output);
            for field in &expression.fields {
                collect_template_calls(&field.value, names, output);
            }
        }
    }
}

fn collect_template_calls(
    expression: &SourceExpression,
    names: &BTreeSet<String>,
    output: &mut BTreeSet<String>,
) {
    match expression {
        SourceExpression::Call {
            callee, arguments, ..
        } => {
            if let SourceExpression::Name { name, .. } = callee.as_ref()
                && names.contains(name)
            {
                output.insert(name.clone());
            } else {
                collect_template_calls(callee, names, output);
            }
            for argument in arguments {
                collect_template_calls(argument, names, output);
            }
        }
        SourceExpression::Unary { operand, .. }
        | SourceExpression::FieldAccess { base: operand, .. } => {
            collect_template_calls(operand, names, output);
        }
        SourceExpression::Binary { left, right, .. }
        | SourceExpression::Vec2 {
            x: left, y: right, ..
        } => {
            collect_template_calls(left, names, output);
            collect_template_calls(right, names, output);
        }
        SourceExpression::Array { elements, .. } => {
            for element in elements {
                collect_template_calls(element, names, output);
            }
        }
        SourceExpression::Object { entries, .. } => {
            for entry in entries {
                collect_template_calls(&entry.value, names, output);
            }
        }
        SourceExpression::Index { base, index, .. } => {
            collect_template_calls(base, names, output);
            collect_template_calls(index, names, output);
        }
        SourceExpression::Choose {
            arms, else_value, ..
        } => {
            for arm in arms {
                collect_template_calls(&arm.condition, names, output);
                collect_template_calls(&arm.value, names, output);
            }
            collect_template_calls(else_value, names, output);
        }
        SourceExpression::Literal { .. }
        | SourceExpression::Reference { .. }
        | SourceExpression::Name { .. } => {}
    }
}

fn find_cycle(graph: &BTreeMap<String, BTreeSet<String>>) -> Option<Vec<String>> {
    let mut finished = BTreeSet::new();
    for node in graph.keys() {
        let mut stack = Vec::new();
        let mut active = BTreeMap::new();
        if let Some(cycle) = visit(node, graph, &mut finished, &mut stack, &mut active) {
            return Some(cycle);
        }
    }
    None
}

fn visit(
    node: &str,
    graph: &BTreeMap<String, BTreeSet<String>>,
    finished: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
    active: &mut BTreeMap<String, usize>,
) -> Option<Vec<String>> {
    if let Some(start) = active.get(node).copied() {
        let mut chain = stack[start..].to_vec();
        chain.push(node.to_owned());
        return Some(chain);
    }
    if finished.contains(node) {
        return None;
    }
    active.insert(node.to_owned(), stack.len());
    stack.push(node.to_owned());
    if let Some(dependencies) = graph.get(node) {
        for dependency in dependencies {
            if let Some(cycle) = visit(dependency, graph, finished, stack, active) {
                return Some(cycle);
            }
        }
    }
    stack.pop();
    active.remove(node);
    finished.insert(node.to_owned());
    None
}

fn collect_const_names(
    expression: &SourceExpression,
    names: &BTreeSet<String>,
    output: &mut BTreeSet<String>,
) {
    match expression {
        SourceExpression::Name { name, .. } => {
            if names.contains(name) {
                output.insert(name.clone());
            }
        }
        SourceExpression::Unary { operand, .. }
        | SourceExpression::FieldAccess { base: operand, .. } => {
            collect_const_names(operand, names, output);
        }
        SourceExpression::Binary { left, right, .. }
        | SourceExpression::Vec2 {
            x: left, y: right, ..
        } => {
            collect_const_names(left, names, output);
            collect_const_names(right, names, output);
        }
        SourceExpression::Array { elements, .. } => {
            for element in elements {
                collect_const_names(element, names, output);
            }
        }
        SourceExpression::Object { entries, .. } => {
            for entry in entries {
                collect_const_names(&entry.value, names, output);
            }
        }
        SourceExpression::Index { base, index, .. } => {
            collect_const_names(base, names, output);
            collect_const_names(index, names, output);
        }
        SourceExpression::Choose {
            arms, else_value, ..
        } => {
            for arm in arms {
                collect_const_names(&arm.condition, names, output);
                collect_const_names(&arm.value, names, output);
            }
            collect_const_names(else_value, names, output);
        }
        SourceExpression::Call {
            callee, arguments, ..
        } => {
            if !matches!(callee.as_ref(), SourceExpression::Name { .. }) {
                collect_const_names(callee, names, output);
            }
            for argument in arguments {
                collect_const_names(argument, names, output);
            }
        }
        SourceExpression::Literal { .. } | SourceExpression::Reference { .. } => {}
    }
}

fn collect_function_calls_in_block(
    statements: &[FunctionStatement],
    names: &BTreeSet<String>,
    output: &mut BTreeSet<String>,
) {
    for statement in statements {
        match statement {
            FunctionStatement::Let(statement) => {
                collect_function_calls(&statement.initializer, names, output);
            }
            FunctionStatement::Return(statement) => {
                collect_function_calls(&statement.value, names, output);
            }
            FunctionStatement::If(statement) => {
                collect_function_calls(&statement.condition, names, output);
                collect_function_calls_in_block(&statement.then_branch, names, output);
                collect_function_calls_in_block(&statement.else_branch, names, output);
            }
        }
    }
}

fn collect_function_calls(
    expression: &SourceExpression,
    names: &BTreeSet<String>,
    output: &mut BTreeSet<String>,
) {
    match expression {
        SourceExpression::Call {
            callee, arguments, ..
        } => {
            if let SourceExpression::Name { name, .. } = callee.as_ref() {
                if names.contains(name) {
                    output.insert(name.clone());
                }
            } else {
                collect_function_calls(callee, names, output);
            }
            for argument in arguments {
                collect_function_calls(argument, names, output);
            }
        }
        SourceExpression::Unary { operand, .. }
        | SourceExpression::FieldAccess { base: operand, .. } => {
            collect_function_calls(operand, names, output);
        }
        SourceExpression::Binary { left, right, .. }
        | SourceExpression::Vec2 {
            x: left, y: right, ..
        } => {
            collect_function_calls(left, names, output);
            collect_function_calls(right, names, output);
        }
        SourceExpression::Array { elements, .. } => {
            for element in elements {
                collect_function_calls(element, names, output);
            }
        }
        SourceExpression::Object { entries, .. } => {
            for entry in entries {
                collect_function_calls(&entry.value, names, output);
            }
        }
        SourceExpression::Index { base, index, .. } => {
            collect_function_calls(base, names, output);
            collect_function_calls(index, names, output);
        }
        SourceExpression::Choose {
            arms, else_value, ..
        } => {
            for arm in arms {
                collect_function_calls(&arm.condition, names, output);
                collect_function_calls(&arm.value, names, output);
            }
            collect_function_calls(else_value, names, output);
        }
        SourceExpression::Literal { .. }
        | SourceExpression::Reference { .. }
        | SourceExpression::Name { .. } => {}
    }
}

#[allow(dead_code)]
fn _span_is_part_of_cycle_diagnostics(_: SourceSpan) {}
