use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fcs"))
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
