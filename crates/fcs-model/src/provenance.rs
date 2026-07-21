//! Source-free distribution provenance and CanonicalCompilation aggregation.
//!
//! I5.6 owns restricted provenance facts, explicit origin state, mapping-rule
//! references, stale-dependency tracking, DistributionMetadata, and the
//! CanonicalCompilation product boundary. Full ConversionReport/repair
//! aggregation remains I5.7; FCBC section encoding remains I7.
//!
//! Nothing in this module may retain source AST nodes, workspace absolute
//! paths, raw source snapshots, authoring expansion graphs, or resource
//! payload copies.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::{CanonicalChart, CanonicalObject, CanonicalResourceBundle};

/// Closed origin-state set from Conversion Specification §5.2.
///
/// Origin must be recorded explicitly. Implementations must not infer origin
/// by comparing a value to a default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OriginState {
    Unset,
    ExplicitDefault,
    ExplicitValue,
    Inherited,
    Imported,
    Generated,
    UserModified,
}

impl OriginState {
    pub const ALL: [Self; 7] = [
        Self::Unset,
        Self::ExplicitDefault,
        Self::ExplicitValue,
        Self::Inherited,
        Self::Imported,
        Self::Generated,
        Self::UserModified,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unset => "unset",
            Self::ExplicitDefault => "explicit-default",
            Self::ExplicitValue => "explicit-value",
            Self::Inherited => "inherited",
            Self::Imported => "imported",
            Self::Generated => "generated",
            Self::UserModified => "user-modified",
        }
    }

    pub fn parse(spelling: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|state| state.as_str() == spelling)
    }
}

impl fmt::Display for OriginState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Optional semantic-status vocabulary used by restricted facts.
///
/// Report aggregation and status roll-up remain I5.7-owned. This enum only
/// records the closed Conversion §5.4 spellings when a fact carries one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SemanticStatus {
    Native,
    Mapped,
    Equivalent,
    Approximated,
    Preserved,
    RuntimeOnly,
    Repaired,
    Dropped,
    Unsupported,
}

impl SemanticStatus {
    pub const ALL: [Self; 9] = [
        Self::Native,
        Self::Mapped,
        Self::Equivalent,
        Self::Approximated,
        Self::Preserved,
        Self::RuntimeOnly,
        Self::Repaired,
        Self::Dropped,
        Self::Unsupported,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Mapped => "mapped",
            Self::Equivalent => "equivalent",
            Self::Approximated => "approximated",
            Self::Preserved => "preserved",
            Self::RuntimeOnly => "runtime-only",
            Self::Repaired => "repaired",
            Self::Dropped => "dropped",
            Self::Unsupported => "unsupported",
        }
    }

    pub fn parse(spelling: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == spelling)
    }
}

/// Stable mapping-rule identity retained by restricted provenance facts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MappingRuleRef {
    id: String,
}

impl MappingRuleRef {
    pub fn new(id: impl Into<String>) -> Result<Self, ProvenanceError> {
        let id = id.into();
        if id.is_empty() {
            return Err(ProvenanceError::EmptyMappingRuleId);
        }
        if !id.is_ascii() || id.chars().any(|ch| ch.is_ascii_control()) {
            return Err(ProvenanceError::InvalidMappingRuleId(id));
        }
        Ok(Self { id })
    }

    pub fn as_str(&self) -> &str {
        &self.id
    }
}

/// Logical source locator that never accepts host absolute paths or URIs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LogicalSourceLocator {
    path: String,
}

impl LogicalSourceLocator {
    pub fn new(path: impl Into<String>) -> Result<Self, ProvenanceError> {
        let path = path.into();
        validate_logical_source_locator(&path)?;
        Ok(Self { path })
    }

    pub fn as_str(&self) -> &str {
        &self.path
    }
}

fn validate_logical_source_locator(path: &str) -> Result<(), ProvenanceError> {
    if path.is_empty() {
        return Err(ProvenanceError::EmptySourceLocator);
    }
    if path.contains('\0') || path.contains('\\') {
        return Err(ProvenanceError::InvalidSourceLocator(path.to_owned()));
    }
    if path.contains("://")
        || path.starts_with("file:")
        || path.starts_with("FILE:")
        || path.starts_with('/')
        || looks_like_windows_absolute(path)
    {
        return Err(ProvenanceError::AbsoluteOrUriSourceLocator(path.to_owned()));
    }
    for component in path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(ProvenanceError::InvalidSourceLocator(path.to_owned()));
        }
    }
    Ok(())
}

fn looks_like_windows_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    matches!(bytes, [drive, b':', ..] if drive.is_ascii_alphabetic())
}

/// One restricted, source-free provenance fact for DistributionMetadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedProvenanceFact {
    id: String,
    source_artifact_id: Option<String>,
    source_locator: Option<LogicalSourceLocator>,
    source_value: Option<String>,
    source_order: Option<u64>,
    mapping_rule_ref: Option<MappingRuleRef>,
    origin_state: OriginState,
    semantic_status: Option<SemanticStatus>,
    dependencies: BTreeSet<String>,
    stale: bool,
}

impl RestrictedProvenanceFact {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        source_artifact_id: Option<String>,
        source_locator: Option<LogicalSourceLocator>,
        source_value: Option<String>,
        source_order: Option<u64>,
        mapping_rule_ref: Option<MappingRuleRef>,
        origin_state: OriginState,
        semantic_status: Option<SemanticStatus>,
        dependencies: impl IntoIterator<Item = String>,
    ) -> Result<Self, ProvenanceError> {
        let id = id.into();
        if id.is_empty() {
            return Err(ProvenanceError::EmptyFactId);
        }
        if let Some(artifact) = &source_artifact_id
            && artifact.is_empty()
        {
            return Err(ProvenanceError::EmptySourceArtifactId);
        }
        let dependencies = dependencies.into_iter().collect::<BTreeSet<_>>();
        if dependencies.iter().any(String::is_empty) {
            return Err(ProvenanceError::EmptyDependencyId);
        }
        if dependencies.contains(&id) {
            return Err(ProvenanceError::SelfDependency(id));
        }
        Ok(Self {
            id,
            source_artifact_id,
            source_locator,
            source_value,
            source_order,
            mapping_rule_ref,
            origin_state,
            semantic_status,
            dependencies,
            stale: false,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn source_artifact_id(&self) -> Option<&str> {
        self.source_artifact_id.as_deref()
    }

    pub fn source_locator(&self) -> Option<&LogicalSourceLocator> {
        self.source_locator.as_ref()
    }

    pub fn source_value(&self) -> Option<&str> {
        self.source_value.as_deref()
    }

    pub const fn source_order(&self) -> Option<u64> {
        self.source_order
    }

    pub fn mapping_rule_ref(&self) -> Option<&MappingRuleRef> {
        self.mapping_rule_ref.as_ref()
    }

    pub const fn origin_state(&self) -> OriginState {
        self.origin_state
    }

    pub const fn semantic_status(&self) -> Option<SemanticStatus> {
        self.semantic_status
    }

    pub fn dependencies(&self) -> &BTreeSet<String> {
        &self.dependencies
    }

    pub const fn is_stale(&self) -> bool {
        self.stale
    }

    /// Marks this fact as a user-modified canonical edit without staling peers.
    pub fn mark_user_modified(&mut self) {
        self.origin_state = OriginState::UserModified;
    }
}

/// Content-hash fact retained by DistributionMetadata.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InputContentHash {
    algorithm: String,
    digest_lower_hex: String,
    logical_source: Option<LogicalSourceLocator>,
}

impl InputContentHash {
    pub fn sha256_lower_hex(
        digest_lower_hex: impl Into<String>,
        logical_source: Option<LogicalSourceLocator>,
    ) -> Result<Self, ProvenanceError> {
        let digest_lower_hex = digest_lower_hex.into();
        if digest_lower_hex.len() != 64
            || !digest_lower_hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(ProvenanceError::InvalidInputHashDigest(digest_lower_hex));
        }
        Ok(Self {
            algorithm: "sha256".into(),
            digest_lower_hex,
            logical_source,
        })
    }

    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    pub fn digest_lower_hex(&self) -> &str {
        &self.digest_lower_hex
    }

    pub fn logical_source(&self) -> Option<&LogicalSourceLocator> {
        self.logical_source.as_ref()
    }
}

/// Deterministic provenance graph with stale-dependency propagation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProvenanceGraph {
    facts: BTreeMap<String, RestrictedProvenanceFact>,
}

impl ProvenanceGraph {
    pub fn new(
        facts: impl IntoIterator<Item = RestrictedProvenanceFact>,
    ) -> Result<Self, ProvenanceError> {
        let mut graph = Self::empty();
        for fact in facts {
            let id = fact.id().to_owned();
            if graph.facts.contains_key(&id) {
                return Err(ProvenanceError::DuplicateFactId(id));
            }
            graph.facts.insert(id, fact);
        }
        graph.validate_dependency_closure()?;
        Ok(graph)
    }

    pub fn empty() -> Self {
        Self {
            facts: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, fact: RestrictedProvenanceFact) -> Result<(), ProvenanceError> {
        let id = fact.id().to_owned();
        if self.facts.contains_key(&id) {
            return Err(ProvenanceError::DuplicateFactId(id));
        }
        self.facts.insert(id.clone(), fact);
        if let Err(error) = self.validate_dependency_closure() {
            self.facts.remove(&id);
            return Err(error);
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&RestrictedProvenanceFact> {
        self.facts.get(id)
    }

    pub fn facts(&self) -> &BTreeMap<String, RestrictedProvenanceFact> {
        &self.facts
    }

    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    pub fn validate_dependency_closure(&self) -> Result<(), ProvenanceError> {
        for fact in self.facts.values() {
            for dependency in fact.dependencies() {
                if !self.facts.contains_key(dependency) {
                    return Err(ProvenanceError::MissingDependency {
                        fact_id: fact.id().to_owned(),
                        dependency_id: dependency.clone(),
                    });
                }
            }
        }
        if let Some(cycle) = self.first_cycle() {
            return Err(ProvenanceError::DependencyCycle(cycle));
        }
        Ok(())
    }

    /// Marks the edited fact `user-modified` and stales every dependent fact.
    ///
    /// Dependencies flow from prerequisite → dependent: if A is listed in B's
    /// dependency set, editing A stales B. The edited fact itself is not
    /// automatically stale unless already recorded as stale.
    pub fn mark_user_modified_and_stale_dependents(
        &mut self,
        fact_id: &str,
    ) -> Result<BTreeSet<String>, ProvenanceError> {
        if !self.facts.contains_key(fact_id) {
            return Err(ProvenanceError::UnknownFactId(fact_id.to_owned()));
        }
        self.facts
            .get_mut(fact_id)
            .expect("fact presence checked")
            .mark_user_modified();

        let mut stale = BTreeSet::new();
        let mut stack = vec![fact_id.to_owned()];
        while let Some(current) = stack.pop() {
            let dependents = self
                .facts
                .values()
                .filter(|fact| fact.dependencies().contains(&current))
                .map(|fact| fact.id().to_owned())
                .collect::<Vec<_>>();
            for dependent in dependents {
                if stale.insert(dependent.clone()) {
                    stack.push(dependent);
                }
            }
        }
        for id in &stale {
            self.facts
                .get_mut(id)
                .expect("dependent ids come from the graph")
                .stale = true;
        }
        Ok(stale)
    }

    fn first_cycle(&self) -> Option<Vec<String>> {
        let mut state = BTreeMap::<&str, u8>::new();
        let mut stack = Vec::new();
        for id in self.facts.keys() {
            if let Some(cycle) = self.dfs_cycle(id, &mut state, &mut stack) {
                return Some(cycle);
            }
        }
        None
    }

    fn dfs_cycle(
        &self,
        id: &str,
        state: &mut BTreeMap<&str, u8>,
        stack: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        match state.get(id).copied() {
            Some(1) => {
                let start = stack.iter().position(|entry| entry == id).unwrap_or(0);
                let mut cycle = stack[start..].to_vec();
                cycle.push(id.to_owned());
                return Some(cycle);
            }
            Some(2) => return None,
            _ => {}
        }
        state.insert(id, 1);
        stack.push(id.to_owned());
        if let Some(fact) = self.facts.get(id) {
            for dependency in fact.dependencies() {
                if let Some(cycle) = self.dfs_cycle(dependency, state, stack) {
                    return Some(cycle);
                }
            }
        }
        stack.pop();
        state.insert(id, 2);
        None
    }
}

/// Optional distribution metadata that never changes chart or resource execution.
///
/// FCBC §16.5 standard key order is `provenance`, `repairRecords`,
/// `inputHashes`, `custom`. Repair-record product aggregation remains I5.7, so
/// this surface keeps empty repair capacity out of the public product until
/// that unit owns typed repair records.
#[derive(Debug, Clone, PartialEq)]
pub struct DistributionMetadata {
    provenance: ProvenanceGraph,
    input_hashes: Vec<InputContentHash>,
    custom: CanonicalObject,
}

impl DistributionMetadata {
    pub fn new(
        provenance: ProvenanceGraph,
        input_hashes: Vec<InputContentHash>,
        custom: CanonicalObject,
    ) -> Result<Self, ProvenanceError> {
        provenance.validate_dependency_closure()?;
        Ok(Self {
            provenance,
            input_hashes,
            custom,
        })
    }

    /// Native FCS compile distribution surface with no conversion provenance.
    pub fn empty() -> Self {
        Self {
            provenance: ProvenanceGraph::empty(),
            input_hashes: Vec::new(),
            custom: CanonicalObject::new(Vec::new()).expect("empty object is valid"),
        }
    }

    pub fn provenance(&self) -> &ProvenanceGraph {
        &self.provenance
    }

    pub fn input_hashes(&self) -> &[InputContentHash] {
        &self.input_hashes
    }

    pub fn custom(&self) -> &CanonicalObject {
        &self.custom
    }

    pub fn is_empty(&self) -> bool {
        self.provenance.is_empty()
            && self.input_hashes.is_empty()
            && self.custom.entries().is_empty()
    }
}

/// FCS §17 aggregate product handed to the separately versioned FCBC writer.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalCompilation {
    chart: CanonicalChart,
    resources: CanonicalResourceBundle,
    distribution: DistributionMetadata,
}

impl CanonicalCompilation {
    pub fn new(
        chart: CanonicalChart,
        resources: CanonicalResourceBundle,
        distribution: DistributionMetadata,
    ) -> Self {
        Self {
            chart,
            resources,
            distribution,
        }
    }

    pub const fn chart(&self) -> &CanonicalChart {
        &self.chart
    }

    pub const fn resources(&self) -> &CanonicalResourceBundle {
        &self.resources
    }

    pub const fn distribution(&self) -> &DistributionMetadata {
        &self.distribution
    }

    /// Returns chart and resources without distribution metadata.
    ///
    /// Stripping distribution must not be required to recover execution identity.
    pub fn without_distribution(self) -> (CanonicalChart, CanonicalResourceBundle) {
        (self.chart, self.resources)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvenanceError {
    EmptyFactId,
    EmptySourceArtifactId,
    EmptyDependencyId,
    EmptyMappingRuleId,
    InvalidMappingRuleId(String),
    EmptySourceLocator,
    InvalidSourceLocator(String),
    AbsoluteOrUriSourceLocator(String),
    InvalidInputHashDigest(String),
    DuplicateFactId(String),
    MissingDependency {
        fact_id: String,
        dependency_id: String,
    },
    UnknownFactId(String),
    SelfDependency(String),
    DependencyCycle(Vec<String>),
}

impl fmt::Display for ProvenanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFactId => write!(formatter, "provenance fact id must not be empty"),
            Self::EmptySourceArtifactId => {
                write!(
                    formatter,
                    "source artifact id must not be empty when present"
                )
            }
            Self::EmptyDependencyId => write!(formatter, "dependency id must not be empty"),
            Self::EmptyMappingRuleId => write!(formatter, "mapping rule id must not be empty"),
            Self::InvalidMappingRuleId(id) => {
                write!(formatter, "mapping rule id is not valid ASCII: {id}")
            }
            Self::EmptySourceLocator => {
                write!(formatter, "logical source locator must not be empty")
            }
            Self::InvalidSourceLocator(path) => {
                write!(formatter, "logical source locator is invalid: {path}")
            }
            Self::AbsoluteOrUriSourceLocator(path) => write!(
                formatter,
                "logical source locator must not be absolute or a URI: {path}"
            ),
            Self::InvalidInputHashDigest(digest) => write!(
                formatter,
                "input hash digest must be 64 lowercase hex digits: {digest}"
            ),
            Self::DuplicateFactId(id) => {
                write!(formatter, "duplicate provenance fact id: {id}")
            }
            Self::MissingDependency {
                fact_id,
                dependency_id,
            } => write!(
                formatter,
                "provenance fact {fact_id} depends on missing fact {dependency_id}"
            ),
            Self::UnknownFactId(id) => write!(formatter, "unknown provenance fact id: {id}"),
            Self::SelfDependency(id) => {
                write!(formatter, "provenance fact cannot depend on itself: {id}")
            }
            Self::DependencyCycle(cycle) => write!(
                formatter,
                "provenance dependency cycle: {}",
                cycle.join(" -> ")
            ),
        }
    }
}

impl std::error::Error for ProvenanceError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line::CanonicalLineGraph;
    use crate::note::CanonicalNoteSet;
    use crate::track::CanonicalTrackSet;
    use crate::{
        Beat, CanonicalMetadata, CanonicalProfile, CanonicalScrollSet, CanonicalSourceVersion,
        ChartTimeMap, TempoPoint,
    };

    fn empty_chart() -> CanonicalChart {
        let time_map = ChartTimeMap::new([TempoPoint {
            beat: Beat::zero(),
            bpm: 120.0,
        }])
        .unwrap();
        CanonicalChart::new(
            CanonicalSourceVersion::new("5.0.0").unwrap(),
            CanonicalProfile::Chart,
            [],
            time_map,
            CanonicalMetadata::new(
                None,
                Default::default(),
                Vec::new(),
                Default::default(),
                None,
                None,
            ),
            CanonicalLineGraph::new([]).unwrap(),
            CanonicalNoteSet::new(Vec::new()).unwrap(),
            CanonicalTrackSet::new(Vec::new()).unwrap(),
            CanonicalScrollSet::new(Vec::new()).unwrap(),
            [],
        )
    }

    fn fact(
        id: &str,
        origin: OriginState,
        dependencies: impl IntoIterator<Item = String>,
    ) -> RestrictedProvenanceFact {
        RestrictedProvenanceFact::new(
            id,
            None,
            Some(LogicalSourceLocator::new("chart.json").unwrap()),
            Some(id.into()),
            Some(0),
            Some(MappingRuleRef::new("demo.rule").unwrap()),
            origin,
            Some(SemanticStatus::Mapped),
            dependencies,
        )
        .unwrap()
    }

    #[test]
    fn origin_states_are_the_closed_conversion_set() {
        assert_eq!(OriginState::ALL.len(), 7);
        assert_eq!(
            OriginState::parse("user-modified"),
            Some(OriginState::UserModified)
        );
        assert_eq!(
            OriginState::parse("explicit-default"),
            Some(OriginState::ExplicitDefault)
        );
        assert_eq!(OriginState::parse("default"), None);
    }

    #[test]
    fn logical_source_locator_rejects_absolute_uri_and_escape_paths() {
        assert!(LogicalSourceLocator::new("chart.json").is_ok());
        assert!(LogicalSourceLocator::new("package/chart.json").is_ok());
        for invalid in [
            "",
            "/tmp/chart.json",
            "C:/chart.json",
            "file:///tmp/chart.json",
            "https://example.test/chart.json",
            "package/../chart.json",
            "package//chart.json",
            r"package\chart.json",
        ] {
            assert!(
                LogicalSourceLocator::new(invalid).is_err(),
                "locator should reject {invalid:?}"
            );
        }
    }

    #[test]
    fn stale_propagation_follows_dependency_edges_only() {
        let mut graph = ProvenanceGraph::new([
            fact("root", OriginState::Imported, []),
            fact("child", OriginState::Imported, ["root".into()]),
            fact("grandchild", OriginState::Imported, ["child".into()]),
            fact("independent", OriginState::ExplicitValue, []),
        ])
        .unwrap();

        let staled = graph
            .mark_user_modified_and_stale_dependents("root")
            .unwrap();
        assert_eq!(
            staled,
            BTreeSet::from(["child".into(), "grandchild".into()])
        );
        assert_eq!(
            graph.get("root").unwrap().origin_state(),
            OriginState::UserModified
        );
        assert!(!graph.get("root").unwrap().is_stale());
        assert!(graph.get("child").unwrap().is_stale());
        assert!(graph.get("grandchild").unwrap().is_stale());
        assert!(!graph.get("independent").unwrap().is_stale());
        assert_eq!(
            graph.get("independent").unwrap().origin_state(),
            OriginState::ExplicitValue
        );
    }

    #[test]
    fn compilation_separates_distribution_from_execution_products() {
        let chart = empty_chart();
        let resources = CanonicalResourceBundle::new(Vec::new()).unwrap();
        let distribution = DistributionMetadata::new(
            ProvenanceGraph::new([fact("title", OriginState::ExplicitValue, [])]).unwrap(),
            vec![InputContentHash::sha256_lower_hex("a".repeat(64), None).unwrap()],
            CanonicalObject::new(Vec::new()).unwrap(),
        )
        .unwrap();
        let compilation = CanonicalCompilation::new(chart.clone(), resources.clone(), distribution);
        assert!(!compilation.distribution().is_empty());
        let (stripped_chart, stripped_resources) = compilation.without_distribution();
        assert_eq!(stripped_chart, chart);
        assert_eq!(stripped_resources, resources);
    }

    #[test]
    fn dependency_cycle_and_missing_edges_are_rejected() {
        let a = fact("a", OriginState::Unset, ["b".into()]);
        let b = fact("b", OriginState::Unset, ["a".into()]);
        assert!(matches!(
            ProvenanceGraph::new([a, b]),
            Err(ProvenanceError::DependencyCycle(_))
        ));

        let orphan = fact("orphan", OriginState::Unset, ["missing".into()]);
        assert!(matches!(
            ProvenanceGraph::new([orphan]),
            Err(ProvenanceError::MissingDependency { .. })
        ));
    }
}
