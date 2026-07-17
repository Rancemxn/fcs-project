//! Deterministic dependency-graph construction and cycle validation.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::ast::{
    Definition, DefinitionsBlock, EntityExpression, FunctionStatement, SchemaValue,
    SourceExpression, TemplateStatement,
};
use crate::diagnostic::ExpansionTraceKind;

use super::{DependencyTraceNode, ElaboratorError as Diagnostic};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NodeKind {
    Const,
    Function,
    Template,
}

impl NodeKind {
    const fn trace_kind(self) -> ExpansionTraceKind {
        match self {
            Self::Const => ExpansionTraceKind::Const,
            Self::Function => ExpansionTraceKind::Function,
            Self::Template => ExpansionTraceKind::Template,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NodeKey {
    kind: NodeKind,
    name: String,
}

#[derive(Debug, Default)]
struct DependencyGraph {
    spans: BTreeMap<NodeKey, crate::ast::SourceSpan>,
    edges: BTreeMap<NodeKey, BTreeSet<NodeKey>>,
}

impl DependencyGraph {
    fn add_node(&mut self, key: NodeKey, span: crate::ast::SourceSpan) {
        self.spans.insert(key.clone(), span);
        self.edges.entry(key).or_default();
    }

    fn add_edge(&mut self, from: NodeKey, to: NodeKey) {
        self.edges.entry(from).or_default().insert(to);
    }
}

#[derive(Debug, Default)]
struct DefinitionNames {
    consts: BTreeSet<String>,
    functions: BTreeSet<String>,
    templates: BTreeSet<String>,
}

impl DefinitionNames {
    fn collect(definitions: &DefinitionsBlock) -> Self {
        let mut names = Self::default();
        for definition in &definitions.declarations {
            match definition {
                Definition::Const(declaration) => {
                    names.consts.insert(declaration.name.clone());
                }
                Definition::Function(declaration) => {
                    names.functions.insert(declaration.name.clone());
                }
                Definition::Template(declaration) => {
                    names.templates.insert(declaration.name.clone());
                }
            }
        }
        names
    }
}

pub(super) fn reject_cycles(definitions: &DefinitionsBlock) -> Result<(), Diagnostic> {
    let names = DefinitionNames::collect(definitions);
    let mut graph = DependencyGraph::default();

    for definition in &definitions.declarations {
        let (key, span) = definition_identity(definition);
        graph.add_node(key, span);
    }
    for definition in &definitions.declarations {
        let (key, _) = definition_identity(definition);
        let mut dependencies = BTreeSet::new();
        collect_definition_dependencies(definition, key.kind, &names, &mut dependencies);
        for dependency in dependencies {
            graph.add_edge(key.clone(), dependency);
        }
    }

    let Some(cycle) = find_shortest_cycle(&graph) else {
        return Ok(());
    };
    let chain = cycle
        .into_iter()
        .map(|key| DependencyTraceNode {
            kind: key.kind.trace_kind(),
            name: key.name.clone(),
            span: graph.spans[&key],
        })
        .collect();
    Err(Diagnostic::RecursiveDependency { chain })
}

fn definition_identity(definition: &Definition) -> (NodeKey, crate::ast::SourceSpan) {
    match definition {
        Definition::Const(declaration) => (
            NodeKey {
                kind: NodeKind::Const,
                name: declaration.name.clone(),
            },
            declaration.name_span,
        ),
        Definition::Function(declaration) => (
            NodeKey {
                kind: NodeKind::Function,
                name: declaration.name.clone(),
            },
            declaration.name_span,
        ),
        Definition::Template(declaration) => (
            NodeKey {
                kind: NodeKind::Template,
                name: declaration.name.clone(),
            },
            declaration.name_span,
        ),
    }
}

fn collect_definition_dependencies(
    definition: &Definition,
    owner: NodeKind,
    names: &DefinitionNames,
    output: &mut BTreeSet<NodeKey>,
) {
    match definition {
        Definition::Const(declaration) => {
            collect_expression(&declaration.initializer, owner, names, output);
        }
        Definition::Function(declaration) => {
            collect_function_block(&declaration.body, owner, names, output);
        }
        Definition::Template(declaration) => {
            collect_template_block(&declaration.body, owner, names, output);
        }
    }
}

fn collect_function_block(
    statements: &[FunctionStatement],
    owner: NodeKind,
    names: &DefinitionNames,
    output: &mut BTreeSet<NodeKey>,
) {
    for statement in statements {
        match statement {
            FunctionStatement::Let(statement) => {
                collect_expression(&statement.initializer, owner, names, output);
            }
            FunctionStatement::Return(statement) => {
                collect_expression(&statement.value, owner, names, output);
            }
            FunctionStatement::If(statement) => {
                collect_expression(&statement.condition, owner, names, output);
                collect_function_block(&statement.then_branch, owner, names, output);
                collect_function_block(&statement.else_branch, owner, names, output);
            }
        }
    }
}

fn collect_template_block(
    statements: &[TemplateStatement],
    owner: NodeKind,
    names: &DefinitionNames,
    output: &mut BTreeSet<NodeKey>,
) {
    for statement in statements {
        match statement {
            TemplateStatement::Let(statement) => {
                collect_expression(&statement.initializer, owner, names, output);
            }
            TemplateStatement::If(statement) => {
                collect_expression(&statement.condition, owner, names, output);
                collect_template_block(&statement.then_branch, owner, names, output);
                collect_template_block(&statement.else_branch, owner, names, output);
            }
            TemplateStatement::Return(statement) => {
                collect_entity_expression(&statement.value, owner, names, output);
            }
        }
    }
}

fn collect_entity_expression(
    expression: &EntityExpression,
    owner: NodeKind,
    names: &DefinitionNames,
    output: &mut BTreeSet<NodeKey>,
) {
    match expression {
        EntityExpression::Constructor(constructor) => {
            for field in &constructor.fields {
                collect_expression(&field.value, owner, names, output);
            }
        }
        EntityExpression::SourceConstructor(constructor) => {
            for field in &constructor.fields {
                collect_schema_value(&field.value, owner, names, output);
            }
        }
        EntityExpression::Source(expression) => {
            collect_expression(expression, owner, names, output);
        }
        EntityExpression::With(expression) => {
            collect_entity_expression(&expression.base, owner, names, output);
            for field in &expression.fields {
                collect_expression(&field.value, owner, names, output);
            }
        }
    }
}

fn collect_schema_value(
    value: &SchemaValue,
    owner: NodeKind,
    names: &DefinitionNames,
    output: &mut BTreeSet<NodeKey>,
) {
    match value {
        SchemaValue::Expression(expression) => {
            collect_expression(expression, owner, names, output);
        }
        SchemaValue::CubicBezier { values, .. } => {
            for expression in values {
                collect_expression(expression, owner, names, output);
            }
        }
        SchemaValue::Interval { start, end, .. } => {
            collect_expression(start, owner, names, output);
            collect_expression(end, owner, names, output);
        }
    }
}

fn collect_expression(
    expression: &SourceExpression,
    owner: NodeKind,
    names: &DefinitionNames,
    output: &mut BTreeSet<NodeKey>,
) {
    match expression {
        SourceExpression::Name { name, .. } => {
            add_const_dependency(name, names, output);
        }
        SourceExpression::Call {
            callee, arguments, ..
        } => {
            if let SourceExpression::Name { name, .. } = callee.as_ref() {
                add_call_dependency(name, owner, names, output);
            } else {
                collect_expression(callee, owner, names, output);
            }
            for argument in arguments {
                collect_expression(argument, owner, names, output);
            }
        }
        SourceExpression::Unary { operand, .. }
        | SourceExpression::FieldAccess { base: operand, .. } => {
            collect_expression(operand, owner, names, output);
        }
        SourceExpression::Binary { left, right, .. }
        | SourceExpression::Vec2 {
            x: left, y: right, ..
        } => {
            collect_expression(left, owner, names, output);
            collect_expression(right, owner, names, output);
        }
        SourceExpression::Array { elements, .. } => {
            for element in elements {
                collect_expression(element, owner, names, output);
            }
        }
        SourceExpression::Object { entries, .. } => {
            for entry in entries {
                collect_expression(&entry.value, owner, names, output);
            }
        }
        SourceExpression::Index { base, index, .. } => {
            collect_expression(base, owner, names, output);
            collect_expression(index, owner, names, output);
        }
        SourceExpression::Choose {
            arms, else_value, ..
        } => {
            for arm in arms {
                collect_expression(&arm.condition, owner, names, output);
                collect_expression(&arm.value, owner, names, output);
            }
            collect_expression(else_value, owner, names, output);
        }
        SourceExpression::Literal { .. } | SourceExpression::Reference { .. } => {}
    }
}

fn add_const_dependency(name: &str, names: &DefinitionNames, output: &mut BTreeSet<NodeKey>) {
    if names.consts.contains(name) {
        output.insert(NodeKey {
            kind: NodeKind::Const,
            name: name.to_owned(),
        });
    }
}

fn add_call_dependency(
    name: &str,
    owner: NodeKind,
    names: &DefinitionNames,
    output: &mut BTreeSet<NodeKey>,
) {
    if names.functions.contains(name) {
        output.insert(NodeKey {
            kind: NodeKind::Function,
            name: name.to_owned(),
        });
    } else if owner == NodeKind::Template && names.templates.contains(name) {
        output.insert(NodeKey {
            kind: NodeKind::Template,
            name: name.to_owned(),
        });
    }
}

fn find_shortest_cycle(graph: &DependencyGraph) -> Option<Vec<NodeKey>> {
    let mut best = None;
    for start in graph.edges.keys() {
        if let Some(candidate) = shortest_cycle_from(start, graph)
            && best
                .as_ref()
                .is_none_or(|current: &Vec<NodeKey>| cycle_order(&candidate, current).is_lt())
        {
            best = Some(candidate);
        }
    }
    best
}

fn shortest_cycle_from(start: &NodeKey, graph: &DependencyGraph) -> Option<Vec<NodeKey>> {
    let mut queue = VecDeque::from([(start.clone(), vec![start.clone()])]);
    let mut visited = BTreeSet::from([start.clone()]);
    while let Some((current, path)) = queue.pop_front() {
        let Some(dependencies) = graph.edges.get(&current) else {
            continue;
        };
        for dependency in dependencies {
            if dependency == start {
                let mut cycle = path.clone();
                cycle.push(start.clone());
                return Some(cycle);
            }
            if visited.insert(dependency.clone()) {
                let mut next_path = path.clone();
                next_path.push(dependency.clone());
                queue.push_back((dependency.clone(), next_path));
            }
        }
    }
    None
}

fn cycle_order(left: &[NodeKey], right: &[NodeKey]) -> std::cmp::Ordering {
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}
