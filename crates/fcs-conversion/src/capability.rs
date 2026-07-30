//! Typed target capability and loss-authorization contracts.
//!
//! The capability descriptor is deliberately owned by the converter rather
//! than inferred from a writer's success.  A target write may only use
//! approximation or drop after an explicit, domain-scoped authorization.

use std::fmt;

use fcs_model::ConversionDomain;

pub use fcs_model::{ApproximationAuthorization, DropAuthorization};

/// One stable capability axis/value declaration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityFeature {
    axis: String,
    value: String,
}

impl CapabilityFeature {
    pub fn new(axis: impl Into<String>, value: impl Into<String>) -> Result<Self, CapabilityError> {
        let axis = axis.into();
        let value = value.into();
        validate_key("feature axis", &axis)?;
        validate_key("feature value", &value)?;
        Ok(Self { axis, value })
    }

    pub fn axis(&self) -> &str {
        &self.axis
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for CapabilityFeature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}={}", self.axis, self.value)
    }
}

/// One named finite numeric capability limit.
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityLimit {
    name: String,
    maximum: f64,
}

impl CapabilityLimit {
    pub fn new(name: impl Into<String>, maximum: f64) -> Result<Self, CapabilityError> {
        let name = name.into();
        validate_key("limit name", &name)?;
        if !maximum.is_finite() || maximum < 0.0 {
            return Err(CapabilityError::InvalidDescriptor(format!(
                "limit {name} must be finite and non-negative"
            )));
        }
        Ok(Self { name, maximum })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn maximum(&self) -> f64 {
        self.maximum
    }
}

fn validate_key(kind: &str, value: &str) -> Result<(), CapabilityError> {
    if value.trim().is_empty()
        || !value.is_ascii()
        || value.chars().any(|character| character.is_ascii_control())
    {
        return Err(CapabilityError::InvalidDescriptor(format!(
            "{kind} must be non-empty ASCII without control characters"
        )));
    }
    Ok(())
}

/// One section 7.2 domain declaration in a target capability descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityDomainDescriptor {
    domain: ConversionDomain,
    exact: bool,
    equivalent: bool,
    approximation: bool,
    preserve: bool,
    drop: bool,
    max_entities: Option<usize>,
    max_bytes: Option<usize>,
    features: Vec<CapabilityFeature>,
    limits: Vec<CapabilityLimit>,
}

impl CapabilityDomainDescriptor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        domain: ConversionDomain,
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
            features: Vec::new(),
            limits: Vec::new(),
        }
    }

    pub fn with_features(
        mut self,
        features: impl IntoIterator<Item = CapabilityFeature>,
    ) -> Result<Self, CapabilityError> {
        self.features = features.into_iter().collect();
        self.features.sort();
        if self.features.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(CapabilityError::InvalidDescriptor(format!(
                "{} capability features must be unique",
                self.domain
            )));
        }
        Ok(self)
    }

    pub fn with_limits(
        mut self,
        limits: impl IntoIterator<Item = CapabilityLimit>,
    ) -> Result<Self, CapabilityError> {
        self.limits = limits.into_iter().collect();
        self.limits
            .sort_by(|left, right| left.name.cmp(&right.name));
        if self
            .limits
            .windows(2)
            .any(|pair| pair[0].name == pair[1].name)
        {
            return Err(CapabilityError::InvalidDescriptor(format!(
                "{} capability limits must be unique",
                self.domain
            )));
        }
        Ok(self)
    }

    pub const fn domain(&self) -> ConversionDomain {
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

    pub fn features(&self) -> &[CapabilityFeature] {
        &self.features
    }

    pub fn supports(&self, axis: &str, value: &str) -> bool {
        self.features
            .iter()
            .any(|feature| feature.axis() == axis && feature.value() == value)
    }

    pub fn limits(&self) -> &[CapabilityLimit] {
        &self.limits
    }

    pub fn limit(&self, name: &str) -> Option<f64> {
        self.limits
            .iter()
            .find(|limit| limit.name() == name)
            .map(CapabilityLimit::maximum)
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
        if (self.max_bytes.is_some()
            || self.limits.iter().any(|limit| limit.name() == "byte.count"))
            && !matches!(
                self.domain,
                ConversionDomain::Resource | ConversionDomain::Package
            )
        {
            return Err(CapabilityError::InvalidDescriptor(format!(
                "{} capability cannot declare a byte limit",
                self.domain
            )));
        }
        Ok(())
    }
}

/// A deterministic, version/profile-bound target descriptor.
#[derive(Debug, Clone, PartialEq)]
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
        if format.trim().is_empty() || version.trim().is_empty() {
            return Err(CapabilityError::InvalidDescriptor(
                "format and version must be non-empty".into(),
            ));
        }
        if profile
            .as_deref()
            .is_some_and(|profile| profile.trim().is_empty())
        {
            return Err(CapabilityError::InvalidDescriptor(
                "profile must be absent or non-empty".into(),
            ));
        }
        for domain in &domains {
            domain.validate()?;
        }
        domains.sort_by_key(|domain| domain.domain());
        if domains.len() != ConversionDomain::ALL.len()
            || ConversionDomain::ALL
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

    pub fn domain(&self, domain: ConversionDomain) -> Option<&CapabilityDomainDescriptor> {
        self.domains.iter().find(|entry| entry.domain() == domain)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityError {
    InvalidDescriptor(String),
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDescriptor(message) => {
                write!(formatter, "invalid capability descriptor: {message}")
            }
        }
    }
}

impl std::error::Error for CapabilityError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact_domains() -> Vec<CapabilityDomainDescriptor> {
        ConversionDomain::ALL
            .into_iter()
            .map(|domain| {
                CapabilityDomainDescriptor::new(
                    domain, true, false, false, false, false, None, None,
                )
            })
            .collect()
    }

    #[test]
    fn capability_identity_rejects_whitespace_only_fields() {
        for (format, version, profile) in [
            ("  ", "1", None),
            ("pgr", "\t", None),
            ("pgr", "1", Some("  ".into())),
        ] {
            assert!(matches!(
                CapabilityDescriptor::new(format, version, profile, exact_domains()),
                Err(CapabilityError::InvalidDescriptor(_))
            ));
        }
    }

    #[test]
    fn feature_and_limit_declarations_are_validated_and_sorted() {
        let descriptor = CapabilityDomainDescriptor::new(
            ConversionDomain::Gameplay,
            true,
            false,
            false,
            false,
            false,
            None,
            None,
        )
        .with_features([
            CapabilityFeature::new("note.kind", "hold").unwrap(),
            CapabilityFeature::new("note.kind", "tap").unwrap(),
        ])
        .unwrap()
        .with_limits([
            CapabilityLimit::new("event.count", 4.0).unwrap(),
            CapabilityLimit::new("entity.count", 2.0).unwrap(),
        ])
        .unwrap();

        assert_eq!(descriptor.features()[0].value(), "hold");
        assert!(descriptor.supports("note.kind", "tap"));
        assert_eq!(descriptor.limit("entity.count"), Some(2.0));
        assert!(CapabilityFeature::new(" ", "tap").is_err());
        assert!(CapabilityLimit::new("event.count", f64::NAN).is_err());
    }

    #[test]
    fn duplicate_feature_and_limit_declarations_are_rejected() {
        let feature = CapabilityFeature::new("note.kind", "tap").unwrap();
        assert!(
            CapabilityDomainDescriptor::new(
                ConversionDomain::Gameplay,
                true,
                false,
                false,
                false,
                false,
                None,
                None,
            )
            .with_features([feature.clone(), feature])
            .is_err()
        );

        let limit = CapabilityLimit::new("entity.count", 1.0).unwrap();
        assert!(
            CapabilityDomainDescriptor::new(
                ConversionDomain::Gameplay,
                true,
                false,
                false,
                false,
                false,
                None,
                None,
            )
            .with_limits([limit.clone(), limit])
            .is_err()
        );
    }

    #[test]
    fn byte_limits_are_restricted_to_resource_and_package_domains() {
        let descriptor = |domain, max_bytes| {
            CapabilityDomainDescriptor::new(
                domain, true, false, false, false, false, None, max_bytes,
            )
        };
        let byte_count = |domain| {
            descriptor(domain, None)
                .with_limits([CapabilityLimit::new("byte.count", 1.0).unwrap()])
                .unwrap()
        };

        assert!(
            descriptor(ConversionDomain::Motion, Some(1))
                .validate()
                .is_err()
        );
        assert!(byte_count(ConversionDomain::Profile).validate().is_err());
        assert!(
            descriptor(ConversionDomain::Package, Some(1))
                .validate()
                .is_ok()
        );
        assert!(byte_count(ConversionDomain::Resource).validate().is_ok());
    }
}
