use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crc::{CRC_32_ISO_HDLC, Crc};
use fcs_runtime::{ScrollEvaluationError, evaluate_line_scroll, evaluate_note_distance};
use fcs_source::ast::{Beat, ExpandedSourceDocument, Type, TypedValue};
use fcs_source::diagnostic::{Diagnostic, DiagnosticStage, ExpansionTraceKind};
use fcs_source::elaborator::{CompileTimeLimits, elaborate};
use fcs_source::parser::parse_document;
use fcs_source::schema::phase2_schema;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

const FCBC_SECTION_CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RootManifest {
    schema_version: u32,
    candidate_baseline: String,
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
struct FcbcManifest {
    schema_version: u32,
    fcbc_version: String,
    execution_abi_version: String,
    fixture: Vec<FcbcFixtureEntry>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FcbcFixtureEntry {
    id: String,
    manifest: String,
    mutations: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FcbcGoldenManifest {
    schema_version: u32,
    id: String,
    fcbc_version: String,
    execution_abi_version: String,
    source_fcs_version: String,
    container_profile: String,
    document_profile: String,
    chart_count: u32,
    resource_count: usize,
    exact_descriptors_only: bool,
    expect: FixtureExpectation,
    path: String,
    decoded_length: u64,
    sha256: String,
    execution: Option<FcbcExecutionExpectation>,
    #[serde(default)]
    resource: Vec<FcbcResourceEntry>,
    section: Vec<FcbcSectionEntry>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FcbcExecutionExpectation {
    vector: String,
    constant_count: usize,
    descriptor_count: usize,
    expression_node_count: usize,
    distance_count: usize,
    line_count: usize,
    note_count: usize,
    descriptor_kinds: Vec<String>,
    distance_classifications: Vec<String>,
    lazy_opcodes: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FcbcResourceEntry {
    canonical_textual_id: String,
    id_namespace: String,
    id: String,
    kind: String,
    media_type: String,
    data_offset: u64,
    data_length: u64,
    payload_hex: String,
    sha256: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FcbcSectionEntry {
    r#type: u32,
    name: String,
    offset: u64,
    length: u64,
    crc32: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FcbcMutationManifest {
    schema_version: u32,
    base: String,
    mutation: Vec<FcbcMutationEntry>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FcbcMutationEntry {
    id: String,
    diagnostic: String,
    patch: Vec<FcbcMutationPatch>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FcbcMutationPatch {
    offset: u64,
    replace_hex: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderManifest {
    schema_version: u32,
    render_profile_version: String,
    fcbc_version: String,
    execution_abi_version: String,
    hash_algorithm: String,
    #[serde(default)]
    binary_fixture: Vec<RenderBinaryFixtureEntry>,
    fixture: Vec<RenderFixtureEntry>,
    source_fixture: Vec<RenderSourceFixtureEntry>,
    binding_fixture: Vec<RenderBindingFixtureEntry>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderBinaryFixtureEntry {
    id: String,
    manifest: String,
    vector: String,
    mutations: String,
    semantic_expected: String,
    raster_expected: String,
    assets: Vec<String>,
    clauses: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderFixtureEntry {
    id: String,
    source: String,
    chart_time_seconds: f64,
    semantic_expected: String,
    raster_expected: String,
    width: u32,
    height: u32,
    color_space: String,
    pixel_format: String,
    clauses: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderSourceFixtureEntry {
    id: String,
    source: String,
    expect: FixtureExpectation,
    diagnostic: Option<String>,
    clauses: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderBindingFixtureEntry {
    id: String,
    source: String,
    workspace_root: String,
    resource_asset: String,
    semantic_expected: String,
    canonical_resource_id: String,
    content_sha256: String,
    payload_length: u64,
    fcbc_resources_section_type: u32,
    fcbc_resource_data_section_type: u32,
    decode: bool,
    clauses: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionManifest {
    schema_version: u32,
    conversion_specification_version: String,
    profile_registry: String,
    parser_dialect_registry: String,
    mapping_rule_registry: String,
    diagnostic_registry: String,
    mapping_vectors: String,
    selection_vectors: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionProfileRegistry {
    schema_version: u32,
    conversion_specification_version: String,
    hash_algorithm: String,
    profile: Vec<ConversionProfileRegistryEntry>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionProfileRegistryEntry {
    id: String,
    version: String,
    format: String,
    directions: Vec<String>,
    profile_class: String,
    strict_eligible: bool,
    path: String,
    content_sha256: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionProfileDescriptor {
    schema_version: u32,
    id: String,
    version: String,
    format: String,
    directions: Vec<String>,
    profile_class: String,
    strict_eligible: bool,
    producer: String,
    runtime: String,
    format_version_policy: String,
    format_versions: Vec<String>,
    parser_dialects: Vec<String>,
    parameters: Vec<ConversionProfileParameter>,
    mapping_rules: Vec<String>,
    contract: Vec<String>,
    known_limitations: Vec<String>,
    report_categories: Vec<String>,
    evidence: Vec<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionProfileParameter {
    name: String,
    value_type: String,
    required_when: String,
    constraint: String,
    allowed_values: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionDialectRegistry {
    schema_version: u32,
    conversion_specification_version: String,
    hash_algorithm: String,
    dialect: Vec<ConversionDialectEntry>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionDialectEntry {
    id: String,
    version: String,
    format: String,
    contract: String,
    content_sha256: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionMappingRuleRegistry {
    schema_version: u32,
    conversion_specification_version: String,
    hash_algorithm: String,
    rule: Vec<ConversionMappingRuleEntry>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionMappingRuleEntry {
    id: String,
    version: String,
    contract: String,
    content_sha256: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionDiagnosticRegistry {
    schema_version: u32,
    conversion_specification_version: String,
    category: Vec<ConversionDiagnosticEntry>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionDiagnosticEntry {
    id: String,
    uses: Vec<String>,
    domain: String,
    description: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionMappingVectors {
    schema_version: u32,
    conversion_specification_version: String,
    rule_registry: String,
    forbidden_rule_ids: Vec<String>,
    vector: Vec<ConversionMappingVector>,
    invalid: Vec<ConversionInvalidVector>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionMappingVector {
    id: String,
    rule_id: String,
    rule_version: String,
    source: toml::Value,
    expected: String,
    unit: String,
    clauses: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionInvalidVector {
    id: String,
    rule_id: String,
    rule_version: String,
    source: toml::Value,
    diagnostic: String,
    clauses: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionSelectionVectors {
    schema_version: u32,
    conversion_specification_version: String,
    profile_registry: String,
    selection: Vec<ConversionSelectionVector>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionSelectionVector {
    id: String,
    direction: String,
    format: String,
    syntax_mode: String,
    profile_selection_mode: String,
    explicit_profile: Option<String>,
    declared_profile: Option<String>,
    configured_default: Option<String>,
    candidate_bindings: Vec<ConversionSelectionBinding>,
    evidence: Vec<String>,
    canonical_equivalent: bool,
    repair_enabled: bool,
    expected_reason: String,
    expected_profile: Option<String>,
    expected_diagnostic: Option<String>,
    ambiguity_impacts: Vec<String>,
    clauses: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionSelectionBinding {
    profile: String,
    parameters: toml::Table,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureEntry {
    id: String,
    path: String,
    workspace_root: Option<String>,
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
    let conformance = repository_root().join("docs/conformance");
    (
        load_manifest(&conformance.join("manifest.toml")),
        load_manifest(&conformance.join("fcs5/manifest.toml")),
    )
}

fn load_render_manifest() -> RenderManifest {
    load_manifest(&repository_root().join("docs/conformance/render/manifest.toml"))
}

fn load_fcbc_manifest() -> FcbcManifest {
    load_manifest(&repository_root().join("docs/conformance/fcbc/manifest.toml"))
}

fn load_conversion_manifest() -> ConversionManifest {
    load_manifest(&repository_root().join("docs/conformance/conversion/manifest.toml"))
}

fn sha256_lower(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

fn versioned_ref(value: &str) -> (&str, &str) {
    let (id, version) = value
        .rsplit_once('@')
        .unwrap_or_else(|| panic!("versioned reference must contain @: {value}"));
    assert!(!id.is_empty(), "versioned reference ID must not be empty");
    assert!(
        !version.is_empty(),
        "versioned reference version must not be empty"
    );
    (id, version)
}

fn is_lower_hex(value: &str, expected_length: usize) -> bool {
    value.len() == expected_length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn decode_lower_hex(value: &str, description: &str) -> Vec<u8> {
    assert!(
        !value.is_empty()
            && value.len().is_multiple_of(2)
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{description} must be nonempty, even-length lowercase hex"
    );

    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |byte: u8| match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => unreachable!("validated lowercase hex"),
            };
            (digit(pair[0]) << 4) | digit(pair[1])
        })
        .collect()
}

fn decode_hex_file(path: &Path) -> Vec<u8> {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {} as UTF-8: {error}", path.display()));
    let digits: Vec<_> = source
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    assert!(
        digits.len().is_multiple_of(2),
        "hex fixture has an odd number of digits: {}",
        path.display()
    );

    digits
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char).to_digit(16).unwrap_or_else(|| {
                panic!("hex fixture contains a non-hex byte: {}", path.display())
            });
            let low = (pair[1] as char).to_digit(16).unwrap_or_else(|| {
                panic!("hex fixture contains a non-hex byte: {}", path.display())
            });
            ((high << 4) | low) as u8
        })
        .collect()
}

fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("checked four-byte FCBC field"),
    )
}

fn read_u16_le(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("checked two-byte FCBC field"),
    )
}

fn read_u64_le(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("checked eight-byte FCBC field"),
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

fn assert_directory_below(
    base: &Path,
    relative: &str,
    canonical_conformance: &Path,
    description: &str,
) -> PathBuf {
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
            "{description} does not resolve to an existing directory ({}): {error}",
            resolved.display()
        )
    });
    assert!(
        canonical.starts_with(canonical_conformance) && canonical != canonical_conformance,
        "{description} escapes the conformance directory: {}",
        canonical.display()
    );
    assert!(
        canonical.is_dir(),
        "{description} is not a directory: {}",
        canonical.display()
    );
    canonical
}

fn fixture<'a>(manifest: &'a FcsManifest, id: &str) -> &'a FixtureEntry {
    manifest
        .fixture
        .iter()
        .find(|fixture| fixture.id == id)
        .unwrap_or_else(|| panic!("missing bound fixture {id}"))
}

fn fixture_limits(fixture: &FixtureEntry) -> CompileTimeLimits {
    let mut limits = CompileTimeLimits::default();
    if let Some(fixture_limits) = &fixture.limits
        && let Some(max_generator_iterations) = fixture_limits.max_generator_iterations
    {
        limits.max_generator_iterations = max_generator_iterations;
    }
    limits
}

fn elaborate_fixture(
    fcs_base: &Path,
    fixture: &FixtureEntry,
) -> Result<ExpandedSourceDocument, Vec<Diagnostic>> {
    let source_path = fcs_base.join(&fixture.path);
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));
    let document = parse_document(&source)
        .into_result()
        .unwrap_or_else(|errors| {
            panic!("{} must parse before elaboration: {errors:?}", fixture.id)
        });
    elaborate(&document, phase2_schema(), fixture_limits(fixture))
}

fn canonical_track_fixture(
    fcs_base: &Path,
    fixture: &FixtureEntry,
) -> Result<fcs_model::CanonicalTrackSet, Vec<Diagnostic>> {
    let source_path = fcs_base.join(&fixture.path);
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));
    let document = parse_document(&source)
        .into_result()
        .unwrap_or_else(|errors| panic!("{} must parse: {errors:?}", fixture.id));
    let lines = document
        .canonical_line_graph()
        .unwrap_or_else(|errors| panic!("{} Line graph failed: {errors:?}", fixture.id));
    let expanded = elaborate(&document, phase2_schema(), fixture_limits(fixture))?;
    let time_map = expanded
        .canonical_time_map()
        .unwrap_or_else(|error| panic!("{} tempo map failed: {error}", fixture.id));
    expanded.canonical_tracks(&time_map, &lines)
}

fn canonical_scroll_fixture(
    fcs_base: &Path,
    fixture: &FixtureEntry,
) -> Result<fcs_model::CanonicalScrollSet, Vec<Diagnostic>> {
    let source_path = fcs_base.join(&fixture.path);
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));
    let document = parse_document(&source)
        .into_result()
        .unwrap_or_else(|errors| panic!("{} must parse: {errors:?}", fixture.id));
    let expanded = elaborate(&document, phase2_schema(), fixture_limits(fixture))?;
    let time_map = expanded
        .canonical_time_map()
        .unwrap_or_else(|error| panic!("{} tempo map failed: {error}", fixture.id));
    document.canonical_scroll_set(&time_map)
}

fn expected_json(fcs_base: &Path, fixture: &FixtureEntry) -> serde_json::Value {
    let expected = fixture
        .expected
        .as_deref()
        .unwrap_or_else(|| panic!("{} must bind an expected output", fixture.id));
    let path = fcs_base.join(expected);
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&source)
        .unwrap_or_else(|error| panic!("failed to parse {} as JSON: {error}", path.display()))
}

fn note_entities(document: &ExpandedSourceDocument) -> Vec<&fcs_source::ast::ExpandedEntity> {
    document
        .collections()
        .find(|collection| collection.name() == "notes")
        .unwrap_or_else(|| panic!("expanded output must contain notes collection"))
        .entities()
        .collect()
}

#[test]
fn typed_manifests_load_with_bound_counts() {
    let (root, fcs) = load_manifests();
    let fcbc = load_fcbc_manifest();
    let render = load_render_manifest();
    let conversion = load_conversion_manifest();

    assert_eq!(root.schema_version, 2);
    assert_eq!(fcs.schema_version, 2);
    assert_eq!(fcbc.schema_version, 2);
    assert_eq!(render.schema_version, 3);
    assert_eq!(conversion.schema_version, 2);
    assert_eq!(root.suite.len(), 6);
    assert_eq!(fcs.fixture.len(), 42);
    assert_eq!(fcbc.fixture.len(), 3);
    assert_eq!(render.binary_fixture.len(), 0);
    assert_eq!(render.fixture.len(), 1);
    assert_eq!(render.source_fixture.len(), 3);
    assert_eq!(render.binding_fixture.len(), 1);
}

#[test]
fn fcs_source_fixtures_execute_at_the_declared_frontend_boundary() {
    let (_, fcs) = load_manifests();
    let fcs_base = repository_root().join("docs/conformance/fcs5");
    let mut parse_success = 0;
    let mut parse_error = 0;
    let mut later_stage = 0;

    for fixture in &fcs.fixture {
        let path = fcs_base.join(&fixture.path);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let output = parse_document(&source);

        match fixture.stage {
            FixtureStage::Parse => match fixture.expect {
                FixtureExpectation::Success => {
                    parse_success += 1;
                    output.into_result().unwrap_or_else(|errors| {
                        panic!("{} must parse successfully: {errors:?}", fixture.id)
                    });
                }
                FixtureExpectation::Error => {
                    parse_error += 1;
                    assert!(
                        output.output().is_none(),
                        "{} must not expose a partial AST",
                        fixture.id
                    );
                    let errors = output
                        .into_result()
                        .expect_err("fixture must fail to parse");
                    let expected = fixture
                        .diagnostic
                        .as_deref()
                        .expect("parse-error fixture has a manifest category");
                    assert!(
                        errors
                            .iter()
                            .any(|diagnostic| diagnostic.code().as_str() == expected),
                        "{} expected {expected}, got {errors:?}",
                        fixture.id
                    );
                    assert!(errors.iter().all(|diagnostic| {
                        let span = diagnostic.primary_span();
                        span.start <= span.end
                            && span.end <= source.len()
                            && source.is_char_boundary(span.start)
                            && source.is_char_boundary(span.end)
                    }));
                }
            },
            FixtureStage::Elaborate | FixtureStage::Canonical | FixtureStage::Evaluate => {
                later_stage += 1;
                output.into_result().unwrap_or_else(|errors| {
                    panic!(
                        "{} ({:?}) must be accepted by the I1 parser: {errors:?}",
                        fixture.id, fixture.stage
                    )
                });
            }
        }
    }

    assert_eq!(parse_success, 3);
    assert_eq!(parse_error, 9);
    assert_eq!(later_stage, 30);
}

#[test]
fn i2_public_conformance_fixtures_execute_through_the_elaborator() {
    let (_, fcs) = load_manifests();
    let fcs_base = repository_root().join("docs/conformance/fcs5");

    let generator_fixture = fixture(&fcs, "source.valid.compile-time-generator");
    let generator = elaborate_fixture(&fcs_base, generator_fixture)
        .expect("compile-time generator fixture must elaborate");
    generator
        .validate_invariants()
        .expect("generator output must satisfy the expanded boundary");
    let generator_expected = expected_json(&fcs_base, generator_fixture);
    assert_eq!(generator_expected["collection"], "notes");
    assert_eq!(generator_expected["entityType"], "Note");
    assert_eq!(
        generator_expected["forbiddenExpandedNodes"],
        serde_json::json!([
            "const", "let", "fn", "template", "with", "if", "generate", "emit", "range", "index"
        ])
    );
    let generator_entities = note_entities(&generator);
    assert_eq!(
        generator_expected["count"].as_u64(),
        Some(generator_entities.len() as u64)
    );
    let expected_times = generator_expected["times"]
        .as_array()
        .expect("generator expected times must be an array");
    assert_eq!(expected_times.len(), generator_entities.len());
    for (entity, expected_time) in generator_entities.iter().zip(expected_times) {
        assert_eq!(entity.entity_type(), &Type::Note);
        assert!(entity.is_lowered());
        let numerator = expected_time
            .get("beatNumerator")
            .and_then(serde_json::Value::as_i64)
            .expect("generator beat numerator must be an integer");
        let denominator = expected_time
            .get("beatDenominator")
            .and_then(serde_json::Value::as_i64)
            .expect("generator beat denominator must be an integer");
        assert_eq!(
            entity.field("gameplay.time").expect("time field").value(),
            &TypedValue::Beat(Beat::new(numerator, denominator).unwrap())
        );
    }

    let template_fixture = fixture(&fcs, "source.valid.template-if-with");
    let template = elaborate_fixture(&fcs_base, template_fixture)
        .expect("template-if-with fixture must elaborate");
    template
        .validate_invariants()
        .expect("template output must satisfy the expanded boundary");
    let template_expected = expected_json(&fcs_base, template_fixture);
    let template_entities = note_entities(&template);
    assert_eq!(
        template_expected["noteCount"].as_u64(),
        Some(template_entities.len() as u64)
    );
    assert_eq!(
        template_entities[0]
            .field("gameplay.judgment.enabled")
            .expect("judgment field")
            .value(),
        &TypedValue::Bool(
            template_expected["judgmentEnabled"]
                .as_bool()
                .expect("judgmentEnabled must be bool")
        )
    );
    assert_eq!(
        template_entities[0]
            .field("presentation.alpha")
            .expect("alpha field")
            .value(),
        &TypedValue::Float(
            template_expected["alpha"]
                .as_f64()
                .expect("alpha must be a number")
        )
    );
    assert_eq!(
        template_expected["forbiddenExpandedNodes"],
        serde_json::json!(["template", "if", "with"])
    );

    let descending_fixture = fixture(&fcs, "source.valid.int-range-descending");
    let descending = elaborate_fixture(&fcs_base, descending_fixture)
        .expect("descending range fixture must elaborate");
    descending
        .validate_invariants()
        .expect("descending output must satisfy the expanded boundary");
    let descending_expected = expected_json(&fcs_base, descending_fixture);
    let descending_entities = note_entities(&descending);
    assert_eq!(
        descending_expected["iterationCount"].as_u64(),
        Some(descending_entities.len() as u64)
    );
    let expected_indices = descending_expected["indices"]
        .as_array()
        .expect("descending indices must be an array");
    let expected_values = descending_expected["values"]
        .as_array()
        .expect("descending values must be an array");
    let expected_positions = descending_expected["positionXPx"]
        .as_array()
        .expect("descending positions must be an array");
    assert_eq!(expected_indices.len(), descending_entities.len());
    assert_eq!(expected_values.len(), descending_entities.len());
    assert_eq!(expected_positions.len(), descending_entities.len());
    for (index, entity) in descending_entities.iter().enumerate() {
        assert_eq!(expected_indices[index].as_i64(), Some(index as i64));
        assert_eq!(
            entity.field("gameplay.time").expect("time field").value(),
            &TypedValue::Beat(Beat::new(0, 1).unwrap())
        );
        let expected_position = expected_positions[index]
            .as_f64()
            .expect("position must be a number");
        assert_eq!(
            entity
                .field("presentation.positionX")
                .expect("position field")
                .value(),
            &TypedValue::Length(expected_position)
        );
        assert_eq!(expected_values[index].as_i64(), Some(4 - index as i64));
    }
}

#[test]
fn i2_elaborate_error_fixtures_keep_static_diagnostics_and_budget_trace() {
    let (_, fcs) = load_manifests();
    let fcs_base = repository_root().join("docs/conformance/fcs5");
    let ids = [
        "source.invalid.unresolved-schema-enum",
        "source.invalid.generator-zero-step",
        "source.invalid.shadowing",
        "source.invalid.template-missing-line",
        "source.invalid.runtime-gameplay",
        "source.invalid.generator-budget",
    ];

    for id in ids {
        let fixture = fixture(&fcs, id);
        assert_eq!(fixture.stage, FixtureStage::Elaborate, "{id}");
        assert_eq!(fixture.expect, FixtureExpectation::Error, "{id}");
        let expected = fixture
            .diagnostic
            .as_deref()
            .expect("elaborate error fixture must bind a diagnostic");
        let source = fs::read_to_string(fcs_base.join(&fixture.path))
            .unwrap_or_else(|error| panic!("failed to read fixture {id}: {error}"));
        let errors = elaborate_fixture(&fcs_base, fixture)
            .expect_err("invalid I2 fixture must not produce expanded output");
        assert_eq!(errors[0].code().as_str(), expected, "{id}");
        assert!(errors.iter().all(|diagnostic| {
            !matches!(diagnostic.stage(), DiagnosticStage::Parse)
                && diagnostic.primary_span().end <= source.len()
                && source.is_char_boundary(diagnostic.primary_span().start)
                && source.is_char_boundary(diagnostic.primary_span().end)
        }));

        if id == "source.invalid.generator-budget" {
            let diagnostic = &errors[0];
            let budget = diagnostic
                .budget()
                .expect("generator budget fixture must expose budget details");
            assert_eq!(budget.kind(), "max_generator_iterations");
            assert_eq!(budget.limit(), 2);
            assert_eq!(budget.observed(), 3);
            for expected_trace in fixture
                .trace_contains
                .as_deref()
                .expect("budget fixture must bind trace fragments")
            {
                match expected_trace.as_str() {
                    "collection=notes" => {
                        assert!(diagnostic.expansion_trace().iter().any(|frame| {
                            frame.kind() == ExpansionTraceKind::Collection
                                && frame.subject() == Some("notes")
                        }))
                    }
                    "index=2" => assert!(diagnostic.expansion_trace().iter().any(|frame| {
                        frame.kind() == ExpansionTraceKind::Index && frame.index() == Some(2)
                    })),
                    "emit=Note" => assert!(diagnostic.expansion_trace().iter().any(|frame| {
                        frame.kind() == ExpansionTraceKind::Emit
                            && frame.emitted_type() == Some("Note")
                    })),
                    other => panic!("unhandled bound trace fragment {other}"),
                }
            }
        }
    }
}

#[test]
fn i3_track_fixtures_execute_at_the_canonical_boundary() {
    let (_, fcs) = load_manifests();
    let fcs_base = repository_root().join("docs/conformance/fcs5");
    let valid = fixture(&fcs, "source.valid.track-boundaries");
    assert_eq!(valid.stage, FixtureStage::Canonical);
    assert_eq!(valid.expect, FixtureExpectation::Success);
    assert_eq!(
        canonical_track_fixture(&fcs_base, valid)
            .expect("valid Track fixture should lower")
            .tracks()
            .len(),
        1
    );

    let invalid = fixture(&fcs, "source.invalid.track-overlap");
    assert_eq!(invalid.stage, FixtureStage::Canonical);
    assert_eq!(invalid.expect, FixtureExpectation::Error);
    let errors = canonical_track_fixture(&fcs_base, invalid)
        .expect_err("overlapping Track fixture should fail");
    assert_eq!(
        errors[0].code().as_str(),
        invalid
            .diagnostic
            .as_deref()
            .expect("invalid Track fixture binds a diagnostic")
    );
}

#[test]
fn i3_scroll_fixture_executes_at_the_canonical_boundary() {
    let (_, fcs) = load_manifests();
    let fcs_base = repository_root().join("docs/conformance/fcs5");
    let fixture = fixture(&fcs, "source.valid.time-scroll-note");
    assert_eq!(fixture.stage, FixtureStage::Canonical);
    assert_eq!(fixture.expect, FixtureExpectation::Success);
    let scroll =
        canonical_scroll_fixture(&fcs_base, fixture).expect("valid scroll fixture should lower");
    assert_eq!(scroll.lines().len(), 1);
    assert_eq!(scroll.lines()[0].coordinate().coordinate(1.0), Ok(2.0));
}

#[test]
fn i4_scroll_inheritance_fixture_binds_literal_composition_vectors() {
    let (_, fcs) = load_manifests();
    let fcs_base = repository_root().join("docs/conformance/fcs5");
    let fixture = fixture(&fcs, "source.valid.scroll-inheritance");
    assert_eq!(fixture.stage, FixtureStage::Evaluate);
    assert_eq!(fixture.expect, FixtureExpectation::Success);
    assert_eq!(
        fixture.expected.as_deref(),
        Some("expected/scroll-inheritance.json")
    );

    let expected = expected_json(&fcs_base, fixture);
    assert_eq!(expected["schemaVersion"].as_u64(), Some(1));
    let queries = expected["queries"]
        .as_array()
        .expect("scroll inheritance expected queries must be an array");
    assert_eq!(queries.len(), 22);

    let source_path = fcs_base.join(&fixture.path);
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));
    let document = parse_document(&source)
        .into_result()
        .unwrap_or_else(|errors| panic!("{} must parse: {errors:?}", fixture.id));
    let expanded = elaborate(&document, phase2_schema(), fixture_limits(fixture))
        .unwrap_or_else(|errors| panic!("{} must elaborate: {errors:?}", fixture.id));
    let time_map = expanded
        .canonical_time_map()
        .unwrap_or_else(|error| panic!("{} tempo map failed: {error}", fixture.id));
    let lines = document
        .canonical_line_graph()
        .unwrap_or_else(|errors| panic!("{} Line graph failed: {errors:?}", fixture.id));
    let scroll = document
        .canonical_scroll_set(&time_map)
        .unwrap_or_else(|errors| panic!("{} scroll lowering failed: {errors:?}", fixture.id));
    let tracks = expanded
        .canonical_tracks(&time_map, &lines)
        .unwrap_or_else(|errors| panic!("{} Track lowering failed: {errors:?}", fixture.id));

    let literal_queries = [
        ("root", -2.0, -4.0, -1.0, 5.0, -1.0, 5.0, "4014000000000000"),
        ("root", 0.0, 0.0, -1.0, 3.0, -1.0, 3.0, "4008000000000000"),
        ("root", 1.0, 2.0, 0.0, 2.0, 0.0, 2.0, "4000000000000000"),
        ("root", 2.0, 4.0, 0.0, 2.0, 0.0, 2.0, "4000000000000000"),
        ("root", 3.0, 5.0, 1.0, 2.0, 1.0, 2.0, "4000000000000000"),
        ("root", 4.0, 6.0, 1.0, 3.0, 1.0, 3.0, "4008000000000000"),
        ("child", -2.0, -2.0, 1.0, 7.0, 0.0, 12.0, "4028000000000000"),
        ("child", 0.0, 0.0, 1.0, 9.0, 0.0, 12.0, "4028000000000000"),
        ("child", 1.0, 1.0, 1.0, 10.0, 1.0, 12.0, "4028000000000000"),
        ("child", 2.0, 2.0, 0.0, 11.0, 0.0, 13.0, "402a000000000000"),
        ("child", 4.0, 6.0, 0.0, 11.0, 1.0, 14.0, "402c000000000000"),
        (
            "grandchild",
            -2.0,
            -4.0,
            2.0,
            -6.0,
            2.0,
            6.0,
            "4018000000000000",
        ),
        (
            "grandchild",
            0.0,
            0.0,
            2.0,
            -2.0,
            2.0,
            10.0,
            "4024000000000000",
        ),
        (
            "grandchild",
            1.0,
            2.0,
            2.0,
            0.0,
            3.0,
            12.0,
            "4028000000000000",
        ),
        (
            "grandchild",
            2.0,
            4.0,
            2.0,
            2.0,
            2.0,
            15.0,
            "402e000000000000",
        ),
        (
            "grandchild",
            3.0,
            6.0,
            2.0,
            4.0,
            2.0,
            17.0,
            "4031000000000000",
        ),
        (
            "grandchild",
            4.0,
            8.0,
            2.0,
            6.0,
            3.0,
            20.0,
            "4034000000000000",
        ),
        ("detached", 0.0, 0.0, 2.0, 3.0, 2.0, 3.0, "4008000000000000"),
        (
            "detached",
            4.0,
            8.0,
            2.0,
            11.0,
            2.0,
            11.0,
            "4026000000000000",
        ),
        (
            "inherited_negative",
            0.0,
            0.0,
            0.0,
            0.0,
            -1.0,
            3.0,
            "4008000000000000",
        ),
        (
            "signed_zero",
            0.0,
            0.0,
            0.0,
            -0.0,
            0.0,
            -0.0,
            "8000000000000000",
        ),
        (
            "signed_zero",
            1.0,
            2.0,
            0.0,
            0.0,
            0.0,
            0.0,
            "0000000000000000",
        ),
    ];

    for (
        line,
        chart_time,
        local_q,
        local_velocity,
        local_floor,
        effective_velocity,
        effective_floor,
        effective_floor_bits,
    ) in literal_queries
    {
        let query = queries
            .iter()
            .find(|query| {
                query["line"].as_str() == Some(line)
                    && query["chartTime"].as_f64() == Some(chart_time)
            })
            .unwrap_or_else(|| panic!("missing scroll query {line} at {chart_time}s"));
        assert_eq!(query["localQ"].as_f64(), Some(local_q));
        assert_eq!(query["localVelocity"].as_f64(), Some(local_velocity));
        assert_eq!(query["localFloor"].as_f64(), Some(local_floor));
        assert_eq!(
            query["effectiveVelocity"].as_f64(),
            Some(effective_velocity)
        );
        assert_eq!(query["effectiveFloor"].as_f64(), Some(effective_floor));
        assert_eq!(
            query["effectiveFloorBits"].as_str(),
            Some(effective_floor_bits)
        );
        assert_eq!(
            format!(
                "{:016x}",
                query["effectiveFloor"].as_f64().unwrap().to_bits()
            ),
            effective_floor_bits
        );

        let line_id = lines
            .line_by_textual_id(line)
            .unwrap_or_else(|| panic!("missing canonical Line {line}"))
            .id();
        let actual = evaluate_line_scroll(&lines, &scroll, &tracks, line_id, chart_time)
            .unwrap_or_else(|error| {
                panic!("product scroll query {line} at {chart_time}s failed: {error}")
            });
        assert_eq!(actual.local_q(), local_q, "{line} at {chart_time}s");
        assert_eq!(
            actual.local_velocity(),
            local_velocity,
            "{line} at {chart_time}s"
        );
        assert_eq!(actual.local_floor(), local_floor, "{line} at {chart_time}s");
        assert_eq!(
            actual.effective_velocity(),
            effective_velocity,
            "{line} at {chart_time}s"
        );
        assert_eq!(
            actual.effective_floor(),
            effective_floor,
            "{line} at {chart_time}s"
        );
        assert_eq!(
            actual.effective_floor().to_bits(),
            u64::from_str_radix(effective_floor_bits, 16).unwrap()
        );
    }

    let signed_zero_origin = queries
        .iter()
        .find(|query| {
            query["line"].as_str() == Some("signed_zero")
                && query["chartTime"].as_f64() == Some(0.0)
        })
        .expect("signed-zero origin query");
    assert_eq!(
        signed_zero_origin["localFloor"].as_f64().unwrap().to_bits(),
        0x8000_0000_0000_0000
    );

    let signed_zero_non_origin = queries
        .iter()
        .find(|query| {
            query["line"].as_str() == Some("signed_zero")
                && query["chartTime"].as_f64() == Some(1.0)
        })
        .expect("signed-zero non-origin query");
    assert_eq!(
        signed_zero_non_origin["effectiveFloor"]
            .as_f64()
            .unwrap()
            .to_bits(),
        0
    );

    let note = &expected["note"];
    assert_eq!(note["id"].as_str(), Some("grandchild-note"));
    assert_eq!(note["line"].as_str(), Some("grandchild"));
    assert_eq!(note["effectiveFloorAtNote"].as_f64(), Some(20.0));
    assert_eq!(note["effectiveFloorAtQuery"].as_f64(), Some(10.0));
    assert_eq!(note["floorScale"].as_f64(), Some(10.0));
    assert_eq!(note["scrollFactor"].as_f64(), Some(0.5));
    assert_eq!(note["distancePx"].as_f64(), Some(50.0));
    assert_eq!(note["distanceBits"].as_str(), Some("4049000000000000"));
    assert_eq!(note["localYPx"].as_f64(), Some(50.0));

    let notes = expanded
        .canonical_notes(&time_map, &lines)
        .unwrap_or_else(|errors| panic!("{} Note lowering failed: {errors:?}", fixture.id));
    let note_entity = notes
        .note_by_textual_id("grandchild-note")
        .expect("grandchild note must lower canonically");
    let distance = evaluate_note_distance(&lines, &scroll, &tracks, note_entity, 0.0)
        .unwrap_or_else(|error| panic!("product Note distance failed: {error}"));
    assert_eq!(distance.distance(), note["distancePx"].as_f64().unwrap());
    assert_eq!(
        distance.distance().to_bits(),
        u64::from_str_radix(note["distanceBits"].as_str().unwrap(), 16).unwrap()
    );
    assert_eq!(distance.local_y(), note["localYPx"].as_f64().unwrap());

    assert_eq!(
        expected["isolatedError"]["line"].as_str(),
        Some("unrelated_gap")
    );
    assert_eq!(
        expected["isolatedError"]["category"].as_str(),
        Some("track.gap")
    );
    let isolated_id = lines
        .line_by_textual_id("unrelated_gap")
        .expect("isolated error Line must lower")
        .id();
    assert!(matches!(
        evaluate_line_scroll(&lines, &scroll, &tracks, isolated_id, 0.0),
        Err(ScrollEvaluationError::Track { .. })
    ));
}

#[test]
fn manifests_preserve_integrity_invariants() {
    let (root, fcs) = load_manifests();
    let fcbc = load_fcbc_manifest();
    let render = load_render_manifest();
    let conversion = load_conversion_manifest();
    let conformance = repository_root().join("docs/conformance");
    let canonical_conformance = conformance
        .canonicalize()
        .expect("conformance directory must exist");
    let fcs_base = conformance.join("fcs5");
    let fcbc_base = conformance.join("fcbc");
    let render_base = conformance.join("render");
    let conversion_base = conformance.join("conversion");

    assert_eq!(root.candidate_baseline, "2026-07-15-s15-cross-spec-closure");
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

    let execution_suite = root
        .suite
        .iter()
        .find(|suite| suite.id == "execution-abi")
        .expect("root manifest must bind the Execution ABI suite");
    assert_eq!(execution_suite.manifest, "fcbc/manifest.toml");
    let numeric_suite = root
        .suite
        .iter()
        .find(|suite| suite.id == "fcs-core-numeric")
        .expect("root manifest must bind the Core numeric suite");
    assert_eq!(numeric_suite.manifest, "fcs5/expected/numeric-vectors.toml");

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
        if let Some(workspace_root) = &fixture.workspace_root {
            let canonical_workspace = assert_directory_below(
                &fcs_base,
                workspace_root,
                &canonical_conformance,
                &format!("fixture {} workspace root", fixture.id),
            );
            let canonical_source =
                fcs_base
                    .join(&fixture.path)
                    .canonicalize()
                    .unwrap_or_else(|error| {
                        panic!(
                            "fixture {} source cannot be canonicalized: {error}",
                            fixture.id
                        )
                    });
            assert!(
                canonical_source.starts_with(&canonical_workspace),
                "fixture {} source is outside its workspace root: {}",
                fixture.id,
                canonical_source.display()
            );
        }
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

    assert_eq!(fcbc.fcbc_version, "2.0.0");
    assert_eq!(fcbc.execution_abi_version, "1.0.0");
    let expected_section_types: Vec<u32> = (1..=13).chain(std::iter::once(20)).collect();
    let mut fcbc_fixture_ids = HashSet::new();
    let mut execution_fixture_count = 0;
    for fixture in &fcbc.fixture {
        assert!(
            fcbc_fixture_ids.insert(&fixture.id),
            "duplicate FCBC fixture ID: {}",
            fixture.id
        );
        for (kind, reference) in [
            ("manifest", fixture.manifest.as_str()),
            ("mutations", fixture.mutations.as_str()),
        ] {
            assert_regular_file_below(
                &fcbc_base,
                reference,
                &canonical_conformance,
                &format!("FCBC fixture {} {kind}", fixture.id),
            );
        }

        let golden_path = fcbc_base.join(&fixture.manifest);
        let golden: FcbcGoldenManifest = load_manifest(&golden_path);
        assert_eq!(golden.schema_version, 2);
        assert_eq!(golden.id, fixture.id);
        assert_eq!(golden.fcbc_version, fcbc.fcbc_version);
        assert_eq!(golden.execution_abi_version, fcbc.execution_abi_version);
        assert_eq!(golden.source_fcs_version, "5.0.0");
        assert_eq!(golden.chart_count, 1);
        assert!(golden.exact_descriptors_only);
        assert_eq!(golden.expect, FixtureExpectation::Success);
        assert_eq!(golden.resource_count, golden.resource.len());
        assert!(is_lower_hex(&golden.sha256, 64));
        if let Some(execution) = &golden.execution {
            execution_fixture_count += 1;
            assert_eq!(fixture.id, "nonempty-execution");
            assert_eq!(golden.container_profile, "strict-runtime");
            assert_eq!(golden.document_profile, "chart");
            assert_regular_file_below(
                &fcbc_base,
                &execution.vector,
                &canonical_conformance,
                &format!("FCBC fixture {} execution vector", fixture.id),
            );
            for (name, count) in [
                ("constants", execution.constant_count),
                ("descriptors", execution.descriptor_count),
                ("expression nodes", execution.expression_node_count),
                ("distances", execution.distance_count),
                ("lines", execution.line_count),
                ("notes", execution.note_count),
            ] {
                assert!(count > 0, "FCBC fixture {} has no {name}", fixture.id);
            }
            for (name, values) in [
                ("descriptor kinds", &execution.descriptor_kinds),
                (
                    "distance classifications",
                    &execution.distance_classifications,
                ),
                ("lazy opcodes", &execution.lazy_opcodes),
            ] {
                assert!(
                    !values.is_empty() && values.iter().all(|value| !value.is_empty()),
                    "FCBC fixture {} has empty {name}",
                    fixture.id
                );
                let unique: HashSet<_> = values.iter().collect();
                assert_eq!(
                    unique.len(),
                    values.len(),
                    "FCBC fixture {} repeats a value in {name}",
                    fixture.id
                );
            }
        } else {
            assert!(
                matches!(fixture.id.as_str(), "minimal-runtime" | "embedded-resource"),
                "FCBC fixture {} needs an [execution] table",
                fixture.id
            );
            assert_eq!(golden.container_profile, "runtime");
            assert_eq!(golden.document_profile, "fragment");
        }
        assert_eq!(
            golden
                .section
                .iter()
                .map(|section| section.r#type)
                .collect::<Vec<_>>(),
            expected_section_types
        );

        assert_regular_file_below(
            &fcbc_base,
            &golden.path,
            &canonical_conformance,
            &format!("FCBC fixture {} golden", fixture.id),
        );
        let bytes = decode_hex_file(&fcbc_base.join(&golden.path));
        assert_eq!(bytes.len() as u64, golden.decoded_length);
        assert_eq!(
            sha256_lower(&bytes),
            golden.sha256,
            "FCBC fixture {} decoded SHA-256 mismatch",
            fixture.id
        );
        assert_eq!(&bytes[0..4], b"FCSB");
        assert_eq!(read_u16_le(&bytes, 4), 128);
        assert_eq!(read_u32_le(&bytes, 36), golden.section.len() as u32);
        assert_eq!(read_u64_le(&bytes, 40), 128);
        assert_eq!(read_u64_le(&bytes, 48), golden.decoded_length);

        let table_end = 128 + golden.section.len() * 40;
        let mut covered_end = table_end as u64;
        for (index, section) in golden.section.iter().enumerate() {
            assert!(!section.name.is_empty());
            assert!(is_lower_hex(&section.crc32, 8));
            assert_eq!(section.offset % 8, 0);
            assert!(section.offset >= covered_end);
            assert!(section.offset + section.length <= golden.decoded_length);
            assert!(
                bytes[covered_end as usize..section.offset as usize]
                    .iter()
                    .all(|byte| *byte == 0),
                "FCBC fixture {} has nonzero inter-section padding",
                fixture.id
            );

            let entry = 128 + index * 40;
            assert_eq!(read_u32_le(&bytes, entry), section.r#type);
            assert_eq!(read_u16_le(&bytes, entry + 4), 1);
            assert_eq!(read_u16_le(&bytes, entry + 10), 1);
            assert_eq!(bytes[entry + 12], 3);
            assert_eq!(read_u64_le(&bytes, entry + 16), section.offset);
            assert_eq!(read_u64_le(&bytes, entry + 24), section.length);
            let expected_crc =
                u32::from_str_radix(&section.crc32, 16).expect("validated section CRC hex");
            assert_eq!(read_u32_le(&bytes, entry + 32), expected_crc);
            assert_eq!(
                FCBC_SECTION_CRC.checksum(
                    &bytes[section.offset as usize..(section.offset + section.length) as usize]
                ),
                expected_crc,
                "FCBC fixture {} section {} CRC-32/ISO-HDLC mismatch",
                fixture.id,
                section.name
            );
            covered_end = section.offset + section.length;
        }
        assert_eq!(covered_end, golden.decoded_length);

        let resource_data = golden
            .section
            .iter()
            .find(|section| section.r#type == 20)
            .expect("FCBC schema 2 fixture must bind ResourceData");
        let mut resource_cursor = 0_u64;
        for resource in &golden.resource {
            resource_cursor = (resource_cursor + 7) & !7;
            assert_eq!(resource.data_offset, resource_cursor);
            assert_eq!(resource.data_length * 2, resource.payload_hex.len() as u64);
            assert!(!resource.canonical_textual_id.is_empty());
            assert_eq!(resource.id_namespace, "fcs.resource");
            assert!(is_lower_hex(&resource.id, 16));
            assert!(!resource.kind.is_empty() && !resource.media_type.is_empty());
            assert!(is_lower_hex(&resource.sha256, 64));

            let payload = decode_lower_hex(
                &resource.payload_hex,
                &format!("FCBC fixture {} resource payload", fixture.id),
            );
            let start = (resource_data.offset + resource.data_offset) as usize;
            let end = start + payload.len();
            let actual_payload = &bytes[start..end];
            assert_eq!(actual_payload, payload);
            assert_eq!(
                sha256_lower(actual_payload),
                resource.sha256,
                "FCBC fixture {} resource {} SHA-256 mismatch",
                fixture.id,
                resource.canonical_textual_id
            );
            resource_cursor += resource.data_length;
        }
        assert_eq!(resource_cursor, resource_data.length);

        let mutations: FcbcMutationManifest = load_manifest(&fcbc_base.join(&fixture.mutations));
        assert_eq!(mutations.schema_version, 2);
        assert_eq!(mutations.base, golden.path);
        assert!(!mutations.mutation.is_empty());
        let mut mutation_ids = HashSet::new();
        for mutation in &mutations.mutation {
            assert!(
                mutation_ids.insert(&mutation.id),
                "duplicate FCBC mutation ID: {}",
                mutation.id
            );
            assert!(mutation.diagnostic.starts_with("fcbc."));
            assert!(!mutation.patch.is_empty());
            let mut patch_ranges = Vec::new();
            for patch in &mutation.patch {
                let replacement = decode_lower_hex(
                    &patch.replace_hex,
                    &format!("FCBC mutation {} patch", mutation.id),
                );
                let end = patch.offset + replacement.len() as u64;
                assert!(end <= golden.decoded_length);
                patch_ranges.push((patch.offset, end));
            }
            patch_ranges.sort_unstable();
            assert!(
                patch_ranges.windows(2).all(|pair| pair[0].1 <= pair[1].0),
                "FCBC mutation {} has overlapping patches",
                mutation.id
            );
        }
    }
    assert_eq!(execution_fixture_count, 1);

    assert_eq!(render.render_profile_version, "1.0.0");
    assert_eq!(render.fcbc_version, "2.0.0");
    assert_eq!(render.execution_abi_version, "1.0.0");
    assert_eq!(render.hash_algorithm, "sha256");
    let mut render_ids = HashSet::new();
    for fixture in &render.binary_fixture {
        assert!(
            render_ids.insert(&fixture.id),
            "duplicate Render binary fixture ID: {}",
            fixture.id
        );
        for (kind, reference) in [
            ("manifest", fixture.manifest.as_str()),
            ("vector", fixture.vector.as_str()),
            ("mutations", fixture.mutations.as_str()),
            ("semantic expectation", fixture.semantic_expected.as_str()),
            ("raster expectation", fixture.raster_expected.as_str()),
        ] {
            assert_regular_file_below(
                &render_base,
                reference,
                &canonical_conformance,
                &format!("Render binary fixture {} {kind}", fixture.id),
            );
        }
        assert!(
            !fixture.assets.is_empty(),
            "Render binary fixture {} needs fixed assets",
            fixture.id
        );
        for asset in &fixture.assets {
            assert_regular_file_below(
                &render_base,
                asset,
                &canonical_conformance,
                &format!("Render binary fixture {} asset", fixture.id),
            );
        }
        assert!(
            !fixture.clauses.is_empty(),
            "Render binary fixture {} needs clauses",
            fixture.id
        );
    }
    for fixture in &render.fixture {
        assert!(
            render_ids.insert(&fixture.id),
            "duplicate Render fixture ID: {}",
            fixture.id
        );
        assert_regular_file_below(
            &render_base,
            &fixture.source,
            &canonical_conformance,
            &format!("Render fixture {} source", fixture.id),
        );
        assert_regular_file_below(
            &render_base,
            &fixture.semantic_expected,
            &canonical_conformance,
            &format!("Render fixture {} semantic expectation", fixture.id),
        );
        assert_regular_file_below(
            &render_base,
            &fixture.raster_expected,
            &canonical_conformance,
            &format!("Render fixture {} raster expectation", fixture.id),
        );
        assert!(
            fixture.chart_time_seconds.is_finite(),
            "Render fixture {} chart time must be finite",
            fixture.id
        );
        assert!(
            fixture.width > 0 && fixture.height > 0,
            "Render fixture {} dimensions must be positive",
            fixture.id
        );
        assert!(
            !fixture.color_space.is_empty()
                && !fixture.pixel_format.is_empty()
                && !fixture.clauses.is_empty(),
            "Render fixture {} needs format and clause metadata",
            fixture.id
        );
    }
    for fixture in &render.source_fixture {
        assert!(
            render_ids.insert(&fixture.id),
            "duplicate Render source fixture ID: {}",
            fixture.id
        );
        assert_regular_file_below(
            &render_base,
            &fixture.source,
            &canonical_conformance,
            &format!("Render source fixture {} source", fixture.id),
        );
        assert!(
            !fixture.clauses.is_empty(),
            "Render source fixture {} needs clauses",
            fixture.id
        );
        match fixture.expect {
            FixtureExpectation::Success => assert!(
                fixture.diagnostic.is_none(),
                "successful Render source fixture {} must not name a diagnostic",
                fixture.id
            ),
            FixtureExpectation::Error => assert!(
                fixture
                    .diagnostic
                    .as_deref()
                    .is_some_and(|diagnostic| !diagnostic.is_empty()),
                "error Render source fixture {} needs a diagnostic",
                fixture.id
            ),
        }
    }
    for fixture in &render.binding_fixture {
        assert!(
            render_ids.insert(&fixture.id),
            "duplicate Render binding fixture ID: {}",
            fixture.id
        );
        for (kind, reference) in [
            ("source", fixture.source.as_str()),
            ("resource asset", fixture.resource_asset.as_str()),
            ("semantic expectation", fixture.semantic_expected.as_str()),
        ] {
            assert_regular_file_below(
                &render_base,
                reference,
                &canonical_conformance,
                &format!("Render binding fixture {} {kind}", fixture.id),
            );
        }
        let workspace = assert_directory_below(
            &render_base,
            &fixture.workspace_root,
            &canonical_conformance,
            &format!("Render binding fixture {} workspace", fixture.id),
        );
        for (kind, reference) in [
            ("source", fixture.source.as_str()),
            ("resource asset", fixture.resource_asset.as_str()),
        ] {
            let canonical = render_base
                .join(reference)
                .canonicalize()
                .unwrap_or_else(|error| {
                    panic!(
                        "Render binding fixture {} {kind} cannot be canonicalized: {error}",
                        fixture.id
                    )
                });
            assert!(
                canonical.starts_with(&workspace),
                "Render binding fixture {} {kind} is outside its workspace",
                fixture.id
            );
        }

        let asset = fs::read(render_base.join(&fixture.resource_asset)).unwrap_or_else(|error| {
            panic!(
                "failed to read Render binding fixture {} asset: {error}",
                fixture.id
            )
        });
        assert_eq!(asset.len() as u64, fixture.payload_length);
        assert!(is_lower_hex(&fixture.canonical_resource_id, 16));
        assert!(is_lower_hex(&fixture.content_sha256, 64));
        assert_eq!(fixture.fcbc_resources_section_type, 6);
        assert_eq!(fixture.fcbc_resource_data_section_type, 20);
        assert!(!fixture.decode);
        assert!(!fixture.clauses.is_empty());

        let expected = fs::read_to_string(render_base.join(&fixture.semantic_expected))
            .unwrap_or_else(|error| {
                panic!(
                    "failed to read Render binding fixture {} expectation: {error}",
                    fixture.id
                )
            });
        assert!(expected.contains(&fixture.canonical_resource_id));
        assert!(expected.contains(&fixture.content_sha256));
    }

    assert_eq!(conversion.conversion_specification_version, "1.0.0");
    for (kind, reference) in [
        ("profile registry", conversion.profile_registry.as_str()),
        (
            "parser dialect registry",
            conversion.parser_dialect_registry.as_str(),
        ),
        (
            "mapping rule registry",
            conversion.mapping_rule_registry.as_str(),
        ),
        (
            "diagnostic registry",
            conversion.diagnostic_registry.as_str(),
        ),
        ("mapping vectors", conversion.mapping_vectors.as_str()),
        ("selection vectors", conversion.selection_vectors.as_str()),
    ] {
        assert_regular_file_below(
            &conversion_base,
            reference,
            &canonical_conformance,
            &format!("Conversion {kind}"),
        );
    }

    let profile_registry: ConversionProfileRegistry =
        load_manifest(&conversion_base.join(&conversion.profile_registry));
    let dialect_registry: ConversionDialectRegistry =
        load_manifest(&conversion_base.join(&conversion.parser_dialect_registry));
    let rule_registry: ConversionMappingRuleRegistry =
        load_manifest(&conversion_base.join(&conversion.mapping_rule_registry));
    let diagnostic_registry: ConversionDiagnosticRegistry =
        load_manifest(&conversion_base.join(&conversion.diagnostic_registry));
    let mapping_vectors: ConversionMappingVectors =
        load_manifest(&conversion_base.join(&conversion.mapping_vectors));
    let selection_vectors: ConversionSelectionVectors =
        load_manifest(&conversion_base.join(&conversion.selection_vectors));

    assert_eq!(profile_registry.schema_version, 1);
    assert_eq!(dialect_registry.schema_version, 1);
    assert_eq!(rule_registry.schema_version, 1);
    assert_eq!(diagnostic_registry.schema_version, 1);
    assert_eq!(mapping_vectors.schema_version, 2);
    assert_eq!(selection_vectors.schema_version, 1);
    for version in [
        &profile_registry.conversion_specification_version,
        &dialect_registry.conversion_specification_version,
        &rule_registry.conversion_specification_version,
        &diagnostic_registry.conversion_specification_version,
        &mapping_vectors.conversion_specification_version,
        &selection_vectors.conversion_specification_version,
    ] {
        assert_eq!(version, &conversion.conversion_specification_version);
    }
    assert_eq!(profile_registry.hash_algorithm, "sha256");
    assert_eq!(dialect_registry.hash_algorithm, "sha256-utf8-contract");
    assert_eq!(rule_registry.hash_algorithm, "sha256-utf8-contract");
    assert_eq!(profile_registry.profile.len(), 12);
    assert_eq!(dialect_registry.dialect.len(), 7);
    assert_eq!(rule_registry.rule.len(), 56);
    assert_eq!(diagnostic_registry.category.len(), 32);
    assert_eq!(mapping_vectors.vector.len(), 38);
    assert_eq!(mapping_vectors.invalid.len(), 5);
    assert_eq!(selection_vectors.selection.len(), 10);
    assert_eq!(
        mapping_vectors.rule_registry,
        conversion.mapping_rule_registry
    );
    assert_eq!(
        selection_vectors.profile_registry,
        conversion.profile_registry
    );

    let mut dialect_refs = HashMap::new();
    for dialect in &dialect_registry.dialect {
        let key = format!("{}@{}", dialect.id, dialect.version);
        assert!(
            dialect_refs
                .insert(key.clone(), dialect.format.as_str())
                .is_none(),
            "duplicate Conversion parser dialect: {key}"
        );
        assert!(!dialect.id.is_empty() && !dialect.format.is_empty());
        assert!(!dialect.contract.is_empty());
        assert!(is_lower_hex(&dialect.content_sha256, 64));
        assert_eq!(
            sha256_lower(dialect.contract.as_bytes()),
            dialect.content_sha256,
            "parser dialect contract hash mismatch: {key}"
        );
    }

    let mut rule_refs = HashSet::new();
    for rule in &rule_registry.rule {
        let key = format!("{}@{}", rule.id, rule.version);
        assert!(
            rule_refs.insert(key.clone()),
            "duplicate Conversion mapping rule: {key}"
        );
        assert!(!rule.id.is_empty() && !rule.contract.is_empty());
        assert!(is_lower_hex(&rule.content_sha256, 64));
        assert_eq!(
            sha256_lower(rule.contract.as_bytes()),
            rule.content_sha256,
            "mapping rule contract hash mismatch: {key}"
        );
    }

    let allowed_conversion_domains = [
        "timing",
        "gameplay",
        "motion",
        "scroll",
        "presentation",
        "resource",
        "metadata",
        "syntax",
        "profile",
        "package",
        "cross-domain",
    ];
    let mut diagnostic_categories = HashMap::new();
    for category in &diagnostic_registry.category {
        let uses: HashSet<_> = category.uses.iter().map(String::as_str).collect();
        assert!(
            diagnostic_categories
                .insert(category.id.as_str(), uses.clone())
                .is_none(),
            "duplicate Conversion diagnostic/report category: {}",
            category.id
        );
        assert!(category.id.starts_with("conversion."));
        assert!(!uses.is_empty());
        assert!(
            uses.iter()
                .all(|usage| matches!(*usage, "diagnostic" | "report-entry"))
        );
        assert!(allowed_conversion_domains.contains(&category.domain.as_str()));
        assert!(!category.description.is_empty());
    }

    let mut profile_refs = HashSet::new();
    let mut profile_parameter_schemas = HashMap::new();
    let mut profile_applicability = HashMap::new();
    for profile in &profile_registry.profile {
        let key = format!("{}@{}", profile.id, profile.version);
        assert!(
            profile_refs.insert(key.clone()),
            "duplicate Conversion semantic profile: {key}"
        );
        assert!(matches!(profile.format.as_str(), "pgr" | "rpe" | "pec"));
        assert!(!profile.directions.is_empty());
        assert!(
            profile
                .directions
                .iter()
                .all(|direction| matches!(direction.as_str(), "source" | "target"))
        );
        assert!(matches!(
            profile.profile_class.as_str(),
            "semantic" | "evidence-profile" | "compatibility-characterization"
        ));
        if profile.profile_class == "compatibility-characterization" {
            assert!(!profile.strict_eligible);
        }
        assert!(is_lower_hex(&profile.content_sha256, 64));
        assert_regular_file_below(
            &conversion_base,
            &profile.path,
            &canonical_conformance,
            &format!("Conversion profile {key}"),
        );

        let descriptor_path = conversion_base.join(&profile.path);
        let descriptor_bytes = fs::read(&descriptor_path).unwrap_or_else(|error| {
            panic!(
                "failed to read Conversion profile descriptor {}: {error}",
                descriptor_path.display()
            )
        });
        assert_eq!(
            sha256_lower(&descriptor_bytes),
            profile.content_sha256,
            "profile descriptor hash mismatch: {key}"
        );
        let descriptor: ConversionProfileDescriptor = load_manifest(&descriptor_path);
        assert_eq!(descriptor.schema_version, 1);
        assert_eq!(descriptor.id, profile.id);
        assert_eq!(descriptor.version, profile.version);
        assert_eq!(descriptor.format, profile.format);
        assert_eq!(descriptor.directions, profile.directions);
        assert_eq!(descriptor.profile_class, profile.profile_class);
        assert_eq!(descriptor.strict_eligible, profile.strict_eligible);
        assert!(!descriptor.producer.is_empty() && !descriptor.runtime.is_empty());
        match descriptor.format_version_policy.as_str() {
            "exact" => assert!(!descriptor.format_versions.is_empty()),
            "evidence-only" | "absent" => assert!(descriptor.format_versions.is_empty()),
            policy => panic!("profile {key} uses unknown format-version policy {policy}"),
        }
        match descriptor.format.as_str() {
            "pgr" => assert_eq!(descriptor.format_version_policy, "exact"),
            "rpe" => assert_eq!(descriptor.format_version_policy, "evidence-only"),
            "pec" => assert_eq!(descriptor.format_version_policy, "absent"),
            _ => unreachable!("validated profile format"),
        }
        assert!(!descriptor.parser_dialects.is_empty());
        assert!(!descriptor.mapping_rules.is_empty());
        assert!(!descriptor.contract.is_empty());
        assert!(!descriptor.known_limitations.is_empty());
        assert!(!descriptor.report_categories.is_empty());
        assert!(!descriptor.evidence.is_empty());
        let mut parameter_names = HashSet::new();
        for parameter in &descriptor.parameters {
            assert!(
                parameter_names.insert(&parameter.name),
                "profile {key} repeats parameter {}",
                parameter.name
            );
            assert!(matches!(
                parameter.value_type.as_str(),
                "length" | "string-enum" | "extension-ref"
            ));
            assert!(matches!(
                parameter.required_when.as_str(),
                "always" | "source-version-absent" | "negative-alpha-present"
            ));
            match parameter.value_type.as_str() {
                "length" => {
                    assert_eq!(parameter.constraint, "finite-positive");
                    assert!(parameter.allowed_values.is_empty());
                }
                "string-enum" => {
                    assert_eq!(parameter.constraint, "one-of");
                    assert!(!parameter.allowed_values.is_empty());
                }
                "extension-ref" => {
                    assert_eq!(parameter.constraint, "registered-required-extension");
                    assert!(parameter.allowed_values.is_empty());
                }
                _ => unreachable!("validated Conversion profile parameter type"),
            }
        }
        assert!(
            profile_parameter_schemas
                .insert(key.clone(), descriptor.parameters.clone())
                .is_none(),
            "duplicate Conversion profile parameter schema: {key}"
        );
        assert!(
            profile_applicability
                .insert(
                    key.clone(),
                    (descriptor.format.clone(), descriptor.directions.clone()),
                )
                .is_none(),
            "duplicate Conversion profile applicability: {key}"
        );
        for dialect_ref in &descriptor.parser_dialects {
            let (id, version) = versioned_ref(dialect_ref);
            let dialect_format = dialect_refs.get(dialect_ref).unwrap_or_else(|| {
                panic!("profile {key} references unknown dialect {id}@{version}")
            });
            assert_eq!(
                *dialect_format, descriptor.format,
                "profile {key} references a dialect for another format"
            );
        }
        for rule_ref in &descriptor.mapping_rules {
            let (id, version) = versioned_ref(rule_ref);
            assert!(
                rule_refs.contains(rule_ref),
                "profile {key} references unknown rule {id}@{version}"
            );
        }
        for category in &descriptor.report_categories {
            assert!(
                diagnostic_categories
                    .get(category.as_str())
                    .is_some_and(|uses| uses.contains("report-entry")),
                "profile {key} references unknown/non-report category {category}"
            );
        }
    }

    assert_eq!(mapping_vectors.forbidden_rule_ids, ["pec.time.tick2048"]);
    assert!(
        rule_registry
            .rule
            .iter()
            .all(|rule| !mapping_vectors.forbidden_rule_ids.contains(&rule.id))
    );
    let mut mapping_vector_ids = HashSet::new();
    for vector in &mapping_vectors.vector {
        assert!(
            mapping_vector_ids.insert(&vector.id),
            "duplicate Conversion mapping vector: {}",
            vector.id
        );
        let rule_ref = format!("{}@{}", vector.rule_id, vector.rule_version);
        assert!(
            rule_refs.contains(&rule_ref),
            "mapping vector {} references unknown rule {rule_ref}",
            vector.id
        );
        assert!(matches!(&vector.source, toml::Value::Table(table) if !table.is_empty()));
        assert!(!vector.expected.is_empty());
        assert!(!vector.unit.is_empty());
        assert!(!vector.clauses.is_empty());
    }
    for vector in &mapping_vectors.invalid {
        assert!(
            mapping_vector_ids.insert(&vector.id),
            "duplicate Conversion mapping/invalid vector: {}",
            vector.id
        );
        let rule_ref = format!("{}@{}", vector.rule_id, vector.rule_version);
        assert!(
            rule_refs.contains(&rule_ref),
            "invalid mapping vector {} references unknown rule {rule_ref}",
            vector.id
        );
        assert!(matches!(&vector.source, toml::Value::Table(table) if !table.is_empty()));
        assert!(vector.diagnostic.starts_with("conversion."));
        assert!(
            diagnostic_categories
                .get(vector.diagnostic.as_str())
                .is_some_and(|uses| uses.contains("diagnostic")),
            "invalid mapping vector {} references unknown/non-diagnostic category {}",
            vector.id,
            vector.diagnostic
        );
        assert!(!vector.clauses.is_empty());
    }

    let allowed_reasons = [
        "explicit",
        "declared",
        "unique-evidence",
        "canonical-equivalent",
        "configured-default",
        "unresolved",
    ];
    let allowed_impacts = [
        "gameplay",
        "timing",
        "motion",
        "scroll",
        "presentation",
        "resource",
        "metadata",
    ];
    let mut selection_ids = HashSet::new();
    for selection in &selection_vectors.selection {
        assert!(
            selection_ids.insert(&selection.id),
            "duplicate Conversion selection vector: {}",
            selection.id
        );
        assert!(matches!(selection.direction.as_str(), "source" | "target"));
        assert!(matches!(selection.format.as_str(), "pgr" | "rpe" | "pec"));
        assert!(matches!(
            selection.syntax_mode.as_str(),
            "strict" | "compatible"
        ));
        assert!(matches!(
            selection.profile_selection_mode.as_str(),
            "strict" | "compatible"
        ));
        assert!(!selection.candidate_bindings.is_empty());
        assert!(allowed_reasons.contains(&selection.expected_reason.as_str()));
        assert!(!selection.clauses.is_empty());
        assert!(
            selection
                .ambiguity_impacts
                .iter()
                .all(|impact| allowed_impacts.contains(&impact.as_str()))
        );
        let mut candidate_refs = HashSet::new();
        for binding in &selection.candidate_bindings {
            let candidate = &binding.profile;
            versioned_ref(candidate);
            assert!(
                profile_refs.contains(candidate),
                "selection {} references unknown profile {candidate}",
                selection.id
            );
            assert!(
                candidate_refs.insert(candidate),
                "selection {} repeats candidate {candidate}",
                selection.id
            );
            let (profile_format, directions) = profile_applicability
                .get(candidate)
                .unwrap_or_else(|| panic!("missing applicability for profile {candidate}"));
            assert_eq!(
                profile_format, &selection.format,
                "selection {} uses profile {candidate} for another format",
                selection.id
            );
            assert!(
                directions.contains(&selection.direction),
                "selection {} uses profile {candidate} in unsupported direction {}",
                selection.id,
                selection.direction
            );

            let parameter_schemas = profile_parameter_schemas
                .get(candidate)
                .unwrap_or_else(|| panic!("missing parameter schema for profile {candidate}"));
            for parameter_name in binding.parameters.keys() {
                assert!(
                    parameter_schemas
                        .iter()
                        .any(|parameter| parameter.name == *parameter_name),
                    "selection {} binds unknown parameter {parameter_name} for {candidate}",
                    selection.id
                );
            }
            for parameter in parameter_schemas {
                let condition_is_present = match parameter.required_when.as_str() {
                    "always" => true,
                    "source-version-absent" => selection
                        .evidence
                        .iter()
                        .any(|evidence| evidence == "input-fact:source-version-absent"),
                    "negative-alpha-present" => selection
                        .evidence
                        .iter()
                        .any(|evidence| evidence == "input-fact:negative-alpha-present"),
                    _ => unreachable!("validated profile parameter condition"),
                };
                let value = binding.parameters.get(&parameter.name);
                if condition_is_present {
                    assert!(
                        value.is_some(),
                        "selection {} omits required parameter {} for {candidate}",
                        selection.id,
                        parameter.name
                    );
                }
                let Some(value) = value else {
                    continue;
                };
                match parameter.value_type.as_str() {
                    "length" => {
                        let source = value.as_str().unwrap_or_else(|| {
                            panic!(
                                "selection {} parameter {} for {candidate} must be a length string",
                                selection.id, parameter.name
                            )
                        });
                        let magnitude = source
                            .strip_suffix("px")
                            .and_then(|value| value.parse::<f64>().ok())
                            .unwrap_or_else(|| {
                                panic!(
                                    "selection {} parameter {} for {candidate} must use px",
                                    selection.id, parameter.name
                                )
                            });
                        assert!(magnitude.is_finite() && magnitude > 0.0);
                    }
                    "string-enum" => {
                        let selected = value.as_str().unwrap_or_else(|| {
                            panic!(
                                "selection {} parameter {} for {candidate} must be a string enum",
                                selection.id, parameter.name
                            )
                        });
                        assert!(
                            parameter
                                .allowed_values
                                .iter()
                                .any(|value| value == selected),
                            "selection {} parameter {} for {candidate} uses unknown value {selected}",
                            selection.id,
                            parameter.name
                        );
                    }
                    "extension-ref" => {
                        let extension = value.as_table().unwrap_or_else(|| {
                            panic!(
                                "selection {} parameter {} for {candidate} must be an extension object",
                                selection.id, parameter.name
                            )
                        });
                        assert_eq!(extension.len(), 3);
                        assert!(
                            extension
                                .get("namespace")
                                .and_then(toml::Value::as_str)
                                .is_some()
                        );
                        assert!(
                            extension
                                .get("version")
                                .and_then(toml::Value::as_str)
                                .is_some()
                        );
                        assert!(
                            extension
                                .get("contentHash")
                                .and_then(toml::Value::as_str)
                                .is_some_and(|hash| is_lower_hex(hash, 64))
                        );
                    }
                    _ => unreachable!("validated profile parameter type"),
                }
            }
        }
        for selected in [
            selection.explicit_profile.as_ref(),
            selection.declared_profile.as_ref(),
            selection.configured_default.as_ref(),
            selection.expected_profile.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                candidate_refs.contains(selected),
                "selection {} chooses non-candidate profile {selected}",
                selection.id
            );
        }
        if selection.expected_reason == "unresolved" {
            assert!(selection.expected_profile.is_none());
            assert!(
                selection
                    .expected_diagnostic
                    .as_deref()
                    .is_some_and(|diagnostic| diagnostic.starts_with("conversion."))
            );
            let diagnostic = selection
                .expected_diagnostic
                .as_deref()
                .expect("unresolved selection requires a diagnostic");
            assert!(
                diagnostic_categories
                    .get(diagnostic)
                    .is_some_and(|uses| uses.contains("diagnostic")),
                "selection {} references unknown/non-diagnostic category {diagnostic}",
                selection.id
            );
        } else {
            assert!(selection.expected_profile.is_some());
            assert!(selection.expected_diagnostic.is_none());
        }
        if selection.expected_reason == "configured-default" {
            assert_eq!(selection.profile_selection_mode, "compatible");
        }
        if selection.canonical_equivalent {
            assert_eq!(selection.expected_reason, "canonical-equivalent");
        }
        if selection.repair_enabled && selection.expected_reason == "unresolved" {
            assert_eq!(
                selection.expected_diagnostic.as_deref(),
                Some("conversion.ambiguous-source-semantics")
            );
        }
        assert!(
            selection
                .evidence
                .iter()
                .all(|evidence| !evidence.is_empty())
        );
    }

    let pgr_v1_ambiguity = selection_vectors
        .selection
        .iter()
        .find(|selection| selection.id == "pgr-v1-format-version-is-insufficient")
        .expect("bound PGR v1 ambiguity vector");
    assert_eq!(
        pgr_v1_ambiguity.expected_diagnostic.as_deref(),
        Some("conversion.ambiguous-source-semantics")
    );
    let repair_ambiguity = selection_vectors
        .selection
        .iter()
        .find(|selection| selection.id == "repair-cannot-select-rpe-semantics")
        .expect("bound Repair/profile boundary vector");
    assert!(repair_ambiguity.repair_enabled);
    assert_eq!(repair_ambiguity.expected_reason, "unresolved");
    let configured_default = selection_vectors
        .selection
        .iter()
        .find(|selection| selection.id == "rpe-compatible-configured-default")
        .expect("bound syntax/profile-selection independence vector");
    assert_eq!(configured_default.syntax_mode, "strict");
    assert_eq!(configured_default.profile_selection_mode, "compatible");

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

    for id in [
        "source.valid.complete-source-grammar",
        "source.valid.escaped-nul-string",
    ] {
        let entry = fixture(&fcs, id);
        assert_eq!(entry.stage, FixtureStage::Parse, "{id}");
        assert_eq!(entry.expect, FixtureExpectation::Success, "{id}");
    }

    let direct = fixture(&fcs, "source.valid.canonical-equivalent-direct");
    let template = fixture(&fcs, "source.valid.canonical-equivalent-template");
    assert_eq!(direct.stage, FixtureStage::Canonical);
    assert_eq!(template.stage, FixtureStage::Canonical);
    assert_eq!(direct.expected, template.expected);
    assert_eq!(
        direct.expected.as_deref(),
        Some("expected/canonical-equivalent.json")
    );

    let exact_expression = fixture(&fcs, "source.valid.exact-expression-dag");
    assert_eq!(exact_expression.stage, FixtureStage::Canonical);
    assert_eq!(
        exact_expression.expected.as_deref(),
        Some("expected/exact-expression-dag.json")
    );

    let note_policies = fixture(&fcs, "source.valid.note-policies");
    assert_eq!(note_policies.stage, FixtureStage::Canonical);
    assert_eq!(
        note_policies.workspace_root.as_deref(),
        Some("source/valid")
    );

    for (id, diagnostic) in [
        (
            "source.invalid.note-policy-disabled-sound",
            "schema.non-constructible",
        ),
        (
            "source.invalid.resource-path-escape",
            "resource.unknown-reference",
        ),
        (
            "source.invalid.resource-hash-mismatch",
            "resource.hash-mismatch",
        ),
    ] {
        let entry = fixture(&fcs, id);
        assert_eq!(entry.stage, FixtureStage::Canonical, "{id}");
        assert_eq!(entry.expect, FixtureExpectation::Error, "{id}");
        assert_eq!(entry.diagnostic.as_deref(), Some(diagnostic), "{id}");
        assert_eq!(entry.workspace_root.as_deref(), Some("source/invalid"));
    }

    for (id, stage, diagnostic) in [
        (
            "source.invalid.duplicate-top-level-block",
            FixtureStage::Parse,
            "name.duplicate",
        ),
        (
            "source.invalid.header-extra-space",
            FixtureStage::Parse,
            "version.invalid",
        ),
        (
            "source.invalid.header-leading-zero",
            FixtureStage::Parse,
            "version.invalid",
        ),
        (
            "source.invalid.nested-generator",
            FixtureStage::Parse,
            "compile-time.nested-generator",
        ),
        (
            "source.invalid.misplaced-generator",
            FixtureStage::Parse,
            "compile-time.misplaced-generator",
        ),
        (
            "source.invalid.unclosed-extension-payload",
            FixtureStage::Parse,
            "syntax.invalid-token",
        ),
        (
            "source.invalid.mixed-beat-literal",
            FixtureStage::Parse,
            "syntax.invalid-token",
        ),
        (
            "source.invalid.unresolved-schema-enum",
            FixtureStage::Elaborate,
            "name.unknown",
        ),
    ] {
        let entry = fixture(&fcs, id);
        assert_eq!(entry.stage, stage, "{id}");
        assert_eq!(entry.expect, FixtureExpectation::Error, "{id}");
        assert_eq!(entry.diagnostic.as_deref(), Some(diagnostic), "{id}");
    }
}
