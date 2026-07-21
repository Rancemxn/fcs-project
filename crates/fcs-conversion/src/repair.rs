//! I6.6 authorized repair execution boundary.
//!
//! Repair only rewrites already-diagnosed illegal or contradictory source facts
//! when explicitly authorized. It never chooses among multiple legal profile
//! interpretations (Conversion §3.3 / §4.4).

use fcs_model::{
    CanonicalValue, LogicalSourceLocator, MappingRuleRef, RepairMode, RepairRecord, ReportError,
};

use crate::profile_selection::{SelectionDecision, SelectionReason};

pub const REPAIR_NOT_AUTHORIZED: &str = "conversion.repair-not-authorized";
pub const REPAIR_AMBIGUITY_FORBIDDEN: &str = "conversion.ambiguous-source-semantics";

/// A proposed repair before authorization.
#[derive(Debug, Clone, PartialEq)]
pub struct RepairProposal {
    id: String,
    source_locator: LogicalSourceLocator,
    diagnostic_category: String,
    action: String,
    rule: MappingRuleRef,
    old_value: CanonicalValue,
    new_value: CanonicalValue,
    semantic_impact: String,
}

impl RepairProposal {
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
    ) -> Self {
        Self {
            id: id.into(),
            source_locator,
            diagnostic_category: diagnostic_category.into(),
            action: action.into(),
            rule,
            old_value,
            new_value,
            semantic_impact: semantic_impact.into(),
        }
    }

    pub fn rule(&self) -> &MappingRuleRef {
        &self.rule
    }
}

/// Apply one repair if and only if the mode authorizes its rule.
pub fn apply_authorized_repair(
    mode: &RepairMode,
    proposal: RepairProposal,
) -> Result<RepairRecord, RepairError> {
    if !mode.enabled() {
        return Err(RepairError::new(
            REPAIR_NOT_AUTHORIZED,
            proposal.rule.as_str(),
            "repair mode is disabled",
        ));
    }
    if !mode.authorizes(&proposal.rule) {
        return Err(RepairError::new(
            REPAIR_NOT_AUTHORIZED,
            proposal.rule.as_str(),
            "repair rule is not in the authorized rule set",
        ));
    }
    RepairRecord::new(
        proposal.id,
        proposal.source_locator,
        proposal.diagnostic_category,
        proposal.action,
        proposal.rule,
        proposal.old_value,
        proposal.new_value,
        proposal.semantic_impact,
    )
    .and_then(|record| record.authorize(mode))
    .map_err(|error| match error {
        ReportError::RepairNotAuthorized { rule_id } => RepairError::new(
            REPAIR_NOT_AUTHORIZED,
            rule_id,
            "repair rule is not authorized by RepairMode",
        ),
        other => RepairError::new(REPAIR_NOT_AUTHORIZED, "repair", other.to_string()),
    })
}

/// True when a selection decision remains unresolved even though repair was enabled.
pub fn repair_cannot_resolve_profile_ambiguity(decision: &SelectionDecision) -> bool {
    decision.repair_enabled()
        && decision.reason() == SelectionReason::Unresolved
        && decision.chosen().is_none()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairError {
    category: &'static str,
    path: String,
    message: String,
}

impl RepairError {
    pub(crate) fn new(
        category: &'static str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            category,
            path: path.into(),
            message: message.into(),
        }
    }

    pub const fn category(&self) -> &'static str {
        self.category
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for RepairError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} at {}: {}",
            self.category, self.path, self.message
        )
    }
}

impl std::error::Error for RepairError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_selection::{
        ProfileBindingCandidate, ProfileRef, ProfileSelectionMode, SelectionDirection,
        SelectionRequest, load_profile_registry, select_profile,
    };
    use fcs_model::{CanonicalValue, LogicalSourceLocator, MappingRuleRef, RepairMode};
    use std::path::PathBuf;

    fn registry() -> crate::profile_selection::ProfileRegistry {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/conformance/conversion/profile-registry.toml");
        load_profile_registry(path).unwrap()
    }

    #[test]
    fn authorized_repair_produces_complete_old_new_record() {
        let rule = MappingRuleRef::new("repair.clamp-alpha@1.0.0").unwrap();
        let mode = RepairMode::new(true, [rule.clone()]);
        let proposal = RepairProposal::new(
            "repair/alpha-0",
            LogicalSourceLocator::new("judgeLineList/0/alpha").unwrap(),
            "conversion.source-invalid",
            "clamp",
            rule,
            CanonicalValue::Int(-1),
            CanonicalValue::Int(0),
            "negative alpha clamped to zero under authorized repair",
        );
        let record = apply_authorized_repair(&mode, proposal).unwrap();
        assert_eq!(record.rule().as_str(), "repair.clamp-alpha@1.0.0");
        assert_eq!(record.old_value(), &CanonicalValue::Int(-1));
        assert_eq!(record.new_value(), &CanonicalValue::Int(0));
    }

    #[test]
    fn unauthorized_or_disabled_repair_is_rejected() {
        let rule = MappingRuleRef::new("repair.clamp-alpha@1.0.0").unwrap();
        let proposal = RepairProposal::new(
            "repair/alpha-0",
            LogicalSourceLocator::new("alpha").unwrap(),
            "conversion.source-invalid",
            "clamp",
            rule.clone(),
            CanonicalValue::Int(-1),
            CanonicalValue::Int(0),
            "clamp",
        );
        assert_eq!(
            apply_authorized_repair(&RepairMode::disabled(), proposal.clone())
                .unwrap_err()
                .category(),
            REPAIR_NOT_AUTHORIZED
        );
        let mode = RepairMode::new(true, [MappingRuleRef::new("repair.other@1.0.0").unwrap()]);
        assert_eq!(
            apply_authorized_repair(&mode, proposal)
                .unwrap_err()
                .category(),
            REPAIR_NOT_AUTHORIZED
        );
    }

    #[test]
    fn repair_enabled_selection_still_unresolved_for_legal_ambiguity() {
        let registry = registry();
        let candidates = vec![
            ProfileBindingCandidate::new(
                ProfileRef::parse("rpe.community.divide-bpmfactor@1.0.0").unwrap(),
                [("speedMode".into(), "modern-eased".into())],
            ),
            ProfileBindingCandidate::new(
                ProfileRef::parse("rpe.docs-example.multiply-bpmfactor@1.0.0").unwrap(),
                [("speedMode".into(), "modern-eased".into())],
            ),
        ];
        let request = SelectionRequest::new(
            SelectionDirection::Source,
            "rpe",
            ProfileSelectionMode::Strict,
            candidates,
            vec!["input-fact:bpmfactor=2".into()],
            None,
            None,
            None,
            false,
            true,
            vec!["timing".into(), "motion".into(), "scroll".into()],
        );
        let decision = select_profile(&registry, &request).unwrap();
        assert_eq!(decision.reason(), SelectionReason::Unresolved);
        assert!(decision.chosen().is_none());
        assert_eq!(
            decision.diagnostic(),
            Some("conversion.ambiguous-source-semantics")
        );
        assert!(repair_cannot_resolve_profile_ambiguity(&decision));
        assert!(decision.repair_enabled());
    }
}
