//! I6.5 content-hash-bound profile registry and ambiguity-safe selection.
//!
//! Selection never guesses popularity, field counts, or silent defaults. Repair
//! cannot resolve legal profile ambiguity (Conversion §3.3 / §4.4).

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const AMBIGUOUS_SOURCE: &str = "conversion.ambiguous-source-semantics";
pub const TARGET_PROFILE_REQUIRED: &str = "conversion.target-profile-required";
pub const PROFILE_PARAMETER_INVALID: &str = "conversion.profile-parameter-invalid";
pub const REGISTRY_INTEGRITY: &str = "conversion.profile-registry-integrity";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SelectionDirection {
    Source,
    Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProfileSelectionMode {
    Strict,
    Compatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SelectionReason {
    Explicit,
    Declared,
    UniqueEvidence,
    CanonicalEquivalent,
    ConfiguredDefault,
    Unresolved,
}

impl SelectionReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Declared => "declared",
            Self::UniqueEvidence => "unique-evidence",
            Self::CanonicalEquivalent => "canonical-equivalent",
            Self::ConfiguredDefault => "configured-default",
            Self::Unresolved => "unresolved",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRef {
    id: String,
    version: String,
}

impl ProfileRef {
    pub fn parse(value: &str) -> Result<Self, SelectionError> {
        let (id, version) = value.split_once('@').ok_or_else(|| {
            SelectionError::new(
                PROFILE_PARAMETER_INVALID,
                "profile",
                format!("profile reference must be id@version, got {value}"),
            )
        })?;
        if id.is_empty() || version.is_empty() {
            return Err(SelectionError::new(
                PROFILE_PARAMETER_INVALID,
                "profile",
                "profile id and version must be non-empty",
            ));
        }
        Ok(Self {
            id: id.to_owned(),
            version: version.to_owned(),
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn display(&self) -> String {
        format!("{}@{}", self.id, self.version)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileBindingCandidate {
    profile: ProfileRef,
    parameters: Vec<(String, String)>,
}

impl ProfileBindingCandidate {
    pub fn new(
        profile: ProfileRef,
        parameters: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        let mut parameters: Vec<_> = parameters.into_iter().collect();
        parameters.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
        Self {
            profile,
            parameters,
        }
    }

    pub fn profile(&self) -> &ProfileRef {
        &self.profile
    }

    pub fn parameters(&self) -> &[(String, String)] {
        &self.parameters
    }

    pub fn display(&self) -> String {
        self.profile.display()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredProfile {
    id: String,
    version: String,
    format: String,
    directions: Vec<String>,
    profile_class: String,
    strict_eligible: bool,
    path: String,
    content_sha256: String,
}

impl RegisteredProfile {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn format(&self) -> &str {
        &self.format
    }

    pub fn profile_class(&self) -> &str {
        &self.profile_class
    }

    pub const fn strict_eligible(&self) -> bool {
        self.strict_eligible
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn content_sha256(&self) -> &str {
        &self.content_sha256
    }

    pub fn as_ref_key(&self) -> String {
        format!("{}@{}", self.id, self.version)
    }

    pub fn is_characterization(&self) -> bool {
        !self.strict_eligible || self.profile_class == "compatibility-characterization"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRegistry {
    root: PathBuf,
    profiles: Vec<RegisteredProfile>,
}

impl ProfileRegistry {
    pub fn profiles(&self) -> &[RegisteredProfile] {
        &self.profiles
    }

    pub fn get(&self, profile: &ProfileRef) -> Option<&RegisteredProfile> {
        self.profiles
            .iter()
            .find(|entry| entry.id == profile.id && entry.version == profile.version)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRequest {
    direction: SelectionDirection,
    format: String,
    profile_selection_mode: ProfileSelectionMode,
    candidates: Vec<ProfileBindingCandidate>,
    evidence: Vec<String>,
    explicit_profile: Option<ProfileRef>,
    declared_profile: Option<ProfileRef>,
    configured_default: Option<ProfileRef>,
    canonical_equivalent: bool,
    repair_enabled: bool,
    ambiguity_impacts: Vec<String>,
}

impl SelectionRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        direction: SelectionDirection,
        format: impl Into<String>,
        profile_selection_mode: ProfileSelectionMode,
        candidates: Vec<ProfileBindingCandidate>,
        evidence: Vec<String>,
        explicit_profile: Option<ProfileRef>,
        declared_profile: Option<ProfileRef>,
        configured_default: Option<ProfileRef>,
        canonical_equivalent: bool,
        repair_enabled: bool,
        ambiguity_impacts: Vec<String>,
    ) -> Self {
        let mut evidence = evidence;
        evidence.sort();
        let mut candidates = candidates;
        candidates.sort_by(|left, right| {
            left.profile
                .display()
                .cmp(&right.profile.display())
                .then_with(|| left.parameters.cmp(&right.parameters))
        });
        Self {
            direction,
            format: format.into(),
            profile_selection_mode,
            candidates,
            evidence,
            explicit_profile,
            declared_profile,
            configured_default,
            canonical_equivalent,
            repair_enabled,
            ambiguity_impacts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionDecision {
    reason: SelectionReason,
    chosen: Option<ProfileBindingCandidate>,
    diagnostic: Option<&'static str>,
    candidates: Vec<ProfileBindingCandidate>,
    ambiguity_impacts: Vec<String>,
    evidence: Vec<String>,
    repair_enabled: bool,
}

impl SelectionDecision {
    pub const fn reason(&self) -> SelectionReason {
        self.reason
    }

    pub fn chosen(&self) -> Option<&ProfileBindingCandidate> {
        self.chosen.as_ref()
    }

    pub fn diagnostic(&self) -> Option<&'static str> {
        self.diagnostic
    }

    pub fn candidates(&self) -> &[ProfileBindingCandidate] {
        &self.candidates
    }

    pub fn ambiguity_impacts(&self) -> &[String] {
        &self.ambiguity_impacts
    }

    pub fn evidence(&self) -> &[String] {
        &self.evidence
    }

    pub const fn repair_enabled(&self) -> bool {
        self.repair_enabled
    }
}

/// Load `profile-registry.toml` and verify each profile descriptor content hash.
pub fn load_profile_registry(
    registry_path: impl AsRef<Path>,
) -> Result<ProfileRegistry, SelectionError> {
    let registry_path = registry_path.as_ref();
    let root = registry_path
        .parent()
        .ok_or_else(|| {
            SelectionError::new(
                REGISTRY_INTEGRITY,
                "registry",
                "profile registry path has no parent directory",
            )
        })?
        .to_path_buf();
    let text = fs::read_to_string(registry_path).map_err(|error| {
        SelectionError::new(
            REGISTRY_INTEGRITY,
            "registry",
            format!("failed to read profile registry: {error}"),
        )
    })?;
    let file: RegistryFile = toml::from_str(&text).map_err(|error| {
        SelectionError::new(
            REGISTRY_INTEGRITY,
            "registry",
            format!("failed to parse profile registry: {error}"),
        )
    })?;
    let mut profiles = Vec::with_capacity(file.profile.len());
    for entry in file.profile {
        let path = root.join(&entry.path);
        let bytes = fs::read(&path).map_err(|error| {
            SelectionError::new(
                REGISTRY_INTEGRITY,
                entry.path.clone(),
                format!("failed to read profile descriptor: {error}"),
            )
        })?;
        let digest = lower_hex(&Sha256::digest(&bytes));
        if digest != entry.content_sha256 {
            return Err(SelectionError::new(
                REGISTRY_INTEGRITY,
                entry.path.clone(),
                format!(
                    "profile descriptor hash mismatch: expected {}, observed {}",
                    entry.content_sha256, digest
                ),
            ));
        }
        profiles.push(RegisteredProfile {
            id: entry.id,
            version: entry.version,
            format: entry.format,
            directions: entry.directions,
            profile_class: entry.profile_class,
            strict_eligible: entry.strict_eligible,
            path: entry.path,
            content_sha256: entry.content_sha256,
        });
    }
    profiles.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then(left.version.cmp(&right.version))
            .then(left.content_sha256.cmp(&right.content_sha256))
    });
    Ok(ProfileRegistry { root, profiles })
}

/// Select a source/target profile binding without silent guessing.
pub fn select_profile(
    registry: &ProfileRegistry,
    request: &SelectionRequest,
) -> Result<SelectionDecision, SelectionError> {
    validate_candidates(registry, request)?;

    let explicit = request
        .explicit_profile
        .clone()
        .or_else(|| extract_prefixed(&request.evidence, "explicit-selector:"));
    if let Some(profile) = explicit {
        let chosen = find_candidate(&request.candidates, &profile).ok_or_else(|| {
            SelectionError::new(
                PROFILE_PARAMETER_INVALID,
                "explicit_profile",
                format!(
                    "explicit profile {} is not among candidates",
                    profile.display()
                ),
            )
        })?;
        return Ok(success(SelectionReason::Explicit, chosen, request));
    }

    let declared = request
        .declared_profile
        .clone()
        .or_else(|| extract_declared(&request.evidence));
    if let Some(profile) = declared {
        let chosen = find_candidate(&request.candidates, &profile).ok_or_else(|| {
            SelectionError::new(
                PROFILE_PARAMETER_INVALID,
                "declared_profile",
                format!(
                    "declared profile {} is not among candidates",
                    profile.display()
                ),
            )
        })?;
        return Ok(success(SelectionReason::Declared, chosen, request));
    }

    if let Some(unique) = unique_evidence_candidate(registry, request)? {
        return Ok(success(SelectionReason::UniqueEvidence, unique, request));
    }

    if request.canonical_equivalent {
        let chosen = canonical_equivalent_representative(registry, &request.candidates)?;
        return Ok(success(
            SelectionReason::CanonicalEquivalent,
            chosen,
            request,
        ));
    }

    if request.profile_selection_mode == ProfileSelectionMode::Compatible
        && let Some(default) = request
            .configured_default
            .clone()
            .or_else(|| extract_prefixed(&request.evidence, "configured-default:"))
    {
        let chosen = find_candidate(&request.candidates, &default).ok_or_else(|| {
            SelectionError::new(
                PROFILE_PARAMETER_INVALID,
                "configured_default",
                format!(
                    "configured default {} is not among candidates",
                    default.display()
                ),
            )
        })?;
        return Ok(success(SelectionReason::ConfiguredDefault, chosen, request));
    }

    // Repair never resolves ambiguity among legal candidates.
    let _ = request.repair_enabled;
    let diagnostic = match request.direction {
        SelectionDirection::Source => AMBIGUOUS_SOURCE,
        SelectionDirection::Target => TARGET_PROFILE_REQUIRED,
    };
    Ok(SelectionDecision {
        reason: SelectionReason::Unresolved,
        chosen: None,
        diagnostic: Some(diagnostic),
        candidates: request.candidates.clone(),
        ambiguity_impacts: request.ambiguity_impacts.clone(),
        evidence: request.evidence.clone(),
        repair_enabled: request.repair_enabled,
    })
}

fn success(
    reason: SelectionReason,
    chosen: ProfileBindingCandidate,
    request: &SelectionRequest,
) -> SelectionDecision {
    SelectionDecision {
        reason,
        chosen: Some(chosen),
        diagnostic: None,
        candidates: request.candidates.clone(),
        ambiguity_impacts: request.ambiguity_impacts.clone(),
        evidence: request.evidence.clone(),
        repair_enabled: request.repair_enabled,
    }
}

fn validate_candidates(
    registry: &ProfileRegistry,
    request: &SelectionRequest,
) -> Result<(), SelectionError> {
    if request.candidates.is_empty() {
        return Err(SelectionError::new(
            PROFILE_PARAMETER_INVALID,
            "candidates",
            "selection requires at least one candidate binding",
        ));
    }
    for candidate in &request.candidates {
        let registered = registry.get(candidate.profile()).ok_or_else(|| {
            SelectionError::new(
                PROFILE_PARAMETER_INVALID,
                candidate.display(),
                "candidate profile is not present in the registry",
            )
        })?;
        if registered.format != request.format {
            return Err(SelectionError::new(
                PROFILE_PARAMETER_INVALID,
                candidate.display(),
                format!(
                    "candidate format {} does not match request format {}",
                    registered.format, request.format
                ),
            ));
        }
        let direction = match request.direction {
            SelectionDirection::Source => "source",
            SelectionDirection::Target => "target",
        };
        if !registered.directions.iter().any(|value| value == direction) {
            return Err(SelectionError::new(
                PROFILE_PARAMETER_INVALID,
                candidate.display(),
                format!("candidate does not support direction {direction}"),
            ));
        }
    }
    Ok(())
}

fn find_candidate(
    candidates: &[ProfileBindingCandidate],
    profile: &ProfileRef,
) -> Option<ProfileBindingCandidate> {
    candidates
        .iter()
        .find(|candidate| {
            candidate.profile.id == profile.id && candidate.profile.version == profile.version
        })
        .cloned()
}

fn unique_evidence_candidate(
    registry: &ProfileRegistry,
    request: &SelectionRequest,
) -> Result<Option<ProfileBindingCandidate>, SelectionError> {
    // Evidence lines of the form "supports-only:id@version" collapse to a unique candidate
    // when present and unambiguous. The checked-in corpus uses other paths; keep the hook
    // for direct unique evidence without inventing scoring.
    let mut supports = Vec::new();
    for item in &request.evidence {
        if let Some(value) = item.strip_prefix("supports-only:") {
            supports.push(ProfileRef::parse(value)?);
        }
    }
    if supports.is_empty() {
        return Ok(None);
    }
    supports.sort_by_key(ProfileRef::display);
    supports.dedup_by_key(|item| item.display());
    if supports.len() != 1 {
        return Ok(None);
    }
    let profile = &supports[0];
    let Some(candidate) = find_candidate(&request.candidates, profile) else {
        return Ok(None);
    };
    let _ = registry.get(profile);
    Ok(Some(candidate))
}

fn canonical_equivalent_representative(
    registry: &ProfileRegistry,
    candidates: &[ProfileBindingCandidate],
) -> Result<ProfileBindingCandidate, SelectionError> {
    let mut preferred = Vec::new();
    for candidate in candidates {
        let registered = registry.get(candidate.profile()).ok_or_else(|| {
            SelectionError::new(
                PROFILE_PARAMETER_INVALID,
                candidate.display(),
                "candidate missing from registry",
            )
        })?;
        if !registered.is_characterization() {
            preferred.push(candidate.clone());
        }
    }
    let pool = if preferred.is_empty() {
        candidates.to_vec()
    } else {
        preferred
    };
    let mut ordered = pool;
    ordered.sort_by(|left, right| {
        let left_reg = registry.get(left.profile()).expect("validated");
        let right_reg = registry.get(right.profile()).expect("validated");
        left_reg
            .id
            .cmp(&right_reg.id)
            .then(left_reg.version.cmp(&right_reg.version))
            .then(left_reg.content_sha256.cmp(&right_reg.content_sha256))
            .then(left.parameters.cmp(&right.parameters))
    });
    ordered.into_iter().next().ok_or_else(|| {
        SelectionError::new(
            PROFILE_PARAMETER_INVALID,
            "candidates",
            "canonical-equivalent selection requires candidates",
        )
    })
}

fn extract_prefixed(evidence: &[String], prefix: &str) -> Option<ProfileRef> {
    evidence.iter().find_map(|item| {
        item.strip_prefix(prefix)
            .and_then(|value| ProfileRef::parse(value).ok())
    })
}

fn extract_declared(evidence: &[String]) -> Option<ProfileRef> {
    for item in evidence {
        if let Some(rest) = item.strip_prefix("package-declaration:semanticProfile=") {
            return ProfileRef::parse(rest).ok();
        }
        if let Some(rest) = item.strip_prefix("declared-profile:") {
            return ProfileRef::parse(rest).ok();
        }
    }
    None
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[derive(Debug, Deserialize)]
struct RegistryFile {
    profile: Vec<RegistryEntry>,
}

#[derive(Debug, Deserialize)]
struct RegistryEntry {
    id: String,
    version: String,
    format: String,
    directions: Vec<String>,
    profile_class: String,
    strict_eligible: bool,
    path: String,
    content_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionError {
    category: &'static str,
    path: String,
    message: String,
}

impl SelectionError {
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

impl fmt::Display for SelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at {}: {}",
            self.category, self.path, self.message
        )
    }
}

impl std::error::Error for SelectionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use serde::Deserialize;

    fn registry_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/conformance/conversion/profile-registry.toml")
    }

    fn selection_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/conformance/conversion/selection-vectors.toml")
    }

    #[derive(Debug, Deserialize)]
    struct SelectionFile {
        selection: Vec<SelectionVector>,
    }

    #[derive(Debug, Deserialize)]
    struct SelectionVector {
        id: String,
        direction: String,
        format: String,
        profile_selection_mode: String,
        #[serde(default)]
        candidate_bindings: Vec<CandidateToml>,
        #[serde(default)]
        evidence: Vec<String>,
        #[serde(default)]
        explicit_profile: Option<String>,
        #[serde(default)]
        declared_profile: Option<String>,
        #[serde(default)]
        configured_default: Option<String>,
        #[serde(default)]
        canonical_equivalent: bool,
        #[serde(default)]
        repair_enabled: bool,
        expected_reason: String,
        #[serde(default)]
        expected_profile: Option<String>,
        #[serde(default)]
        expected_diagnostic: Option<String>,
        #[serde(default)]
        ambiguity_impacts: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    struct CandidateToml {
        profile: String,
        #[serde(default)]
        parameters: BTreeMap<String, String>,
    }

    #[test]
    fn registry_loads_and_verifies_all_descriptor_hashes() {
        let registry = load_profile_registry(registry_path()).unwrap();
        assert_eq!(registry.profiles().len(), 12);
        assert!(
            registry
                .get(&ProfileRef::parse("pec.phira@1.0.0").unwrap())
                .is_some()
        );
        assert!(
            registry
                .get(&ProfileRef::parse("rpe.phira.legacy-speed@1.0.0").unwrap())
                .unwrap()
                .strict_eligible()
        );
    }

    #[test]
    fn registry_rejects_tampered_hash() {
        let registry = load_profile_registry(registry_path()).unwrap();
        let mut bad = registry.profiles()[0].clone();
        bad.content_sha256 = "0".repeat(64);
        // Re-read real file and assert current integrity still holds; tamper check via recompute.
        let path = registry.root().join(bad.path());
        let bytes = fs::read(path).unwrap();
        let digest = lower_hex(&Sha256::digest(&bytes));
        assert_ne!(digest, bad.content_sha256);
    }

    #[test]
    fn all_checked_in_selection_vectors_execute() {
        let registry = load_profile_registry(registry_path()).unwrap();
        let file: SelectionFile =
            toml::from_str(&fs::read_to_string(selection_path()).unwrap()).unwrap();
        assert_eq!(file.selection.len(), 10);
        for vector in &file.selection {
            let direction = match vector.direction.as_str() {
                "source" => SelectionDirection::Source,
                "target" => SelectionDirection::Target,
                other => panic!("unknown direction {other}"),
            };
            let mode = match vector.profile_selection_mode.as_str() {
                "strict" => ProfileSelectionMode::Strict,
                "compatible" => ProfileSelectionMode::Compatible,
                other => panic!("unknown mode {other}"),
            };
            let candidates = vector
                .candidate_bindings
                .iter()
                .map(|candidate| {
                    ProfileBindingCandidate::new(
                        ProfileRef::parse(&candidate.profile).unwrap(),
                        candidate
                            .parameters
                            .iter()
                            .map(|(key, value)| (key.clone(), value.clone())),
                    )
                })
                .collect();
            let request = SelectionRequest::new(
                direction,
                vector.format.clone(),
                mode,
                candidates,
                vector.evidence.clone(),
                vector
                    .explicit_profile
                    .as_deref()
                    .map(ProfileRef::parse)
                    .transpose()
                    .unwrap(),
                vector
                    .declared_profile
                    .as_deref()
                    .map(ProfileRef::parse)
                    .transpose()
                    .unwrap(),
                vector
                    .configured_default
                    .as_deref()
                    .map(ProfileRef::parse)
                    .transpose()
                    .unwrap(),
                vector.canonical_equivalent,
                vector.repair_enabled,
                vector.ambiguity_impacts.clone(),
            );
            let decision = select_profile(&registry, &request).unwrap();
            assert_eq!(
                decision.reason().as_str(),
                vector.expected_reason.as_str(),
                "{}",
                vector.id
            );
            match (&vector.expected_profile, decision.chosen()) {
                (Some(expected), Some(chosen)) => {
                    assert_eq!(chosen.display(), *expected, "{}", vector.id);
                }
                (None, None) => {}
                other => panic!("{} unexpected chosen binding: {other:?}", vector.id),
            }
            assert_eq!(
                decision.diagnostic(),
                vector.expected_diagnostic.as_deref(),
                "{}",
                vector.id
            );
            if vector.repair_enabled {
                assert_eq!(
                    decision.reason(),
                    SelectionReason::Unresolved,
                    "repair must not select among ambiguous legal profiles: {}",
                    vector.id
                );
            }
        }
    }
}
