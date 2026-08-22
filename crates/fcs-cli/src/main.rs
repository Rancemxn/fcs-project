//! Product FCS CLI surface (I10.1–I10.4).
//!
//! Commands call domain crates only. Exit categories are stable machine values.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};

use clap::{Args, Parser, Subcommand, ValueEnum};
use fcs_conversion::{
    ApproximationAuthorization, ArtifactRole, CapabilitySet, DecimalLimits, DropAuthorization,
    ExactDecimal, ExportError, ExportOptions, PecLimits, PecProfile, PecProfileBinding, PgrLimits,
    PgrProfile, PgrProfileBinding, RpeProfileBinding, RpeSpeedMode, RpeVersionEra, SourceArtifact,
    SourceFormat, format_fcs_source, interpret_pec, interpret_pgr, interpret_rpe_semantics,
    lower_pec_to_canonical, lower_pgr_to_canonical, lower_rpe_to_canonical, parse_json_document,
    parse_pec_document, parse_pgr_document, parse_rpe_document,
};
use fcs_fcbc::{ContainerProfile, load_chart, load_container, write_from_compilation_with_profile};
use fcs_model::{
    CanonicalCompilation, CanonicalValue, CanonicalValueType, ConversionEntry, ConversionPolicy,
    ConversionReport, ErrorMetric, LogicalSourceLocator, MappingRuleRef, RepairMode, RepairRecord,
};
use fcs_render::{DecodedRenderChart, evaluate_semantic_draw_list, load_render};
use fcs_runtime::evaluate_easing;
use fcs_source::ResourceLimits;
use fcs_source::diagnostic::{DiagnosticSeverity, DiagnosticStage};
use fcs_source::elaborator::CompileTimeLimits;
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

static OUTPUT_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn write_output_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "output path has no file name")
    })?;

    for _ in 0..128 {
        let counter = OUTPUT_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".{}.{}.{}.tmp",
            name.to_string_lossy(),
            std::process::id(),
            counter
        ));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };

        let result = file.write_all(bytes).and_then(|_| file.sync_all());
        drop(file);
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }

        let result = replace_output_file(&temporary, path);
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        return result;
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique temporary output path",
    ))
}

#[cfg(not(windows))]
fn replace_output_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(temporary, destination)
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn MoveFileExW(existing_file_name: *const u16, new_file_name: *const u16, flags: u32) -> i32;
}

#[cfg(windows)]
fn replace_output_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    let temporary: Vec<u16> = temporary
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let success = unsafe {
        MoveFileExW(
            temporary.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if success == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
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
        /// Explicit workspace root for resolving declared resources.
        #[arg(long)]
        resolver_root: Option<PathBuf>,
    },
    /// Format FCS source with the fixed text policy.
    Format {
        /// Path to a `.fcs` source file.
        path: PathBuf,
        /// Write formatted source to this path (default: stdout).
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Compile FCS source through the product frontend and emit an FCBC package.
    Compile {
        /// Path to a `.fcs` source file.
        path: PathBuf,
        /// Optional output path for the FCBC package.
        #[arg(long)]
        output: Option<PathBuf>,
        #[command(flatten)]
        limits: CompileOptions,
    },
    /// Inspect an FCBC container after framing and Core validation.
    Inspect {
        /// Path to an FCBC binary (or `.hex` lowercase hex dump).
        path: PathBuf,
        /// Emit JSON.
        #[arg(long)]
        json: bool,
        /// When the package carries Render, evaluate a semantic draw-list summary.
        #[arg(long)]
        render: bool,
    },
    /// Convert an external chart into the product canonical import path.
    Convert {
        /// Source format family.
        #[arg(long, value_enum)]
        format: ConvertFormat,
        /// Emit JSON ConversionReport summary.
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        options: ConvertOptions,
    },
    /// Print a ConversionReport summary for an external import (alias of convert --json).
    Report {
        #[arg(long, value_enum)]
        format: ConvertFormat,
        #[command(flatten)]
        options: ConvertOptions,
    },
}

#[derive(Debug, Args)]
struct CompileOptions {
    /// FCBC container profile.
    #[arg(long, value_enum, default_value_t = FcbcProfileArg::StrictRuntime)]
    profile: FcbcProfileArg,
    /// Explicit workspace root for resolving declared resources.
    #[arg(long)]
    resolver_root: Option<PathBuf>,
    #[arg(long)]
    max_expansion_depth: Option<usize>,
    #[arg(long)]
    max_generated_nodes: Option<usize>,
    #[arg(long)]
    max_generator_iterations: Option<usize>,
    #[arg(long)]
    max_template_instances: Option<usize>,
    #[arg(long)]
    max_compile_time_operations: Option<usize>,
    #[arg(long)]
    max_expression_nodes: Option<usize>,
    #[arg(long)]
    max_resources: Option<usize>,
    #[arg(long)]
    max_single_resource_bytes: Option<usize>,
    #[arg(long)]
    max_total_resource_bytes: Option<usize>,
}

#[derive(Debug, Args)]
struct ConvertOptions {
    /// Explicit source semantic profile id or id@version.
    #[arg(long = "source-profile", alias = "profile")]
    source_profile: String,
    /// Explicit target semantic profile id or id@version. Supplying it enables export.
    #[arg(long)]
    target_profile: Option<String>,
    /// Explicit built-in target capability descriptor required for export.
    #[arg(long, value_enum)]
    target_capability: Option<TargetCapabilityArg>,
    /// Conversion policy used by an export.
    #[arg(long, value_enum, default_value_t = ConversionPolicyArg::Strict)]
    policy: ConversionPolicyArg,
    /// Path to the source chart bytes.
    path: PathBuf,
    /// Typed floor scale parameter for a PGR/PEC source profile.
    #[arg(long)]
    source_floor_scale_px: Option<String>,
    /// Typed floor scale parameter for a PGR/PEC target profile.
    #[arg(long)]
    target_floor_scale_px: Option<String>,
    /// Write target bytes here when exporting (defaults beside the source).
    #[arg(long)]
    output: Option<PathBuf>,
    /// Enable explicitly authorized source repair rules.
    #[arg(long)]
    repair: bool,
    /// Authorized repair rule; repeat for multiple rules.
    #[arg(long = "repair-rule")]
    repair_rules: Vec<String>,
    /// Approximation domain selector; repeat for multiple selectors.
    #[arg(long = "approximation-domain")]
    approximation_domains: Vec<String>,
    /// Approximation metric budget in `metric=bound` form; repeat as needed.
    #[arg(long = "approximation-budget")]
    approximation_budgets: Vec<String>,
    /// Maximum target segments for an authorized approximation.
    #[arg(long)]
    approximation_max_segments: Option<usize>,
    /// Approximation algorithm id.
    #[arg(long)]
    approximation_algorithm_id: Option<String>,
    /// Approximation algorithm version.
    #[arg(long)]
    approximation_algorithm_version: Option<String>,
    /// Drop selector in `domain.entity[.field]` form; repeat for multiple selectors.
    #[arg(long = "drop-selector")]
    drop_selectors: Vec<String>,
    /// Human reason required when drop selectors are supplied.
    #[arg(long)]
    drop_reason: Option<String>,
    /// Typed RPE speedMode binding for the source profile.
    #[arg(long, value_enum)]
    source_rpe_speed_mode: Option<RpeSpeedModeArg>,
    /// Typed RPE version-era binding for the source profile.
    #[arg(long, value_enum)]
    source_rpe_version_era: Option<RpeVersionEraArg>,
    /// Typed RPE speedMode binding for the target profile.
    #[arg(long, value_enum)]
    target_rpe_speed_mode: Option<RpeSpeedModeArg>,
    /// Typed RPE version-era binding for the target profile.
    #[arg(long, value_enum)]
    target_rpe_version_era: Option<RpeVersionEraArg>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FcbcProfileArg {
    Runtime,
    Fidelity,
    StrictRuntime,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ConversionPolicyArg {
    Semantic,
    Roundtrip,
    Strict,
}

impl From<ConversionPolicyArg> for ConversionPolicy {
    fn from(policy: ConversionPolicyArg) -> Self {
        match policy {
            ConversionPolicyArg::Semantic => Self::Semantic,
            ConversionPolicyArg::Roundtrip => Self::Roundtrip,
            ConversionPolicyArg::Strict => Self::Strict,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TargetCapabilityArg {
    PgrV1,
    PgrV3,
    RpeJson,
    PecLine,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum RpeSpeedModeArg {
    LegacyLinear,
    LegacyDerivative,
    ModernEased,
}

impl From<RpeSpeedModeArg> for RpeSpeedMode {
    fn from(mode: RpeSpeedModeArg) -> Self {
        match mode {
            RpeSpeedModeArg::LegacyLinear => Self::LegacyLinear,
            RpeSpeedModeArg::LegacyDerivative => Self::LegacyDerivative,
            RpeSpeedModeArg::ModernEased => Self::ModernEased,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum RpeVersionEraArg {
    #[value(name = "pre170")]
    Pre170,
    #[value(name = "at-least-170")]
    AtLeast170,
}

impl From<RpeVersionEraArg> for RpeVersionEra {
    fn from(era: RpeVersionEraArg) -> Self {
        match era {
            RpeVersionEraArg::Pre170 => Self::Pre170,
            RpeVersionEraArg::AtLeast170 => Self::AtLeast170,
        }
    }
}

impl From<FcbcProfileArg> for ContainerProfile {
    fn from(profile: FcbcProfileArg) -> Self {
        match profile {
            FcbcProfileArg::Runtime => Self::Runtime,
            FcbcProfileArg::Fidelity => Self::Fidelity,
            FcbcProfileArg::StrictRuntime => Self::StrictRuntime,
        }
    }
}

impl CompileOptions {
    fn compile_time_limits(&self) -> CompileTimeLimits {
        let defaults = CompileTimeLimits::default();
        CompileTimeLimits {
            max_expansion_depth: self
                .max_expansion_depth
                .unwrap_or(defaults.max_expansion_depth),
            max_generated_nodes: self
                .max_generated_nodes
                .unwrap_or(defaults.max_generated_nodes),
            max_generator_iterations: self
                .max_generator_iterations
                .unwrap_or(defaults.max_generator_iterations),
            max_template_instances: self
                .max_template_instances
                .unwrap_or(defaults.max_template_instances),
            max_compile_time_operations: self
                .max_compile_time_operations
                .unwrap_or(defaults.max_compile_time_operations),
            max_expression_nodes: self
                .max_expression_nodes
                .unwrap_or(defaults.max_expression_nodes),
        }
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(
            self.max_resources
                .unwrap_or(ResourceLimits::DEFAULT_MAX_RESOURCES),
            self.max_single_resource_bytes
                .unwrap_or(ResourceLimits::DEFAULT_MAX_SINGLE_RESOURCE_BYTES),
            self.max_total_resource_bytes
                .unwrap_or(ResourceLimits::DEFAULT_MAX_TOTAL_RESOURCE_BYTES),
        )
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ConvertFormat {
    Pgr,
    Rpe,
    Pec,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Commands::Check {
            path,
            json,
            resolver_root,
        } => cmd_check(&path, json, resolver_root.as_deref()),
        Commands::Format { path, output } => cmd_format(&path, output.as_deref()),
        Commands::Compile {
            path,
            output,
            limits,
        } => cmd_compile(&path, output.as_deref(), &limits),
        Commands::Inspect { path, json, render } => cmd_inspect(&path, json, render),
        Commands::Convert {
            format,
            json,
            options,
        } => cmd_convert(format, &options, json),
        Commands::Report { format, options } => cmd_convert(format, &options, true),
    }
}

fn cmd_check(path: &Path, json: bool, resolver_root: Option<&Path>) -> ExitCode {
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
    let document = match parse_document(text).into_result() {
        Ok(document) => document,
        Err(diagnostics) => {
            let message = diagnostics
                .first()
                .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
                .unwrap_or_else(|| "source invalid".into());
            if json {
                println!("{}", diagnostics_json(&diagnostics));
            } else {
                eprintln!("error: {message}");
            }
            return ExitCategory::InputInvalid.code();
        }
    };
    let workspace = resolver_root
        .or_else(|| path.parent())
        .unwrap_or_else(|| Path::new("."));
    match document.canonical_compilation_with_source(
        text,
        CompileTimeLimits::default(),
        workspace,
        ResourceLimits::default(),
    ) {
        Ok(_) => {
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
                println!("{}", diagnostics_json(&diagnostics));
            } else {
                eprintln!("error: {message}");
            }
            ExitCategory::InputInvalid.code()
        }
    }
}

fn cmd_format(path: &Path, output: Option<&Path>) -> ExitCode {
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
    let formatted = match format_fcs_source(text) {
        Ok(formatted) => formatted,
        Err(error) => {
            let message = format!("{}: {}", error.category(), error.message());
            eprintln!("error: cannot format invalid source: {message}");
            return ExitCategory::InputInvalid.code();
        }
    };
    match output {
        Some(path) => {
            if let Err(error) = write_output_atomically(path, formatted.as_bytes()) {
                eprintln!("error: failed to write {}: {error}", path.display());
                return ExitCategory::Internal.code();
            }
        }
        None => print!("{formatted}"),
    }
    ExitCategory::Success.code()
}

fn cmd_compile(path: &Path, output: Option<&Path>, options: &CompileOptions) -> ExitCode {
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
    let document = match parse_document(text).into_result() {
        Ok(document) => document,
        Err(diagnostics) => {
            let message = diagnostics
                .first()
                .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
                .unwrap_or_else(|| "source invalid".into());
            eprintln!("error: {message}");
            return ExitCategory::InputInvalid.code();
        }
    };
    let default_workspace = path.parent().unwrap_or_else(|| Path::new("."));
    let workspace = options
        .resolver_root
        .as_deref()
        .unwrap_or(default_workspace);
    let compilation = match document.canonical_compilation_with_source(
        text,
        options.compile_time_limits(),
        workspace,
        options.resource_limits(),
    ) {
        Ok(compilation) => compilation,
        Err(diagnostics) => {
            let message = diagnostics
                .first()
                .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
                .unwrap_or_else(|| "canonical compilation failed".into());
            eprintln!("error: {message}");
            return ExitCategory::InputInvalid.code();
        }
    };
    let chart = compilation.chart();
    let source_lines = chart.lines().lines().count();
    let source_notes = chart.notes().notes().len();
    let fcbc = match write_from_compilation_with_profile(&compilation, options.profile.into()) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("error: {}: {}", error.category(), error.message());
            return match error.category() {
                "fcbc.profile-requirement-missing" | "fcbc.unsupported-profile" => {
                    ExitCategory::Unsupported.code()
                }
                _ => ExitCategory::Internal.code(),
            };
        }
    };
    // Validate framing before the mandatory Core load.
    let container = match load_container(&fcbc) {
        Ok(container) => container,
        Err(error) => {
            eprintln!(
                "error: compiled FCBC failed product framing load: {}: {}",
                error.category(),
                error.message()
            );
            return ExitCategory::Internal.code();
        }
    };
    if let Err(error) = load_chart(&fcbc) {
        eprintln!("error: compiled FCBC failed Core load: {error}");
        return ExitCategory::Internal.code();
    }
    let out_path = output
        .map(PathBuf::from)
        .unwrap_or_else(|| path.with_extension("fcbc"));
    if let Err(error) = write_output_atomically(&out_path, &fcbc) {
        eprintln!("error: failed to write {}: {error}", out_path.display());
        return ExitCategory::Internal.code();
    }
    println!(
        "compiled {} -> {} bytes={} sections={} sourceLines={} sourceNotes={} coreLoaded=true",
        path.display(),
        out_path.display(),
        container.byte_length,
        container.sections.len(),
        source_lines,
        source_notes
    );
    ExitCategory::Success.code()
}

fn cmd_inspect(path: &Path, json: bool, render: bool) -> ExitCode {
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
    let core = match load_chart(&bytes) {
        Ok(chart) => chart,
        Err(error) => {
            eprintln!("error: Core load failed: {error}");
            return ExitCategory::InputInvalid.code();
        }
    };
    let render_summary = if render {
        match load_render(&bytes) {
            Ok(decoded) => match render_summary(&decoded) {
                Ok(summary) => Some(summary),
                Err(category) => {
                    eprintln!("error: render evaluation failed: {category}");
                    return ExitCategory::InputInvalid.code();
                }
            },
            Err(category) => {
                eprintln!("error: render load failed: {category}");
                return ExitCategory::InputInvalid.code();
            }
        }
    } else {
        None
    };
    // Prove runtime domain assembly is live for CLI (easing identity at t=0).
    let _ = evaluate_easing(fcs_runtime::EasingId::Linear as u16, 0.0);
    if json {
        let body = serde_json::json!({
            "byteLength": container.byte_length,
            "sha256": lower_hex(&container.content_sha256),
            "profile": container.header.profile.as_str(),
            "sectionCount": container.sections.len(),
            "sectionTypes": container.section_types(),
            "coreLoaded": true,
            "lineCount": core.lines.len(),
            "noteCount": core.notes.len(),
            "render": render_summary,
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
        println!(
            "core lines={} notes={} descriptors={}",
            core.lines.len(),
            core.notes.len(),
            core.descriptors.len()
        );
        if let Some(summary) = render_summary {
            println!(
                "render layers={} nodes={} drawOps={}",
                summary["layerCount"], summary["nodeCount"], summary["drawOps"]
            );
        }
    }
    ExitCategory::Success.code()
}

fn render_summary(decoded: &DecodedRenderChart) -> Result<serde_json::Value, &'static str> {
    let draw = evaluate_semantic_draw_list(decoded)?;
    Ok(serde_json::json!({
        "layerCount": decoded.layers.len(),
        "nodeCount": decoded.nodes.len(),
        "drawOps": draw.len(),
        "viewport": [decoded.viewport_width, decoded.viewport_height],
    }))
}

struct ImportOutcome {
    compilation: CanonicalCompilation,
    report: ConversionReport,
}

#[derive(Debug, Clone, Copy)]
enum TargetFormat {
    Pgr,
    Rpe,
    Pec,
}

#[derive(Debug)]
struct TargetExportError {
    category: String,
    message: String,
    report: Option<Box<ConversionReport>>,
}

impl TargetExportError {
    fn category(&self) -> &str {
        &self.category
    }

    fn message(&self) -> &str {
        &self.message
    }

    fn report(&self) -> Option<&ConversionReport> {
        self.report.as_deref()
    }
}

impl From<(String, String)> for TargetExportError {
    fn from((category, message): (String, String)) -> Self {
        Self {
            category,
            message,
            report: None,
        }
    }
}

impl From<ExportError> for TargetExportError {
    fn from(error: ExportError) -> Self {
        Self {
            category: error.category().to_owned(),
            message: error.message().to_owned(),
            report: error.report().cloned().map(Box::new),
        }
    }
}

impl TargetFormat {
    const fn extension(self) -> &'static str {
        match self {
            Self::Pgr | Self::Rpe => "json",
            Self::Pec => "pec",
        }
    }
}

fn cmd_convert(format: ConvertFormat, options: &ConvertOptions, json: bool) -> ExitCode {
    let path = &options.path;
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("error: failed to read {}: {error}", path.display());
            return ExitCategory::Usage.code();
        }
    };
    // Logical IDs must be relative (Conversion/locator rules reject absolute paths and URIs).
    let logical_id = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("chart");
    let artifact = match SourceArtifact::new(logical_id, ArtifactRole::Chart, bytes) {
        Ok(artifact) => artifact,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCategory::InputInvalid.code();
        }
    };
    let source_profile = match profile_reference(&options.source_profile) {
        Ok(profile) => profile,
        Err((category, message)) => {
            eprintln!("error: {category}: {message}");
            return conversion_exit_category(&category).code();
        }
    };
    if let Err((category, message)) = validate_profile_parameters(options) {
        eprintln!("error: {category}: {message}");
        return conversion_exit_category(&category).code();
    }
    if options.target_profile.is_none()
        && (options.output.is_some()
            || options.target_capability.is_some()
            || options.target_floor_scale_px.is_some()
            || options.target_rpe_speed_mode.is_some()
            || options.target_rpe_version_era.is_some()
            || options.repair
            || !options.repair_rules.is_empty()
            || !options.approximation_domains.is_empty()
            || !options.approximation_budgets.is_empty()
            || options.approximation_max_segments.is_some()
            || options.approximation_algorithm_id.is_some()
            || options.approximation_algorithm_version.is_some()
            || !options.drop_selectors.is_empty()
            || options.drop_reason.is_some())
    {
        eprintln!(
            "error: conversion.target-profile-required: target export options require --target-profile"
        );
        return ExitCategory::Unsupported.code();
    }
    let import = match import_source(
        format,
        &options.source_profile,
        &artifact,
        options.source_floor_scale_px.as_deref(),
        options.source_rpe_speed_mode,
        options.source_rpe_version_era,
    ) {
        Ok(import) => import,
        Err((category, message)) => {
            eprintln!("error: {category}: {message}");
            return conversion_exit_category(&category).code();
        }
    };
    let export = match options.target_profile.as_deref() {
        Some(target_profile) => match export_target(&import, target_profile, options) {
            Ok(export) => Some(export),
            Err(error) => {
                if json {
                    let body = conversion_failure_json(
                        &error,
                        source_profile.as_str(),
                        options.target_profile.as_deref(),
                        options.target_capability.map(TargetCapabilityArg::as_str),
                        options.policy.into(),
                    );
                    println!("{body}");
                } else {
                    eprintln!("error: {}: {}", error.category(), error.message());
                }
                return conversion_exit_category(error.category()).code();
            }
        },
        None => None,
    };
    if let Some((_, outcome, output)) = &export
        && let Err(error) = write_output_atomically(output, outcome.bytes())
    {
        eprintln!("error: failed to write {}: {error}", output.display());
        return ExitCategory::Internal.code();
    }

    let chart = import.compilation.chart();
    let report = export
        .as_ref()
        .map_or(&import.report, |(_, outcome, _)| outcome.report());
    let output_path = export
        .as_ref()
        .map(|(_, _, output)| output.display().to_string());
    let profile = options
        .target_profile
        .as_deref()
        .unwrap_or(options.source_profile.as_str());
    if json {
        let body = serde_json::json!({
            "status": report.status().as_str(),
            "lines": chart.lines().lines().count(),
            "notes": chart.notes().notes().len(),
            "profile": profile,
            "sourceProfile": source_profile,
            "sourceStatus": import.report.status().as_str(),
            "targetProfile": options.target_profile,
            "targetCapability": options.target_capability.map(TargetCapabilityArg::as_str),
            "policy": report.conversion_policy().as_str(),
            "output": output_path,
            "report": report_summary(report),
        });
        println!("{body}");
    } else if let Some((_, outcome, output)) = &export {
        println!(
            "converted status={} lines={} notes={} sourceProfile={} targetProfile={} bytes={} output={}",
            report.status().as_str(),
            chart.lines().lines().count(),
            chart.notes().notes().len(),
            source_profile,
            options.target_profile.as_deref().unwrap_or(""),
            outcome.bytes().len(),
            output.display()
        );
    } else {
        println!(
            "converted status={} lines={} notes={} profile={}",
            report.status().as_str(),
            chart.lines().lines().count(),
            chart.notes().notes().len(),
            options.source_profile
        );
    }
    ExitCategory::Success.code()
}

fn import_source(
    format: ConvertFormat,
    profile: &str,
    artifact: &SourceArtifact,
    floor_scale_px: Option<&str>,
    rpe_speed_mode: Option<RpeSpeedModeArg>,
    rpe_version_era: Option<RpeVersionEraArg>,
) -> Result<ImportOutcome, (String, String)> {
    match format {
        ConvertFormat::Pgr => import_pgr(artifact, profile, floor_scale_px),
        ConvertFormat::Rpe => import_rpe(artifact, profile, rpe_speed_mode, rpe_version_era),
        ConvertFormat::Pec => import_pec(artifact, profile, floor_scale_px),
    }
}

fn import_pgr(
    artifact: &SourceArtifact,
    profile: &str,
    floor_scale_px: Option<&str>,
) -> Result<ImportOutcome, (String, String)> {
    let profile = match profile_id(profile)? {
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
    let binding = PgrProfileBinding::new(
        profile,
        parse_required_decimal(floor_scale_px, "--source-floor-scale-px")?,
    )
    .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let parsed = parse_json_document(SourceFormat::Pgr, artifact)
        .map_err(|error| ("conversion.source-invalid".into(), error.to_string()))?;
    let source = parse_pgr_document(&parsed, PgrLimits::default())
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let semantic = interpret_pgr(&source, &binding)
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let import = lower_pgr_to_canonical(&semantic, artifact)
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let (compilation, report) = import.into_parts();
    Ok(ImportOutcome {
        compilation,
        report,
    })
}

fn import_rpe(
    artifact: &SourceArtifact,
    profile: &str,
    speed_mode: Option<RpeSpeedModeArg>,
    version_era: Option<RpeVersionEraArg>,
) -> Result<ImportOutcome, (String, String)> {
    let binding = source_rpe_binding(profile, speed_mode, version_era)?;
    let parsed = parse_json_document(SourceFormat::Rpe, artifact)
        .map_err(|error| ("conversion.source-invalid".into(), error.to_string()))?;
    let source = parse_rpe_document(&parsed, fcs_conversion::RpeLimits::default())
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let semantic = interpret_rpe_semantics(&source, &binding)
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let import = lower_rpe_to_canonical(&semantic, artifact)
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let (compilation, report) = import.into_parts();
    Ok(ImportOutcome {
        compilation,
        report,
    })
}

fn import_pec(
    artifact: &SourceArtifact,
    profile: &str,
    floor_scale_px: Option<&str>,
) -> Result<ImportOutcome, (String, String)> {
    let profile = match profile_id(profile)? {
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
    let binding = PecProfileBinding::new(
        profile,
        parse_required_decimal(floor_scale_px, "--source-floor-scale-px")?,
    )
    .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let source = parse_pec_document(artifact, PecLimits::default())
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let semantic = interpret_pec(&source, &binding)
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let import = lower_pec_to_canonical(&semantic, artifact)
        .map_err(|error| (error.category().to_owned(), error.to_string()))?;
    let (compilation, report) = import.into_parts();
    Ok(ImportOutcome {
        compilation,
        report,
    })
}

fn export_target(
    import: &ImportOutcome,
    profile: &str,
    options: &ConvertOptions,
) -> Result<(TargetFormat, fcs_conversion::ExportOutcome, PathBuf), TargetExportError> {
    let target_profile = profile_reference(profile)?;
    let target_id = profile_id(profile)?;
    let target_format = if target_id.starts_with("pgr.") {
        TargetFormat::Pgr
    } else if target_id.starts_with("rpe.") {
        TargetFormat::Rpe
    } else if target_id.starts_with("pec.") {
        TargetFormat::Pec
    } else {
        return Err((
            "conversion.profile-not-found".into(),
            format!("unsupported target profile {target_id}"),
        )
            .into());
    };
    let capability = options.target_capability.ok_or_else(|| {
        (
            "conversion.capability-mismatch".into(),
            "target export requires --target-capability".into(),
        )
    })?;
    let descriptor = target_capability(capability).descriptor(Some(target_profile.clone()));
    let policy: ConversionPolicy = options.policy.into();
    let mut export_options = match policy {
        ConversionPolicy::Strict => ExportOptions::strict(descriptor),
        ConversionPolicy::Semantic | ConversionPolicy::Roundtrip => {
            ExportOptions::semantic(descriptor)
        }
    };
    export_options.policy = policy;
    export_options = export_options.with_target_profile(target_profile);
    export_options = export_options.with_repair_mode(repair_mode(options)?);
    export_options = export_options.with_approximation(approximation_authorization(options)?);
    export_options = export_options.with_drop(drop_authorization(options)?);

    let outcome = match target_format {
        TargetFormat::Pgr => {
            export_options = export_options.with_floor_scale_px(parse_required_decimal(
                options.target_floor_scale_px.as_deref(),
                "--target-floor-scale-px",
            )?);
            fcs_conversion::export_pgr_compilation_with_options(
                &import.compilation,
                &export_options,
            )
        }?,
        TargetFormat::Rpe => {
            let binding = target_rpe_binding(
                profile,
                options.target_rpe_speed_mode,
                options.target_rpe_version_era,
            )?;
            export_options = export_options.with_rpe_profile_binding(binding);
            fcs_conversion::export_rpe_compilation_with_options(
                &import.compilation,
                &export_options,
            )
        }?,
        TargetFormat::Pec => {
            export_options = export_options.with_floor_scale_px(parse_required_decimal(
                options.target_floor_scale_px.as_deref(),
                "--target-floor-scale-px",
            )?);
            fcs_conversion::export_pec_compilation_with_options(
                &import.compilation,
                &export_options,
            )
        }?,
    };
    let output = options
        .output
        .clone()
        .unwrap_or_else(|| default_export_path(&options.path, target_format.extension()));
    Ok((target_format, outcome, output))
}

fn default_export_path(source: &Path, extension: &str) -> PathBuf {
    let candidate = source.with_extension(extension);
    if candidate != source {
        candidate
    } else {
        source.with_extension(format!("converted.{extension}"))
    }
}

fn source_rpe_binding(
    profile: &str,
    speed_mode: Option<RpeSpeedModeArg>,
    version_era: Option<RpeVersionEraArg>,
) -> Result<RpeProfileBinding, (String, String)> {
    let profile = profile_id(profile)?;
    let speed_mode = speed_mode.map(RpeSpeedMode::from);
    let version_era = version_era.map(RpeVersionEra::from);
    match profile {
        "rpe.community.divide-bpmfactor" => speed_mode
            .map(RpeProfileBinding::community_divide)
            .ok_or_else(|| {
                parameter_error("this RPE source profile requires --source-rpe-speed-mode")
            }),
        "rpe.docs-example.multiply-bpmfactor" => speed_mode
            .map(RpeProfileBinding::docs_example_multiply)
            .ok_or_else(|| {
                parameter_error("this RPE source profile requires --source-rpe-speed-mode")
            }),
        "rpe.phira.legacy-speed" => {
            reject_rpe_parameters(speed_mode, version_era)?;
            Ok(RpeProfileBinding::phira_legacy_speed())
        }
        "rpe.phira.rpe170-speed" => {
            if speed_mode.is_some() {
                return Err(parameter_error(
                    "rpe.phira.rpe170-speed does not accept --source-rpe-speed-mode",
                ));
            }
            Ok(RpeProfileBinding::phira_rpe170_speed(version_era))
        }
        "rpe.phichain-import" => {
            reject_rpe_parameters(speed_mode, version_era)?;
            Ok(RpeProfileBinding::phichain_import())
        }
        other => Err((
            "conversion.profile-not-found".into(),
            format!("unsupported RPE profile {other}"),
        )),
    }
}

fn target_rpe_binding(
    profile: &str,
    speed_mode: Option<RpeSpeedModeArg>,
    version_era: Option<RpeVersionEraArg>,
) -> Result<RpeProfileBinding, (String, String)> {
    let profile = profile_id(profile)?;
    let speed_mode = speed_mode.map(RpeSpeedMode::from);
    let version_era = version_era.map(RpeVersionEra::from);
    match profile {
        "rpe.community.divide-bpmfactor" => {
            if version_era.is_some() {
                return Err(parameter_error(
                    "rpe.community.divide-bpmfactor does not accept --target-rpe-version-era",
                ));
            }
            speed_mode
                .map(RpeProfileBinding::community_divide)
                .ok_or_else(|| {
                    parameter_error("this RPE target profile requires --target-rpe-speed-mode")
                })
        }
        "rpe.docs-example.multiply-bpmfactor" => {
            if version_era.is_some() {
                return Err(parameter_error(
                    "rpe.docs-example.multiply-bpmfactor does not accept --target-rpe-version-era",
                ));
            }
            speed_mode
                .map(RpeProfileBinding::docs_example_multiply)
                .ok_or_else(|| {
                    parameter_error("this RPE target profile requires --target-rpe-speed-mode")
                })
        }
        "rpe.phira.legacy-speed" => {
            reject_rpe_parameters(speed_mode, version_era)?;
            Ok(RpeProfileBinding::phira_legacy_speed())
        }
        "rpe.phira.rpe170-speed" => {
            if speed_mode.is_some() {
                return Err(parameter_error(
                    "rpe.phira.rpe170-speed does not accept --target-rpe-speed-mode",
                ));
            }
            let version_era = version_era.ok_or_else(|| {
                parameter_error("this RPE target profile requires --target-rpe-version-era")
            })?;
            Ok(RpeProfileBinding::phira_rpe170_speed(Some(version_era)))
        }
        "rpe.phichain-import" => {
            reject_rpe_parameters(speed_mode, version_era)?;
            Ok(RpeProfileBinding::phichain_import())
        }
        other => Err((
            "conversion.profile-not-found".into(),
            format!("unsupported RPE target profile {other}"),
        )),
    }
}

fn reject_rpe_parameters(
    speed_mode: Option<RpeSpeedMode>,
    version_era: Option<RpeVersionEra>,
) -> Result<(), (String, String)> {
    if speed_mode.is_some() || version_era.is_some() {
        Err(parameter_error(
            "the selected RPE profile does not accept the supplied typed parameters",
        ))
    } else {
        Ok(())
    }
}

fn repair_mode(options: &ConvertOptions) -> Result<RepairMode, (String, String)> {
    if !options.repair && !options.repair_rules.is_empty() {
        return Err(parameter_error("--repair-rule requires --repair"));
    }
    let rules = options
        .repair_rules
        .iter()
        .map(|rule| MappingRuleRef::new(rule).map_err(|error| parameter_error(error.to_string())))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RepairMode::new(options.repair, rules))
}

fn approximation_authorization(
    options: &ConvertOptions,
) -> Result<ApproximationAuthorization, (String, String)> {
    let configured = !options.approximation_domains.is_empty()
        || !options.approximation_budgets.is_empty()
        || options.approximation_max_segments.is_some()
        || options.approximation_algorithm_id.is_some()
        || options.approximation_algorithm_version.is_some();
    if !configured {
        return Ok(ApproximationAuthorization::disabled());
    }
    let maximum_segments = options
        .approximation_max_segments
        .ok_or_else(|| parameter_error("approximation requires --approximation-max-segments"))?;
    let algorithm_id = options
        .approximation_algorithm_id
        .clone()
        .ok_or_else(|| parameter_error("approximation requires --approximation-algorithm-id"))?;
    let algorithm_version = options
        .approximation_algorithm_version
        .clone()
        .ok_or_else(|| {
            parameter_error("approximation requires --approximation-algorithm-version")
        })?;
    let mut budgets = Vec::with_capacity(options.approximation_budgets.len());
    for raw in &options.approximation_budgets {
        let (metric, value) = raw
            .split_once('=')
            .ok_or_else(|| parameter_error("approximation budget must use metric=bound"))?;
        let value = value
            .parse::<f64>()
            .map_err(|error| parameter_error(error.to_string()))?;
        if !value.is_finite() || value < 0.0 {
            return Err(parameter_error(
                "approximation budget must be finite and non-negative",
            ));
        }
        budgets.push((metric.to_owned(), value));
    }
    ApproximationAuthorization::new(
        options.approximation_domains.clone(),
        budgets,
        maximum_segments,
        algorithm_id,
        algorithm_version,
    )
    .map_err(|error| parameter_error(error.to_string()))
}

fn drop_authorization(options: &ConvertOptions) -> Result<DropAuthorization, (String, String)> {
    if options.drop_selectors.is_empty() && options.drop_reason.is_none() {
        return Ok(DropAuthorization::disabled());
    }
    let reason = options
        .drop_reason
        .clone()
        .ok_or_else(|| parameter_error("drop authorization requires --drop-reason"))?;
    if options.drop_selectors.is_empty() {
        return Err(parameter_error(
            "drop authorization requires --drop-selector",
        ));
    }
    DropAuthorization::new(options.drop_selectors.clone(), reason)
        .map_err(|error| parameter_error(error.to_string()))
}

fn target_capability(capability: TargetCapabilityArg) -> CapabilitySet {
    match capability {
        TargetCapabilityArg::PgrV1 => CapabilitySet::pgr_v1(),
        TargetCapabilityArg::PgrV3 => CapabilitySet::pgr_v3(),
        TargetCapabilityArg::RpeJson => CapabilitySet::rpe_json(),
        TargetCapabilityArg::PecLine => CapabilitySet::pec_line(),
    }
}

impl TargetCapabilityArg {
    const fn as_str(self) -> &'static str {
        match self {
            Self::PgrV1 => "pgr-v1",
            Self::PgrV3 => "pgr-v3",
            Self::RpeJson => "rpe-json",
            Self::PecLine => "pec-line",
        }
    }
}

fn report_summary(report: &ConversionReport) -> serde_json::Value {
    serde_json::json!({
        "specificationVersion": report.specification_version(),
        "operationId": report.operation_id(),
        "conversionPolicy": report.conversion_policy().as_str(),
        "status": report.status().as_str(),
        "repairMode": repair_mode_json(report.repair_mode()),
        "approximationAuthorization": report
            .approximation_authorization()
            .map(approximation_authorization_json),
        "dropAuthorization": report.drop_authorization().map(drop_authorization_json),
        "entryCount": report.entries().len(),
        "entries": report.entries().iter().map(conversion_entry_json).collect::<Vec<_>>(),
        "repairs": report.repairs().iter().map(repair_record_json).collect::<Vec<_>>(),
        "repairCount": report.summary().repair_count(),
        "dropCount": report.summary().drop_count(),
        "outputHash": report.output_hash(),
        "bySeverity": report.summary().by_severity(),
        "byStatus": report.summary().by_status(),
        "byCategory": report.summary().by_category(),
        "byDomain": report.summary().by_domain(),
    })
}

fn conversion_failure_json(
    error: &TargetExportError,
    source_profile: &str,
    target_profile: Option<&str>,
    target_capability: Option<&str>,
    policy: ConversionPolicy,
) -> serde_json::Value {
    serde_json::json!({
        "status": error.report().map_or("failed", |report| report.status().as_str()),
        "category": error.category(),
        "message": error.message(),
        "profile": target_profile.unwrap_or(source_profile),
        "sourceProfile": source_profile,
        "targetProfile": target_profile,
        "targetCapability": target_capability,
        "policy": policy.as_str(),
        "output": serde_json::Value::Null,
        "report": error.report().map(report_summary),
    })
}

fn repair_mode_json(mode: &RepairMode) -> serde_json::Value {
    serde_json::json!({
        "enabled": mode.enabled(),
        "authorizedRules": mode
            .authorized_rules()
            .iter()
            .map(MappingRuleRef::as_str)
            .collect::<Vec<_>>(),
    })
}

fn approximation_authorization_json(
    authorization: &ApproximationAuthorization,
) -> serde_json::Value {
    serde_json::json!({
        "enabled": authorization.enabled(),
        "targetDomains": authorization.target_domains(),
        "errorBudgets": authorization.error_budgets(),
        "maximumSegments": authorization.maximum_segments(),
        "algorithmId": authorization.algorithm_id(),
        "algorithmVersion": authorization.algorithm_version(),
    })
}

fn drop_authorization_json(authorization: &DropAuthorization) -> serde_json::Value {
    serde_json::json!({
        "enabled": authorization.enabled(),
        "targetSelectors": authorization.target_selectors(),
        "reason": authorization.reason(),
    })
}

fn conversion_entry_json(entry: &ConversionEntry) -> serde_json::Value {
    serde_json::json!({
        "id": entry.id(),
        "category": entry.category(),
        "domain": entry.domain().as_str(),
        "severity": entry.severity().as_str(),
        "semanticStatus": entry.semantic_status().as_str(),
        "phase": entry.phase().as_str(),
        "sourceLocator": entry.source_locator().map(locator_string),
        "targetLocator": entry.target_locator().map(locator_string),
        "entityId": entry.entity_id(),
        "fieldKey": entry.field_key(),
        "rule": entry.rule().map(MappingRuleRef::as_str),
        "sourceValue": entry.source_value().map(canonical_value_json),
        "interpretedValue": entry.interpreted_value().map(canonical_value_json),
        "canonicalValue": entry.canonical_value().map(canonical_value_json),
        "targetValue": entry.target_value().map(canonical_value_json),
        "message": entry.message(),
        "errorMetric": entry.error_metric().map(error_metric_json),
        "dependencies": entry.dependencies(),
    })
}

fn repair_record_json(record: &RepairRecord) -> serde_json::Value {
    serde_json::json!({
        "id": record.id(),
        "sourceLocator": record.source_locator().as_str(),
        "diagnosticCategory": record.diagnostic_category(),
        "action": record.action(),
        "rule": record.rule().as_str(),
        "oldValue": canonical_value_json(record.old_value()),
        "newValue": canonical_value_json(record.new_value()),
        "semanticImpact": record.semantic_impact(),
    })
}

fn error_metric_json(metric: &ErrorMetric) -> serde_json::Value {
    serde_json::json!({
        "domain": metric.domain().as_str(),
        "metric": metric.metric(),
        "declaredMaximum": json_float(metric.declared_maximum()),
        "verifiedMaximum": json_float(metric.verified_maximum()),
        "verificationMethod": metric.verification_method(),
        "sampleCount": metric.sample_count(),
        "segmentCount": metric.segment_count(),
        "forcedBoundaries": metric
            .forced_boundaries()
            .iter()
            .copied()
            .map(json_float)
            .collect::<Vec<_>>(),
        "sourceDescriptorHash": metric.source_descriptor_hash(),
    })
}

fn locator_string(locator: &LogicalSourceLocator) -> String {
    locator.as_str().to_owned()
}

fn canonical_value_json(value: &CanonicalValue) -> serde_json::Value {
    match value {
        CanonicalValue::Null => serde_json::json!({"type": "null"}),
        CanonicalValue::Bool(value) => serde_json::json!({"type": "bool", "value": value}),
        CanonicalValue::Int(value) => serde_json::json!({"type": "int", "value": value}),
        CanonicalValue::Float(value) => {
            serde_json::json!({"type": "float", "value": json_float(*value)})
        }
        CanonicalValue::String(value) => serde_json::json!({"type": "string", "value": value}),
        CanonicalValue::Time(value) => {
            serde_json::json!({"type": "time", "value": json_float(*value)})
        }
        CanonicalValue::Beat(value) => serde_json::json!({
            "type": "beat",
            "numerator": value.numerator(),
            "denominator": value.denominator(),
        }),
        CanonicalValue::Color(value) => serde_json::json!({
            "type": "color",
            "linear": [
                json_float(value.red()),
                json_float(value.green()),
                json_float(value.blue()),
                json_float(value.alpha()),
            ],
        }),
        CanonicalValue::ResourceReference(value) => {
            serde_json::json!({"type": "resource-reference", "value": value})
        }
        CanonicalValue::ContributorReference(value) => {
            serde_json::json!({"type": "contributor-reference", "value": value})
        }
        CanonicalValue::Array {
            element_type,
            values,
        } => serde_json::json!({
            "type": "array",
            "elementType": canonical_value_type_name(element_type),
            "values": values.iter().map(canonical_value_json).collect::<Vec<_>>(),
        }),
        CanonicalValue::Object(object) => serde_json::json!({
            "type": "object",
            "entries": object
                .entries()
                .iter()
                .map(|entry| serde_json::json!({
                    "key": entry.key(),
                    "value": canonical_value_json(entry.value()),
                }))
                .collect::<Vec<_>>(),
        }),
    }
}

fn canonical_value_type_name(value_type: &CanonicalValueType) -> String {
    match value_type {
        CanonicalValueType::Null => "null".into(),
        CanonicalValueType::Bool => "bool".into(),
        CanonicalValueType::Int => "int".into(),
        CanonicalValueType::Float => "float".into(),
        CanonicalValueType::String => "string".into(),
        CanonicalValueType::Time => "time".into(),
        CanonicalValueType::Beat => "beat".into(),
        CanonicalValueType::Color => "color".into(),
        CanonicalValueType::ResourceReference => "resource-reference".into(),
        CanonicalValueType::ContributorReference => "contributor-reference".into(),
        CanonicalValueType::Array(element_type) => {
            format!("array<{}>", canonical_value_type_name(element_type))
        }
        CanonicalValueType::Object => "object".into(),
    }
}

fn json_float(value: f64) -> serde_json::Value {
    serde_json::Number::from_f64(value)
        .map(serde_json::Value::Number)
        .unwrap_or_else(|| serde_json::Value::String(value.to_string()))
}

fn parse_decimal(value: &str) -> Result<ExactDecimal, (String, String)> {
    ExactDecimal::parse(value, DecimalLimits::default())
        .map_err(|error| parameter_error(error.to_string()))
}

fn parse_required_decimal(
    value: Option<&str>,
    flag: &str,
) -> Result<ExactDecimal, (String, String)> {
    parse_decimal(value.ok_or_else(|| parameter_error(format!("{flag} is required")))?)
}

fn validate_profile_parameters(options: &ConvertOptions) -> Result<(), (String, String)> {
    let source = profile_id(&options.source_profile)?;
    let target = options
        .target_profile
        .as_deref()
        .map(profile_id)
        .transpose()?;
    let source_uses_floor = source.starts_with("pgr.") || source.starts_with("pec.");
    if options.source_floor_scale_px.is_some() && !source_uses_floor {
        return Err(parameter_error(
            "--source-floor-scale-px requires a PGR or PEC source profile",
        ));
    }
    let target_uses_floor =
        target.is_some_and(|profile| profile.starts_with("pgr.") || profile.starts_with("pec."));
    if options.target_floor_scale_px.is_some() && !target_uses_floor {
        return Err(parameter_error(
            "--target-floor-scale-px requires a PGR or PEC target profile",
        ));
    }
    if (options.source_rpe_speed_mode.is_some() || options.source_rpe_version_era.is_some())
        && !source.starts_with("rpe.")
    {
        return Err(parameter_error(
            "source RPE parameters require an RPE source profile",
        ));
    }
    if (options.target_rpe_speed_mode.is_some() || options.target_rpe_version_era.is_some())
        && !target.is_some_and(|profile| profile.starts_with("rpe."))
    {
        return Err(parameter_error(
            "target RPE parameters require an RPE target profile",
        ));
    }
    Ok(())
}

fn profile_id(profile: &str) -> Result<&str, (String, String)> {
    let profile = profile.trim();
    if profile.is_empty() {
        return Err(parameter_error("profile reference must not be empty"));
    }
    if let Some((id, version)) = profile.split_once('@') {
        if id.is_empty() || version != "1.0.0" {
            return Err(parameter_error(
                "only registered 1.0.0 profile references are supported",
            ));
        }
        Ok(id)
    } else {
        Ok(profile)
    }
}

fn profile_reference(profile: &str) -> Result<String, (String, String)> {
    Ok(format!("{}@1.0.0", profile_id(profile)?))
}

fn parameter_error(message: impl Into<String>) -> (String, String) {
    (
        "conversion.profile-parameter-invalid".into(),
        message.into(),
    )
}

fn conversion_exit_category(category: &str) -> ExitCategory {
    match category {
        "conversion.capability-mismatch"
        | "conversion.profile-not-applicable"
        | "conversion.profile-not-found"
        | "conversion.target-profile-required"
        | "conversion.approximation-not-authorized"
        | "conversion.approximation-budget-exceeded"
        | "conversion.drop-not-authorized"
        | "conversion.unsupported-format"
        | "conversion.unsupported-format-version" => ExitCategory::Unsupported,
        _ => ExitCategory::InputInvalid,
    }
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
        let filtered: Vec<u8> = text
            .bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect();
        if !filtered.len().is_multiple_of(2) {
            eprintln!("error: odd hex length");
            return Err(ExitCategory::InputInvalid);
        }
        let mut out = Vec::with_capacity(filtered.len() / 2);
        for pair in filtered.as_chunks::<2>().0 {
            let pair = std::str::from_utf8(pair).map_err(|_| {
                eprintln!("error: invalid hex");
                ExitCategory::InputInvalid
            })?;
            out.push(u8::from_str_radix(pair, 16).map_err(|_| {
                eprintln!("error: invalid hex");
                ExitCategory::InputInvalid
            })?);
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

fn diagnostics_json(diagnostics: &[fcs_source::diagnostic::Diagnostic]) -> String {
    let diagnostics = diagnostics
        .iter()
        .map(|diagnostic| {
            let span = diagnostic.primary_span();
            let labels = diagnostic
                .labels()
                .iter()
                .map(|label| {
                    let span = label.span();
                    serde_json::json!({
                        "span": {"start": span.start, "end": span.end},
                        "message": label.message(),
                    })
                })
                .collect::<Vec<_>>();
            let mut value = serde_json::json!({
                "code": diagnostic.code().as_str(),
                "stage": diagnostic_stage_name(diagnostic.stage()),
                "severity": diagnostic_severity_name(diagnostic.severity()),
                "message": diagnostic.message(),
                "span": {"start": span.start, "end": span.end},
                "labels": labels,
            });
            if let Some(budget) = diagnostic.budget() {
                value["budget"] = serde_json::json!({
                    "kind": budget.kind(),
                    "limit": budget.limit(),
                    "observed": budget.observed(),
                });
            }
            value
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "status": "failed",
        "category": "source.invalid",
        "diagnostics": diagnostics,
    })
    .to_string()
}

const fn diagnostic_stage_name(stage: DiagnosticStage) -> &'static str {
    match stage {
        DiagnosticStage::Decode => "decode",
        DiagnosticStage::Parse => "parse",
        DiagnosticStage::Elaborate => "elaborate",
        DiagnosticStage::Canonical => "canonical",
        DiagnosticStage::Evaluate => "evaluate",
        DiagnosticStage::Implementation => "implementation",
    }
}

const fn diagnostic_severity_name(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
    }
}

#[cfg(test)]
mod output_tests {
    use std::fs;

    use fcs_model::{ConversionPolicy, ConversionReport, ConversionStatus, RepairMode};

    use super::{TargetExportError, conversion_failure_json, write_output_atomically};

    #[test]
    fn atomic_output_replaces_existing_file_without_leaving_a_temp_file() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("result.bin");
        fs::write(&output, b"old output").unwrap();

        write_output_atomically(&output, b"new output").unwrap();

        assert_eq!(fs::read(&output).unwrap(), b"new output");
        assert_eq!(directory.path().read_dir().unwrap().count(), 1);
    }

    #[test]
    fn conversion_failure_json_retains_the_failed_report() {
        let report = ConversionReport::new(
            "rpe-export-failed-test",
            ConversionPolicy::Strict,
            RepairMode::disabled(),
            Vec::<fcs_model::ConversionEntry>::new(),
            Vec::<fcs_model::RepairRecord>::new(),
            [ConversionStatus::Failed],
            None,
        )
        .unwrap();
        let error = TargetExportError {
            category: "conversion.roundtrip-mismatch".into(),
            message: "target canonical reparse failed".into(),
            report: Some(Box::new(report)),
        };

        let body = conversion_failure_json(
            &error,
            "rpe.phira.legacy-speed@1.0.0",
            Some("rpe.phira.legacy-speed@1.0.0"),
            Some("rpe-json"),
            ConversionPolicy::Strict,
        );

        assert_eq!(body["status"], "failed");
        assert_eq!(body["category"], "conversion.roundtrip-mismatch");
        assert_eq!(body["report"]["status"], "failed");
        assert!(body["report"]["entries"].is_array());
        assert!(body["report"]["outputHash"].is_null());
    }
}
