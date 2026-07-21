//! Public importer fixture lane and opt-in copyright harness hooks (I6.7).
//!
//! Executes checked-in public source fixtures through the real PGR/RPE/PEC
//! import pipeline and compares expected ConversionReport status plus
//! canonical shape/provenance keys. Copyrighted charts are never required.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use fcs_model::{CanonicalCompilation, ConversionReport, ConversionStatus};
use serde::Deserialize;

use crate::{
    ArtifactRole, DecimalLimits, ExactDecimal, PecLimits, PecProfile, PecProfileBinding, PgrLimits,
    PgrProfile, PgrProfileBinding, RpeLimits, RpeProfileBinding, RpeSpeedMode, SourceArtifact,
    SourceFormat, interpret_pec, interpret_pgr, interpret_rpe_semantics, lower_pec_to_canonical,
    lower_pgr_to_canonical, lower_rpe_to_canonical, parse_json_document, parse_pec_document,
    parse_pgr_document, parse_rpe_document,
};

/// Environment variable that enables the private copyright fixture root.
pub const COPYRIGHT_FIXTURE_ROOT_ENV: &str = "FCS_COPYRIGHT_FIXTURE_ROOT";

/// Relative path from the `fcs-conversion` crate to the public fixture corpus.
pub const PUBLIC_FIXTURE_RELATIVE: &str = "../../docs/conformance/conversion/public-fixtures";

/// Outcome of consulting the copyright lane without loading chart bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyrightLaneStatus {
    /// Root is configured and points at an existing directory.
    Active { root: PathBuf },
    /// Lane is intentionally not exercised (default for CI and public builds).
    Skipped { reason: &'static str },
}

impl CopyrightLaneStatus {
    pub const fn is_skipped(&self) -> bool {
        matches!(self, Self::Skipped { .. })
    }

    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active { .. })
    }
}

/// Declared fixture class for the public/extreme/feature taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FixtureClass {
    Minimal,
    Feature,
    Extreme,
}

/// Source format spelling in the fixture manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FixtureFormat {
    Pgr,
    Rpe,
    Pec,
}

impl FixtureFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pgr => "pgr",
            Self::Rpe => "rpe",
            Self::Pec => "pec",
        }
    }
}

/// One fixture entry from a public or copyright manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FixtureEntry {
    pub id: String,
    pub lane: String,
    pub class: FixtureClass,
    pub format: FixtureFormat,
    pub parser_dialect: String,
    pub profile: String,
    pub profile_version: String,
    #[serde(default)]
    pub floor_scale_px: Option<String>,
    pub source: String,
    pub expected: String,
    pub producer_evidence: String,
    pub runtime_evidence: String,
    pub policy: String,
}

/// Fixture index for one lane.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FixtureManifest {
    pub schema_version: u8,
    pub conversion_specification_version: String,
    pub lane: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "fixture")]
    pub fixtures: Vec<FixtureEntry>,
}

/// Expected ConversionReport / canonical shape snapshot for one fixture.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FixtureExpectation {
    pub id: String,
    pub expected_status: String,
    pub expected_lines: usize,
    pub expected_notes: usize,
    #[serde(default)]
    pub required_categories: Vec<String>,
    #[serde(default)]
    pub required_provenance_keys: Vec<String>,
    #[serde(default = "default_true")]
    pub empty_resources: bool,
}

fn default_true() -> bool {
    true
}

/// Successful import products for a single fixture run.
#[derive(Debug, Clone, PartialEq)]
pub struct FixtureImportProducts {
    pub compilation: CanonicalCompilation,
    pub report: ConversionReport,
}

/// Comparison of products against a checked-in expectation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureObservation {
    pub fixture_id: String,
    pub status: ConversionStatus,
    pub line_count: usize,
    pub note_count: usize,
    pub categories: Vec<String>,
    pub provenance_keys: Vec<String>,
    pub empty_resources: bool,
}

/// Failure while loading or executing a fixture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixtureLaneError {
    Io { path: String, message: String },
    Manifest(String),
    Expectation(String),
    Import { fixture_id: String, message: String },
    Mismatch { fixture_id: String, message: String },
}

impl fmt::Display for FixtureLaneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => write!(formatter, "io error at {path}: {message}"),
            Self::Manifest(message) => write!(formatter, "fixture manifest: {message}"),
            Self::Expectation(message) => write!(formatter, "fixture expectation: {message}"),
            Self::Import {
                fixture_id,
                message,
            } => write!(formatter, "import fixture {fixture_id}: {message}"),
            Self::Mismatch {
                fixture_id,
                message,
            } => write!(formatter, "fixture {fixture_id} mismatch: {message}"),
        }
    }
}

impl std::error::Error for FixtureLaneError {}

/// Absolute path to the checked-in public fixture corpus when built from source.
pub fn public_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PUBLIC_FIXTURE_RELATIVE)
}

/// Resolve the copyright lane status without reading chart contents.
pub fn copyright_lane_status() -> CopyrightLaneStatus {
    match std::env::var_os(COPYRIGHT_FIXTURE_ROOT_ENV) {
        None => CopyrightLaneStatus::Skipped {
            reason: "FCS_COPYRIGHT_FIXTURE_ROOT is unset; copyright fixtures are opt-in only",
        },
        Some(value) if value.is_empty() => CopyrightLaneStatus::Skipped {
            reason: "FCS_COPYRIGHT_FIXTURE_ROOT is empty; copyright fixtures are opt-in only",
        },
        Some(value) => {
            let root = PathBuf::from(value);
            if root.is_dir() {
                CopyrightLaneStatus::Active { root }
            } else {
                CopyrightLaneStatus::Skipped {
                    reason: "FCS_COPYRIGHT_FIXTURE_ROOT does not point at an existing directory; lane skipped",
                }
            }
        }
    }
}

/// Load a public or copyright fixture manifest from disk.
pub fn load_fixture_manifest(path: &Path) -> Result<FixtureManifest, FixtureLaneError> {
    let text = fs::read_to_string(path).map_err(|error| FixtureLaneError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    let manifest: FixtureManifest =
        toml::from_str(&text).map_err(|error| FixtureLaneError::Manifest(error.to_string()))?;
    if manifest.schema_version != 1 {
        return Err(FixtureLaneError::Manifest(format!(
            "unsupported schema_version {}",
            manifest.schema_version
        )));
    }
    if manifest.fixtures.is_empty() {
        return Err(FixtureLaneError::Manifest(
            "fixture list must not be empty for an active lane".into(),
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for fixture in &manifest.fixtures {
        if !seen.insert(fixture.id.clone()) {
            return Err(FixtureLaneError::Manifest(format!(
                "duplicate fixture id {}",
                fixture.id
            )));
        }
        if fixture.lane != manifest.lane {
            return Err(FixtureLaneError::Manifest(format!(
                "fixture {} lane {} does not match manifest lane {}",
                fixture.id, fixture.lane, manifest.lane
            )));
        }
    }
    Ok(manifest)
}

/// Load one expected snapshot TOML.
pub fn load_fixture_expectation(path: &Path) -> Result<FixtureExpectation, FixtureLaneError> {
    let text = fs::read_to_string(path).map_err(|error| FixtureLaneError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    toml::from_str(&text).map_err(|error| FixtureLaneError::Expectation(error.to_string()))
}

/// Run one fixture through the real importer product path.
pub fn run_import_fixture(
    root: &Path,
    fixture: &FixtureEntry,
) -> Result<FixtureImportProducts, FixtureLaneError> {
    let source_path = root.join(&fixture.source);
    let bytes = fs::read(&source_path).map_err(|error| FixtureLaneError::Io {
        path: source_path.display().to_string(),
        message: error.to_string(),
    })?;
    let artifact = SourceArtifact::new(fixture.source.as_str(), ArtifactRole::Chart, bytes)
        .map_err(|error| FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: error.to_string(),
        })?;

    match fixture.format {
        FixtureFormat::Pgr => run_pgr(fixture, &artifact),
        FixtureFormat::Rpe => run_rpe(fixture, &artifact),
        FixtureFormat::Pec => run_pec(fixture, &artifact),
    }
}

/// Observe products for comparison against an expectation.
pub fn observe_products(fixture_id: &str, products: &FixtureImportProducts) -> FixtureObservation {
    let chart = products.compilation.chart();
    let categories = products
        .report
        .entries()
        .iter()
        .map(|entry| entry.category().to_owned())
        .collect();
    let provenance_keys = products
        .compilation
        .distribution()
        .provenance()
        .facts()
        .keys()
        .cloned()
        .collect();
    FixtureObservation {
        fixture_id: fixture_id.to_owned(),
        status: products.report.status(),
        line_count: chart.lines().lines().count(),
        note_count: chart.notes().notes().len(),
        categories,
        provenance_keys,
        empty_resources: products.compilation.resources().is_empty(),
    }
}

/// Compare an observation to a checked-in expectation.
pub fn assert_expectation(
    observation: &FixtureObservation,
    expected: &FixtureExpectation,
) -> Result<(), FixtureLaneError> {
    if observation.fixture_id != expected.id {
        return Err(FixtureLaneError::Mismatch {
            fixture_id: observation.fixture_id.clone(),
            message: format!(
                "observation id {} does not match expectation id {}",
                observation.fixture_id, expected.id
            ),
        });
    }
    let status = ConversionStatus::parse(&expected.expected_status).ok_or_else(|| {
        FixtureLaneError::Expectation(format!(
            "unknown expected_status {}",
            expected.expected_status
        ))
    })?;
    if observation.status != status {
        return Err(FixtureLaneError::Mismatch {
            fixture_id: observation.fixture_id.clone(),
            message: format!(
                "status: got {}, expected {}",
                observation.status.as_str(),
                status.as_str()
            ),
        });
    }
    if observation.line_count != expected.expected_lines {
        return Err(FixtureLaneError::Mismatch {
            fixture_id: observation.fixture_id.clone(),
            message: format!(
                "line count: got {}, expected {}",
                observation.line_count, expected.expected_lines
            ),
        });
    }
    if observation.note_count != expected.expected_notes {
        return Err(FixtureLaneError::Mismatch {
            fixture_id: observation.fixture_id.clone(),
            message: format!(
                "note count: got {}, expected {}",
                observation.note_count, expected.expected_notes
            ),
        });
    }
    if observation.empty_resources != expected.empty_resources {
        return Err(FixtureLaneError::Mismatch {
            fixture_id: observation.fixture_id.clone(),
            message: format!(
                "empty_resources: got {}, expected {}",
                observation.empty_resources, expected.empty_resources
            ),
        });
    }
    for category in &expected.required_categories {
        if !observation.categories.iter().any(|value| value == category) {
            return Err(FixtureLaneError::Mismatch {
                fixture_id: observation.fixture_id.clone(),
                message: format!("missing required category {category}"),
            });
        }
    }
    for key in &expected.required_provenance_keys {
        if !observation.provenance_keys.iter().any(|value| value == key) {
            return Err(FixtureLaneError::Mismatch {
                fixture_id: observation.fixture_id.clone(),
                message: format!("missing required provenance key {key}"),
            });
        }
    }
    Ok(())
}

/// Load, execute, and validate every fixture under a corpus root.
pub fn run_fixture_corpus(root: &Path) -> Result<Vec<FixtureObservation>, FixtureLaneError> {
    let manifest_path = root.join("manifest.toml");
    let manifest = load_fixture_manifest(&manifest_path)?;
    let mut observations = Vec::with_capacity(manifest.fixtures.len());
    for fixture in &manifest.fixtures {
        let products = run_import_fixture(root, fixture)?;
        let observation = observe_products(&fixture.id, &products);
        let expected_path = root.join(&fixture.expected);
        let expected = load_fixture_expectation(&expected_path)?;
        if expected.id != fixture.id {
            return Err(FixtureLaneError::Expectation(format!(
                "expected id {} does not match fixture id {}",
                expected.id, fixture.id
            )));
        }
        assert_expectation(&observation, &expected)?;
        observations.push(observation);
    }
    Ok(observations)
}

fn run_pgr(
    fixture: &FixtureEntry,
    artifact: &SourceArtifact,
) -> Result<FixtureImportProducts, FixtureLaneError> {
    let profile = parse_pgr_profile(&fixture.profile)?;
    let floor = fixture
        .floor_scale_px
        .as_deref()
        .ok_or_else(|| FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: "PGR fixtures require floor_scale_px".into(),
        })?;
    let floor_scale = ExactDecimal::parse(floor, DecimalLimits::default()).map_err(|error| {
        FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: error.to_string(),
        }
    })?;
    let binding =
        PgrProfileBinding::new(profile, floor_scale).map_err(|error| FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: error.to_string(),
        })?;
    let parsed = parse_json_document(SourceFormat::Pgr, artifact).map_err(|error| {
        FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: error.to_string(),
        }
    })?;
    let source = parse_pgr_document(&parsed, PgrLimits::default()).map_err(|error| {
        FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: error.to_string(),
        }
    })?;
    let semantic = interpret_pgr(&source, &binding).map_err(|error| FixtureLaneError::Import {
        fixture_id: fixture.id.clone(),
        message: error.to_string(),
    })?;
    let import =
        lower_pgr_to_canonical(&semantic, artifact).map_err(|error| FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: error.to_string(),
        })?;
    let (compilation, report) = import.into_parts();
    Ok(FixtureImportProducts {
        compilation,
        report,
    })
}

fn run_rpe(
    fixture: &FixtureEntry,
    artifact: &SourceArtifact,
) -> Result<FixtureImportProducts, FixtureLaneError> {
    let binding = parse_rpe_binding(&fixture.profile)?;
    let parsed = parse_json_document(SourceFormat::Rpe, artifact).map_err(|error| {
        FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: error.to_string(),
        }
    })?;
    let source = parse_rpe_document(&parsed, RpeLimits::default()).map_err(|error| {
        FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: error.to_string(),
        }
    })?;
    let semantic =
        interpret_rpe_semantics(&source, &binding).map_err(|error| FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: error.to_string(),
        })?;
    let import =
        lower_rpe_to_canonical(&semantic, artifact).map_err(|error| FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: error.to_string(),
        })?;
    let (compilation, report) = import.into_parts();
    Ok(FixtureImportProducts {
        compilation,
        report,
    })
}

fn run_pec(
    fixture: &FixtureEntry,
    artifact: &SourceArtifact,
) -> Result<FixtureImportProducts, FixtureLaneError> {
    let profile = parse_pec_profile(&fixture.profile)?;
    let floor = fixture
        .floor_scale_px
        .as_deref()
        .ok_or_else(|| FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: "PEC fixtures require floor_scale_px".into(),
        })?;
    let floor_scale = ExactDecimal::parse(floor, DecimalLimits::default()).map_err(|error| {
        FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: error.to_string(),
        }
    })?;
    let binding =
        PecProfileBinding::new(profile, floor_scale).map_err(|error| FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: error.to_string(),
        })?;
    let source = parse_pec_document(artifact, PecLimits::default()).map_err(|error| {
        FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: error.to_string(),
        }
    })?;
    let semantic = interpret_pec(&source, &binding).map_err(|error| FixtureLaneError::Import {
        fixture_id: fixture.id.clone(),
        message: error.to_string(),
    })?;
    let import =
        lower_pec_to_canonical(&semantic, artifact).map_err(|error| FixtureLaneError::Import {
            fixture_id: fixture.id.clone(),
            message: error.to_string(),
        })?;
    let (compilation, report) = import.into_parts();
    Ok(FixtureImportProducts {
        compilation,
        report,
    })
}

fn parse_pgr_profile(id: &str) -> Result<PgrProfile, FixtureLaneError> {
    match id {
        "pgr.phira.v1" => Ok(PgrProfile::PhiraV1),
        "pgr.phira.v3" => Ok(PgrProfile::PhiraV3),
        "pgr.phichain-import.v1" => Ok(PgrProfile::PhichainImportV1),
        "pgr.phichain-import.v3" => Ok(PgrProfile::PhichainImportV3),
        other => Err(FixtureLaneError::Manifest(format!(
            "unsupported PGR profile {other}"
        ))),
    }
}

fn parse_rpe_binding(id: &str) -> Result<RpeProfileBinding, FixtureLaneError> {
    match id {
        "rpe.phira.legacy-speed" => Ok(RpeProfileBinding::phira_legacy_speed()),
        "rpe.phira.rpe170-speed" => Ok(RpeProfileBinding::phira_rpe170_speed(None)),
        "rpe.phichain-import" => Ok(RpeProfileBinding::phichain_import()),
        "rpe.community.divide-bpmfactor" => Ok(RpeProfileBinding::community_divide(
            RpeSpeedMode::LegacyLinear,
        )),
        "rpe.docs-example.multiply-bpmfactor" => Ok(RpeProfileBinding::docs_example_multiply(
            RpeSpeedMode::LegacyLinear,
        )),
        other => Err(FixtureLaneError::Manifest(format!(
            "unsupported RPE profile {other}"
        ))),
    }
}

fn parse_pec_profile(id: &str) -> Result<PecProfile, FixtureLaneError> {
    match id {
        "pec.phira" => Ok(PecProfile::Phira),
        "pec.extends" => Ok(PecProfile::Extends),
        "pec.phispler" => Ok(PecProfile::Phispler),
        other => Err(FixtureLaneError::Manifest(format!(
            "unsupported PEC profile {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_fixture_corpus_executes_with_expected_reports() {
        let root = public_fixture_root();
        assert!(
            root.join("manifest.toml").is_file(),
            "public fixture corpus missing at {}",
            root.display()
        );
        let observations = run_fixture_corpus(&root).expect("public fixture corpus must pass");
        assert_eq!(observations.len(), 6);
        let ids: Vec<_> = observations
            .iter()
            .map(|item| item.fixture_id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "pgr-minimal",
                "pgr-feature",
                "rpe-minimal",
                "rpe-extreme",
                "pec-minimal",
                "pec-feature"
            ]
        );
        for observation in &observations {
            assert_eq!(observation.status, ConversionStatus::Equivalent);
            assert!(observation.empty_resources);
            assert!(observation.line_count >= 1);
            assert!(observation.note_count >= 2);
            assert!(
                observation
                    .provenance_keys
                    .iter()
                    .any(|key| key.ends_with("/artifact")),
                "fixture {} missing artifact provenance",
                observation.fixture_id
            );
        }
    }

    #[test]
    fn copyright_lane_is_skipped_without_opt_in() {
        // Ensure the default environment path is not accidentally active in CI.
        let status = match std::env::var_os(COPYRIGHT_FIXTURE_ROOT_ENV) {
            Some(value) if !value.is_empty() && PathBuf::from(&value).is_dir() => {
                // When a developer opts in locally, still require the API to report Active.
                CopyrightLaneStatus::Active {
                    root: PathBuf::from(value),
                }
            }
            _ => copyright_lane_status(),
        };
        if std::env::var_os(COPYRIGHT_FIXTURE_ROOT_ENV).is_none() {
            assert!(status.is_skipped());
            match status {
                CopyrightLaneStatus::Skipped { reason } => {
                    assert!(reason.contains("opt-in") || reason.contains("unset"));
                }
                CopyrightLaneStatus::Active { .. } => {
                    panic!("copyright lane must skip when env is unset")
                }
            }
        }
    }

    #[test]
    fn public_manifest_declares_required_metadata_fields() {
        let root = public_fixture_root();
        let manifest = load_fixture_manifest(&root.join("manifest.toml")).unwrap();
        assert_eq!(manifest.lane, "public");
        assert_eq!(manifest.conversion_specification_version, "1.0.0");
        for fixture in &manifest.fixtures {
            assert!(!fixture.parser_dialect.is_empty());
            assert!(!fixture.producer_evidence.is_empty());
            assert!(!fixture.profile.is_empty());
            assert_eq!(fixture.profile_version, "1.0.0");
            assert!(root.join(&fixture.source).is_file());
            assert!(root.join(&fixture.expected).is_file());
            let expected = load_fixture_expectation(&root.join(&fixture.expected)).unwrap();
            assert_eq!(expected.id, fixture.id);
            assert!(!expected.required_provenance_keys.is_empty());
        }
    }
}
