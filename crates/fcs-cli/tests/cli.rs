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
        .arg("--output")
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
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
    assert!(stdout.contains("\"sectionCount\":14") || stdout.contains("\"profile\""));
}
