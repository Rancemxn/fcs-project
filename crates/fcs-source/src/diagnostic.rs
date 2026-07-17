use std::fmt;

use crate::ast::SourceSpan;

/// A stable diagnostic category exposed by the FCS source API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiagnosticCode(&'static str);

impl DiagnosticCode {
    pub const DECODE_INVALID_UTF8: Self = Self("decode.invalid-utf8");
    pub const VERSION_MISSING_HEADER: Self = Self("version.missing-header");
    pub const VERSION_INVALID: Self = Self("version.invalid");
    pub const VERSION_UNSUPPORTED: Self = Self("version.unsupported");
    pub const SYNTAX_INVALID_TOKEN: Self = Self("syntax.invalid-token");
    pub const SYNTAX_UNCLOSED_COMMENT: Self = Self("syntax.unclosed-comment");
    pub const SYNTAX_UNCLOSED_STRING: Self = Self("syntax.unclosed-string");
    pub const SYNTAX_TRAILING_INPUT: Self = Self("syntax.trailing-input");
    pub const SYNTAX_MISPLACED_BLOCK: Self = Self("syntax.misplaced-block");
    pub const NAME_UNKNOWN: Self = Self("name.unknown");
    pub const NAME_DUPLICATE: Self = Self("name.duplicate");
    pub const NAME_SHADOWED: Self = Self("name.shadowed");
    pub const NAME_CYCLE: Self = Self("name.cycle");
    pub const TYPE_MISMATCH: Self = Self("type.mismatch");
    pub const TYPE_INVALID_OPERATION: Self = Self("type.invalid-operation");
    pub const TYPE_INVALID_CONVERSION: Self = Self("type.invalid-conversion");
    pub const SCHEMA_UNKNOWN_FIELD: Self = Self("schema.unknown-field");
    pub const SCHEMA_DUPLICATE_FIELD: Self = Self("schema.duplicate-field");
    pub const SCHEMA_MISSING_REQUIRED_FIELD: Self = Self("schema.missing-required-field");
    pub const SCHEMA_NON_CONSTRUCTIBLE: Self = Self("schema.non-constructible");
    pub const SCHEMA_COLLECTION_TYPE_MISMATCH: Self = Self("schema.collection-type-mismatch");
    pub const SCHEMA_DYNAMIC_FIELD_FORBIDDEN: Self = Self("schema.dynamic-field-forbidden");
    pub const COMPILE_TIME_NON_CONSTANT_CONDITION: Self =
        Self("compile-time.non-constant-condition");
    pub const COMPILE_TIME_INVALID_RANGE: Self = Self("compile-time.invalid-range");
    pub const COMPILE_TIME_ZERO_STEP: Self = Self("compile-time.zero-step");
    pub const COMPILE_TIME_NESTED_GENERATOR: Self = Self("compile-time.nested-generator");
    pub const COMPILE_TIME_MISPLACED_GENERATOR: Self = Self("compile-time.misplaced-generator");
    pub const COMPILE_TIME_BUDGET_EXCEEDED: Self = Self("compile-time.budget-exceeded");
    pub const NUMERIC_NON_FINITE: Self = Self("numeric.non-finite");
    pub const NUMERIC_DIVIDE_BY_ZERO: Self = Self("numeric.divide-by-zero");
    pub const NUMERIC_DOMAIN: Self = Self("numeric.domain");
    pub const NUMERIC_OVERFLOW: Self = Self("numeric.overflow");
    pub const TEMPO_INVALID: Self = Self("tempo.invalid");
    pub const TEMPO_NON_MONOTONIC: Self = Self("tempo.non-monotonic");
    pub const TRACK_INVALID_INTERVAL: Self = Self("track.invalid-interval");
    pub const TRACK_OVERLAP: Self = Self("track.overlap");
    pub const TRACK_REPLACE_CONFLICT: Self = Self("track.replace-conflict");
    pub const TRACK_GAP: Self = Self("track.gap");
    pub const TRACK_INVALID_EASING: Self = Self("track.invalid-easing");
    pub const GRAPH_UNKNOWN_PARENT: Self = Self("graph.unknown-parent");
    pub const GRAPH_CYCLE: Self = Self("graph.cycle");
    pub const NOTE_INVALID_HOLD: Self = Self("note.invalid-hold");
    pub const RESOURCE_UNKNOWN_REFERENCE: Self = Self("resource.unknown-reference");
    pub const RESOURCE_TYPE_MISMATCH: Self = Self("resource.type-mismatch");
    pub const RESOURCE_HASH_MISMATCH: Self = Self("resource.hash-mismatch");
    pub const RESOURCE_LIMIT_EXCEEDED: Self = Self("resource.limit-exceeded");
    pub const EXPRESSION_CYCLE: Self = Self("expression.cycle");
    pub const EXPRESSION_ENVIRONMENT_UNAVAILABLE: Self = Self("expression.environment-unavailable");
    pub const BAKING_ERROR_BUDGET_UNSATISFIED: Self = Self("baking.error-budget-unsatisfied");
    pub const EXTENSION_UNSUPPORTED_REQUIRED: Self = Self("extension.unsupported-required");
    pub const PROFILE_REQUIREMENT_MISSING: Self = Self("profile.requirement-missing");
    pub const REPAIR_APPLIED: Self = Self("repair.applied");
    pub const IMPLEMENTATION_FEATURE_UNAVAILABLE: Self = Self("implementation.feature-unavailable");

    /// Returns the stable dotted diagnostic code.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticStage {
    Decode,
    Parse,
    Elaborate,
    Canonical,
    Evaluate,
    Implementation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticLabel {
    span: SourceSpan,
    message: String,
}

impl DiagnosticLabel {
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn new(span: SourceSpan, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExpansionTraceKind {
    Const,
    Function,
    Template,
    Collection,
    Range,
    Generator,
    Index,
    Emit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionTraceFrame {
    kind: ExpansionTraceKind,
    subject: Option<String>,
    index: Option<usize>,
    emitted_type: Option<String>,
    span: Option<SourceSpan>,
}

impl ExpansionTraceFrame {
    pub const fn kind(&self) -> ExpansionTraceKind {
        self.kind
    }

    pub fn subject(&self) -> Option<&str> {
        self.subject.as_deref()
    }

    pub const fn index(&self) -> Option<usize> {
        self.index
    }

    pub fn emitted_type(&self) -> Option<&str> {
        self.emitted_type.as_deref()
    }

    pub const fn span(&self) -> Option<SourceSpan> {
        self.span
    }

    pub(crate) fn new(
        kind: ExpansionTraceKind,
        subject: Option<String>,
        index: Option<usize>,
        emitted_type: Option<String>,
        span: Option<SourceSpan>,
    ) -> Self {
        Self {
            kind,
            subject,
            index,
            emitted_type,
            span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetDetails {
    kind: String,
    limit: usize,
    observed: usize,
}

impl BudgetDetails {
    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub const fn limit(&self) -> usize {
        self.limit
    }

    pub const fn observed(&self) -> usize {
        self.observed
    }

    pub(crate) fn new(kind: impl Into<String>, limit: usize, observed: usize) -> Self {
        Self {
            kind: kind.into(),
            limit,
            observed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    code: DiagnosticCode,
    stage: DiagnosticStage,
    severity: DiagnosticSeverity,
    message: String,
    primary_span: SourceSpan,
    labels: Box<[DiagnosticLabel]>,
    expansion_trace: Box<[ExpansionTraceFrame]>,
    budget: Option<Box<BudgetDetails>>,
}

impl Diagnostic {
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    pub const fn stage(&self) -> DiagnosticStage {
        self.stage
    }

    pub const fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn primary_span(&self) -> SourceSpan {
        self.primary_span
    }

    pub fn labels(&self) -> &[DiagnosticLabel] {
        &self.labels
    }

    pub fn expansion_trace(&self) -> &[ExpansionTraceFrame] {
        &self.expansion_trace
    }

    pub fn budget(&self) -> Option<&BudgetDetails> {
        self.budget.as_deref()
    }

    pub(crate) fn new(
        code: DiagnosticCode,
        stage: DiagnosticStage,
        message: impl Into<String>,
        primary_span: SourceSpan,
    ) -> Self {
        Self {
            code,
            stage,
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            primary_span,
            labels: Vec::new().into_boxed_slice(),
            expansion_trace: Vec::new().into_boxed_slice(),
            budget: None,
        }
    }

    pub(crate) fn with_label(mut self, label: DiagnosticLabel) -> Self {
        let mut labels = self.labels.into_vec();
        labels.push(label);
        self.labels = labels.into_boxed_slice();
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_severity(mut self, severity: DiagnosticSeverity) -> Self {
        self.severity = severity;
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_trace_frame(mut self, frame: ExpansionTraceFrame) -> Self {
        let mut trace = self.expansion_trace.into_vec();
        trace.push(frame);
        self.expansion_trace = trace.into_boxed_slice();
        self
    }

    pub(crate) fn with_expansion_trace(mut self, trace: Vec<ExpansionTraceFrame>) -> Self {
        let mut frames = self.expansion_trace.into_vec();
        frames.extend(trace);
        self.expansion_trace = frames.into_boxed_slice();
        self
    }

    pub(crate) fn with_budget(
        mut self,
        kind: impl Into<String>,
        limit: usize,
        observed: usize,
    ) -> Self {
        self.budget = Some(Box::new(BudgetDetails::new(kind, limit, observed)));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOutput<T> {
    output: Option<T>,
    diagnostics: Vec<Diagnostic>,
}

impl<T> ParseOutput<T> {
    pub(crate) fn new(output: Option<T>, mut diagnostics: Vec<Diagnostic>) -> Self {
        diagnostics.sort_by(|left, right| {
            left.primary_span
                .start
                .cmp(&right.primary_span.start)
                .then_with(|| left.primary_span.end.cmp(&right.primary_span.end))
                .then_with(|| left.code.cmp(&right.code))
        });
        diagnostics.dedup_by(|right, left| {
            right.primary_span == left.primary_span && right.code == left.code
        });
        let output = diagnostics.is_empty().then_some(output).flatten();
        Self {
            output,
            diagnostics,
        }
    }

    pub fn output(&self) -> Option<&T> {
        self.output.as_ref()
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn into_result(self) -> Result<T, Vec<Diagnostic>> {
        match (self.output, self.diagnostics) {
            (Some(output), diagnostics) if diagnostics.is_empty() => Ok(output),
            (_, diagnostics) => Err(diagnostics),
        }
    }
}
