//! Type checking and pure compile-time evaluation for FCS 5 definitions.

mod cycle;
mod entities;
mod eval;
mod scope;

use crate::ast::{Document, ExpandedSourceDocument, SourceSpan, Type};
use crate::schema::ConstructionSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompileTimeLimits {
    pub max_expansion_depth: usize,
    pub max_generated_nodes: usize,
    pub max_generator_iterations: usize,
    pub max_template_instances: usize,
    pub max_compile_time_operations: usize,
    pub max_expression_nodes: usize,
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
pub enum Diagnostic {
    FeatureUnavailable {
        feature: &'static str,
        span: SourceSpan,
    },
    ShadowedBinding {
        name: String,
        span: SourceSpan,
        previous_span: SourceSpan,
    },
    TypeMismatch {
        expected: Type,
        actual: Type,
        span: SourceSpan,
    },
    UnknownName {
        name: String,
        span: SourceSpan,
    },
    RecursiveConst {
        chain: Vec<String>,
        span: SourceSpan,
    },
    RecursiveFunction {
        chain: Vec<String>,
        span: SourceSpan,
    },
    MissingReturn {
        function: String,
        span: SourceSpan,
    },
    InvalidReturn {
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
    NonConstantStructuralCondition {
        span: SourceSpan,
    },
    RecursiveTemplate {
        chain: Vec<String>,
        span: SourceSpan,
    },
    InvalidOperation {
        message: &'static str,
        span: SourceSpan,
    },
    LimitExceeded {
        limit: &'static str,
        span: SourceSpan,
    },
}

pub fn elaborate(
    document: &Document,
    schema: &ConstructionSchema,
    limits: CompileTimeLimits,
) -> Result<ExpandedSourceDocument, Diagnostic> {
    if let Some(definitions) = &document.definitions {
        cycle::reject_cycles(definitions)?;
        eval::check_and_evaluate(definitions, limits)?;
    }
    let collections = entities::expand_collections(document, schema, limits)?;
    Ok(ExpandedSourceDocument::from_collections(
        document.source_version,
        document.profile,
        document.tempo_map.clone(),
        collections,
    ))
}
