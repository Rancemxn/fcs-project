//! Product FCS CLI surface (I10.1–I10.4).
//!
//! Commands call domain crates only. Exit categories are stable machine values.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use fcs_conversion::{
    ArtifactRole, DecimalLimits, ExactDecimal, PecLimits, PecProfile, PecProfileBinding, PgrLimits,
    PgrProfile, PgrProfileBinding, RpeProfileBinding, SourceArtifact, SourceFormat, interpret_pec,
    interpret_pgr, interpret_rpe_semantics, lower_pec_to_canonical, lower_pgr_to_canonical,
    lower_rpe_to_canonical, parse_json_document, parse_pec_document, parse_pgr_document,
    parse_rpe_document,
};
use fcs_fcbc::{load_chart, load_container};
use fcs_source::parser::parse_document;

/// Stable process exit categories for the product CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum ExitCategory {
    Success = 0,
    Usage = 2,
    InputInvalid = 3,
    Unsupported = 4,
    Internal = 5,
}

impl ExitCategory {
    fn code(self) -> ExitCode {
        ExitCode::from(self as u8)
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "fcs",
    version,
    about = "FCS 5 product CLI: check, format, compile, inspect, convert, report"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Parse and validate an FCS source document.
    Check {
        /// Path to a `.fcs` source file.
        path: PathBuf,
        /// Emit JSON diagnostic summary on failure.
        #[arg(long)]
        json: bool,
    },
    /// Format FCS source (semantic-preserving pass-through of UTF-8 bytes in this RC unit).
    Format {
        /// Path to a `.fcs` source file.
        path: PathBuf,
        /// Write formatted source to this path (default: stdout).
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Compile FCS source through the product frontend and emit a compile report.
    Compile {
        /// Path to a `.fcs` source file.
        path: PathBuf,
        /// Optional output path for a JSON compile summary.
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Inspect an FCBC container (framing + Core load when possible).
    Inspect {
        /// Path to an FCBC binary (or `.hex` lowercase hex dump).
        path: PathBuf,
        /// Emit JSON.
        #[arg(long)]
        json: bool,
    },
    /// Convert an external chart into the product canonical import path.
    Convert {
        /// Source format family.
        #[arg(long, value_enum)]
        format: ConvertFormat,
        /// Semantic profile id (for example `pgr.phira.v1`).
        #[arg(long)]
        profile: String,
        /// Path to the source chart bytes.
        path: PathBuf,
        /// Optional floor scale for PGR/PEC profiles.
        #[arg(long, default_value = "120")]
        floor_scale_px: String,
        /// Emit JSON ConversionReport summary.
        #[arg(long)]
        json: bool,
    },
    /// Print a ConversionReport summary for an external import (alias of convert --json).
    Report {
        #[arg(long, value_enum)]
        format: ConvertFormat,
        #[arg(long)]
        profile: String,
        path: PathBuf,
        #[arg(long, default_value = "120")]
        floor_scale_px: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ConvertFormat {
    Pgr,
    Rpe,
    Pec,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Commands::Check { path, json } => cmd_check(&path, json),
        Commands::Format { path, output } => cmd_format(&path, output.as_deref()),
        Commands::Compile { path, output } => cmd_compile(&path, output.as_deref()),
        Commands::Inspect { path, json } => cmd_inspect(&path, json),
        Commands::Convert {
            format,
            profile,
            path,
            floor_scale_px,
            json,
        } => cmd_convert(format, &profile, &path, &floor_scale_px, json),
        Commands::Report {
            format,
            profile,
            path,
            floor_scale_px,
        } => cmd_convert(format, &profile, &path, &floor_scale_px, true),
    }
}

fn cmd_check(path: &Path, json: bool) -> ExitCode {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("error: failed to read {}: {error}", path.display());
            return ExitCategory::Usage.code();
        }
    };
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(_) => {
            eprintln!("error: source is not valid UTF-8");
            return ExitCategory::InputInvalid.code();
        }
    };
    match parse_document(text).into_result() {
        Ok(_document) => {
            if json {
                println!(
                    r#"{{"status":"ok","path":{}}}"#,
                    json_string(&path.display().to_string())
                );
            } else {
                println!("ok: {}", path.display());
            }
            ExitCategory::Success.code()
        }
        Err(diagnostics) => {
            let message = diagnostics
                .first()
                .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
                .unwrap_or_else(|| "source invalid".into());
            if json {
                println!(
                    r#"{{"status":"failed","category":"source.invalid","message":{}}}"#,
                    json_string(&message)
                );
            } else {
                eprintln!("error: {message}");
            }
            ExitCategory::InputInvalid.code()
        }
    }
}

fn cmd_format(path: &Path, output: Option<&Path>) -> ExitCode {
    // This RC unit keeps a product identity formatter: validated UTF-8 source is
    // rewritten unchanged. A full FCS pretty-printer remains a later I8.1 unit.
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("error: failed to read {}: {error}", path.display());
            return ExitCategory::Usage.code();
        }
    };
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(_) => {
            eprintln!("error: source is not valid UTF-8");
            return ExitCategory::InputInvalid.code();
        }
    };
    if let Err(diagnostics) = parse_document(text).into_result() {
        let message = diagnostics
            .first()
            .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
            .unwrap_or_else(|| "source invalid".into());
        eprintln!("error: cannot format invalid source: {message}");
        return ExitCategory::InputInvalid.code();
    }
    match output {
        Some(path) => {
            if let Err(error) = fs::write(path, text.as_bytes()) {
                eprintln!("error: failed to write {}: {error}", path.display());
                return ExitCategory::Internal.code();
            }
        }
        None => print!("{text}"),
    }
    ExitCategory::Success.code()
}

fn cmd_compile(path: &Path, output: Option<&Path>) -> ExitCode {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("error: failed to read {}: {error}", path.display());
            return ExitCategory::Usage.code();
        }
    };
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(_) => {
            eprintln!("error: source is not valid UTF-8");
            return ExitCategory::InputInvalid.code();
        }
    };
    if let Err(diagnostics) = parse_document(text).into_result() {
        let message = diagnostics
            .first()
            .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
            .unwrap_or_else(|| "source invalid".into());
        eprintln!("error: {message}");
        return ExitCategory::InputInvalid.code();
    }
    // Full source→CanonicalCompilation→FCBC product compiler remains a later
    // assembly unit. This command still exercises the product parser boundary
    // and emits a deterministic compile summary for CLI conformance wiring.
    let summary = serde_json::json!({
        "status": "parsed",
        "path": path.display().to_string(),
        "sourceVersion": "5.0.0",
        "note": "product frontend parse succeeded; general FCBC emit from arbitrary source remains a later stage"
    });
    let rendered = summary.to_string();
    match output {
        Some(path) => {
            if let Err(error) = fs::write(path, rendered.as_bytes()) {
                eprintln!("error: failed to write {}: {error}", path.display());
                return ExitCategory::Internal.code();
            }
        }
        None => println!("{rendered}"),
    }
    ExitCategory::Success.code()
}

fn cmd_inspect(path: &Path, json: bool) -> ExitCode {
    let bytes = match read_fcbc_bytes(path) {
        Ok(bytes) => bytes,
        Err(category) => return category.code(),
    };
    let container = match load_container(&bytes) {
        Ok(container) => container,
        Err(error) => {
            eprintln!("error: {}: {}", error.category(), error.message());
            return ExitCategory::InputInvalid.code();
        }
    };
    let core = load_chart(&bytes).ok();
    if json {
        let body = serde_json::json!({
            "byteLength": container.byte_length,
            "sha256": lower_hex(&container.content_sha256),
            "profile": container.header.profile.as_str(),
            "sectionCount": container.sections.len(),
            "sectionTypes": container.section_types(),
            "coreLoaded": core.is_some(),
            "lineCount": core.as_ref().map(|chart| chart.lines.len()),
            "noteCount": core.as_ref().map(|chart| chart.notes.len()),
        });
        println!("{body}");
    } else {
        println!(
            "fcbc profile={} bytes={} sections={} sha256={}",
            container.header.profile.as_str(),
            container.byte_length,
            container.sections.len(),
            lower_hex(&container.content_sha256)
        );
        if let Some(chart) = core {
            println!(
                "core lines={} notes={} descriptors={}",
                chart.lines.len(),
                chart.notes.len(),
                chart.descriptors.len()
            );
        } else {
            println!("core: framing-only (full Core load not applicable for this file)");
        }
    }
    ExitCategory::Success.code()
}

fn cmd_convert(
    format: ConvertFormat,
    profile: &str,
    path: &Path,
    floor_scale_px: &str,
    json: bool,
) -> ExitCode {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("error: failed to read {}: {error}", path.display());
            return ExitCategory::Usage.code();
        }
    };
    let artifact = match SourceArtifact::new(path.display().to_string(), ArtifactRole::Chart, bytes)
    {
        Ok(artifact) => artifact,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCategory::InputInvalid.code();
        }
    };
    let result = match format {
        ConvertFormat::Pgr => convert_pgr(&artifact, profile, floor_scale_px),
        ConvertFormat::Rpe => convert_rpe(&artifact, profile),
        ConvertFormat::Pec => convert_pec(&artifact, profile, floor_scale_px),
    };
    match result {
        Ok((status, lines, notes)) => {
            if json {
                println!(
                    r#"{{"status":{},"lines":{},"notes":{},"profile":{}}}"#,
                    json_string(&status),
                    lines,
                    notes,
                    json_string(profile)
                );
            } else {
                println!("converted status={status} lines={lines} notes={notes} profile={profile}");
            }
            ExitCategory::Success.code()
        }
        Err((category, message)) => {
            eprintln!("error: {category}: {message}");
            if category.starts_with("conversion.unsupported") {
                ExitCategory::Unsupported.code()
            } else {
                ExitCategory::InputInvalid.code()
            }
        }
    }
}

fn convert_pgr(
    artifact: &SourceArtifact,
    profile: &str,
    floor_scale_px: &str,
) -> Result<(String, usize, usize), (String, String)> {
    let profile = match profile {
        "pgr.phira.v1" => PgrProfile::PhiraV1,
        "pgr.phira.v3" => PgrProfile::PhiraV3,
        "pgr.phichain-import.v1" => PgrProfile::PhichainImportV1,
        "pgr.phichain-import.v3" => PgrProfile::PhichainImportV3,
        other => {
            return Err((
                "conversion.profile-not-found".into(),
                format!("unsupported PGR profile {other}"),
            ));
        }
    };
    let floor = ExactDecimal::parse(floor_scale_px, DecimalLimits::default()).map_err(|error| {
        (
            "conversion.profile-parameter-invalid".into(),
            error.to_string(),
        )
    })?;
    let binding = PgrProfileBinding::new(profile, floor)
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let parsed = parse_json_document(SourceFormat::Pgr, artifact)
        .map_err(|error| ("conversion.source-invalid".into(), error.to_string()))?;
    let source = parse_pgr_document(&parsed, PgrLimits::default())
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let semantic = interpret_pgr(&source, &binding)
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let import = lower_pgr_to_canonical(&semantic, artifact)
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let chart = import.compilation().chart();
    Ok((
        import.report().status().as_str().to_owned(),
        chart.lines().lines().count(),
        chart.notes().notes().len(),
    ))
}

fn convert_rpe(
    artifact: &SourceArtifact,
    profile: &str,
) -> Result<(String, usize, usize), (String, String)> {
    let binding = match profile {
        "rpe.phira.legacy-speed" => RpeProfileBinding::phira_legacy_speed(),
        "rpe.phira.rpe170-speed" => RpeProfileBinding::phira_rpe170_speed(None),
        "rpe.phichain-import" => RpeProfileBinding::phichain_import(),
        other => {
            return Err((
                "conversion.profile-not-found".into(),
                format!("unsupported RPE profile {other}"),
            ));
        }
    };
    let parsed = parse_json_document(SourceFormat::Rpe, artifact)
        .map_err(|error| ("conversion.source-invalid".into(), error.to_string()))?;
    let source = parse_rpe_document(&parsed, fcs_conversion::RpeLimits::default())
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let semantic = interpret_rpe_semantics(&source, &binding)
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let import = lower_rpe_to_canonical(&semantic, artifact)
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let chart = import.compilation().chart();
    Ok((
        import.report().status().as_str().to_owned(),
        chart.lines().lines().count(),
        chart.notes().notes().len(),
    ))
}

fn convert_pec(
    artifact: &SourceArtifact,
    profile: &str,
    floor_scale_px: &str,
) -> Result<(String, usize, usize), (String, String)> {
    let profile = match profile {
        "pec.phira" => PecProfile::Phira,
        "pec.extends" => PecProfile::Extends,
        "pec.phispler" => PecProfile::Phispler,
        other => {
            return Err((
                "conversion.profile-not-found".into(),
                format!("unsupported PEC profile {other}"),
            ));
        }
    };
    let floor = ExactDecimal::parse(floor_scale_px, DecimalLimits::default()).map_err(|error| {
        (
            "conversion.profile-parameter-invalid".into(),
            error.to_string(),
        )
    })?;
    let binding = PecProfileBinding::new(profile, floor)
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let source = parse_pec_document(artifact, PecLimits::default())
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let semantic = interpret_pec(&source, &binding)
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let import = lower_pec_to_canonical(&semantic, artifact)
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let chart = import.compilation().chart();
    Ok((
        import.report().status().as_str().to_owned(),
        chart.lines().lines().count(),
        chart.notes().notes().len(),
    ))
}

fn read_fcbc_bytes(path: &Path) -> Result<Vec<u8>, ExitCategory> {
    let bytes = fs::read(path).map_err(|error| {
        eprintln!("error: failed to read {}: {error}", path.display());
        ExitCategory::Usage
    })?;
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("hex"))
    {
        let text = String::from_utf8(bytes).map_err(|_| {
            eprintln!("error: hex dump is not UTF-8");
            ExitCategory::InputInvalid
        })?;
        let filtered: String = text
            .chars()
            .filter(|ch| !ch.is_ascii_whitespace())
            .collect();
        if !filtered.len().is_multiple_of(2) {
            eprintln!("error: odd hex length");
            return Err(ExitCategory::InputInvalid);
        }
        let mut out = Vec::with_capacity(filtered.len() / 2);
        for index in (0..filtered.len()).step_by(2) {
            out.push(
                u8::from_str_radix(&filtered[index..index + 2], 16).map_err(|_| {
                    eprintln!("error: invalid hex");
                    ExitCategory::InputInvalid
                })?,
            );
        }
        Ok(out)
    } else {
        Ok(bytes)
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(
            output.status.code(),
            Some(ExitCategory::InputInvalid as i32)
        );
    }
}
