use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fcs_model::{CanonicalContentSha256, EntityKind, derive_stable_id};
use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fcs"))
}

fn load_toml(path: &Path) -> toml::Value {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    toml::from_str(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn decode_hex_file(path: &Path) -> Vec<u8> {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    decode_hex_string(&source)
}

fn decode_hex_string(source: &str) -> Vec<u8> {
    let filtered: String = source
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect();
    assert!(filtered.len().is_multiple_of(2), "odd hex string length");
    (0..filtered.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&filtered[index..index + 2], 16).unwrap())
        .collect()
}

fn read_slice<'a>(bytes: &'a [u8], offset: &mut usize, length: usize) -> &'a [u8] {
    let end = offset.checked_add(length).unwrap();
    let slice = bytes.get(*offset..end).unwrap();
    *offset = end;
    slice
}

fn read_u8(bytes: &[u8], offset: &mut usize) -> u8 {
    read_slice(bytes, offset, 1)[0]
}

fn read_u16(bytes: &[u8], offset: &mut usize) -> u16 {
    u16::from_le_bytes(read_slice(bytes, offset, 2).try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> u32 {
    u32::from_le_bytes(read_slice(bytes, offset, 4).try_into().unwrap())
}

fn read_u64(bytes: &[u8], offset: &mut usize) -> u64 {
    u64::from_le_bytes(read_slice(bytes, offset, 8).try_into().unwrap())
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn fcbc_section_name(section_type: u32) -> &'static str {
    match section_type {
        1 => "StringTable",
        2 => "ConstantPool",
        3 => "Meta",
        4 => "Contributors",
        5 => "Credits",
        6 => "Resources",
        7 => "Sync",
        8 => "TempoMap",
        9 => "Lines",
        10 => "Notes",
        11 => "Tracks",
        12 => "Expressions",
        13 => "Distance",
        14 => "Render",
        15 => "Extensions",
        16 => "Fidelity",
        17 => "ConversionReport",
        18 => "DistributionMetadata",
        19 => "Debug",
        20 => "ResourceData",
        other => panic!("unsupported FCBC section type {other}"),
    }
}

fn fcbc_string(container: &fcs_fcbc::ValidatedContainer, bytes: &[u8], reference: u32) -> String {
    let table = container.section_payload(bytes, 1).unwrap();
    let mut offset = 0;
    let count = read_u32(table, &mut offset) as usize;
    let offsets = (0..=count)
        .map(|_| read_u32(table, &mut offset) as usize)
        .collect::<Vec<_>>();
    let data = &table[offset..];
    let start = offsets[reference as usize];
    let end = offsets[reference as usize + 1];
    String::from_utf8(data[start..end].to_vec()).unwrap()
}

fn resource_kind(kind: &str) -> u16 {
    match kind {
        "audio" => 1,
        "image" => 2,
        "font" => 3,
        "texture" => 4,
        "path" => 5,
        "shader" => 6,
        "binary" => 7,
        other => panic!("unsupported FCBC resource kind {other}"),
    }
}

fn skip_value(bytes: &[u8], offset: &mut usize) -> u8 {
    let tag = read_u8(bytes, offset);
    read_slice(bytes, offset, 3);
    let payload_length = read_u32(bytes, offset) as usize;
    read_slice(bytes, offset, payload_length);
    let padding = (8 - (8 + payload_length) % 8) % 8;
    read_slice(bytes, offset, padding);
    tag
}

fn assert_fcbc_golden_framing(
    id: &str,
    golden: &toml::Value,
    bytes: &[u8],
    container: &fcs_fcbc::ValidatedContainer,
) {
    assert_eq!(golden["expect"].as_str(), Some("success"), "{id}");
    assert_eq!(
        container.byte_length as u64,
        golden["decoded_length"].as_integer().unwrap() as u64,
        "{id}: decoded length"
    );
    assert_eq!(
        lower_hex(&container.content_sha256),
        golden["sha256"].as_str().unwrap(),
        "{id}: file hash"
    );
    assert_eq!(
        container.header.profile.as_str(),
        golden["container_profile"].as_str().unwrap(),
        "{id}: profile"
    );
    assert_eq!(
        u32::from(container.header.chart_count),
        golden["chart_count"].as_integer().unwrap() as u32,
        "{id}: chart count"
    );

    let sections = golden["section"].as_array().unwrap();
    assert_eq!(
        container.sections.len(),
        sections.len(),
        "{id}: section count"
    );
    for (entry, expected) in container.sections.iter().zip(sections) {
        let section_type = expected["type"].as_integer().unwrap() as u32;
        assert_eq!(entry.section_type, section_type, "{id}: section type");
        assert_eq!(
            expected["name"].as_str(),
            Some(fcbc_section_name(section_type)),
            "{id}: section name"
        );
        assert_eq!(
            entry.offset,
            expected["offset"].as_integer().unwrap() as u64,
            "{id}: section {section_type} offset"
        );
        assert_eq!(
            entry.length,
            expected["length"].as_integer().unwrap() as u64,
            "{id}: section {section_type} length"
        );
        assert_eq!(
            format!("{:08x}", entry.checksum),
            expected["crc32"].as_str().unwrap(),
            "{id}: section {section_type} checksum"
        );
    }

    let resource_section = container.section_payload(bytes, 6).unwrap();
    let mut offset = 0;
    let resource_count = read_u32(resource_section, &mut offset) as usize;
    let expected_resources: &[toml::Value] = golden
        .get("resource")
        .and_then(|value| value.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    assert_eq!(
        resource_count,
        golden["resource_count"].as_integer().unwrap() as usize,
        "{id}: resource count"
    );
    assert_eq!(
        resource_count,
        expected_resources.len(),
        "{id}: resource manifest"
    );

    let resource_data = container.section_payload(bytes, 20).unwrap();
    for expected in expected_resources {
        let record_start = offset;
        let record_length = read_u32(resource_section, &mut offset) as usize;
        let record_end = record_start.checked_add(record_length).unwrap();
        assert_eq!(
            read_u16(resource_section, &mut offset),
            1,
            "{id}: resource version"
        );
        assert_eq!(
            read_u16(resource_section, &mut offset),
            0,
            "{id}: resource flags"
        );
        let stable_id = read_u64(resource_section, &mut offset);
        assert_eq!(
            format!("{stable_id:016x}"),
            expected["id"].as_str().unwrap(),
            "{id}: resource id"
        );
        assert_eq!(
            expected["id_namespace"].as_str(),
            Some(fcs_model::RESOURCE_NAMESPACE),
            "{id}: resource namespace"
        );
        assert_eq!(
            stable_id,
            derive_stable_id(
                EntityKind::Resource,
                expected["canonical_textual_id"].as_str().unwrap()
            ),
            "{id}: derived resource id"
        );
        assert_eq!(
            read_u16(resource_section, &mut offset),
            resource_kind(expected["kind"].as_str().unwrap()),
            "{id}: resource kind"
        );
        assert_eq!(
            read_u16(resource_section, &mut offset),
            0,
            "{id}: resource reserved flags"
        );
        let media_type = fcbc_string(container, bytes, read_u32(resource_section, &mut offset));
        assert_eq!(
            media_type,
            expected["media_type"].as_str().unwrap(),
            "{id}: resource media type"
        );
        assert_eq!(
            read_u16(resource_section, &mut offset),
            1,
            "{id}: resource hash algorithm"
        );
        assert_eq!(
            read_u16(resource_section, &mut offset),
            0,
            "{id}: resource hash reserved"
        );
        let data_offset = read_u64(resource_section, &mut offset);
        let data_length = read_u64(resource_section, &mut offset);
        assert_eq!(
            data_offset,
            expected["data_offset"].as_integer().unwrap() as u64,
            "{id}: resource data offset"
        );
        assert_eq!(
            data_length,
            expected["data_length"].as_integer().unwrap() as u64,
            "{id}: resource data length"
        );
        let hash_length = read_u32(resource_section, &mut offset) as usize;
        assert_eq!(hash_length, 32, "{id}: resource hash length");
        let hash = read_slice(resource_section, &mut offset, hash_length);
        assert_eq!(
            lower_hex(hash),
            expected["sha256"].as_str().unwrap(),
            "{id}: resource stored hash"
        );
        assert_eq!(
            skip_value(resource_section, &mut offset),
            14,
            "{id}: resource metadata"
        );
        assert_eq!(offset, record_end, "{id}: resource record length");

        let data_start = data_offset as usize;
        let data_end = data_start + data_length as usize;
        let payload = resource_data.get(data_start..data_end).unwrap();
        let expected_payload = decode_hex_string(expected["payload_hex"].as_str().unwrap());
        assert_eq!(
            payload,
            expected_payload.as_slice(),
            "{id}: resource payload"
        );
        let payload_hash = CanonicalContentSha256::digest(payload).as_bytes();
        assert_eq!(
            lower_hex(&payload_hash),
            expected["sha256"].as_str().unwrap(),
            "{id}: resource payload hash"
        );
    }
    assert_eq!(
        offset,
        resource_section.len(),
        "{id}: resource section length"
    );
    if expected_resources.is_empty() {
        assert!(resource_data.is_empty(), "{id}: empty resource data");
    }
}

const CLI_CHECK_CANONICAL_FIXTURES: &[&str] = &[
    "source.valid.minimal-chart",
    "source.valid.track-boundaries",
    "source.valid.appendix-a-minimal-complete",
    "source.valid.time-scroll-note",
    "source.valid.runtime-choose",
    "source.valid.canonical-equivalent-direct",
    "source.valid.canonical-equivalent-template",
    "source.valid.canonical-id-direct",
    "source.valid.canonical-id-template",
    "source.valid.exact-expression-dag",
    "source.valid.note-policies",
    "source.valid.metadata-credits-resources-sync",
    "source.valid.profile-publishable-both",
    "source.valid.contributor-credit-closure",
    "source.invalid.contributor-missing-name",
    "source.invalid.credit-duplicate-contributor",
    "source.invalid.credit-resource-reference",
    "source.invalid.credit-missing-id",
    "source.invalid.credit-duplicate-id",
    "source.invalid.profile-fragment-feature",
    "source.invalid.profile-publishable-requirements",
    "source.invalid.hold-end",
    "source.invalid.track-overlap",
    "source.invalid.parent-cycle",
    "source.invalid.note-policy-disabled-sound",
    "source.invalid.unknown-resource",
    "source.invalid.sync-preview-without-audio",
    "source.invalid.sync-preview-domain",
    "source.invalid.resource-path-escape",
    "source.invalid.resource-hash-mismatch",
    "source.invalid.resource-missing-member",
    "source.invalid.custom-duplicate-key",
];

const CLI_COMPILE_CANONICAL_FIXTURES: &[&str] = &[
    "source.valid.minimal-chart",
    "source.valid.track-boundaries",
    "source.valid.appendix-a-minimal-complete",
    "source.valid.time-scroll-note",
    "source.valid.runtime-choose",
    "source.valid.canonical-equivalent-direct",
    "source.valid.canonical-equivalent-template",
    "source.valid.canonical-id-direct",
    "source.valid.canonical-id-template",
    "source.valid.exact-expression-dag",
    "source.valid.note-policies",
    "source.valid.metadata-credits-resources-sync",
];

fn manifest_fixture<'a>(manifest: &'a toml::Value, id: &str) -> &'a toml::Value {
    manifest["fixture"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fixture| fixture["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("fixture list contains stale ID {id}"))
}

fn validate_fixture_list(manifest: &toml::Value, ids: &[&str], label: &str) -> HashSet<String> {
    let mut listed = HashSet::new();
    for id in ids {
        assert!(
            listed.insert((*id).to_owned()),
            "{label} lists duplicate fixture {id}"
        );
        let fixture = manifest_fixture(manifest, id);
        assert_eq!(
            fixture["stage"].as_str(),
            Some("canonical"),
            "{label} lists non-canonical fixture {id}"
        );
    }
    listed
}

fn assert_canonical_fixture_partition(manifest: &toml::Value) {
    let mut canonical_ids = HashSet::new();
    for fixture in manifest["fixture"].as_array().unwrap() {
        if fixture["stage"].as_str() == Some("canonical") {
            let id = fixture["id"].as_str().unwrap();
            assert!(
                canonical_ids.insert(id.to_owned()),
                "manifest repeats canonical fixture {id}"
            );
        }
    }

    let check_ids = validate_fixture_list(
        manifest,
        CLI_CHECK_CANONICAL_FIXTURES,
        "CLI check canonical fixtures",
    );
    let compile_ids = validate_fixture_list(
        manifest,
        CLI_COMPILE_CANONICAL_FIXTURES,
        "CLI compile canonical fixtures",
    );
    assert_eq!(canonical_ids.len(), 32);
    assert_eq!(check_ids.len(), 32);
    assert_eq!(compile_ids.len(), 12);
    assert!(compile_ids.is_subset(&check_ids));
    assert!(compile_ids.iter().all(|id| {
        manifest_fixture(manifest, id.as_str())["expect"].as_str() == Some("success")
    }));

    assert_eq!(check_ids, canonical_ids);
}

fn is_listed_canonical_fixture(fixture: &toml::Value, ids: &[&str]) -> bool {
    fixture["stage"].as_str() == Some("canonical") && ids.contains(&fixture["id"].as_str().unwrap())
}

#[test]
fn version_reports_workspace_version() {
    let output = bin().arg("--version").output().unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"fcs 5.0.0\n");
}

#[test]
fn check_accepts_minimal_valid_source() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = root.join("docs/conformance/fcs5/source/valid/minimal-chart.fcs");
    let output = bin().arg("check").arg(&source).output().unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn check_rejects_canonical_profile_errors() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = root.join("docs/conformance/fcs5/source/invalid/profile-fragment-feature.fcs");
    let output = bin().arg("check").arg(&source).output().unwrap();

    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&output.stderr).contains("profile.requirement-missing"));
}

#[test]
fn canonical_fixture_product_partition_is_exhaustive() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let conformance = root.join("docs/conformance/fcs5");
    let manifest = load_toml(&conformance.join("manifest.toml"));
    assert_canonical_fixture_partition(&manifest);
}

#[test]
fn check_executes_manifest_declared_canonical_fixtures() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let conformance = root.join("docs/conformance/fcs5");
    let manifest = load_toml(&conformance.join("manifest.toml"));

    let mut mismatches = Vec::new();
    for fixture in manifest["fixture"].as_array().unwrap() {
        if !is_listed_canonical_fixture(fixture, CLI_CHECK_CANONICAL_FIXTURES) {
            continue;
        }
        let id = fixture["id"].as_str().unwrap();
        let source = conformance.join(fixture["path"].as_str().unwrap());
        let mut command = bin();
        command.arg("check").arg(&source).arg("--json");
        if let Some(workspace_root) = fixture
            .get("workspace_root")
            .and_then(|value| value.as_str())
        {
            command
                .arg("--resolver-root")
                .arg(conformance.join(workspace_root));
        }
        let output = command.output().unwrap();
        let expected = fixture["expect"].as_str().unwrap();
        let expected_category = fixture
            .get("diagnostic")
            .and_then(|value| value.as_str())
            .unwrap_or("success");
        let report = serde_json::from_slice::<serde_json::Value>(&output.stdout).ok();
        let actual_categories: Vec<_> = report
            .as_ref()
            .and_then(|report| report["diagnostics"].as_array())
            .map(|diagnostics| {
                diagnostics
                    .iter()
                    .filter_map(|diagnostic| diagnostic["code"].as_str())
                    .collect()
            })
            .unwrap_or_default();
        let actual_category = if actual_categories.is_empty() {
            if output.status.success() {
                "success".to_owned()
            } else {
                "<none>".to_owned()
            }
        } else {
            actual_categories.join(",")
        };
        let status_matches = match expected {
            "success" => output.status.success(),
            "error" => output.status.code() == Some(3),
            other => {
                mismatches.push(format!(
                    "{id}: unsupported expectation {other}; expected category {expected_category}, actual category {actual_category}, stdout={}, stderr={}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ));
                continue;
            }
        };
        let category_matches =
            expected == "success" || actual_categories.contains(&expected_category);
        if !status_matches || !category_matches {
            mismatches.push(format!(
                "{id}: expected status/category {expected:?}/{expected_category}, actual status/category {:?}/{actual_category}, stdout={}, stderr={}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "canonical CLI fixture mismatches:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn repository_fcs_examples_execute_at_their_applicable_product_boundaries() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let examples_directory = root.join("examples/fcs");
    let expected = vec![
        "chart.fcs".to_owned(),
        "fragment.fcs".to_owned(),
        "templates.fcs".to_owned(),
    ];
    let mut discovered: Vec<_> = fs::read_dir(&examples_directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".fcs"))
        .collect();
    discovered.sort();
    assert_eq!(discovered, expected);

    for name in &expected {
        let source = examples_directory.join(name);
        let mut command = bin();
        if name == "fragment.fcs" {
            command.arg("format").arg(&source);
        } else {
            command.arg("check").arg(&source).arg("--json");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}: stderr={}",
            source.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        if name == "fragment.fcs" {
            let formatted = std::str::from_utf8(&output.stdout)
                .unwrap_or_else(|error| panic!("{name}: formatter output is not UTF-8: {error}"));
            parse_document(formatted)
                .into_result()
                .unwrap_or_else(|errors| {
                    panic!("{name}: formatted output does not parse: {errors:?}")
                });
        }
    }
}

#[test]
fn compile_executes_manifest_declared_canonical_fixtures_through_core_load() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let conformance = root.join("docs/conformance/fcs5");
    let manifest = load_toml(&conformance.join("manifest.toml"));
    let output_directory = tempfile::tempdir().unwrap();

    for fixture in manifest["fixture"].as_array().unwrap() {
        if !is_listed_canonical_fixture(fixture, CLI_COMPILE_CANONICAL_FIXTURES) {
            continue;
        }
        let id = fixture["id"].as_str().unwrap();
        let source = conformance.join(fixture["path"].as_str().unwrap());
        let output_path = output_directory.path().join(format!("{id}.fcbc"));
        let mut command = bin();
        command
            .arg("compile")
            .arg(&source)
            .arg("--output")
            .arg(&output_path);
        if let Some(workspace_root) = fixture
            .get("workspace_root")
            .and_then(|value| value.as_str())
        {
            command
                .arg("--resolver-root")
                .arg(conformance.join(workspace_root));
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{id}: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output_path.is_file(), "{id}: compiler produced no FCBC");

        let inspect = bin()
            .arg("inspect")
            .arg(&output_path)
            .arg("--json")
            .output()
            .unwrap();
        assert!(
            inspect.status.success(),
            "{id}: inspect stderr={}",
            String::from_utf8_lossy(&inspect.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&inspect.stdout).unwrap();
        assert_eq!(report["coreLoaded"], true, "{id}");
    }
}

#[test]
fn check_json_reports_structured_diagnostics() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("invalid.fcs");
    fs::write(&source, b"not an FCS document\n").unwrap();

    let output = bin()
        .arg("check")
        .arg(&source)
        .arg("--json")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(3));
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "failed");
    assert_eq!(report["category"], "source.invalid");
    let diagnostic = &report["diagnostics"][0];
    assert!(
        diagnostic["code"]
            .as_str()
            .is_some_and(|code| !code.is_empty())
    );
    assert!(
        diagnostic["stage"]
            .as_str()
            .is_some_and(|stage| !stage.is_empty())
    );
    assert!(
        diagnostic["severity"]
            .as_str()
            .is_some_and(|severity| !severity.is_empty())
    );
    assert!(diagnostic["span"]["start"].is_u64());
    assert!(diagnostic["span"]["end"].is_u64());
    assert!(diagnostic["labels"].is_array());
}

#[test]
fn inspect_accepts_nonempty_execution_hex() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let hex = root.join("docs/conformance/fcbc/nonempty-execution.hex");
    let output = bin()
        .arg("inspect")
        .arg(&hex)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"profile\":\"strict-runtime\""));
    assert!(stdout.contains("\"sectionCount\":14"));
}

#[test]
fn inspect_rejects_framed_but_core_invalid_fcbc() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let hex =
        fs::read_to_string(root.join("docs/conformance/fcbc/nonempty-execution.hex")).unwrap();
    let filtered: String = hex.chars().filter(|ch| !ch.is_ascii_whitespace()).collect();
    let mut bytes: Vec<u8> = (0..filtered.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&filtered[index..index + 2], 16).unwrap())
        .collect();
    // Reuse the checked-in forbidden-descriptor mutation: the second patch
    // repairs the Tracks section checksum so framing still succeeds.
    bytes[1773] = 5;
    bytes[560..564].copy_from_slice(&[0x7f, 0x9a, 0x4f, 0x08]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid-core.fcbc");
    fs::write(&path, bytes).unwrap();

    let output = bin()
        .arg("inspect")
        .arg(&path)
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("fcbc.forbidden-descriptor"));
}

#[test]
fn inspect_rejects_non_ascii_hex_without_panicking() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid.hex");
    fs::write(&path, "00e9\u{e9}").unwrap();

    let output = bin().arg("inspect").arg(&path).output().unwrap();

    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid hex"));
}

#[test]
fn convert_runs_public_pgr_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let chart =
        root.join("docs/conformance/conversion/public-fixtures/sources/pgr-minimal.pgr.json");
    let output = bin()
        .arg("convert")
        .arg("--format")
        .arg("pgr")
        .arg("--profile")
        .arg("pgr.phira.v1")
        .arg("--source-floor-scale-px")
        .arg("120")
        .arg(&chart)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"status\":\"equivalent\""));
}

#[test]
fn convert_requires_separate_source_and_target_floor_scale_bindings() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let chart =
        root.join("docs/conformance/conversion/public-fixtures/sources/pgr-minimal.pgr.json");
    let output = bin()
        .arg("convert")
        .arg("--format")
        .arg("pgr")
        .arg("--source-profile")
        .arg("pgr.phira.v1")
        .arg("--source-floor-scale-px")
        .arg("120")
        .arg("--target-profile")
        .arg("pgr.phira.v1")
        .arg("--target-capability")
        .arg("pgr-v1")
        .arg(&chart)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--target-floor-scale-px"));
}

#[test]
fn convert_rejects_unbound_typed_profile_parameters() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let chart =
        root.join("docs/conformance/conversion/public-fixtures/sources/pgr-minimal.pgr.json");
    let output = bin()
        .arg("convert")
        .arg("--format")
        .arg("pgr")
        .arg("--profile")
        .arg("pgr.phira.v1")
        .arg("--source-rpe-speed-mode")
        .arg("legacy-linear")
        .arg("--source-floor-scale-px")
        .arg("120")
        .arg("--json")
        .arg(&chart)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("profile-parameter-invalid"));
}

#[test]
fn report_runs_public_rpe_and_pec_fixtures() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cases = [
        (
            "rpe",
            "rpe.phira.legacy-speed",
            root.join("docs/conformance/conversion/public-fixtures/sources/rpe-minimal.rpe.json"),
            None,
        ),
        (
            "pec",
            "pec.phira",
            root.join("docs/conformance/conversion/public-fixtures/sources/pec-minimal.pec"),
            Some("100"),
        ),
    ];
    for (format, profile, path, floor_scale) in cases {
        let mut command = bin();
        command
            .arg("report")
            .arg("--format")
            .arg(format)
            .arg("--profile")
            .arg(profile);
        if let Some(floor_scale) = floor_scale {
            command.arg("--source-floor-scale-px").arg(floor_scale);
        }
        let output = command.arg(path).output().unwrap();
        assert!(
            output.status.success(),
            "{format} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["status"], "equivalent");
        assert!(report["report"]["entries"].is_array());
        assert!(report["report"]["repairs"].is_array());
        assert!(report["report"]["repairMode"]["enabled"].is_boolean());
        assert_eq!(
            report["report"]["entryCount"].as_u64(),
            report["report"]["entries"]
                .as_array()
                .map(Vec::len)
                .map(|len| len as u64)
        );
    }
}

#[test]
fn convert_exports_public_pgr_fixture_with_explicit_target_capability() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let chart =
        root.join("docs/conformance/conversion/public-fixtures/sources/pgr-minimal.pgr.json");
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("exported.pgr.json");
    let output = bin()
        .arg("convert")
        .arg("--format")
        .arg("pgr")
        .arg("--profile")
        .arg("pgr.phira.v1")
        .arg("--source-floor-scale-px")
        .arg("120")
        .arg("--target-profile")
        .arg("pgr.phira.v1")
        .arg("--target-capability")
        .arg("pgr-v1")
        .arg("--target-floor-scale-px")
        .arg("120")
        .arg("--output")
        .arg(&target)
        .arg("--json")
        .arg(&chart)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(target.is_file());
    assert!(fs::read(&target).unwrap().starts_with(b"{"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"targetProfile\":\"pgr.phira.v1\""));
}

#[test]
fn convert_json_reports_target_export_failure_without_success_output() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let chart =
        root.join("docs/conformance/conversion/public-fixtures/sources/pgr-minimal.pgr.json");
    let directory = tempfile::tempdir().unwrap();
    let output_path = directory.path().join("should-not-exist.pgr.json");
    let output = bin()
        .arg("convert")
        .arg("--format")
        .arg("pgr")
        .arg("--profile")
        .arg("pgr.phira.v1")
        .arg("--source-floor-scale-px")
        .arg("120")
        .arg("--target-profile")
        .arg("pgr.phira.v1")
        .arg("--target-capability")
        .arg("rpe-json")
        .arg("--target-floor-scale-px")
        .arg("120")
        .arg("--output")
        .arg(&output_path)
        .arg("--json")
        .arg(&chart)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(4));
    assert!(output.stderr.is_empty());
    let body: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(body["status"], "failed");
    assert_eq!(body["category"], "conversion.capability-mismatch");
    assert!(body["output"].is_null());
    assert!(body["report"].is_null());
    assert!(!output_path.exists());
}

#[test]
fn convert_default_output_does_not_overwrite_same_format_source() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("chart.pgr.json");
    let bytes = fs::read(
        root.join("docs/conformance/conversion/public-fixtures/sources/pgr-minimal.pgr.json"),
    )
    .unwrap();
    fs::write(&source, &bytes).unwrap();

    let output = bin()
        .arg("convert")
        .arg("--format")
        .arg("pgr")
        .arg("--profile")
        .arg("pgr.phira.v1")
        .arg("--source-floor-scale-px")
        .arg("120")
        .arg("--target-profile")
        .arg("pgr.phira.v1")
        .arg("--target-capability")
        .arg("pgr-v1")
        .arg("--target-floor-scale-px")
        .arg("120")
        .arg(&source)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert!(dir.path().join("chart.pgr.converted.json").is_file());
}

#[test]
fn convert_exports_public_rpe_and_pec_fixtures() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        (
            "rpe",
            "rpe.phira.legacy-speed",
            "rpe.phira.legacy-speed",
            "rpe-json",
            root.join("docs/conformance/conversion/public-fixtures/sources/rpe-minimal.rpe.json"),
            None,
        ),
        (
            "pec",
            "pec.phira",
            "pec.phira",
            "pec-line",
            root.join("docs/conformance/conversion/public-fixtures/sources/pec-minimal.pec"),
            Some("100"),
        ),
    ];
    for (format, source_profile, target_profile, capability, path, floor_scale) in cases {
        let output_path = dir.path().join(format!("exported.{format}"));
        let mut command = bin();
        command
            .arg("convert")
            .arg("--format")
            .arg(format)
            .arg("--profile")
            .arg(source_profile)
            .arg("--target-profile")
            .arg(target_profile)
            .arg("--target-capability")
            .arg(capability)
            .arg("--policy")
            .arg("semantic")
            .arg("--output")
            .arg(&output_path);
        if let Some(floor_scale) = floor_scale {
            command
                .arg("--source-floor-scale-px")
                .arg(floor_scale)
                .arg("--target-floor-scale-px")
                .arg(floor_scale);
        }
        let output = command.arg(path).arg("--json").output().unwrap();
        assert!(
            output.status.success(),
            "{format} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output_path.is_file());
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains(&format!("\"targetProfile\":\"{target_profile}\""))
        );
    }
}

#[test]
fn format_rejects_invalid_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.fcs");
    fs::write(&path, b"not a chart").unwrap();
    let output = bin().arg("format").arg(&path).output().unwrap();
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn format_uses_the_fixed_text_policy_and_preserves_canonical_chart() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source =
        fs::read_to_string(root.join("docs/conformance/fcs5/source/valid/minimal-chart.fcs"))
            .unwrap();
    let noisy = format!("{}\r\n\t\r\n", source.replace('\n', "  \r\n"));
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("noisy.fcs");
    fs::write(&input, noisy).unwrap();

    let output = bin().arg("format").arg(&input).output().unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let formatted = String::from_utf8(output.stdout).unwrap();
    assert!(!formatted.contains('\r'));
    assert!(formatted.ends_with('\n'));
    assert!(!formatted.ends_with("\n\n"));
    assert!(
        !formatted
            .lines()
            .any(|line| line.ends_with(' ') || line.ends_with('\t'))
    );

    let original_chart = parse_document(&source)
        .into_result()
        .unwrap()
        .canonical_chart(CompileTimeLimits::default())
        .unwrap();
    let formatted_chart = parse_document(&formatted)
        .into_result()
        .unwrap()
        .canonical_chart(CompileTimeLimits::default())
        .unwrap();
    assert_eq!(formatted_chart, original_chart);

    let output_path = dir.path().join("formatted.fcs");
    let file_output = bin()
        .arg("format")
        .arg(&input)
        .arg("--output")
        .arg(&output_path)
        .output()
        .unwrap();
    assert!(file_output.status.success());
    assert!(file_output.stdout.is_empty());
    assert_eq!(fs::read(output_path).unwrap(), formatted.as_bytes());
}

#[test]
fn compile_emits_loadable_fcbc_from_chart_with_line_and_note() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("chart.fcs");
    fs::write(
        &source,
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
"#,
    )
    .unwrap();
    let out = dir.path().join("out.fcbc");
    let output = bin()
        .arg("compile")
        .arg(&source)
        .arg("--profile")
        .arg("runtime")
        .arg("--output")
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("coreLoaded=true"));
    assert!(out.is_file());
    let bytes = fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"FCSB"));
    assert!(bytes.len() > 128);
    let inspect = bin()
        .arg("inspect")
        .arg(&out)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        inspect.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    let stdout = String::from_utf8_lossy(&inspect.stdout);
    assert!(stdout.contains("\"profile\":\"runtime\""));
    assert!(stdout.contains("\"sectionCount\":14") || stdout.contains("\"profile\""));
}

#[test]
fn compile_preserves_render_source_and_inspect_loads_it() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("render.fcs");
    fs::copy(
        root.join("docs/conformance/render/solid-rect-4x4.fcs"),
        &source,
    )
    .unwrap();
    let out = dir.path().join("render.fcbc");
    let output = bin()
        .arg("compile")
        .arg(&source)
        .arg("--output")
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let inspect = bin()
        .arg("inspect")
        .arg(&out)
        .arg("--render")
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        inspect.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    let stdout = String::from_utf8_lossy(&inspect.stdout);
    assert!(stdout.contains("\"layerCount\":1"));
    assert!(stdout.contains("\"nodeCount\":1"));
    assert!(stdout.contains("\"drawOps\":1"));
}

#[test]
fn compile_honors_public_compile_limits() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("chart.fcs");
    fs::write(
        &source,
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
"#,
    )
    .unwrap();
    let out = dir.path().join("out.fcbc");
    let output = bin()
        .arg("compile")
        .arg(&source)
        .arg("--output")
        .arg(&out)
        .arg("--max-generated-nodes")
        .arg("0")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(3));
    assert!(!out.exists());
}

#[test]
fn compile_uses_explicit_resource_resolver_root() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures = root.join("docs/conformance/fcs5/source/valid");
    let source_dir = tempfile::tempdir().unwrap();
    let resolver_dir = tempfile::tempdir().unwrap();
    fs::create_dir(resolver_dir.path().join("assets")).unwrap();
    fs::copy(
        fixtures.join("assets/opaque-resource.bin"),
        resolver_dir.path().join("assets/opaque-resource.bin"),
    )
    .unwrap();
    let source = source_dir.path().join("chart.fcs");
    fs::copy(fixtures.join("note-policies.fcs"), &source).unwrap();
    let out = source_dir.path().join("out.fcbc");
    let output = bin()
        .arg("compile")
        .arg(&source)
        .arg("--output")
        .arg(&out)
        .arg("--resolver-root")
        .arg(resolver_dir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out.is_file());
}

#[test]
fn compile_emits_a_loadable_fidelity_section() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("chart.fcs");
    fs::write(
        &source,
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
"#,
    )
    .unwrap();
    let out = dir.path().join("out.fcbc");
    let output = bin()
        .arg("compile")
        .arg(&source)
        .arg("--profile")
        .arg("fidelity")
        .arg("--output")
        .arg(&out)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&out).unwrap();
    let container = fcs_fcbc::load_container(&bytes).unwrap();
    assert_eq!(
        container.header.profile,
        fcs_fcbc::ContainerProfile::Fidelity
    );
    assert!(
        container
            .header
            .feature_flags
            .contains(fcs_fcbc::FeatureFlags::HAS_FIDELITY)
    );
    assert!(container.section_types().contains(&16));
    fcs_fcbc::load_chart(&bytes).unwrap();
}

#[test]
fn report_executes_every_public_conversion_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture_root = root.join("docs/conformance/conversion/public-fixtures");
    let manifest = load_toml(&fixture_root.join("manifest.toml"));

    for fixture in manifest["fixture"].as_array().unwrap() {
        let id = fixture["id"].as_str().unwrap();
        let format = fixture["format"].as_str().unwrap();
        let profile = format!(
            "{}@{}",
            fixture["profile"].as_str().unwrap(),
            fixture["profile_version"].as_str().unwrap()
        );
        let source = fixture_root.join(fixture["source"].as_str().unwrap());
        let expected = load_toml(&fixture_root.join(fixture["expected"].as_str().unwrap()));
        let mut command = bin();
        command
            .arg("report")
            .arg("--format")
            .arg(format)
            .arg("--source-profile")
            .arg(&profile);
        if let Some(floor_scale) = fixture
            .get("floor_scale_px")
            .and_then(|value| value.as_str())
        {
            command.arg("--source-floor-scale-px").arg(floor_scale);
        }
        let output = command.arg(source).output().unwrap();
        assert!(
            output.status.success(),
            "{id}: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("{id}: invalid report JSON: {error}"));
        assert_eq!(
            report["status"].as_str(),
            expected["expected_status"].as_str(),
            "{id}"
        );
        assert_eq!(
            report["sourceProfile"].as_str(),
            Some(profile.as_str()),
            "{id}"
        );
        assert_eq!(
            report["lines"].as_u64(),
            expected["expected_lines"]
                .as_integer()
                .map(|value| value as u64),
            "{id}"
        );
        assert_eq!(
            report["notes"].as_u64(),
            expected["expected_notes"]
                .as_integer()
                .map(|value| value as u64),
            "{id}"
        );
        if fixture.get("export_reparse").is_none() {
            let entries = report["report"]["entries"].as_array().unwrap();
            for category in expected["required_categories"].as_array().unwrap() {
                let category = category.as_str().unwrap();
                assert!(
                    entries
                        .iter()
                        .any(|entry| entry["category"].as_str() == Some(category)),
                    "{id}: missing report category {category}"
                );
            }
        }
    }
}

#[test]
fn convert_executes_every_declared_public_export_reparse_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture_root = root.join("docs/conformance/conversion/public-fixtures");
    let manifest = load_toml(&fixture_root.join("manifest.toml"));
    let directory = tempfile::tempdir().unwrap();

    for fixture in manifest["fixture"].as_array().unwrap() {
        let Some(target) = fixture.get("export_reparse") else {
            continue;
        };
        let id = fixture["id"].as_str().unwrap();
        let format = fixture["format"].as_str().unwrap();
        let source_profile = format!(
            "{}@{}",
            fixture["profile"].as_str().unwrap(),
            fixture["profile_version"].as_str().unwrap()
        );
        let source = fixture_root.join(fixture["source"].as_str().unwrap());
        let expected = load_toml(&fixture_root.join(fixture["expected"].as_str().unwrap()));
        let output_path = directory.path().join(format!("{id}.{format}"));
        let target_profile = format!(
            "{}@{}",
            target["target_profile"].as_str().unwrap(),
            target["target_profile_version"].as_str().unwrap()
        );
        let capability = match format {
            "pgr" => "pgr-v3",
            "rpe" => "rpe-json",
            "pec" => "pec-line",
            other => panic!("{id}: unsupported fixture format {other}"),
        };
        let mut command = bin();
        command
            .arg("convert")
            .arg("--format")
            .arg(format)
            .arg("--source-profile")
            .arg(&source_profile)
            .arg("--target-profile")
            .arg(&target_profile)
            .arg("--target-capability")
            .arg(capability)
            .arg("--policy")
            .arg(target["policy"].as_str().unwrap())
            .arg("--output")
            .arg(&output_path)
            .arg("--json");
        if let Some(floor_scale) = fixture
            .get("floor_scale_px")
            .and_then(|value| value.as_str())
        {
            command.arg("--source-floor-scale-px").arg(floor_scale);
        }
        if let Some(floor_scale) = target
            .get("floor_scale_px")
            .and_then(|value| value.as_str())
        {
            command.arg("--target-floor-scale-px").arg(floor_scale);
        }
        let output = command.arg(source).output().unwrap();
        assert!(
            output.status.success(),
            "{id}: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("{id}: invalid conversion JSON: {error}"));
        assert_eq!(report["status"], "equivalent", "{id}");
        assert_eq!(
            report["sourceProfile"].as_str(),
            Some(source_profile.as_str()),
            "{id}"
        );
        assert_eq!(
            report["targetProfile"].as_str(),
            Some(target_profile.as_str()),
            "{id}"
        );
        assert_eq!(
            report["lines"].as_u64(),
            expected["expected_lines"]
                .as_integer()
                .map(|value| value as u64),
            "{id}"
        );
        assert_eq!(
            report["notes"].as_u64(),
            expected["expected_notes"]
                .as_integer()
                .map(|value| value as u64),
            "{id}"
        );
        let entries = report["report"]["entries"].as_array().unwrap();
        for category in expected["required_categories"].as_array().unwrap() {
            let category = category.as_str().unwrap();
            assert!(
                entries
                    .iter()
                    .any(|entry| entry["category"].as_str() == Some(category)),
                "{id}: missing report category {category}"
            );
        }
        assert!(output_path.is_file(), "{id}: target output was not written");
    }
}

#[test]
fn inspect_executes_every_fcbc_golden_through_declared_core_contract() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let conformance = root.join("docs/conformance/fcbc");
    let manifest = load_toml(&conformance.join("manifest.toml"));
    let mut mismatches = Vec::new();

    for fixture in manifest["fixture"].as_array().unwrap() {
        let id = fixture["id"].as_str().unwrap();
        let golden = load_toml(&conformance.join(fixture["manifest"].as_str().unwrap()));
        let hex = conformance.join(golden["path"].as_str().unwrap());
        let bytes = decode_hex_file(&hex);
        let container = fcs_fcbc::load_container(&bytes)
            .unwrap_or_else(|error| panic!("{id}: framing failed: {error}"));
        assert_fcbc_golden_framing(id, &golden, &bytes, &container);

        let output = bin()
            .arg("inspect")
            .arg(&hex)
            .arg("--json")
            .output()
            .unwrap();
        let expected_core = golden["core_expect"].as_str().unwrap();
        match expected_core {
            "success" => {
                if !output.status.success() {
                    mismatches.push(format!(
                        "{id}: expected Core success, exit={:?}, stderr={}",
                        output.status.code(),
                        String::from_utf8_lossy(&output.stderr)
                    ));
                    continue;
                }
                let report: serde_json::Value = serde_json::from_slice(&output.stdout)
                    .unwrap_or_else(|error| panic!("{id}: invalid inspect JSON: {error}"));
                if report["coreLoaded"] != true {
                    mismatches.push(format!(
                        "{id}: successful inspect did not report coreLoaded=true"
                    ));
                }
                if report["byteLength"].as_u64()
                    != golden["decoded_length"]
                        .as_integer()
                        .map(|value| value as u64)
                {
                    mismatches.push(format!("{id}: inspect byteLength disagrees with golden"));
                }
                if report["sha256"].as_str() != golden["sha256"].as_str() {
                    mismatches.push(format!("{id}: inspect sha256 disagrees with golden"));
                }
                if report["profile"].as_str() != golden["container_profile"].as_str() {
                    mismatches.push(format!("{id}: inspect profile disagrees with golden"));
                }
            }
            "error" => {
                let expected_category = golden["core_diagnostic"].as_str().unwrap();
                let stderr = String::from_utf8_lossy(&output.stderr);
                if output.status.success()
                    || output.status.code() != Some(3)
                    || !stderr.contains(expected_category)
                    || !output.stdout.is_empty()
                {
                    mismatches.push(format!(
                        "{id}: expected Core error {expected_category}, exit={:?}, stdout={}, stderr={stderr}",
                        output.status.code(),
                        String::from_utf8_lossy(&output.stdout)
                    ));
                }
            }
            other => mismatches.push(format!("{id}: unsupported core_expect {other}")),
        }
    }

    assert!(
        mismatches.is_empty(),
        "FCBC CLI Core contract mismatches:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn inspect_executes_every_declared_fcbc_mutation() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let conformance = root.join("docs/conformance/fcbc");
    let manifest = load_toml(&conformance.join("manifest.toml"));
    let directory = tempfile::tempdir().unwrap();
    let mut checked = 0;

    for fixture in manifest["fixture"].as_array().unwrap() {
        let fixture_id = fixture["id"].as_str().unwrap();
        let mutations = load_toml(&conformance.join(fixture["mutations"].as_str().unwrap()));
        let base = decode_hex_file(&conformance.join(mutations["base"].as_str().unwrap()));

        for mutation in mutations["mutation"].as_array().unwrap() {
            let mutation_id = mutation["id"].as_str().unwrap();
            let mut bytes = base.clone();
            for patch in mutation["patch"].as_array().unwrap() {
                let offset = patch["offset"].as_integer().unwrap() as usize;
                let replacement = decode_hex_string(patch["replace_hex"].as_str().unwrap());
                let end = offset.checked_add(replacement.len()).unwrap();
                assert!(
                    end <= bytes.len(),
                    "{fixture_id}/{mutation_id}: mutation patch exceeds base"
                );
                bytes[offset..end].copy_from_slice(&replacement);
            }

            let path = directory
                .path()
                .join(format!("{fixture_id}-{mutation_id}.hex"));
            fs::write(&path, lower_hex(&bytes)).unwrap();
            let output = bin()
                .arg("inspect")
                .arg(&path)
                .arg("--json")
                .output()
                .unwrap();
            let expected = mutation["diagnostic"].as_str().unwrap();
            assert_eq!(
                output.status.code(),
                Some(3),
                "{fixture_id}/{mutation_id}: expected invalid-input exit, stderr={}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                output.stdout.is_empty(),
                "{fixture_id}/{mutation_id}: failed inspect must not emit JSON"
            );
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(expected),
                "{fixture_id}/{mutation_id}: expected {expected}, stderr={}",
                String::from_utf8_lossy(&output.stderr)
            );
            checked += 1;
        }
    }

    assert!(checked > 0, "FCBC mutation manifest must not be empty");
}

#[test]
fn render_manifest_source_and_product_paths_are_exercised() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let render = root.join("docs/conformance/render");
    let manifest = load_toml(&render.join("manifest.toml"));

    for fixture in manifest["source_fixture"].as_array().unwrap() {
        let id = fixture["id"].as_str().unwrap();
        let source = render.join(fixture["source"].as_str().unwrap());
        let output = bin()
            .arg("check")
            .arg(&source)
            .arg("--json")
            .output()
            .unwrap();
        match fixture["expect"].as_str().unwrap() {
            "success" => assert!(output.status.success(), "{id}: check failed"),
            "error" => {
                assert_eq!(output.status.code(), Some(3), "{id}");
                let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                let expected = fixture["diagnostic"].as_str().unwrap();
                assert!(
                    report["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|diagnostic| diagnostic["code"] == expected),
                    "{id}: expected {expected}"
                );
            }
            other => panic!("{id}: unsupported expectation {other}"),
        }
    }

    let fixture = &manifest["fixture"].as_array().unwrap()[0];
    let source = render.join(fixture["source"].as_str().unwrap());
    let expected: serde_json::Value =
        fs::read_to_string(render.join(fixture["semantic_expected"].as_str().unwrap()))
            .unwrap()
            .parse()
            .unwrap();
    let expected_draw_ops = expected["drawOrder"].as_array().unwrap().len() as u64;
    let directory = tempfile::tempdir().unwrap();
    let output_path = directory.path().join("render.fcbc");
    let compile = bin()
        .arg("compile")
        .arg(&source)
        .arg("--output")
        .arg(&output_path)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "render fixture: stderr={}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let inspect = bin()
        .arg("inspect")
        .arg(&output_path)
        .arg("--render")
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        inspect.status.success(),
        "render fixture inspect: stderr={}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&inspect.stdout).unwrap();
    assert!(report["render"]["layerCount"].as_u64().unwrap() > 0);
    assert!(report["render"]["nodeCount"].as_u64().unwrap() >= expected_draw_ops);
    assert_eq!(
        report["render"]["drawOps"].as_u64(),
        Some(expected_draw_ops)
    );
    let viewport = report["render"]["viewport"]
        .as_array()
        .unwrap_or_else(|| panic!("{source:?}: missing Render viewport"));
    assert_eq!(viewport.len(), 2, "{source:?}: Render viewport dimensions");
    assert_eq!(
        viewport[0].as_f64(),
        Some(fixture["width"].as_integer().unwrap() as f64),
        "{source:?}: Render viewport width"
    );
    assert_eq!(
        viewport[1].as_f64(),
        Some(fixture["height"].as_integer().unwrap() as f64),
        "{source:?}: Render viewport height"
    );

    let binding = &manifest["binding_fixture"].as_array().unwrap()[0];
    let binding_expected: serde_json::Value =
        fs::read_to_string(render.join(binding["semantic_expected"].as_str().unwrap()))
            .unwrap()
            .parse()
            .unwrap();
    assert_eq!(
        binding_expected["canonicalResourceId"].as_str(),
        binding["canonical_resource_id"].as_str()
    );
    assert_eq!(
        binding_expected["contentSha256"].as_str(),
        binding["content_sha256"].as_str()
    );
    assert_eq!(
        binding_expected["fcbcResourcesSectionType"].as_u64(),
        binding["fcbc_resources_section_type"]
            .as_integer()
            .map(|value| value as u64)
    );
    assert_eq!(
        binding_expected["fcbcResourceDataSectionType"].as_u64(),
        binding["fcbc_resource_data_section_type"]
            .as_integer()
            .map(|value| value as u64)
    );
    let binding_asset = render.join(binding["resource_asset"].as_str().unwrap());
    assert_eq!(
        fs::metadata(&binding_asset).unwrap().len(),
        binding["payload_length"].as_integer().unwrap() as u64
    );
    let binding_source = render.join(binding["source"].as_str().unwrap());
    let binding_dir = tempfile::tempdir().unwrap();
    let binding_output = binding_dir.path().join("resource-image.fcbc");
    let compile = bin()
        .arg("compile")
        .arg(&binding_source)
        .arg("--resolver-root")
        .arg(render.join(binding["workspace_root"].as_str().unwrap()))
        .arg("--output")
        .arg(&binding_output)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "render binding: stderr={}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let inspect = bin()
        .arg("inspect")
        .arg(&binding_output)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        inspect.status.success(),
        "render binding inspect: stderr={}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&inspect.stdout).unwrap();
    assert_eq!(report["coreLoaded"], true);
    assert!(report["render"].is_null());
    let section_types = report["sectionTypes"].as_array().unwrap();
    for section_type in [
        binding["fcbc_resources_section_type"].as_integer().unwrap(),
        binding["fcbc_resource_data_section_type"]
            .as_integer()
            .unwrap(),
    ] {
        assert!(
            section_types
                .iter()
                .any(|value| value.as_i64() == Some(section_type)),
            "missing binding section type {section_type}"
        );
    }
}
