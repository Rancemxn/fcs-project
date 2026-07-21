//! Deterministic ConversionReport and RepairRecord model.
//!
//! I5.7 owns the source-free report/repair data model, status aggregation,
//! deterministic entry ordering, and repair-record attachment to
//! DistributionMetadata. Importer execution, FCBC section encoding, and CLI
//! converter assembly remain later stages.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::{CanonicalValue, LogicalSourceLocator, MappingRuleRef, SemanticStatus};

/// Conversion §3.3 repair-mode policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairMode {
    enabled: bool,
    authorized_rules: Vec<MappingRuleRef>,
}

impl RepairMode {
    pub fn new(enabled: bool, authorized_rules: impl IntoIterator<Item = MappingRuleRef>) -> Self {
        let mut seen = BTreeSet::new();
        let authorized_rules = authorized_rules
            .into_iter()
            .filter(|rule| seen.insert(rule.clone()))
            .collect();
        Self {
            enabled,
            authorized_rules,
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            authorized_rules: Vec::new(),
        }
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn authorized_rules(&self) -> &[MappingRuleRef] {
        &self.authorized_rules
    }

    pub fn authorizes(&self, rule: &MappingRuleRef) -> bool {
        self.enabled
            && self
                .authorized_rules
                .iter()
                .any(|authorized| authorized.as_str() == rule.as_str())
    }
}

/// Conversion §7.1 top-level status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConversionStatus {
    Lossless,
    Equivalent,
    Approximate,
    PreservedOnly,
    Repaired,
    Unsupported,
    Failed,
}

impl ConversionStatus {
    pub const ALL: [Self; 7] = [
        Self::Lossless,
        Self::Equivalent,
        Self::Approximate,
        Self::PreservedOnly,
        Self::Repaired,
        Self::Unsupported,
        Self::Failed,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lossless => "lossless",
            Self::Equivalent => "equivalent",
            Self::Approximate => "approximate",
            Self::PreservedOnly => "preserved-only",
            Self::Repaired => "repaired",
            Self::Unsupported => "unsupported",
            Self::Failed => "failed",
        }
    }

    /// Lower numbers win aggregation (Conversion §7.1).
    pub const fn aggregation_rank(self) -> u8 {
        match self {
            Self::Failed => 0,
            Self::Unsupported => 1,
            Self::Repaired => 2,
            Self::PreservedOnly => 3,
            Self::Approximate => 4,
            Self::Equivalent => 5,
            Self::Lossless => 6,
        }
    }

    pub fn parse(spelling: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == spelling)
    }
}

impl fmt::Display for ConversionStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Conversion §7.2 severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConversionSeverity {
    Info,
    Warning,
    Error,
}

impl ConversionSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// Conversion §7.2 domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConversionDomain {
    Timing,
    Gameplay,
    Motion,
    Scroll,
    Presentation,
    Resource,
    Metadata,
    Syntax,
    Profile,
    Package,
}

impl ConversionDomain {
    pub const ALL: [Self; 10] = [
        Self::Timing,
        Self::Gameplay,
        Self::Motion,
        Self::Scroll,
        Self::Presentation,
        Self::Resource,
        Self::Metadata,
        Self::Syntax,
        Self::Profile,
        Self::Package,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Timing => "timing",
            Self::Gameplay => "gameplay",
            Self::Motion => "motion",
            Self::Scroll => "scroll",
            Self::Presentation => "presentation",
            Self::Resource => "resource",
            Self::Metadata => "metadata",
            Self::Syntax => "syntax",
            Self::Profile => "profile",
            Self::Package => "package",
        }
    }

    pub fn parse(spelling: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|domain| domain.as_str() == spelling)
    }
}

/// Conversion policy from Conversion §6.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConversionPolicy {
    Semantic,
    Roundtrip,
    Strict,
}

impl ConversionPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Semantic => "semantic",
            Self::Roundtrip => "roundtrip",
            Self::Strict => "strict",
        }
    }
}

/// Stable phase key used for deterministic entry ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConversionPhase {
    Parse = 0,
    ProfileSelection = 1,
    Repair = 2,
    Lowering = 3,
    CapabilityNegotiation = 4,
    Export = 5,
    ReparseCompare = 6,
}

impl ConversionPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::ProfileSelection => "profile-selection",
            Self::Repair => "repair",
            Self::Lowering => "lowering",
            Self::CapabilityNegotiation => "capability-negotiation",
            Self::Export => "export",
            Self::ReparseCompare => "reparse-compare",
        }
    }
}

/// One deterministic conversion entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ConversionEntry {
    id: String,
    category: String,
    domain: ConversionDomain,
    severity: ConversionSeverity,
    semantic_status: SemanticStatus,
    phase: ConversionPhase,
    source_locator: Option<LogicalSourceLocator>,
    target_locator: Option<LogicalSourceLocator>,
    entity_id: Option<String>,
    field_key: Option<String>,
    rule: Option<MappingRuleRef>,
    source_value: Option<CanonicalValue>,
    interpreted_value: Option<CanonicalValue>,
    canonical_value: Option<CanonicalValue>,
    target_value: Option<CanonicalValue>,
    message: String,
    dependencies: Vec<String>,
}

impl ConversionEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        category: impl Into<String>,
        domain: ConversionDomain,
        severity: ConversionSeverity,
        semantic_status: SemanticStatus,
        phase: ConversionPhase,
        source_locator: Option<LogicalSourceLocator>,
        target_locator: Option<LogicalSourceLocator>,
        entity_id: Option<String>,
        field_key: Option<String>,
        rule: Option<MappingRuleRef>,
        source_value: Option<CanonicalValue>,
        interpreted_value: Option<CanonicalValue>,
        canonical_value: Option<CanonicalValue>,
        target_value: Option<CanonicalValue>,
        message: impl Into<String>,
        dependencies: impl IntoIterator<Item = String>,
    ) -> Result<Self, ReportError> {
        let id = id.into();
        let category = category.into();
        if id.is_empty() {
            return Err(ReportError::EmptyEntryId);
        }
        if category.is_empty() {
            return Err(ReportError::EmptyCategory);
        }
        if !category.is_ascii() || category.chars().any(|ch| ch.is_ascii_control()) {
            return Err(ReportError::InvalidCategory(category));
        }
        Ok(Self {
            id,
            category,
            domain,
            severity,
            semantic_status,
            phase,
            source_locator,
            target_locator,
            entity_id,
            field_key,
            rule,
            source_value,
            interpreted_value,
            canonical_value,
            target_value,
            message: message.into(),
            dependencies: dependencies.into_iter().collect(),
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn category(&self) -> &str {
        &self.category
    }

    pub const fn domain(&self) -> ConversionDomain {
        self.domain
    }

    pub const fn severity(&self) -> ConversionSeverity {
        self.severity
    }

    pub const fn semantic_status(&self) -> SemanticStatus {
        self.semantic_status
    }

    pub const fn phase(&self) -> ConversionPhase {
        self.phase
    }

    pub fn source_locator(&self) -> Option<&LogicalSourceLocator> {
        self.source_locator.as_ref()
    }

    pub fn target_locator(&self) -> Option<&LogicalSourceLocator> {
        self.target_locator.as_ref()
    }

    pub fn entity_id(&self) -> Option<&str> {
        self.entity_id.as_deref()
    }

    pub fn field_key(&self) -> Option<&str> {
        self.field_key.as_deref()
    }

    pub fn rule(&self) -> Option<&MappingRuleRef> {
        self.rule.as_ref()
    }

    pub fn source_value(&self) -> Option<&CanonicalValue> {
        self.source_value.as_ref()
    }

    pub fn interpreted_value(&self) -> Option<&CanonicalValue> {
        self.interpreted_value.as_ref()
    }

    pub fn canonical_value(&self) -> Option<&CanonicalValue> {
        self.canonical_value.as_ref()
    }

    pub fn target_value(&self) -> Option<&CanonicalValue> {
        self.target_value.as_ref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    fn sort_key(&self) -> (u8, &str, &str, &str, &str) {
        (
            self.phase as u8,
            self.entity_id.as_deref().unwrap_or(""),
            self.field_key.as_deref().unwrap_or(""),
            self.rule.as_ref().map(MappingRuleRef::as_str).unwrap_or(""),
            self.id.as_str(),
        )
    }
}

/// One authorized repair application record.
#[derive(Debug, Clone, PartialEq)]
pub struct RepairRecord {
    id: String,
    source_locator: LogicalSourceLocator,
    diagnostic_category: String,
    action: String,
    rule: MappingRuleRef,
    old_value: CanonicalValue,
    new_value: CanonicalValue,
    semantic_impact: String,
}

impl RepairRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        source_locator: LogicalSourceLocator,
        diagnostic_category: impl Into<String>,
        action: impl Into<String>,
        rule: MappingRuleRef,
        old_value: CanonicalValue,
        new_value: CanonicalValue,
        semantic_impact: impl Into<String>,
    ) -> Result<Self, ReportError> {
        let id = id.into();
        let diagnostic_category = diagnostic_category.into();
        let action = action.into();
        let semantic_impact = semantic_impact.into();
        if id.is_empty() {
            return Err(ReportError::EmptyRepairId);
        }
        if diagnostic_category.is_empty() {
            return Err(ReportError::EmptyCategory);
        }
        if action.is_empty() {
            return Err(ReportError::EmptyRepairAction);
        }
        if semantic_impact.is_empty() {
            return Err(ReportError::EmptySemanticImpact);
        }
        Ok(Self {
            id,
            source_locator,
            diagnostic_category,
            action,
            rule,
            old_value,
            new_value,
            semantic_impact,
        })
    }

    pub fn authorize(self, mode: &RepairMode) -> Result<Self, ReportError> {
        if mode.authorizes(&self.rule) {
            Ok(self)
        } else {
            Err(ReportError::RepairNotAuthorized {
                rule_id: self.rule.as_str().to_owned(),
            })
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn source_locator(&self) -> &LogicalSourceLocator {
        &self.source_locator
    }

    pub fn diagnostic_category(&self) -> &str {
        &self.diagnostic_category
    }

    pub fn action(&self) -> &str {
        &self.action
    }

    pub fn rule(&self) -> &MappingRuleRef {
        &self.rule
    }

    pub const fn old_value(&self) -> &CanonicalValue {
        &self.old_value
    }

    pub const fn new_value(&self) -> &CanonicalValue {
        &self.new_value
    }

    pub fn semantic_impact(&self) -> &str {
        &self.semantic_impact
    }
}

/// Count summary by severity/status/category/domain.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConversionReportSummary {
    by_severity: BTreeMap<String, u64>,
    by_status: BTreeMap<String, u64>,
    by_category: BTreeMap<String, u64>,
    by_domain: BTreeMap<String, u64>,
    repair_count: u64,
    drop_count: u64,
}

impl ConversionReportSummary {
    pub fn by_severity(&self) -> &BTreeMap<String, u64> {
        &self.by_severity
    }

    pub fn by_status(&self) -> &BTreeMap<String, u64> {
        &self.by_status
    }

    pub fn by_category(&self) -> &BTreeMap<String, u64> {
        &self.by_category
    }

    pub fn by_domain(&self) -> &BTreeMap<String, u64> {
        &self.by_domain
    }

    pub const fn repair_count(&self) -> u64 {
        self.repair_count
    }

    pub const fn drop_count(&self) -> u64 {
        self.drop_count
    }
}

/// Deterministic ConversionReport product.
#[derive(Debug, Clone, PartialEq)]
pub struct ConversionReport {
    specification_version: String,
    operation_id: String,
    conversion_policy: ConversionPolicy,
    repair_mode: RepairMode,
    status: ConversionStatus,
    entries: Vec<ConversionEntry>,
    repairs: Vec<RepairRecord>,
    summary: ConversionReportSummary,
    output_hash: Option<String>,
}

impl ConversionReport {
    pub const SPECIFICATION_VERSION: &'static str = "1.0.0";

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        operation_id: impl Into<String>,
        conversion_policy: ConversionPolicy,
        repair_mode: RepairMode,
        mut entries: Vec<ConversionEntry>,
        mut repairs: Vec<RepairRecord>,
        status_signals: impl IntoIterator<Item = ConversionStatus>,
        output_hash: Option<String>,
    ) -> Result<Self, ReportError> {
        let operation_id = operation_id.into();
        if operation_id.is_empty() {
            return Err(ReportError::EmptyOperationId);
        }
        if let Some(hash) = &output_hash {
            validate_sha256_lower_hex(hash)?;
        }

        let mut seen_entry_ids = BTreeSet::new();
        for entry in &entries {
            if !seen_entry_ids.insert(entry.id().to_owned()) {
                return Err(ReportError::DuplicateEntryId(entry.id().to_owned()));
            }
        }
        let mut seen_repair_ids = BTreeSet::new();
        for repair in &repairs {
            if !seen_repair_ids.insert(repair.id().to_owned()) {
                return Err(ReportError::DuplicateRepairId(repair.id().to_owned()));
            }
            if !repair_mode.authorizes(repair.rule()) {
                return Err(ReportError::RepairNotAuthorized {
                    rule_id: repair.rule().as_str().to_owned(),
                });
            }
        }

        entries.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        let mut status = ConversionStatus::Lossless;
        for signal in status_signals {
            if signal.aggregation_rank() < status.aggregation_rank() {
                status = signal;
            }
        }
        for entry in &entries {
            let signal = match entry.semantic_status() {
                SemanticStatus::Unsupported => Some(ConversionStatus::Unsupported),
                SemanticStatus::Repaired => Some(ConversionStatus::Repaired),
                SemanticStatus::Approximated | SemanticStatus::Dropped => {
                    Some(ConversionStatus::Approximate)
                }
                _ => None,
            };
            if let Some(signal) = signal
                && signal.aggregation_rank() < status.aggregation_rank()
            {
                status = signal;
            }
        }
        if !repairs.is_empty()
            && status.aggregation_rank() > ConversionStatus::Repaired.aggregation_rank()
        {
            status = ConversionStatus::Repaired;
        }
        if matches!(status, ConversionStatus::Lossless) && !repairs.is_empty() {
            status = ConversionStatus::Repaired;
        }

        let summary = summarize(&entries, &repairs, status);
        Ok(Self {
            specification_version: Self::SPECIFICATION_VERSION.into(),
            operation_id,
            conversion_policy,
            repair_mode,
            status,
            entries,
            repairs,
            summary,
            output_hash,
        })
    }

    pub fn specification_version(&self) -> &str {
        &self.specification_version
    }

    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub const fn conversion_policy(&self) -> ConversionPolicy {
        self.conversion_policy
    }

    pub fn repair_mode(&self) -> &RepairMode {
        &self.repair_mode
    }

    pub const fn status(&self) -> ConversionStatus {
        self.status
    }

    pub fn entries(&self) -> &[ConversionEntry] {
        &self.entries
    }

    pub fn repairs(&self) -> &[RepairRecord] {
        &self.repairs
    }

    pub fn summary(&self) -> &ConversionReportSummary {
        &self.summary
    }

    pub fn output_hash(&self) -> Option<&str> {
        self.output_hash.as_deref()
    }
}

fn summarize(
    entries: &[ConversionEntry],
    repairs: &[RepairRecord],
    status: ConversionStatus,
) -> ConversionReportSummary {
    let mut summary = ConversionReportSummary::default();
    *summary
        .by_status
        .entry(status.as_str().to_owned())
        .or_default() += 1;
    for entry in entries {
        *summary
            .by_severity
            .entry(entry.severity().as_str().to_owned())
            .or_default() += 1;
        *summary
            .by_category
            .entry(entry.category().to_owned())
            .or_default() += 1;
        *summary
            .by_domain
            .entry(entry.domain().as_str().to_owned())
            .or_default() += 1;
        if entry.semantic_status() == SemanticStatus::Dropped {
            summary.drop_count += 1;
        }
    }
    summary.repair_count = repairs.len() as u64;
    summary
}

fn validate_sha256_lower_hex(digest: &str) -> Result<(), ReportError> {
    if digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(ReportError::InvalidOutputHash(digest.to_owned()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportError {
    EmptyOperationId,
    EmptyEntryId,
    EmptyRepairId,
    EmptyCategory,
    InvalidCategory(String),
    EmptyRepairAction,
    EmptySemanticImpact,
    DuplicateEntryId(String),
    DuplicateRepairId(String),
    RepairNotAuthorized { rule_id: String },
    InvalidOutputHash(String),
}

impl fmt::Display for ReportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyOperationId => write!(formatter, "operation id must not be empty"),
            Self::EmptyEntryId => write!(formatter, "conversion entry id must not be empty"),
            Self::EmptyRepairId => write!(formatter, "repair record id must not be empty"),
            Self::EmptyCategory => write!(formatter, "category must not be empty"),
            Self::InvalidCategory(category) => {
                write!(formatter, "category is not valid ASCII: {category}")
            }
            Self::EmptyRepairAction => write!(formatter, "repair action must not be empty"),
            Self::EmptySemanticImpact => write!(formatter, "semantic impact must not be empty"),
            Self::DuplicateEntryId(id) => write!(formatter, "duplicate conversion entry id: {id}"),
            Self::DuplicateRepairId(id) => write!(formatter, "duplicate repair record id: {id}"),
            Self::RepairNotAuthorized { rule_id } => {
                write!(formatter, "repair rule is not authorized: {rule_id}")
            }
            Self::InvalidOutputHash(hash) => {
                write!(
                    formatter,
                    "output hash must be 64 lowercase hex digits: {hash}"
                )
            }
        }
    }
}

impl std::error::Error for ReportError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LogicalSourceLocator;

    fn rule(id: &str) -> MappingRuleRef {
        MappingRuleRef::new(id).unwrap()
    }

    fn entry(
        id: &str,
        phase: ConversionPhase,
        entity: Option<&str>,
        field: Option<&str>,
        rule_id: Option<&str>,
        status: SemanticStatus,
    ) -> ConversionEntry {
        ConversionEntry::new(
            id,
            "conversion.tool-rewrite",
            ConversionDomain::Metadata,
            ConversionSeverity::Info,
            status,
            phase,
            Some(LogicalSourceLocator::new("chart.json").unwrap()),
            None,
            entity.map(str::to_owned),
            field.map(str::to_owned),
            rule_id.map(rule),
            Some(CanonicalValue::String("old".into())),
            None,
            Some(CanonicalValue::String("canonical".into())),
            None,
            "human message is not a stable API",
            [],
        )
        .unwrap()
    }

    #[test]
    fn status_aggregation_follows_conversion_precedence() {
        let report = ConversionReport::new(
            "op-1",
            ConversionPolicy::Semantic,
            RepairMode::disabled(),
            vec![entry(
                "e1",
                ConversionPhase::Lowering,
                Some("note:a"),
                Some("time"),
                Some("note.time"),
                SemanticStatus::Equivalent,
            )],
            Vec::new(),
            [
                ConversionStatus::Equivalent,
                ConversionStatus::Approximate,
                ConversionStatus::Unsupported,
            ],
            None,
        )
        .unwrap();
        assert_eq!(report.status(), ConversionStatus::Unsupported);
    }

    #[test]
    fn repairs_force_at_least_repaired_and_cannot_be_lossless() {
        let mode = RepairMode::new(true, [rule("repair.clamp-alpha")]);
        let repair = RepairRecord::new(
            "r1",
            LogicalSourceLocator::new("note/1/alpha").unwrap(),
            "conversion.source-invalid",
            "clamp",
            rule("repair.clamp-alpha"),
            CanonicalValue::Float(1.5),
            CanonicalValue::Float(1.0),
            "alpha clamped to unit interval",
        )
        .unwrap()
        .authorize(&mode)
        .unwrap();
        let report = ConversionReport::new(
            "op-repair",
            ConversionPolicy::Semantic,
            mode,
            Vec::new(),
            vec![repair],
            [ConversionStatus::Lossless],
            None,
        )
        .unwrap();
        assert_eq!(report.status(), ConversionStatus::Repaired);
        assert_eq!(report.summary().repair_count(), 1);
    }

    #[test]
    fn unauthorized_repair_is_rejected() {
        let mode = RepairMode::new(true, [rule("repair.allowed")]);
        let repair = RepairRecord::new(
            "r1",
            LogicalSourceLocator::new("note/1").unwrap(),
            "conversion.source-invalid",
            "drop-field",
            rule("repair.forbidden"),
            CanonicalValue::Null,
            CanonicalValue::Null,
            "would drop illegal field",
        )
        .unwrap();
        assert!(matches!(
            repair.authorize(&mode),
            Err(ReportError::RepairNotAuthorized { .. })
        ));
        assert!(matches!(
            ConversionReport::new(
                "op",
                ConversionPolicy::Strict,
                mode,
                Vec::new(),
                vec![
                    RepairRecord::new(
                        "r1",
                        LogicalSourceLocator::new("note/1").unwrap(),
                        "conversion.source-invalid",
                        "drop-field",
                        rule("repair.forbidden"),
                        CanonicalValue::Null,
                        CanonicalValue::Null,
                        "would drop illegal field",
                    )
                    .unwrap()
                ],
                [],
                None,
            ),
            Err(ReportError::RepairNotAuthorized { .. })
        ));
    }

    #[test]
    fn entries_order_by_phase_entity_field_rule_and_id() {
        let report = ConversionReport::new(
            "op-order",
            ConversionPolicy::Roundtrip,
            RepairMode::disabled(),
            vec![
                entry(
                    "b",
                    ConversionPhase::Lowering,
                    Some("note:2"),
                    Some("time"),
                    Some("note.time"),
                    SemanticStatus::Mapped,
                ),
                entry(
                    "a",
                    ConversionPhase::Parse,
                    Some("note:1"),
                    Some("kind"),
                    Some("note.kind"),
                    SemanticStatus::Native,
                ),
                entry(
                    "c",
                    ConversionPhase::Lowering,
                    Some("note:1"),
                    Some("time"),
                    Some("note.time"),
                    SemanticStatus::Mapped,
                ),
            ],
            Vec::new(),
            [ConversionStatus::Equivalent],
            None,
        )
        .unwrap();
        let ids = report
            .entries()
            .iter()
            .map(ConversionEntry::id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["a", "c", "b"]);
    }

    #[test]
    fn ordered_rule_and_repair_arrays_preserve_caller_order() {
        let mode = RepairMode::new(true, [rule("repair.z"), rule("repair.a"), rule("repair.z")]);
        assert_eq!(
            mode.authorized_rules()
                .iter()
                .map(MappingRuleRef::as_str)
                .collect::<Vec<_>>(),
            vec!["repair.z", "repair.a"]
        );

        let repair = |id, locator, rule_id| {
            RepairRecord::new(
                id,
                LogicalSourceLocator::new(locator).unwrap(),
                "conversion.source-invalid",
                "replace",
                rule(rule_id),
                CanonicalValue::Null,
                CanonicalValue::String("fixed".into()),
                "invalid value replaced",
            )
            .unwrap()
        };
        let report = ConversionReport::new(
            "op-repair-order",
            ConversionPolicy::Semantic,
            mode,
            Vec::new(),
            vec![
                repair("r2", "z/value", "repair.z"),
                repair("r1", "a/value", "repair.a"),
            ],
            [],
            None,
        )
        .unwrap();
        assert_eq!(
            report
                .repairs()
                .iter()
                .map(RepairRecord::id)
                .collect::<Vec<_>>(),
            vec!["r2", "r1"]
        );
    }

    #[test]
    fn entry_semantic_status_limits_top_level_status() {
        let report = ConversionReport::new(
            "op-drop",
            ConversionPolicy::Semantic,
            RepairMode::disabled(),
            vec![entry(
                "drop",
                ConversionPhase::Export,
                Some("note:1"),
                Some("presentation"),
                None,
                SemanticStatus::Dropped,
            )],
            Vec::new(),
            [ConversionStatus::Lossless],
            None,
        )
        .unwrap();
        assert_eq!(report.status(), ConversionStatus::Approximate);
        assert_eq!(report.summary().drop_count(), 1);
    }

    #[test]
    fn failed_outranks_repaired() {
        let mode = RepairMode::new(true, [rule("repair.clamp-alpha")]);
        let repair = RepairRecord::new(
            "r1",
            LogicalSourceLocator::new("note/1/alpha").unwrap(),
            "conversion.source-invalid",
            "clamp",
            rule("repair.clamp-alpha"),
            CanonicalValue::Float(2.0),
            CanonicalValue::Float(1.0),
            "alpha clamped",
        )
        .unwrap();
        let report = ConversionReport::new(
            "op-fail",
            ConversionPolicy::Semantic,
            mode,
            Vec::new(),
            vec![repair],
            [ConversionStatus::Failed, ConversionStatus::Repaired],
            None,
        )
        .unwrap();
        assert_eq!(report.status(), ConversionStatus::Failed);
    }
}
