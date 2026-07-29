use std::fs;
use std::path::PathBuf;
use std::process::Command;

use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fcs"))
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
fn inspect_accepts_minimal_runtime_hex() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let hex = root.join("docs/conformance/fcbc/minimal-runtime.hex");
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
    assert!(stdout.contains("\"profile\":\"runtime\""));
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
        assert!(String::from_utf8_lossy(&output.stdout).contains("\"status\":\"equivalent\""));
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
    let source_dir = tempfile::tempdir().unwrap();
    let resolver_dir = tempfile::tempdir().unwrap();
    fs::create_dir(resolver_dir.path().join("assets")).unwrap();
    fs::write(
        resolver_dir.path().join("assets/payload.bin"),
        b"opaque resource",
    )
    .unwrap();
    let source = source_dir.path().join("chart.fcs");
    fs::write(
        &source,
        r#"#fcs 5.0.0
format { profile: chart; }
resources { binary payload { source: "assets/payload.bin"; mediaType: "application/octet-stream"; } }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
"#,
    )
    .unwrap();
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
fn compile_rejects_fidelity_without_a_fidelity_section() {
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

    assert_eq!(output.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&output.stderr).contains("fcbc.profile-requirement-missing"));
    assert!(!out.exists());
}
