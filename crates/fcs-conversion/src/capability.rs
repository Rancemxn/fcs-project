//! Typed target capability and loss-authorization contracts.
//!
//! The capability descriptor is deliberately owned by the converter rather
//! than inferred from a writer's success.  A target write may only use
//! approximation or drop after an explicit, domain-scoped authorization.

use std::collections::BTreeMap;
use std::fmt;

/// Canonical feature domains used by Conversion §6.2 and §7.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CapabilityDomain {
    Timing,
    Gameplay,
    Motion,
    Scroll,
    Presentation,
    Resource,
    Metadata,
    Numeric,
    Entity,
    Limits,
    Expression,
    Package,
}

impl CapabilityDomain {
    pub const ALL: [Self; 12] = [
        Self::Timing,
        Self::Gameplay,
        Self::Motion,
        Self::Scroll,
        Self::Presentation,
        Self::Resource,
        Self::Metadata,
        Self::Numeric,
        Self::Entity,
        Self::Limits,
        Self::Expression,
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
            Self::Numeric => "numeric",
            Self::Entity => "entity",
            Self::Limits => "limits",
            Self::Expression => "expression",
            Self::Package => "package",
        }
    }
}

impl fmt::Display for CapabilityDomain {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One typed domain declaration in a target capability descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDomainDescriptor {
    domain: CapabilityDomain,
    exact: bool,
    equivalent: bool,
    approximation: bool,
    preserve: bool,
    drop: bool,
    max_entities: Option<usize>,
    max_bytes: Option<usize>,
}

impl CapabilityDomainDescriptor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        domain: CapabilityDomain,
        exact: bool,
        equivalent: bool,
        approximation: bool,
        preserve: bool,
        drop: bool,
        max_entities: Option<usize>,
        max_bytes: Option<usize>,
    ) -> Self {
        Self {
            domain,
            exact,
            equivalent,
            approximation,
            preserve,
            drop,
            max_entities,
            max_bytes,
        }
    }

    pub const fn domain(&self) -> CapabilityDomain {
        self.domain
    }

    pub const fn exact(&self) -> bool {
        self.exact
    }

    pub const fn equivalent(&self) -> bool {
        self.equivalent
    }

    pub const fn approximation(&self) -> bool {
        self.approximation
    }

    pub const fn preserve(&self) -> bool {
        self.preserve
    }

    pub const fn drop(&self) -> bool {
        self.drop
    }

    pub const fn max_entities(&self) -> Option<usize> {
        self.max_entities
    }

    pub const fn max_bytes(&self) -> Option<usize> {
        self.max_bytes
    }

    fn validate(&self) -> Result<(), CapabilityError> {
        let modes = [
            self.exact,
            self.equivalent,
            self.approximation,
            self.preserve,
            self.drop,
        ]
        .into_iter()
        .filter(|enabled| *enabled)
        .count();
        if modes > 1 {
            return Err(CapabilityError::InvalidDescriptor(format!(
                "{} capability must declare at most one representation mode",
                self.domain
            )));
        }
        if self.max_entities == Some(0) || self.max_bytes == Some(0) {
            return Err(CapabilityError::InvalidDescriptor(format!(
                "{} capability limits must be positive",
                self.domain
            )));
        }
        Ok(())
    }
}

/// A deterministic, version/profile-bound target descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDescriptor {
    format: String,
    version: String,
    profile: Option<String>,
    domains: Vec<CapabilityDomainDescriptor>,
}

impl CapabilityDescriptor {
    pub fn new(
        format: impl Into<String>,
        version: impl Into<String>,
        profile: Option<String>,
        mut domains: Vec<CapabilityDomainDescriptor>,
    ) -> Result<Self, CapabilityError> {
        let format = format.into();
        let version = version.into();
        if format.is_empty() || version.is_empty() {
            return Err(CapabilityError::InvalidDescriptor(
                "format and version must be non-empty".into(),
            ));
        }
        if profile.as_deref().is_some_and(str::is_empty) {
            return Err(CapabilityError::InvalidDescriptor(
                "profile must be absent or non-empty".into(),
            ));
        }
        for domain in &domains {
            domain.validate()?;
        }
        domains.sort_by_key(|domain| domain.domain());
        if domains.len() != CapabilityDomain::ALL.len()
            || CapabilityDomain::ALL
                .into_iter()
                .any(|domain| !domains.iter().any(|entry| entry.domain() == domain))
        {
            return Err(CapabilityError::InvalidDescriptor(
                "capability descriptor must declare every canonical domain".into(),
            ));
        }
        if domains
            .windows(2)
            .any(|pair| pair[0].domain() == pair[1].domain())
        {
            return Err(CapabilityError::InvalidDescriptor(
                "capability domains must be unique".into(),
            ));
        }
        Ok(Self {
            format,
            version,
            profile,
            domains,
        })
    }

    pub fn format(&self) -> &str {
        &self.format
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn profile(&self) -> Option<&str> {
        self.profile.as_deref()
    }

    pub fn domains(&self) -> &[CapabilityDomainDescriptor] {
        &self.domains
    }

    pub fn domain(&self, domain: CapabilityDomain) -> Option<&CapabilityDomainDescriptor> {
        self.domains.iter().find(|entry| entry.domain() == domain)
    }
}

/// Explicit approximation authorization from Conversion §6.3.
#[derive(Debug, Clone, PartialEq)]
pub struct ApproximationAuthorization {
    enabled: bool,
    target_domains: Vec<String>,
    error_budgets: BTreeMap<String, f64>,
    maximum_segments: usize,
    algorithm_id: String,
    algorithm_version: String,
}

impl ApproximationAuthorization {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            target_domains: Vec::new(),
            error_budgets: BTreeMap::new(),
            maximum_segments: 0,
            algorithm_id: String::new(),
            algorithm_version: String::new(),
        }
    }

    pub fn new(
        target_domains: impl IntoIterator<Item = String>,
        budgets: impl IntoIterator<Item = (String, f64)>,
        maximum_segments: usize,
        algorithm_id: impl Into<String>,
        algorithm_version: impl Into<String>,
    ) -> Result<Self, CapabilityError> {
        let mut target_domains: Vec<_> = target_domains.into_iter().collect();
        target_domains.sort();
        target_domains.dedup();
        let mut error_budgets = BTreeMap::new();
        for (metric, budget) in budgets {
            if metric.is_empty() || !budget.is_finite() || budget < 0.0 {
                return Err(CapabilityError::InvalidAuthorization(
                    "error budgets require non-empty metrics and finite non-negative values".into(),
                ));
            }
            if error_budgets.insert(metric, budget).is_some() {
                return Err(CapabilityError::InvalidAuthorization(
                    "error budget metrics must be unique".into(),
                ));
            }
        }
        let algorithm_id = algorithm_id.into();
        let algorithm_version = algorithm_version.into();
        if target_domains.is_empty()
            || error_budgets.is_empty()
            || maximum_segments == 0
            || algorithm_id.is_empty()
            || algorithm_version.is_empty()
        {
            return Err(CapabilityError::InvalidAuthorization(
                "enabled approximation requires domains, budgets, segment limit, and algorithm identity"
                    .into(),
            ));
        }
        if target_domains.iter().any(|domain| {
            !error_budgets.keys().any(|metric| {
                metric == domain
                    || metric
                        .strip_prefix(domain)
                        .is_some_and(|suffix| suffix.starts_with('.'))
            })
        }) {
            return Err(CapabilityError::InvalidAuthorization(
                "every approximation domain requires at least one domain-scoped metric budget"
                    .into(),
            ));
        }
        Ok(Self {
            enabled: true,
            target_domains,
            error_budgets,
            maximum_segments,
            algorithm_id,
            algorithm_version,
        })
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn target_domains(&self) -> &[String] {
        &self.target_domains
    }

    pub fn error_budgets(&self) -> &BTreeMap<String, f64> {
        &self.error_budgets
    }

    pub const fn maximum_segments(&self) -> usize {
        self.maximum_segments
    }

    pub fn algorithm_id(&self) -> &str {
        &self.algorithm_id
    }

    pub fn algorithm_version(&self) -> &str {
        &self.algorithm_version
    }

    pub fn allows(&self, domain: &str) -> bool {
        self.enabled
            && self.target_domains.iter().any(|allowed| {
                domain == allowed
                    || domain
                        .strip_prefix(allowed)
                        .is_some_and(|suffix| suffix.starts_with('.'))
            })
    }

    pub fn budget(&self, metric: &str) -> Option<f64> {
        if !self.enabled {
            return None;
        }
        self.error_budgets.get(metric).copied()
    }
}

/// Explicit drop authorization from Conversion §6.3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropAuthorization {
    enabled: bool,
    target_domains: Vec<String>,
    reason: String,
}

impl DropAuthorization {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            target_domains: Vec::new(),
            reason: String::new(),
        }
    }

    pub fn new(
        target_domains: impl IntoIterator<Item = String>,
        reason: impl Into<String>,
    ) -> Result<Self, CapabilityError> {
        let mut target_domains: Vec<_> = target_domains.into_iter().collect();
        target_domains.sort();
        target_domains.dedup();
        let reason = reason.into();
        if target_domains.is_empty() || reason.trim().is_empty() {
            return Err(CapabilityError::InvalidAuthorization(
                "enabled drop requires target domains and a reason".into(),
            ));
        }
        Ok(Self {
            enabled: true,
            target_domains,
            reason,
        })
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn target_domains(&self) -> &[String] {
        &self.target_domains
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    pub fn allows(&self, domain: &str) -> bool {
        self.enabled
            && self.target_domains.iter().any(|allowed| {
                domain == allowed
                    || domain
                        .strip_prefix(allowed)
                        .is_some_and(|suffix| suffix.starts_with('.'))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityError {
    InvalidDescriptor(String),
    InvalidAuthorization(String),
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDescriptor(message) => {
                write!(formatter, "invalid capability descriptor: {message}")
            }
            Self::InvalidAuthorization(message) => {
                write!(formatter, "invalid authorization: {message}")
            }
        }
    }
}

impl std::error::Error for CapabilityError {}
