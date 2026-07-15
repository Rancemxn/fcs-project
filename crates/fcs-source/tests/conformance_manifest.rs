use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::de::DeserializeOwned;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RootManifest {
    schema_version: u32,
    freeze_baseline: String,
    suite: Vec<SuiteEntry>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SuiteEntry {
    id: String,
    specification: String,
    version: String,
    manifest: String,
    mutations: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FcsManifest {
    schema_version: u32,
    fcs_version: String,
    fixture: Vec<FixtureEntry>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureEntry {
    id: String,
    path: String,
    stage: FixtureStage,
    expect: FixtureExpectation,
    diagnostic: Option<String>,
    expected: Option<String>,
    vector: Option<String>,
    limits: Option<FixtureLimits>,
    trace_contains: Option<Vec<String>>,
    clauses: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum FixtureStage {
    Parse,
    Elaborate,
    Canonical,
    Evaluate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum FixtureExpectation {
    Success,
    Error,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureLimits {
    #[serde(rename = "maxGeneratorIterations")]
    max_generator_iterations: Option<usize>,
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_manifest<T: DeserializeOwned>(path: &Path) -> T {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {} as UTF-8: {error}", path.display()));
    toml::from_str(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn load_manifests() -> (RootManifest, FcsManifest) {
    let conformance = repository_root().join("conformance");
    (
        load_manifest(&conformance.join("manifest.toml")),
        load_manifest(&conformance.join("fcs5/manifest.toml")),
    )
}

fn assert_regular_file_below(
    base: &Path,
    relative: &str,
    canonical_conformance: &Path,
    description: &str,
) {
    let relative_path = Path::new(relative);
    assert!(
        !relative_path.as_os_str().is_empty(),
        "{description} path must not be empty"
    );
    assert!(
        relative_path
            .components()
            .all(|component| matches!(component, Component::Normal(_))),
        "{description} path must be a component-normalized relative path: {relative}"
    );

    let resolved = base.join(relative_path);
    let canonical = resolved.canonicalize().unwrap_or_else(|error| {
        panic!(
            "{description} does not resolve to an existing file ({}): {error}",
            resolved.display()
        )
    });
    assert!(
        canonical.starts_with(canonical_conformance) && canonical != canonical_conformance,
        "{description} escapes the conformance directory: {}",
        canonical.display()
    );
    assert!(
        canonical.is_file(),
        "{description} is not a regular file: {}",
        canonical.display()
    );
}

fn fixture<'a>(manifest: &'a FcsManifest, id: &str) -> &'a FixtureEntry {
    manifest
        .fixture
        .iter()
        .find(|fixture| fixture.id == id)
        .unwrap_or_else(|| panic!("missing Frozen fixture {id}"))
}

#[test]
fn typed_manifests_load_with_frozen_counts() {
    let (root, fcs) = load_manifests();

    assert_eq!(root.schema_version, 1);
    assert_eq!(fcs.schema_version, 1);
    assert_eq!(root.suite.len(), 6);
    assert_eq!(fcs.fixture.len(), 22);
}

#[test]
fn manifests_preserve_integrity_invariants() {
    let (root, fcs) = load_manifests();
    let conformance = repository_root().join("conformance");
    let canonical_conformance = conformance
        .canonicalize()
        .expect("conformance directory must exist");
    let fcs_base = conformance.join("fcs5");

    assert_eq!(root.freeze_baseline, "2026-07-14");
    assert_eq!(fcs.fcs_version, "5.0.0");

    let mut suite_ids = HashSet::new();
    for suite in &root.suite {
        assert!(!suite.id.is_empty(), "suite ID must not be empty");
        assert!(
            suite_ids.insert(&suite.id),
            "duplicate suite ID: {}",
            suite.id
        );
        assert!(
            !suite.specification.is_empty(),
            "suite {} must name its specification",
            suite.id
        );
        assert!(
            !suite.version.is_empty(),
            "suite {} must name its version",
            suite.id
        );
        assert_regular_file_below(
            &conformance,
            &suite.manifest,
            &canonical_conformance,
            &format!("suite {} manifest", suite.id),
        );
        if let Some(mutations) = &suite.mutations {
            assert_regular_file_below(
                &conformance,
                mutations,
                &canonical_conformance,
                &format!("suite {} mutations", suite.id),
            );
        }
    }

    let mut fixture_ids = HashSet::new();
    for fixture in &fcs.fixture {
        assert!(!fixture.id.is_empty(), "fixture ID must not be empty");
        assert!(
            fixture_ids.insert(&fixture.id),
            "duplicate fixture ID: {}",
            fixture.id
        );
        assert_regular_file_below(
            &fcs_base,
            &fixture.path,
            &canonical_conformance,
            &format!("fixture {} source", fixture.id),
        );
        for (kind, reference) in [
            ("expected", fixture.expected.as_deref()),
            ("vector", fixture.vector.as_deref()),
        ] {
            if let Some(reference) = reference {
                assert_regular_file_below(
                    &fcs_base,
                    reference,
                    &canonical_conformance,
                    &format!("fixture {} {kind}", fixture.id),
                );
            }
        }

        match fixture.expect {
            FixtureExpectation::Success => assert!(
                fixture.diagnostic.is_none(),
                "successful fixture {} must not name a diagnostic",
                fixture.id
            ),
            FixtureExpectation::Error => {
                let diagnostic = fixture
                    .diagnostic
                    .as_deref()
                    .unwrap_or_else(|| panic!("error fixture {} needs a diagnostic", fixture.id));
                assert!(
                    !diagnostic.is_empty(),
                    "error fixture {} needs a nonempty diagnostic",
                    fixture.id
                );
                assert!(
                    !diagnostic.starts_with("implementation."),
                    "fixture {} uses a temporary implementation diagnostic",
                    fixture.id
                );
            }
        }
        assert!(
            !fixture.clauses.is_empty(),
            "fixture {} must cite at least one clause",
            fixture.id
        );

        if let Some(limits) = &fixture.limits {
            assert!(
                limits.max_generator_iterations.is_some(),
                "fixture {} has an empty limits table",
                fixture.id
            );
        }
        if let Some(trace) = &fixture.trace_contains {
            assert!(
                !trace.is_empty(),
                "fixture {} has an empty trace expectation",
                fixture.id
            );
        }
    }

    let bare_range = fixture(&fcs, "source.invalid.bare-range");
    assert_eq!(bare_range.stage, FixtureStage::Parse);
    assert_eq!(bare_range.expect, FixtureExpectation::Error);
    assert_eq!(
        bare_range.diagnostic.as_deref(),
        Some("syntax.invalid-token")
    );

    let zero_step = fixture(&fcs, "source.invalid.generator-zero-step");
    assert_eq!(zero_step.stage, FixtureStage::Elaborate);
    assert_eq!(zero_step.expect, FixtureExpectation::Error);
    assert_eq!(
        zero_step.diagnostic.as_deref(),
        Some("compile-time.zero-step")
    );

    let generator = fixture(&fcs, "source.valid.compile-time-generator");
    assert_eq!(generator.stage, FixtureStage::Elaborate);
    assert_eq!(generator.expect, FixtureExpectation::Success);
    assert_eq!(
        generator.expected.as_deref(),
        Some("expected/compile-time-generator.json")
    );
}
