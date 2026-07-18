//! Type checking and pure compile-time evaluation for FCS 5 definitions.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

mod cycle;
mod entities;
mod eval;
mod generator;
mod resolve;
mod scope;

use crate::ast::{Definition, Document, ExpandedSourceDocument, SourceSpan, Type};
use crate::diagnostic::{
    DiagnosticCode, DiagnosticLabel, DiagnosticStage, ExpansionTraceFrame, ExpansionTraceKind,
};
use crate::schema::ConstructionSchema;

pub use crate::diagnostic::Diagnostic;
pub use generator::{GeneratorRange, GeneratorRangeError, evaluate_generator_range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompileTimeLimits {
    pub max_expansion_depth: usize,
    pub max_generated_nodes: usize,
    pub max_generator_iterations: usize,
    pub max_template_instances: usize,
    pub max_compile_time_operations: usize,
    pub max_expression_nodes: usize,
}

#[derive(Clone)]
pub(super) struct CompileTimeContext {
    state: Rc<RefCell<BudgetState>>,
}

#[derive(Clone)]
struct BudgetState {
    limits: CompileTimeLimits,
    generated_nodes: usize,
    generator_iterations: usize,
    template_instances: usize,
    compile_time_operations: usize,
    expression_nodes: usize,
    trace: Vec<ExpansionTraceFrame>,
}

impl CompileTimeContext {
    pub(super) fn new(limits: CompileTimeLimits) -> Self {
        Self {
            state: Rc::new(RefCell::new(BudgetState {
                limits,
                generated_nodes: 0,
                generator_iterations: 0,
                template_instances: 0,
                compile_time_operations: 0,
                expression_nodes: 0,
                trace: Vec::new(),
            })),
        }
    }

    pub(super) fn consume(
        &self,
        limit: &'static str,
        span: SourceSpan,
    ) -> Result<(), ElaboratorError> {
        let mut state = self.state.borrow_mut();
        let (observed, bound) = match limit {
            "max_generated_nodes" => {
                state.generated_nodes = state.generated_nodes.saturating_add(1);
                (state.generated_nodes, state.limits.max_generated_nodes)
            }
            "max_generator_iterations" => {
                state.generator_iterations = state.generator_iterations.saturating_add(1);
                (
                    state.generator_iterations,
                    state.limits.max_generator_iterations,
                )
            }
            "max_template_instances" => {
                state.template_instances = state.template_instances.saturating_add(1);
                (
                    state.template_instances,
                    state.limits.max_template_instances,
                )
            }
            "max_compile_time_operations" => {
                state.compile_time_operations = state.compile_time_operations.saturating_add(1);
                (
                    state.compile_time_operations,
                    state.limits.max_compile_time_operations,
                )
            }
            "max_expression_nodes" => {
                state.expression_nodes = state.expression_nodes.saturating_add(1);
                (state.expression_nodes, state.limits.max_expression_nodes)
            }
            _ => return Ok(()),
        };
        if observed > bound {
            Err(ElaboratorError::LimitExceeded {
                limit,
                bound,
                observed,
                span,
                trace: state.trace.clone(),
            })
        } else {
            Ok(())
        }
    }

    pub(super) fn push_trace(&self, frame: ExpansionTraceFrame) {
        self.state.borrow_mut().trace.push(frame);
    }

    pub(super) fn pop_trace(&self) {
        self.state.borrow_mut().trace.pop();
    }

    pub(super) fn check_expansion_depth(
        &self,
        depth: usize,
        span: SourceSpan,
    ) -> Result<(), ElaboratorError> {
        let state = self.state.borrow();
        if depth > state.limits.max_expansion_depth {
            Err(ElaboratorError::LimitExceeded {
                limit: "max_expansion_depth",
                bound: state.limits.max_expansion_depth,
                observed: depth,
                span,
                trace: state.trace.clone(),
            })
        } else {
            Ok(())
        }
    }
}

impl Default for CompileTimeLimits {
    fn default() -> Self {
        Self {
            max_expansion_depth: 128,
            max_generated_nodes: 100_000,
            max_generator_iterations: 100_000,
            max_template_instances: 10_000,
            max_compile_time_operations: 1_000_000,
            max_expression_nodes: 100_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ElaboratorError {
    FeatureUnavailable {
        feature: &'static str,
        span: SourceSpan,
    },
    ShadowedBinding {
        name: String,
        span: SourceSpan,
        previous_span: SourceSpan,
    },
    DuplicateBinding {
        name: String,
        span: SourceSpan,
        previous_span: SourceSpan,
    },
    DuplicateLineId {
        name: String,
        span: SourceSpan,
        previous_span: SourceSpan,
    },
    TypeMismatch {
        expected: Type,
        actual: Type,
        span: SourceSpan,
    },
    InvalidConversion {
        expected: Type,
        actual: Type,
        span: SourceSpan,
    },
    UnknownName {
        name: String,
        span: SourceSpan,
    },
    RecursiveDependency {
        chain: Vec<DependencyTraceNode>,
    },
    MissingReturn {
        function: String,
        span: SourceSpan,
    },
    WrongArity {
        callee: String,
        expected: usize,
        actual: usize,
        span: SourceSpan,
    },
    UnknownEntityField {
        entity: Type,
        field: String,
        span: SourceSpan,
    },
    DuplicateEntityField {
        field: String,
        span: SourceSpan,
        previous_span: SourceSpan,
    },
    MissingRequiredField {
        entity: Type,
        field: String,
        span: SourceSpan,
    },
    NonConstructibleEntity {
        entity: Type,
        span: SourceSpan,
    },
    UnknownTemplate {
        name: String,
        span: SourceSpan,
    },
    UnknownCollection {
        name: String,
        span: SourceSpan,
    },
    CollectionTypeMismatch {
        collection: String,
        expected: Type,
        actual: Type,
        span: SourceSpan,
    },
    DynamicFieldForbidden {
        field: String,
        span: SourceSpan,
    },
    InvalidGeneratorRange {
        span: SourceSpan,
        message: &'static str,
    },
    ZeroGeneratorStep {
        span: SourceSpan,
    },
    NonConstantStructuralCondition {
        span: SourceSpan,
    },
    NumericOverflow {
        span: SourceSpan,
    },
    DivideByZero {
        span: SourceSpan,
    },
    NonFinite {
        span: SourceSpan,
    },
    NumericDomain {
        span: SourceSpan,
    },
    InvalidOperation {
        message: &'static str,
        span: SourceSpan,
    },
    LimitExceeded {
        limit: &'static str,
        bound: usize,
        observed: usize,
        span: SourceSpan,
        trace: Vec<ExpansionTraceFrame>,
    },
}

pub fn elaborate(
    document: &Document,
    schema: &ConstructionSchema,
    limits: CompileTimeLimits,
) -> Result<ExpandedSourceDocument, Vec<Diagnostic>> {
    elaborate_inner(document, schema, limits).map_err(|error| vec![error.into_diagnostic()])
}

/// Evaluates one metadata expression in the same pure compile-time environment
/// used by source definitions. Container expressions and metadata references are
/// handled by the canonical adapter; this helper supplies scalar arithmetic and
/// constant/function semantics without exposing the elaborator internals.
pub(crate) fn evaluate_metadata_expression(
    expression: &crate::ast::SourceExpression,
    definitions: Option<&crate::ast::DefinitionsBlock>,
) -> Result<crate::ast::TypedValue, Diagnostic> {
    let context = CompileTimeContext::new(CompileTimeLimits::default());
    eval::evaluate_with_context(
        expression,
        definitions,
        &BTreeMap::new(),
        crate::schema::phase2_schema(),
        &context,
    )
    .map_err(ElaboratorError::into_diagnostic)
}

fn elaborate_inner(
    document: &Document,
    schema: &ConstructionSchema,
    limits: CompileTimeLimits,
) -> Result<ExpandedSourceDocument, ElaboratorError> {
    preflight_names(document)?;
    resolve::check_document(document)?;
    let context = CompileTimeContext::new(limits);
    if let Some(definitions) = &document.definitions {
        cycle::reject_cycles(definitions)?;
        eval::check_and_evaluate_with_context(definitions, schema, &context)?;
    }
    let collections = entities::expand_collections(document, schema, context)?;
    ExpandedSourceDocument::try_from_collections(
        document.source_version.clone(),
        document.profile,
        document.tempo_map.clone(),
        collections,
    )
    .map_err(|violation| ElaboratorError::InvalidOperation {
        message: violation.message(),
        span: SourceSpan::new(0, 0),
    })
}

fn preflight_names(document: &Document) -> Result<(), ElaboratorError> {
    let mut names = BTreeMap::<String, SourceSpan>::new();
    if let Some(definitions) = &document.definitions {
        for definition in &definitions.declarations {
            let (name, span) = match definition {
                Definition::Const(declaration) => (&declaration.name, declaration.name_span),
                Definition::Function(declaration) => (&declaration.name, declaration.name_span),
                Definition::Template(declaration) => (&declaration.name, declaration.name_span),
            };
            if let Some(previous_span) = names.insert(name.clone(), span) {
                return Err(ElaboratorError::DuplicateBinding {
                    name: name.clone(),
                    span,
                    previous_span,
                });
            }
        }
    }
    Ok(())
}

impl ElaboratorError {
    fn into_diagnostic(self) -> Diagnostic {
        match self {
            Self::FeatureUnavailable { feature, span } => Diagnostic::new(
                DiagnosticCode::IMPLEMENTATION_FEATURE_UNAVAILABLE,
                DiagnosticStage::Implementation,
                format!("{feature} elaboration is not available in this implementation"),
                span,
            ),
            Self::ShadowedBinding {
                name,
                span,
                previous_span,
            } => Diagnostic::new(
                DiagnosticCode::NAME_SHADOWED,
                DiagnosticStage::Elaborate,
                format!("binding {name} shadows an enclosing binding"),
                span,
            )
            .with_label(DiagnosticLabel::new(previous_span, "previous binding")),
            Self::DuplicateBinding {
                name,
                span,
                previous_span,
            } => Diagnostic::new(
                DiagnosticCode::NAME_DUPLICATE,
                DiagnosticStage::Elaborate,
                format!("binding {name} is declared more than once in this scope"),
                span,
            )
            .with_label(DiagnosticLabel::new(previous_span, "previous binding")),
            Self::DuplicateLineId {
                name,
                span,
                previous_span,
            } => Diagnostic::new(
                DiagnosticCode::NAME_DUPLICATE,
                DiagnosticStage::Elaborate,
                format!("Line ID {name} is declared more than once"),
                span,
            )
            .with_label(DiagnosticLabel::new(previous_span, "previous Line ID")),
            Self::TypeMismatch {
                expected,
                actual,
                span,
            } => Diagnostic::new(
                DiagnosticCode::TYPE_MISMATCH,
                DiagnosticStage::Elaborate,
                format!("expected type {expected}, found {actual}"),
                span,
            ),
            Self::InvalidConversion {
                expected,
                actual,
                span,
            } => Diagnostic::new(
                DiagnosticCode::TYPE_INVALID_CONVERSION,
                DiagnosticStage::Elaborate,
                format!("cannot convert {actual} to {expected}"),
                span,
            ),
            Self::UnknownName { name, span }
            | Self::UnknownTemplate { name, span }
            | Self::UnknownCollection { name, span } => Diagnostic::new(
                DiagnosticCode::NAME_UNKNOWN,
                DiagnosticStage::Elaborate,
                format!("unknown name {name}"),
                span,
            ),
            Self::RecursiveDependency { chain } => {
                let primary_span = chain
                    .first()
                    .map(|node| node.span)
                    .unwrap_or(SourceSpan::new(0, 0));
                let edge_labels = chain
                    .windows(2)
                    .flat_map(|window| window[0].edge_spans.iter().copied())
                    .enumerate()
                    .map(|(index, span)| {
                        DiagnosticLabel::new(span, format!("dependency edge {}", index + 1))
                    })
                    .collect::<Vec<_>>();
                let trace = chain
                    .into_iter()
                    .map(|node| {
                        ExpansionTraceFrame::new(
                            node.kind,
                            Some(node.name),
                            None,
                            None,
                            Some(node.span),
                        )
                    })
                    .collect();
                let diagnostic = Diagnostic::new(
                    DiagnosticCode::NAME_CYCLE,
                    DiagnosticStage::Elaborate,
                    "cyclic name expansion",
                    primary_span,
                );
                edge_labels
                    .into_iter()
                    .fold(diagnostic, |diagnostic, label| diagnostic.with_label(label))
                    .with_expansion_trace(trace)
            }
            Self::MissingReturn { function, span } => Diagnostic::new(
                DiagnosticCode::TYPE_MISMATCH,
                DiagnosticStage::Elaborate,
                format!("function {function} does not return on every path"),
                span,
            ),
            Self::WrongArity {
                callee,
                expected,
                actual,
                span,
            } => Diagnostic::new(
                DiagnosticCode::TYPE_MISMATCH,
                DiagnosticStage::Elaborate,
                format!("{callee} expects {expected} arguments but received {actual}"),
                span,
            ),
            Self::UnknownEntityField {
                entity,
                field,
                span,
            } => Diagnostic::new(
                DiagnosticCode::SCHEMA_UNKNOWN_FIELD,
                DiagnosticStage::Elaborate,
                format!("{entity} has no field {field}"),
                span,
            ),
            Self::DuplicateEntityField {
                field,
                span,
                previous_span,
            } => Diagnostic::new(
                DiagnosticCode::SCHEMA_DUPLICATE_FIELD,
                DiagnosticStage::Elaborate,
                format!("field {field} is assigned more than once"),
                span,
            )
            .with_label(DiagnosticLabel::new(
                previous_span,
                "previous field assignment",
            )),
            Self::MissingRequiredField {
                entity,
                field,
                span,
            } => Diagnostic::new(
                DiagnosticCode::SCHEMA_MISSING_REQUIRED_FIELD,
                DiagnosticStage::Elaborate,
                format!("{entity} is missing required field {field}"),
                span,
            ),
            Self::NonConstructibleEntity { entity, span } => Diagnostic::new(
                DiagnosticCode::SCHEMA_NON_CONSTRUCTIBLE,
                DiagnosticStage::Elaborate,
                format!("{entity} is not constructible in this schema"),
                span,
            ),
            Self::CollectionTypeMismatch {
                collection,
                expected,
                actual,
                span,
            } => Diagnostic::new(
                DiagnosticCode::SCHEMA_COLLECTION_TYPE_MISMATCH,
                DiagnosticStage::Elaborate,
                format!("collection {collection} expects {expected}, found {actual}"),
                span,
            ),
            Self::DynamicFieldForbidden { field, span } => Diagnostic::new(
                DiagnosticCode::SCHEMA_DYNAMIC_FIELD_FORBIDDEN,
                DiagnosticStage::Elaborate,
                format!("field {field} cannot depend on a runtime expression"),
                span,
            ),
            Self::InvalidGeneratorRange { span, message } => Diagnostic::new(
                DiagnosticCode::COMPILE_TIME_INVALID_RANGE,
                DiagnosticStage::Elaborate,
                message,
                span,
            ),
            Self::ZeroGeneratorStep { span } => Diagnostic::new(
                DiagnosticCode::COMPILE_TIME_ZERO_STEP,
                DiagnosticStage::Elaborate,
                "generator range step must not be zero",
                span,
            ),
            Self::NonConstantStructuralCondition { span } => Diagnostic::new(
                DiagnosticCode::COMPILE_TIME_NON_CONSTANT_CONDITION,
                DiagnosticStage::Elaborate,
                "structural condition is not compile-time constant",
                span,
            ),
            Self::NumericOverflow { span } => Diagnostic::new(
                DiagnosticCode::NUMERIC_OVERFLOW,
                DiagnosticStage::Elaborate,
                "integer magnitude is outside the signed 64-bit range",
                span,
            ),
            Self::DivideByZero { span } => Diagnostic::new(
                DiagnosticCode::NUMERIC_DIVIDE_BY_ZERO,
                DiagnosticStage::Elaborate,
                "division or remainder by zero",
                span,
            ),
            Self::NonFinite { span } => Diagnostic::new(
                DiagnosticCode::NUMERIC_NON_FINITE,
                DiagnosticStage::Elaborate,
                "compile-time arithmetic produced a non-finite value",
                span,
            ),
            Self::NumericDomain { span } => Diagnostic::new(
                DiagnosticCode::NUMERIC_DOMAIN,
                DiagnosticStage::Elaborate,
                "compile-time value is outside the builtin domain",
                span,
            ),
            Self::InvalidOperation { message, span } => Diagnostic::new(
                DiagnosticCode::TYPE_INVALID_OPERATION,
                DiagnosticStage::Elaborate,
                message,
                span,
            ),
            Self::LimitExceeded {
                limit,
                bound,
                observed,
                span,
                trace,
            } => Diagnostic::new(
                DiagnosticCode::COMPILE_TIME_BUDGET_EXCEEDED,
                DiagnosticStage::Evaluate,
                format!("compile-time budget {limit} was exceeded"),
                span,
            )
            .with_budget(limit, bound, observed)
            .with_expansion_trace(trace),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DependencyTraceNode {
    pub(super) kind: ExpansionTraceKind,
    pub(super) name: String,
    pub(super) span: SourceSpan,
    pub(super) edge_spans: Vec<SourceSpan>,
}
