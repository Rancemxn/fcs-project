//! I8 product export and FCS format surfaces.
//!
//! `format_fcs_source` validates source then rewrites UTF-8 without inventing
//! semantics. `export_pgr_v3` emits a formatVersion-3 PGR chart from a product
//! CanonicalChart so target reparse can run through the existing importer.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fcs_model::{
    CanonicalChart, CanonicalColor, CanonicalCompilation, CanonicalJudgeShape, CanonicalLine,
    CanonicalLineInherit, CanonicalNoteKind, CanonicalNoteScorePolicy, CanonicalNoteSide,
    CanonicalNoteSoundPolicy, CanonicalResourceBundle, CanonicalTextualId, CanonicalTrack,
    CanonicalTrackBlend, CanonicalTrackFill, CanonicalTrackInterpolation, CanonicalTrackPiece,
    CanonicalTrackTarget, CanonicalTrackValue, CanonicalValue, ConversionDomain, ConversionEntry,
    ConversionPhase, ConversionPolicy, ConversionReport, ConversionSeverity, ConversionStatus,
    EntityKind, ErrorMetric, ExpansionPath, RepairMode, ReportError, SemanticLoss, SemanticStatus,
    StableId, StableIdRegistry,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::comparison::{EntityAlignment, MAX_REPORT_ENTRIES};
use crate::{
    ApproximationAuthorization, ArtifactRole, CapabilityDescriptor, CapabilityDomainDescriptor,
    CapabilityFeature, DecimalLimits, DropAuthorization, ExactDecimal, PecLimits, PecProfile,
    PecProfileBinding, PgrLimits, PgrProfile, PgrProfileBinding, RpeLimits, RpeProfile,
    RpeProfileBinding, RpeVersionEra, SourceArtifact, SourceFormat,
    compare_canonical_charts_with_resources_with_budgets_and_ignored, interpret_pec, interpret_pgr,
    interpret_rpe_semantics, lower_pec_to_canonical, lower_pgr_to_canonical,
    lower_rpe_to_canonical, parse_json_document, parse_pec_document, parse_pgr_document,
    parse_rpe_document,
};

/// Stable formatter / exporter diagnostic category.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportError {
    category: &'static str,
    message: String,
    entries: Vec<ConversionEntry>,
    report: Option<Box<ConversionReport>>,
}

#[derive(Default)]
struct PgrLineTracks<'a> {
    position: Option<&'a CanonicalTrack>,
    rotation: Option<&'a CanonicalTrack>,
    alpha: Option<&'a CanonicalTrack>,
    speed: Option<&'a CanonicalTrack>,
}

fn pgr_line_tracks<'a>(
    chart: &'a CanonicalChart,
    owner: u64,
    negotiation: &NegotiationPlan,
) -> Result<PgrLineTracks<'a>, ExportError> {
    let mut found = PgrLineTracks::default();
    for track in chart
        .tracks()
        .tracks()
        .iter()
        .filter(|track| track.owner().value() == owner)
    {
        let slot = match (track.name(), track.target()) {
            ("pgr.position", CanonicalTrackTarget::Position) => &mut found.position,
            ("pgr.rotation", CanonicalTrackTarget::Rotation) => &mut found.rotation,
            ("pgr.alpha", CanonicalTrackTarget::Alpha) => &mut found.alpha,
            ("pgr.speed", CanonicalTrackTarget::ScrollSpeed) => &mut found.speed,
            _ if negotiation.drops(ConversionDomain::Motion) => continue,
            _ => {
                return Err(ExportError::new(
                    "conversion.capability-mismatch",
                    format!(
                        "Track {} is not representable by the PGR target",
                        track.name()
                    ),
                ));
            }
        };
        if slot.replace(track).is_some() {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!("duplicate PGR Track slot for {}", track.name()),
            ));
        }
    }
    Ok(found)
}

fn require_pgr_chart_shape(
    chart: &CanonicalChart,
    options: &ExportOptions,
    negotiation: &NegotiationPlan,
) -> Result<(), ExportError> {
    require_external_payload_losses(chart, negotiation, "PGR")?;
    let floor_scale = options.floor_scale_px.to_f64().map_err(|error| {
        ExportError::new("conversion.profile-parameter-invalid", error.to_string())
    })?;
    for line in chart.lines().lines() {
        let base = line.base();
        if !negotiation.drops(ConversionDomain::Motion)
            && (line.parent().is_some()
                || line.inherit() != &CanonicalLineInherit::default()
                || base.position().x() != 0.0
                || base.position().y() != 0.0
                || base.rotation() != 0.0
                || base.scale().x() != 1.0
                || base.scale().y() != 1.0
                || base.alpha() != 1.0
                || base.transform_origin().x() != 0.0
                || base.transform_origin().y() != 0.0
                || base.texture_anchor().x() != 0.5
                || base.texture_anchor().y() != 0.5
                || base.floor_scale() != floor_scale
                || base.integration_origin() != 0.0
                || base.initial_floor_position() != 0.0
                || base.allow_reverse_scroll()
                || base.z_order() != 0)
        {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!(
                    "Line {} has a base or parent not representable by PGR",
                    line.document_order()
                ),
            ));
        }
        let tracks = pgr_line_tracks(chart, line.id().value(), negotiation)?;
        if tracks.speed.is_none() {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!("Line {} requires a PGR speed Track", line.document_order()),
            ));
        }
    }
    for note in chart.notes().notes() {
        let gameplay = note.gameplay();
        let presentation = note.presentation();
        let gameplay_unsupported = !gameplay.judgment_enabled()
            || gameplay.judge_shape() != &CanonicalJudgeShape::LineDefault
            || gameplay.sound_policy() != &CanonicalNoteSoundPolicy::Default
            || gameplay.score_policy() != &CanonicalNoteScorePolicy::Default;
        let presentation_unsupported = presentation.x_offset() != 0.0
            || presentation.y_offset() != 0.0
            || presentation.alpha() != 1.0
            || presentation.scale_x() != 1.0
            || presentation.scale_y() != 1.0
            || presentation.rotation() != 0.0
            || presentation.color() != CanonicalColor::rgba(255, 255, 255, 255)
            || presentation.texture().is_some()
            || !presentation.render_enabled()
            || presentation.visible_from().is_some()
            || presentation.visible_until().is_some()
            || presentation.scroll_factor() < 0.0;
        if (gameplay_unsupported && !negotiation.drops(ConversionDomain::Gameplay))
            || (presentation_unsupported
                && !negotiation.drops(ConversionDomain::Presentation)
                && !negotiation.approximates(ConversionDomain::Presentation))
        {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!(
                    "Note {} has gameplay or presentation fields not representable by PGR",
                    note.document_order()
                ),
            ));
        }
    }
    Ok(())
}

fn pgr_track_events(
    track: Option<&CanonicalTrack>,
    bpm: f64,
    profile: PgrProfile,
) -> Result<Vec<Value>, ExportError> {
    let Some(track) = track else {
        return Ok(Vec::new());
    };
    let expected_fills = if track.target() == CanonicalTrackTarget::ScrollSpeed {
        (
            CanonicalTrackFill::Error,
            CanonicalTrackFill::HoldBefore,
            CanonicalTrackFill::HoldAfter,
        )
    } else {
        (
            CanonicalTrackFill::HoldAfter,
            CanonicalTrackFill::Base,
            CanonicalTrackFill::HoldAfter,
        )
    };
    if track.blend() != CanonicalTrackBlend::Replace
        || track.priority() != 0
        || (
            track.fill(),
            track.extrapolate_before(),
            track.extrapolate_after(),
        ) != expected_fills
    {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("Track {} has unsupported blend/fill behavior", track.name()),
        ));
    }
    let mut events = Vec::with_capacity(track.pieces().len());
    let mut previous_end = 0.0;
    let mut floor_position = 0.0;
    for piece in track.pieces() {
        let (start, end, start_value, end_value, interpolation) = match piece {
            CanonicalTrackPiece::Segment(segment) => (
                segment.start().chart_time_seconds(),
                segment.end().chart_time_seconds(),
                segment.start_value(),
                segment.end_value(),
                Some(segment.interpolation()),
            ),
            CanonicalTrackPiece::Point(point) => (
                point.time().chart_time_seconds(),
                point.time().chart_time_seconds(),
                point.value(),
                point.value(),
                None,
            ),
        };
        if start != previous_end {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!(
                    "Track {} is not contiguous from chart time zero",
                    track.name()
                ),
            ));
        }
        let start_t = chart_time_to_pgr_t(start, bpm);
        let end_t = chart_time_to_pgr_t(end, bpm);
        let event = match track.target() {
            CanonicalTrackTarget::Position => {
                if interpolation.is_some_and(|value| value != &CanonicalTrackInterpolation::Linear)
                {
                    return Err(ExportError::new(
                        "conversion.capability-mismatch",
                        "PGR position Track requires linear segments",
                    ));
                }
                let (
                    CanonicalTrackValue::Vec2Length(start_value),
                    CanonicalTrackValue::Vec2Length(end_value),
                ) = (start_value, end_value)
                else {
                    unreachable!("canonical Track target validates value types")
                };
                pgr_move_event(start_t, end_t, start_value, end_value, profile)?
            }
            CanonicalTrackTarget::Rotation => {
                if interpolation.is_some_and(|value| value != &CanonicalTrackInterpolation::Linear)
                {
                    return Err(ExportError::new(
                        "conversion.capability-mismatch",
                        "PGR rotation Track requires linear segments",
                    ));
                }
                let (
                    CanonicalTrackValue::Angle(start_value),
                    CanonicalTrackValue::Angle(end_value),
                ) = (start_value, end_value)
                else {
                    unreachable!("canonical Track target validates value types")
                };
                json!({
                    "startTime": start_t,
                    "endTime": end_t,
                    "start": -start_value * 180.0 / std::f64::consts::PI,
                    "end": -end_value * 180.0 / std::f64::consts::PI
                })
            }
            CanonicalTrackTarget::Alpha => {
                if interpolation.is_some_and(|value| value != &CanonicalTrackInterpolation::Linear)
                {
                    return Err(ExportError::new(
                        "conversion.capability-mismatch",
                        "PGR alpha Track requires linear segments",
                    ));
                }
                let (
                    CanonicalTrackValue::Float(start_value),
                    CanonicalTrackValue::Float(end_value),
                ) = (start_value, end_value)
                else {
                    unreachable!("canonical Track target validates value types")
                };
                json!({
                    "startTime": start_t,
                    "endTime": end_t,
                    "start": start_value,
                    "end": end_value
                })
            }
            CanonicalTrackTarget::ScrollSpeed => {
                let (
                    CanonicalTrackValue::Float(start_value),
                    CanonicalTrackValue::Float(end_value),
                ) = (start_value, end_value)
                else {
                    unreachable!("canonical Track target validates value types")
                };
                if start_value != end_value
                    || interpolation != Some(&CanonicalTrackInterpolation::Step)
                    || end <= start
                    || start_value < 0.0
                {
                    return Err(ExportError::new(
                        "conversion.capability-mismatch",
                        "PGR speed Track requires positive-duration constant Step segments",
                    ));
                }
                let value = json!({
                    "startTime": start_t,
                    "endTime": end_t,
                    "value": start_value,
                    "floorPosition": floor_position
                });
                floor_position += (end - start) * start_value;
                value
            }
            _ => {
                return Err(ExportError::new(
                    "conversion.capability-mismatch",
                    format!("Track {} target is not representable by PGR", track.name()),
                ));
            }
        };
        events.push(event);
        previous_end = end;
    }
    Ok(events)
}

fn pgr_move_event(
    start_t: f64,
    end_t: f64,
    start: fcs_model::CanonicalVec2,
    end: fcs_model::CanonicalVec2,
    profile: PgrProfile,
) -> Result<Value, ExportError> {
    if profile == PgrProfile::PhiraV3 {
        let [start_x, start_y, end_x, end_y] = [
            start.x() / 1920.0 + 0.5,
            start.y() / 1080.0 + 0.5,
            end.x() / 1920.0 + 0.5,
            end.y() / 1080.0 + 0.5,
        ];
        if [start_x, start_y, end_x, end_y]
            .into_iter()
            .any(|value| !(0.0..=1.0).contains(&value))
        {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                "PGR v3 move coordinate lies outside the normalized canvas",
            ));
        }
        Ok(json!({
            "startTime": start_t,
            "endTime": end_t,
            "start": start_x,
            "end": end_x,
            "start2": start_y,
            "end2": end_y
        }))
    } else {
        Ok(json!({
            "startTime": start_t,
            "endTime": end_t,
            "start": pgr_v1_packed(start)?,
            "end": pgr_v1_packed(end)?
        }))
    }
}

fn pgr_v1_packed(value: fcs_model::CanonicalVec2) -> Result<f64, ExportError> {
    let x = (value.x() / 1920.0 + 0.5) * 880.0;
    let y = (value.y() / 1080.0 + 0.5) * 520.0;
    let x_integer = x.round();
    if !(0.0..=880.0).contains(&x_integer)
        || !(0.0..=520.0).contains(&y)
        || (x - x_integer).abs() > f64::EPSILON * 8.0
    {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            "PGR v1 move coordinate is outside or not exactly representable on its packed canvas",
        ));
    }
    Ok(x_integer * 1000.0 + y)
}

fn pgr_floor_position(track: &CanonicalTrack, time: f64) -> Result<f64, ExportError> {
    let mut distance = 0.0;
    for piece in track.pieces() {
        let CanonicalTrackPiece::Segment(segment) = piece else {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                "PGR speed Track cannot contain points",
            ));
        };
        let CanonicalTrackValue::Float(value) = segment.start_value() else {
            unreachable!("canonical Track target validates value types")
        };
        let start = segment.start().chart_time_seconds();
        let end = segment.end().chart_time_seconds();
        if time <= end {
            if time < start {
                break;
            }
            return Ok(distance + (time - start) * value);
        }
        distance += (end - start) * value;
    }
    Err(ExportError::new(
        "conversion.capability-mismatch",
        "PGR speed Track does not cover a Note endpoint",
    ))
}

/// Options which make target semantics explicit at the exporter boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportOptions {
    pub policy: ConversionPolicy,
    pub repair_mode: RepairMode,
    pub target_profile: Option<String>,
    pub rpe_profile_binding: Option<RpeProfileBinding>,
    pub capabilities: CapabilityDescriptor,
    pub floor_scale_px: ExactDecimal,
    pub approximation: ApproximationAuthorization,
    pub drop: DropAuthorization,
}

impl ExportOptions {
    pub fn semantic(capabilities: CapabilityDescriptor) -> Self {
        Self {
            policy: ConversionPolicy::Semantic,
            repair_mode: RepairMode::disabled(),
            target_profile: capabilities.profile().map(str::to_owned),
            rpe_profile_binding: None,
            capabilities,
            floor_scale_px: ExactDecimal::parse("120", DecimalLimits::default())
                .expect("static exporter floor scale"),
            approximation: ApproximationAuthorization::disabled(),
            drop: DropAuthorization::disabled(),
        }
    }

    pub fn strict(capabilities: CapabilityDescriptor) -> Self {
        Self {
            policy: ConversionPolicy::Strict,
            repair_mode: RepairMode::disabled(),
            target_profile: None,
            rpe_profile_binding: None,
            capabilities,
            floor_scale_px: ExactDecimal::parse("120", DecimalLimits::default())
                .expect("static exporter floor scale"),
            approximation: ApproximationAuthorization::disabled(),
            drop: DropAuthorization::disabled(),
        }
    }

    pub fn with_target_profile(mut self, profile: impl Into<String>) -> Self {
        self.target_profile = Some(profile.into());
        self
    }

    /// Bind all target-specific RPE parameters and select the same profile ID.
    pub fn with_rpe_profile_binding(mut self, binding: RpeProfileBinding) -> Self {
        let profile = binding.profile();
        self.target_profile = Some(profile_reference(profile.id(), profile.version()));
        self.rpe_profile_binding = Some(binding);
        self
    }

    pub fn with_repair_mode(mut self, repair_mode: RepairMode) -> Self {
        self.repair_mode = repair_mode;
        self
    }

    pub fn with_approximation(mut self, authorization: ApproximationAuthorization) -> Self {
        self.approximation = authorization;
        self
    }

    pub fn with_drop(mut self, authorization: DropAuthorization) -> Self {
        self.drop = authorization;
        self
    }

    pub fn with_floor_scale_px(mut self, floor_scale_px: ExactDecimal) -> Self {
        self.floor_scale_px = floor_scale_px;
        self
    }
}

/// Successful target bytes with the decisions and proof that authorized them.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportOutcome {
    bytes: Vec<u8>,
    negotiation: NegotiationPlan,
    comparison: crate::CanonicalComparison,
    report: ConversionReport,
    distribution: Option<fcs_model::DistributionMetadata>,
}

impl ExportOutcome {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub const fn negotiation(&self) -> &NegotiationPlan {
        &self.negotiation
    }

    pub const fn comparison(&self) -> &crate::CanonicalComparison {
        &self.comparison
    }

    pub const fn report(&self) -> &ConversionReport {
        &self.report
    }

    /// Source-free metadata for a compilation export, including any structured
    /// preserve facts that must be written through FCBC Fidelity.
    pub fn distribution(&self) -> Option<&fcs_model::DistributionMetadata> {
        self.distribution.as_ref()
    }
}

/// One domain decision made before target bytes are written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiationEntry {
    domain: ConversionDomain,
    action: NegotiationAction,
    category: &'static str,
    status: SemanticStatus,
}

impl NegotiationEntry {
    pub const fn domain(&self) -> ConversionDomain {
        self.domain
    }

    pub const fn action(&self) -> NegotiationAction {
        self.action
    }

    pub const fn category(&self) -> &'static str {
        self.category
    }

    pub const fn semantic_status(&self) -> SemanticStatus {
        self.status
    }
}

/// Deterministic capability negotiation result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiationPlan {
    entries: Vec<NegotiationEntry>,
}

impl NegotiationPlan {
    pub fn entries(&self) -> &[NegotiationEntry] {
        &self.entries
    }

    pub fn action(&self) -> NegotiationAction {
        self.entries
            .iter()
            .map(|entry| entry.action)
            .max_by_key(|action| action.rank())
            .unwrap_or(NegotiationAction::Direct)
    }

    pub fn action_for(&self, domain: ConversionDomain) -> Option<NegotiationAction> {
        self.entries
            .iter()
            .find(|entry| entry.domain == domain)
            .map(|entry| entry.action)
    }

    pub fn drops(&self, domain: ConversionDomain) -> bool {
        self.action_for(domain) == Some(NegotiationAction::Drop)
    }

    pub fn preserves(&self, domain: ConversionDomain) -> bool {
        self.action_for(domain) == Some(NegotiationAction::Preserve)
    }

    fn omits(&self, domain: ConversionDomain) -> bool {
        matches!(
            self.action_for(domain),
            Some(NegotiationAction::Drop | NegotiationAction::Preserve)
        )
    }

    pub fn approximates(&self, domain: ConversionDomain) -> bool {
        self.action_for(domain) == Some(NegotiationAction::Bake)
    }

    fn has_unsupported(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.action == NegotiationAction::Unsupported)
    }
}

// ponytail: fixed 1e-3 grid; add adaptive segments when tighter budgets need it.
const LINEAR_SEGMENT_QUANTUM: f64 = 0.001;

fn baked_presentation_value(value: f64, negotiation: &NegotiationPlan) -> f64 {
    if negotiation.approximates(ConversionDomain::Presentation) {
        (value / LINEAR_SEGMENT_QUANTUM).round() * LINEAR_SEGMENT_QUANTUM
    } else {
        value
    }
}

impl ExportError {
    pub fn new(category: &'static str, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
            entries: Vec::new(),
            report: None,
        }
    }

    fn with_entries(mut self, entries: Vec<ConversionEntry>) -> Self {
        let mut entries = entries;
        entries.truncate(MAX_REPORT_ENTRIES);
        self.entries = entries;
        self
    }

    fn with_failed_report(
        mut self,
        format: &str,
        options: &ExportOptions,
        attempted_bytes: &[u8],
    ) -> Result<Self, ReportError> {
        let attempted_hash = lower_hex(Sha256::digest(attempted_bytes));
        let report = ConversionReport::new_with_authorizations(
            format!("{format}-export-failed-{attempted_hash}"),
            options.policy,
            options.repair_mode.clone(),
            options
                .approximation
                .enabled()
                .then(|| options.approximation.clone()),
            options.drop.enabled().then(|| options.drop.clone()),
            self.entries,
            Vec::new(),
            [ConversionStatus::Failed],
            None,
        )?;
        self.entries = report.entries().to_vec();
        self.report = Some(Box::new(report));
        Ok(self)
    }

    pub const fn category(&self) -> &'static str {
        self.category
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn entries(&self) -> &[ConversionEntry] {
        &self.entries
    }

    pub fn report(&self) -> Option<&ConversionReport> {
        self.report.as_deref()
    }
}

impl fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.category, self.message)
    }
}

impl std::error::Error for ExportError {}

fn report_limit_error(observed: usize, entries: Vec<ConversionEntry>) -> ExportError {
    ExportError::new(
        "conversion.report-limit",
        format!(
            "conversion report entry limit exceeded: maximum {MAX_REPORT_ENTRIES}, observed {observed}"
        ),
    )
    .with_entries(entries)
}

fn push_report_entry(
    entries: &mut Vec<ConversionEntry>,
    entry: ConversionEntry,
) -> Result<(), ExportError> {
    if entries.len() >= MAX_REPORT_ENTRIES {
        let observed = entries.len().saturating_add(1);
        let mut retained = std::mem::take(entries);
        retained.truncate(MAX_REPORT_ENTRIES);
        return Err(report_limit_error(observed, retained));
    }
    entries.push(entry);
    Ok(())
}

/// Validate FCS source and apply its deterministic token-layout policy.
pub fn format_fcs_source(source: &str) -> Result<String, ExportError> {
    let formatted = fcs_source::parser::canonicalize_source_layout(source)
        .into_result()
        .map_err(|diagnostics| {
            let message = diagnostics
                .first()
                .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
                .unwrap_or_else(|| "formatted source invalid".into());
            ExportError::new("source.invalid", message)
        })?;
    fcs_source::parser::parse_document(&formatted)
        .into_result()
        .map_err(|diagnostics| {
            let message = diagnostics
                .first()
                .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
                .unwrap_or_else(|| "formatted source invalid".into());
            ExportError::new("source.invalid", message)
        })?;
    Ok(formatted)
}

/// Export with the explicit Phira v3 compatibility binding.
pub fn export_pgr_v3(chart: &CanonicalChart) -> Result<Vec<u8>, ExportError> {
    let profile = PgrProfile::PhiraV3;
    let options = ExportOptions::semantic(
        CapabilitySet::pgr_v3()
            .descriptor(Some(profile_reference(profile.id(), profile.version()))),
    );
    Ok(export_pgr_v3_with_options(chart, &options)?.into_bytes())
}

/// Export PGR v3, re-import it with the same target profile, and prove the
/// canonical result before returning bytes.
pub fn export_pgr_v3_with_options(
    chart: &CanonicalChart,
    options: &ExportOptions,
) -> Result<ExportOutcome, ExportError> {
    let profile = selected_pgr_profile(options)?;
    if profile != PgrProfile::PhiraV3 {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            "export_pgr_v3_with_options requires pgr.phira.v3@1.0.0",
        ));
    }
    export_pgr_with_options(chart, options)
}

/// Export a complete canonical product. External chart-only targets must
/// explicitly negotiate any resource/package loss before this wrapper succeeds.
pub fn export_pgr_compilation_with_options(
    compilation: &CanonicalCompilation,
    options: &ExportOptions,
) -> Result<ExportOutcome, ExportError> {
    validate_compilation_resource_closure(compilation)?;
    let outcome = export_pgr_with_resource_context(
        compilation.chart(),
        Some(compilation.resources()),
        options,
        true,
    )?;
    record_compilation_roundtrip_context(outcome, compilation, options)
}

/// Export PGR v1 or v3 according to the explicit target profile.
pub fn export_pgr_with_options(
    chart: &CanonicalChart,
    options: &ExportOptions,
) -> Result<ExportOutcome, ExportError> {
    export_pgr_with_resource_context(chart, None, options, false)
}

fn export_pgr_with_resource_context(
    chart: &CanonicalChart,
    expected_resources: Option<&CanonicalResourceBundle>,
    options: &ExportOptions,
    fidelity_sink: bool,
) -> Result<ExportOutcome, ExportError> {
    let profile = selected_pgr_profile(options)?;
    if !profile.strict_eligible() {
        return Err(ExportError::new(
            "conversion.profile-not-applicable",
            "the selected PGR profile is source-only and cannot be an export target",
        ));
    }
    let (negotiation, entries) =
        negotiate_export_with_fidelity_sink(chart, options, fidelity_sink)?;
    require_pgr_chart_shape(chart, options, &negotiation)?;
    let bpm = single_global_bpm(chart, "PGR")?;
    let offset = chart
        .metadata()
        .sync()
        .map(|sync| sync.audio_offset().seconds())
        .unwrap_or(0.0);
    let mut lines = Vec::new();
    let mut ordered_lines: Vec<_> = chart.lines().lines().collect();
    ordered_lines.sort_by_key(|line| line.document_order());
    let mut target_ids = StableIdRegistry::new();
    let mut line_alignment = Vec::with_capacity(ordered_lines.len());
    let mut note_alignment = Vec::with_capacity(chart.notes().notes().len());
    let mut note_order = 0u64;
    for (line_index, line) in ordered_lines.into_iter().enumerate() {
        line_alignment.push((
            line.id().clone(),
            generated_target_id(
                &mut target_ids,
                EntityKind::Line,
                "pgrLines",
                line_index as u64,
            )?,
        ));
        let line_id = line.id().value();
        let tracks = pgr_line_tracks(chart, line_id, &negotiation)?;
        let speed = tracks.speed.ok_or_else(|| {
            ExportError::new(
                "conversion.capability-mismatch",
                format!(
                    "Line {} has no representable PGR speed Track",
                    line.document_order()
                ),
            )
        })?;
        let mut notes_above = Vec::new();
        let mut notes_below = Vec::new();
        let mut note_ids_above = Vec::new();
        let mut note_ids_below = Vec::new();
        let mut notes: Vec<_> = chart
            .notes()
            .notes()
            .iter()
            .filter(|note| note.gameplay().line().value() == line_id)
            .collect();
        notes.sort_by_key(|note| note.document_order());
        for note in notes {
            let start_seconds = note.gameplay().time().chart_time_seconds();
            let time_t = chart_time_to_pgr_t(start_seconds, bpm);
            let hold_time = note
                .gameplay()
                .end_time()
                .map(|end| chart_time_to_pgr_t(end.chart_time_seconds(), bpm) - time_t)
                .unwrap_or(0.0)
                .max(0.0);
            let position_x = if negotiation.drops(ConversionDomain::Presentation) {
                0.0
            } else {
                baked_presentation_value(note.presentation().position_x(), &negotiation) / 108.0
            };
            let floor_position = pgr_floor_position(speed, start_seconds)?;
            let note_type = match note.kind() {
                CanonicalNoteKind::Tap => 1,
                CanonicalNoteKind::Drag => 2,
                CanonicalNoteKind::Hold => 3,
                CanonicalNoteKind::Flick => 4,
            };
            let payload = json!({
                "type": note_type,
                "time": time_t,
                "holdTime": hold_time,
                "positionX": position_x,
                "speed": if negotiation.drops(ConversionDomain::Presentation) {
                    1.0
                } else {
                    baked_presentation_value(note.presentation().scroll_factor(), &negotiation)
                },
                "floorPosition": floor_position
            });
            match note.gameplay().side() {
                CanonicalNoteSide::Above => {
                    notes_above.push(payload);
                    note_ids_above.push(note.id().clone());
                }
                CanonicalNoteSide::Below => {
                    notes_below.push(payload);
                    note_ids_below.push(note.id().clone());
                }
            }
        }
        for note_id in note_ids_above.into_iter().chain(note_ids_below) {
            note_alignment.push((
                note_id,
                generated_target_id(&mut target_ids, EntityKind::Note, "pgrNotes", note_order)?,
            ));
            note_order = note_order.saturating_add(1);
        }
        let (move_events, rotate_events, alpha_events) =
            if negotiation.drops(ConversionDomain::Motion) {
                (Vec::new(), Vec::new(), Vec::new())
            } else {
                (
                    pgr_track_events(tracks.position, bpm, profile)?,
                    pgr_track_events(tracks.rotation, bpm, profile)?,
                    pgr_track_events(tracks.alpha, bpm, profile)?,
                )
            };
        let speed_events = pgr_track_events(Some(speed), bpm, profile)?;
        lines.push(json!({
            "bpm": bpm,
            "judgeLineMoveEvents": move_events,
            "judgeLineRotateEvents": rotate_events,
            "judgeLineDisappearEvents": alpha_events,
            "speedEvents": speed_events,
            "notesAbove": notes_above,
            "notesBelow": notes_below
        }));
    }
    if lines.is_empty() {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            "PGR requires at least one canonical Line with a speed Track",
        ));
    }
    let format_version = if profile == PgrProfile::PhiraV1 { 1 } else { 3 };
    let root = json!({
        "formatVersion": format_version,
        "offset": offset,
        "judgeLineList": lines
    });
    let bytes = serde_json::to_vec_pretty(&root).map_err(|error| {
        ExportError::new(
            "conversion.internal",
            format!("failed to serialize PGR JSON: {error}"),
        )
    })?;
    let artifact = SourceArtifact::new("export.pgr.json", ArtifactRole::Chart, bytes.clone())
        .map_err(|error| ExportError::new("conversion.source-invalid", error.to_string()))?;
    let parsed = parse_json_document(SourceFormat::Pgr, &artifact)
        .map_err(|error| ExportError::new("conversion.source-invalid", error.to_string()))?;
    let source = parse_pgr_document(&parsed, PgrLimits::default())
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let binding = PgrProfileBinding::new(profile, options.floor_scale_px.clone())
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let semantic = interpret_pgr(&source, &binding)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let reparsed = lower_pgr_to_canonical(&semantic, &artifact)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let alignment = entity_alignment(line_alignment, note_alignment)?;
    finish_export(
        "pgr",
        chart,
        reparsed.compilation().chart(),
        expected_resources,
        reparsed.compilation().resources(),
        &alignment,
        options,
        negotiation,
        entries,
        bytes,
    )
}

/// Import → export → same-profile re-import a PGR chart and return the full
/// canonical semantic comparison rather than topology counts.
pub fn roundtrip_pgr_v3_public_bytes(
    bytes: &[u8],
) -> Result<crate::CanonicalComparison, ExportError> {
    let artifact = SourceArtifact::new("chart.json", ArtifactRole::Chart, bytes.to_vec())
        .map_err(|error| ExportError::new("conversion.source-invalid", error.to_string()))?;
    let parsed = parse_json_document(SourceFormat::Pgr, &artifact)
        .map_err(|error| ExportError::new("conversion.source-invalid", error.to_string()))?;
    let source = parse_pgr_document(&parsed, PgrLimits::default())
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let floor = ExactDecimal::parse("120", DecimalLimits::default()).map_err(|error| {
        ExportError::new("conversion.profile-parameter-invalid", error.to_string())
    })?;
    let binding = PgrProfileBinding::new(PgrProfile::PhiraV3, floor)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let semantic = interpret_pgr(&source, &binding)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let first = lower_pgr_to_canonical(&semantic, &artifact)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let profile = PgrProfile::PhiraV3;
    let options = ExportOptions::semantic(
        CapabilitySet::pgr_v3()
            .descriptor(Some(profile_reference(profile.id(), profile.version()))),
    );
    let outcome = export_pgr_v3_with_options(first.compilation().chart(), &options)?;
    Ok(outcome.comparison().clone())
}

fn chart_time_to_pgr_t(chart_time_seconds: f64, bpm: f64) -> f64 {
    // Inverse of importer: chart_time = T * 60 / (32 * bpm)
    chart_time_seconds * 32.0 * bpm / 60.0
}

/// Negotiation outcome before writing a target format (I8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegotiationAction {
    Direct,
    Equivalent,
    Bake,
    Preserve,
    Drop,
    Unsupported,
}

impl NegotiationAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Equivalent => "equivalent",
            Self::Bake => "bake",
            Self::Preserve => "preserve",
            Self::Drop => "drop",
            Self::Unsupported => "unsupported",
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::Direct => 0,
            Self::Equivalent => 1,
            Self::Preserve => 2,
            Self::Bake => 3,
            Self::Drop => 4,
            Self::Unsupported => 5,
        }
    }
}

/// Compatibility capability surface retained for existing callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySet {
    pub format: &'static str,
    pub version: &'static str,
    pub time: bool,
    pub notes: bool,
    pub tracks: bool,
    pub expressions: bool,
    pub resources: bool,
}

impl CapabilitySet {
    pub const fn pgr_v1() -> Self {
        Self {
            format: "pgr",
            version: "1",
            time: true,
            notes: true,
            tracks: true,
            expressions: false,
            resources: false,
        }
    }

    pub const fn pgr_v3() -> Self {
        Self {
            format: "pgr",
            version: "3",
            time: true,
            notes: true,
            tracks: true,
            expressions: false,
            resources: false,
        }
    }

    pub const fn rpe_json() -> Self {
        Self {
            format: "rpe",
            version: "json",
            time: true,
            notes: true,
            tracks: false,
            expressions: false,
            resources: false,
        }
    }

    pub const fn pec_line() -> Self {
        Self {
            format: "pec",
            version: "line-command",
            time: true,
            notes: true,
            tracks: true,
            expressions: false,
            resources: false,
        }
    }

    pub fn descriptor(&self, profile: Option<String>) -> CapabilityDescriptor {
        let exact = |domain, supported, features: Vec<CapabilityFeature>| {
            CapabilityDomainDescriptor::new(
                domain, supported, false, false, false, false, None, None,
            )
            .with_features(features)
            .expect("static compatibility capability features")
        };
        let line_motion = self.format == "rpe" || self.format == "pec";
        CapabilityDescriptor::new(
            self.format,
            self.version,
            profile,
            vec![
                exact(
                    ConversionDomain::Timing,
                    self.time,
                    self.features(ConversionDomain::Timing),
                ),
                exact(
                    ConversionDomain::Gameplay,
                    self.notes,
                    self.features(ConversionDomain::Gameplay),
                ),
                exact(
                    ConversionDomain::Motion,
                    self.tracks || line_motion,
                    self.features(ConversionDomain::Motion),
                ),
                exact(
                    ConversionDomain::Scroll,
                    self.time,
                    self.features(ConversionDomain::Scroll),
                ),
                exact(
                    ConversionDomain::Presentation,
                    self.notes,
                    self.features(ConversionDomain::Presentation),
                ),
                exact(
                    ConversionDomain::Resource,
                    self.resources,
                    self.features(ConversionDomain::Resource),
                ),
                exact(
                    ConversionDomain::Metadata,
                    false,
                    self.features(ConversionDomain::Metadata),
                ),
                exact(
                    ConversionDomain::Syntax,
                    true,
                    self.features(ConversionDomain::Syntax),
                ),
                exact(
                    ConversionDomain::Profile,
                    true,
                    self.features(ConversionDomain::Profile),
                ),
                exact(
                    ConversionDomain::Package,
                    self.resources,
                    self.features(ConversionDomain::Package),
                ),
            ],
        )
        .expect("static compatibility capability descriptor")
    }

    fn features(&self, domain: ConversionDomain) -> Vec<CapabilityFeature> {
        let mut features = Vec::new();
        let mut add = |axis: &str, value: &str| {
            features.push(capability_feature(axis, value));
        };
        match domain {
            ConversionDomain::Timing if self.time => {
                add("time.domain", "chartTime");
                add("time.exactness", "exact");
                add("time.precision", "binary64");
            }
            ConversionDomain::Gameplay if self.notes => {
                for value in ["tap", "hold", "flick", "drag"] {
                    add("note.kind", value);
                }
                add("note.judge-shape", "line-default");
                add("note.judgment", "enabled");
                add("note.sound", "default");
                add("note.score", "default");
                if self.format != "pgr" {
                    add("note.judgment", "disabled");
                    add("note.sound", "none");
                    add("note.score", "none");
                }
                add("note.hold-geometry", "canonical");
            }
            ConversionDomain::Motion => {
                if self.format == "rpe" {
                    add("line.parent", "linked");
                    add("line.inherit", "rpe-compatible");
                    add("line.transform", "custom");
                    for value in ["position", "rotation", "alpha", "scroll-speed"] {
                        add("track.target", value);
                    }
                    for value in ["linear", "point", "easing", "cubic-bezier"] {
                        add("track.interpolation", value);
                    }
                    add("track.blend", "add");
                    add("track.fill", "zero");
                }
                if self.format != "rpe" {
                    add("line.parent", "none");
                    add("line.inherit", "default");
                }
                add("line.transform", "default");
                if self.format == "pec" {
                    for value in ["position", "rotation", "alpha", "scroll-speed"] {
                        add("track.target", value);
                    }
                    for value in ["linear", "point", "easing"] {
                        add("track.interpolation", value);
                    }
                    add("track.blend", "replace");
                    add("track.fill", "base");
                } else if self.tracks {
                    for value in ["position", "rotation", "alpha", "scroll-speed"] {
                        add("track.target", value);
                    }
                    for value in ["linear", "point", "step"] {
                        add("track.interpolation", value);
                    }
                    add("track.blend", "replace");
                    for value in ["base", "hold-before", "hold-after", "error"] {
                        add("track.fill", value);
                    }
                }
            }
            ConversionDomain::Scroll if self.time => {
                add("scroll.speed", "canonical");
                add("scroll.distance", "canonical");
                add("scroll.hold-geometry", "canonical");
            }
            ConversionDomain::Presentation if self.notes => {
                add("note.presentation", "default");
                add("note.position-x", "canonical");
                add("note.scroll-factor", "canonical");
            }
            ConversionDomain::Resource if self.resources => {
                add("resource.bytes", "raw");
            }
            ConversionDomain::Profile => {
                add("numeric.values", "finite-binary64");
                add("entity.identity", "stable");
                if self.expressions {
                    add("expression.descriptor", "typed");
                    add("runtime.extension", "declared");
                }
            }
            ConversionDomain::Package if self.resources => {
                add("package.resources", "contained");
            }
            _ => {}
        }
        features
    }
}

fn capability_feature(axis: &str, value: impl Into<String>) -> CapabilityFeature {
    CapabilityFeature::new(axis, value).expect("canonical capability feature")
}

fn required_capability_features(
    chart: &CanonicalChart,
    domain: ConversionDomain,
) -> Vec<CapabilityFeature> {
    let mut features = BTreeSet::new();
    let mut add = |axis: &str, value: &str| {
        features.insert(capability_feature(axis, value));
    };

    match domain {
        ConversionDomain::Timing => {
            add("time.domain", "chartTime");
            add("time.exactness", "exact");
            add("time.precision", "binary64");
        }
        ConversionDomain::Gameplay => {
            for note in chart.notes().notes() {
                add("note.kind", canonical_note_kind(note.kind()));
                let gameplay = note.gameplay();
                add(
                    "note.judgment",
                    if gameplay.judgment_enabled() {
                        "enabled"
                    } else {
                        "disabled"
                    },
                );
                add(
                    "note.judge-shape",
                    canonical_judge_shape(gameplay.judge_shape()),
                );
                add(
                    "note.sound",
                    canonical_sound_policy(gameplay.sound_policy()),
                );
                add(
                    "note.score",
                    canonical_score_policy(gameplay.score_policy()),
                );
                if note.kind() == CanonicalNoteKind::Hold {
                    add("note.hold-geometry", "canonical");
                }
            }
        }
        ConversionDomain::Motion => {
            for track in chart.tracks().tracks() {
                add("track.target", canonical_track_target(track.target()));
                let mut has_segment = false;
                for piece in track.pieces() {
                    if let CanonicalTrackPiece::Segment(segment) = piece {
                        has_segment = true;
                        add(
                            "track.interpolation",
                            canonical_track_interpolation(segment.interpolation()),
                        );
                    }
                }
                if !has_segment {
                    add("track.interpolation", "point");
                }
                add("track.blend", canonical_track_blend(track.blend()));
                add("track.fill", canonical_track_fill(track.fill()));
            }
            for line in chart.lines().lines() {
                if line.parent().is_some() {
                    add("line.parent", "linked");
                }
                if line.inherit() != &CanonicalLineInherit::default() {
                    let inherit = line.inherit();
                    add(
                        "line.inherit",
                        if inherit.position()
                            && inherit.scale()
                            && inherit.alpha()
                            && inherit.scroll()
                        {
                            "rpe-compatible"
                        } else {
                            "custom"
                        },
                    );
                }
                let base = line.base();
                if base.position().x() != 0.0
                    || base.position().y() != 0.0
                    || base.rotation() != 0.0
                    || base.scale().x() != 1.0
                    || base.scale().y() != 1.0
                    || base.alpha() != 1.0
                    || base.transform_origin().x() != 0.0
                    || base.transform_origin().y() != 0.0
                    || base.texture_anchor().x() != 0.5
                    || base.texture_anchor().y() != 0.5
                    || base.integration_origin() != 0.0
                    || base.initial_floor_position() != 0.0
                    || base.allow_reverse_scroll()
                    || base.z_order() != 0
                {
                    add("line.transform", "custom");
                }
            }
        }
        ConversionDomain::Scroll => {
            if !chart.scroll().lines().is_empty() {
                add("scroll.speed", "canonical");
                add("scroll.distance", "canonical");
            }
            if chart
                .scroll()
                .lines()
                .iter()
                .any(|line| line.allow_reverse_scroll())
            {
                add("scroll.reverse", "enabled");
            }
            if chart
                .notes()
                .notes()
                .iter()
                .any(|note| note.kind() == CanonicalNoteKind::Hold)
            {
                add("scroll.hold-geometry", "canonical");
            }
        }
        ConversionDomain::Presentation if !chart.notes().notes().is_empty() => {
            for note in chart.notes().notes() {
                add("note.presentation", note_presentation_mode(note));
                if note.presentation().position_x() != 0.0 {
                    add("note.position-x", "canonical");
                }
                if note.presentation().scroll_factor() != 1.0 {
                    add("note.scroll-factor", "canonical");
                }
            }
        }
        ConversionDomain::Resource if !chart.metadata().resources().is_empty() => {
            add("resource.bytes", "raw");
        }
        ConversionDomain::Package if !chart.metadata().resources().is_empty() => {
            add("package.resources", "contained");
        }
        ConversionDomain::Profile => {
            add("numeric.values", "finite-binary64");
            add("entity.identity", "stable");
            if chart.descriptors().is_some() {
                add("expression.descriptor", "typed");
            }
            for extension in chart.required_extensions() {
                features.insert(capability_feature(
                    "runtime.extension",
                    format!("{}@{}", extension.namespace(), extension.version()),
                ));
            }
        }
        _ => {}
    }
    features.into_iter().collect()
}

fn canonical_note_kind(kind: CanonicalNoteKind) -> &'static str {
    match kind {
        CanonicalNoteKind::Tap => "tap",
        CanonicalNoteKind::Hold => "hold",
        CanonicalNoteKind::Flick => "flick",
        CanonicalNoteKind::Drag => "drag",
    }
}

fn canonical_judge_shape(shape: &CanonicalJudgeShape) -> &'static str {
    match shape {
        CanonicalJudgeShape::LineDefault => "line-default",
        CanonicalJudgeShape::Rectangle { .. } => "rectangle",
        CanonicalJudgeShape::Circle { .. } => "circle",
    }
}

fn canonical_sound_policy(policy: &CanonicalNoteSoundPolicy) -> &'static str {
    match policy {
        CanonicalNoteSoundPolicy::Default => "default",
        CanonicalNoteSoundPolicy::None => "none",
        CanonicalNoteSoundPolicy::Resource(_) => "resource",
    }
}

fn canonical_score_policy(policy: &CanonicalNoteScorePolicy) -> &'static str {
    match policy {
        CanonicalNoteScorePolicy::Default => "default",
        CanonicalNoteScorePolicy::None => "none",
        CanonicalNoteScorePolicy::Custom(_) => "custom",
    }
}

fn note_presentation_mode(note: &fcs_model::CanonicalNote) -> &'static str {
    let presentation = note.presentation();
    if presentation.x_offset() != 0.0
        || presentation.y_offset() != 0.0
        || presentation.alpha() != 1.0
        || presentation.scale_x() != 1.0
        || presentation.scale_y() != 1.0
        || presentation.rotation() != 0.0
        || presentation.color() != CanonicalColor::rgba(255, 255, 255, 255)
        || presentation.texture().is_some()
        || !presentation.render_enabled()
        || presentation.visible_from().is_some()
        || presentation.visible_until().is_some()
    {
        "extended"
    } else {
        "default"
    }
}

fn canonical_track_target(target: CanonicalTrackTarget) -> &'static str {
    match target {
        CanonicalTrackTarget::Position => "position",
        CanonicalTrackTarget::Rotation => "rotation",
        CanonicalTrackTarget::Scale => "scale",
        CanonicalTrackTarget::Alpha => "alpha",
        CanonicalTrackTarget::ScrollSpeed => "scroll-speed",
    }
}

fn canonical_track_interpolation(interpolation: &CanonicalTrackInterpolation) -> &'static str {
    match interpolation {
        CanonicalTrackInterpolation::Step => "step",
        CanonicalTrackInterpolation::Linear => "linear",
        CanonicalTrackInterpolation::Easing(_) => "easing",
        CanonicalTrackInterpolation::CubicBezier(_) => "cubic-bezier",
    }
}

fn canonical_track_blend(blend: CanonicalTrackBlend) -> &'static str {
    match blend {
        CanonicalTrackBlend::Replace => "replace",
        CanonicalTrackBlend::Add => "add",
        CanonicalTrackBlend::Multiply => "multiply",
    }
}

fn canonical_track_fill(fill: CanonicalTrackFill) -> &'static str {
    match fill {
        CanonicalTrackFill::Base => "base",
        CanonicalTrackFill::Zero => "zero",
        CanonicalTrackFill::One => "one",
        CanonicalTrackFill::HoldBefore => "hold-before",
        CanonicalTrackFill::HoldAfter => "hold-after",
        CanonicalTrackFill::Error => "error",
    }
}

/// Build a deterministic per-domain plan and its report entries before writing.
pub fn negotiate_export_with_options(
    chart: &CanonicalChart,
    options: &ExportOptions,
) -> Result<(NegotiationPlan, Vec<ConversionEntry>), ExportError> {
    negotiate_export_with_fidelity_sink(chart, options, false)
}

fn negotiate_export_with_fidelity_sink(
    chart: &CanonicalChart,
    options: &ExportOptions,
    fidelity_sink: bool,
) -> Result<(NegotiationPlan, Vec<ConversionEntry>), ExportError> {
    if options.policy == ConversionPolicy::Strict
        && options
            .target_profile
            .as_deref()
            .is_none_or(|profile| profile.trim().is_empty())
    {
        return Err(ExportError::new(
            "conversion.target-profile-required",
            "strict export requires an explicit target semantic profile",
        ));
    }
    if let Some(bound) = options.capabilities.profile()
        && options.target_profile.as_deref() != Some(bound)
    {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!(
                "target profile {:?} does not match capability descriptor profile {bound}",
                options.target_profile
            ),
        ));
    }

    let mut entries = Vec::new();
    let mut plan = Vec::new();
    for domain in ConversionDomain::ALL {
        let descriptor = options.capabilities.domain(domain);
        let required_features = required_capability_features(chart, domain);
        let needed = match domain {
            ConversionDomain::Timing => true,
            ConversionDomain::Gameplay => !chart.notes().notes().is_empty(),
            ConversionDomain::Motion => {
                chart.lines().lines().next().is_some()
                    || !chart.tracks().tracks().is_empty()
                    || !required_features.is_empty()
            }
            ConversionDomain::Scroll => {
                !chart.scroll().lines().is_empty() || !required_features.is_empty()
            }
            ConversionDomain::Presentation => !chart.notes().notes().is_empty(),
            ConversionDomain::Resource => {
                !chart.metadata().resources().is_empty()
                    || chart.metadata().sync().is_some_and(|sync| {
                        sync.primary_audio().is_some() || sync.preview().is_some()
                    })
            }
            ConversionDomain::Metadata => {
                chart.metadata().meta().is_some()
                    || !chart.metadata().contributors().is_empty()
                    || !chart.metadata().credits().is_empty()
                    || chart.metadata().artwork().is_some()
            }
            ConversionDomain::Syntax | ConversionDomain::Profile => true,
            ConversionDomain::Package => {
                !chart.metadata().resources().is_empty()
                    || chart.metadata().sync().is_some_and(|sync| {
                        sync.primary_audio().is_some() || sync.preview().is_some()
                    })
            }
        };
        if !needed {
            continue;
        }
        let missing_features = descriptor
            .map(|descriptor| {
                required_features
                    .iter()
                    .filter(|feature| !descriptor.supports(feature.axis(), feature.value()))
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let limit_exceeded =
            descriptor.and_then(|descriptor| capability_limit_failure(descriptor, chart, domain));
        let requested_approximation_segments = descriptor
            .filter(|descriptor| {
                descriptor.approximation() && options.approximation.allows(domain.as_str())
            })
            .map_or(0, |_| approximation_segment_count(chart, domain));
        let action = match descriptor {
            _ if limit_exceeded.is_some() => NegotiationAction::Unsupported,
            Some(descriptor)
                if descriptor.approximation() && options.approximation.allows(domain.as_str()) =>
            {
                NegotiationAction::Bake
            }
            Some(descriptor)
                if descriptor.drop() && options.drop.allows_domain(domain.as_str()) =>
            {
                NegotiationAction::Drop
            }
            _ if !missing_features.is_empty() => NegotiationAction::Unsupported,
            Some(descriptor) if descriptor.exact() => NegotiationAction::Direct,
            Some(descriptor) if descriptor.equivalent() => NegotiationAction::Equivalent,
            Some(descriptor) if descriptor.preserve() => NegotiationAction::Preserve,
            _ => NegotiationAction::Unsupported,
        };
        let category = match action {
            NegotiationAction::Unsupported if limit_exceeded.is_some() => {
                "conversion.capability-mismatch"
            }
            NegotiationAction::Unsupported
                if descriptor.is_some_and(CapabilityDomainDescriptor::approximation) =>
            {
                "conversion.approximation-not-authorized"
            }
            NegotiationAction::Unsupported
                if descriptor.is_some_and(CapabilityDomainDescriptor::drop) =>
            {
                "conversion.drop-not-authorized"
            }
            NegotiationAction::Unsupported => "conversion.capability-mismatch",
            _ => "conversion.capability-negotiated",
        };
        let status = match action {
            NegotiationAction::Direct => SemanticStatus::Native,
            NegotiationAction::Equivalent => SemanticStatus::Equivalent,
            NegotiationAction::Bake => SemanticStatus::Approximated,
            NegotiationAction::Preserve => SemanticStatus::Preserved,
            NegotiationAction::Drop => SemanticStatus::Dropped,
            NegotiationAction::Unsupported => SemanticStatus::Unsupported,
        };
        plan.push(NegotiationEntry {
            domain,
            action,
            category,
            status,
        });
        push_report_entry(
            &mut entries,
            ConversionEntry::new(
                format!("capability/{}", domain.as_str()),
                category,
                domain,
                if action == NegotiationAction::Unsupported {
                    ConversionSeverity::Error
                } else if matches!(
                    action,
                    NegotiationAction::Preserve | NegotiationAction::Drop | NegotiationAction::Bake
                ) {
                    ConversionSeverity::Warning
                } else {
                    ConversionSeverity::Info
                },
                status,
                ConversionPhase::CapabilityNegotiation,
                None,
                None,
                None,
                Some(
                    missing_features
                        .first()
                        .map(ToString::to_string)
                        .or_else(|| limit_exceeded.clone())
                        .unwrap_or_else(|| domain.as_str().to_owned()),
                ),
                None,
                None,
                None,
                None,
                None,
                negotiation_message(
                    domain,
                    action,
                    options,
                    requested_approximation_segments,
                    &missing_features,
                    limit_exceeded.as_deref(),
                ),
                [],
            )
            .map_err(|error| ExportError::new("conversion.report-limit", error.to_string()))?,
        )?;
    }
    let plan = NegotiationPlan { entries: plan };
    if fidelity_sink
        && let Some(entry) = plan.entries.iter().find(|entry| {
            entry.action == NegotiationAction::Preserve
                && !matches!(
                    entry.domain,
                    ConversionDomain::Metadata
                        | ConversionDomain::Resource
                        | ConversionDomain::Package
                )
        })
    {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!(
                "{} preservation cannot omit canonical target data from the external writer",
                entry.domain
            ),
        )
        .with_entries(entries));
    }
    if !fidelity_sink
        && let Some(entry) = plan
            .entries
            .iter()
            .find(|entry| entry.action == NegotiationAction::Preserve)
    {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!(
                "{} preservation requires a structured Fidelity or external sidecar sink",
                entry.domain
            ),
        )
        .with_entries(entries));
    }
    if plan.has_unsupported() {
        let category = entries
            .iter()
            .find(|entry| entry.severity() == ConversionSeverity::Error)
            .map(ConversionEntry::category)
            .unwrap_or("conversion.capability-mismatch");
        return Err(ExportError::new(
            match category {
                "conversion.approximation-not-authorized" => {
                    "conversion.approximation-not-authorized"
                }
                "conversion.drop-not-authorized" => "conversion.drop-not-authorized",
                _ => "conversion.capability-mismatch",
            },
            plan.entries
                .iter()
                .filter(|entry| entry.action == NegotiationAction::Unsupported)
                .map(|entry| entry.domain.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        )
        .with_entries(entries));
    }
    if plan
        .entries
        .iter()
        .any(|entry| entry.action == NegotiationAction::Bake)
        && (options.approximation.algorithm_id() != "linear-segment"
            || options.approximation.algorithm_version() != "1.0.0")
    {
        return Err(ExportError::new(
            "conversion.approximation-not-authorized",
            "the exporter only implements linear-segment@1.0.0 baking",
        )
        .with_entries(entries));
    }
    if options.policy == ConversionPolicy::Strict
        && plan.entries.iter().any(|entry| {
            !matches!(
                entry.action,
                NegotiationAction::Direct | NegotiationAction::Equivalent
            )
        })
    {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            "strict export cannot preserve, approximate, or drop canonical semantics",
        )
        .with_entries(entries));
    }
    Ok((plan, entries))
}

fn capability_entity_count(chart: &CanonicalChart, domain: ConversionDomain) -> usize {
    match domain {
        ConversionDomain::Timing => chart.time_map().segments().count(),
        ConversionDomain::Gameplay | ConversionDomain::Presentation => chart.notes().notes().len(),
        ConversionDomain::Motion => chart.lines().lines().count() + chart.tracks().tracks().len(),
        ConversionDomain::Scroll => chart.scroll().lines().len(),
        ConversionDomain::Resource | ConversionDomain::Package => {
            chart.metadata().resources().len()
        }
        ConversionDomain::Metadata => {
            chart.metadata().meta().map_or(0, BTreeMap::len)
                + chart.metadata().contributors().len()
                + chart.metadata().credits().len()
                + usize::from(chart.metadata().artwork().is_some())
        }
        ConversionDomain::Profile => {
            chart
                .descriptors()
                .map_or(0, |table| table.descriptors().len())
                + chart.required_extensions().len()
        }
        ConversionDomain::Syntax => 0,
    }
}

fn capability_limit_failure(
    descriptor: &CapabilityDomainDescriptor,
    chart: &CanonicalChart,
    domain: ConversionDomain,
) -> Option<String> {
    for limit in descriptor.limits() {
        let count = match limit.name() {
            "entity.count" => Some(capability_entity_count(chart, domain)),
            "event.count" => Some(capability_event_count(chart, domain)),
            "resource.count" => Some(chart.metadata().resources().len()),
            "byte.count" => None,
            _ => return Some(format!("unsupported:{}", limit.name())),
        };
        if count.is_some_and(|count| (count as f64) > limit.maximum()) {
            return Some(limit.name().to_owned());
        }
    }
    if descriptor
        .max_entities()
        .is_some_and(|limit| capability_entity_count(chart, domain) > limit)
    {
        return Some("max_entities".into());
    }
    None
}

fn capability_event_count(chart: &CanonicalChart, domain: ConversionDomain) -> usize {
    match domain {
        ConversionDomain::Timing => chart.time_map().segments().count(),
        ConversionDomain::Gameplay | ConversionDomain::Presentation => chart.notes().notes().len(),
        ConversionDomain::Motion => chart
            .tracks()
            .tracks()
            .iter()
            .map(|track| track.pieces().len())
            .sum(),
        ConversionDomain::Scroll => chart
            .scroll()
            .lines()
            .iter()
            .map(|line| line.coordinate().points().len())
            .sum(),
        ConversionDomain::Resource | ConversionDomain::Package => {
            chart.metadata().resources().len()
        }
        ConversionDomain::Metadata => capability_entity_count(chart, domain),
        ConversionDomain::Syntax => 0,
        ConversionDomain::Profile => capability_entity_count(chart, domain),
    }
}

fn approximation_segment_count(chart: &CanonicalChart, domain: ConversionDomain) -> usize {
    match domain {
        ConversionDomain::Timing => chart.time_map().segments().count(),
        ConversionDomain::Gameplay | ConversionDomain::Presentation => chart.notes().notes().len(),
        ConversionDomain::Motion => chart
            .tracks()
            .tracks()
            .iter()
            .map(|track| track.pieces().len())
            .sum(),
        ConversionDomain::Scroll => chart
            .scroll()
            .lines()
            .iter()
            .map(|line| line.coordinate().points().len())
            .sum(),
        ConversionDomain::Resource | ConversionDomain::Package => {
            chart.metadata().resources().len()
        }
        ConversionDomain::Metadata => capability_entity_count(chart, domain),
        ConversionDomain::Syntax => 0,
        ConversionDomain::Profile => capability_entity_count(chart, domain),
    }
}

fn negotiation_message(
    domain: ConversionDomain,
    action: NegotiationAction,
    options: &ExportOptions,
    approximation_segments: usize,
    missing_features: &[CapabilityFeature],
    limit_failure: Option<&str>,
) -> String {
    match action {
        NegotiationAction::Unsupported if !missing_features.is_empty() => format!(
            "{} target is missing required capability features: {}",
            domain,
            missing_features
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        NegotiationAction::Unsupported if limit_failure.is_some() => format!(
            "{} target cannot satisfy declared capability limit {}",
            domain,
            limit_failure.unwrap_or_default()
        ),
        NegotiationAction::Bake => {
            let budgets = options
                .approximation
                .error_budgets()
                .iter()
                .filter(|(metric, _)| {
                    metric.as_str() == domain.as_str()
                        || metric
                            .strip_prefix(domain.as_str())
                            .is_some_and(|suffix| suffix.starts_with('.'))
                })
                .map(|(metric, budget)| format!("{metric}={budget}"))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{} domain negotiated as bake using {}@{} with {} input features, output segment cap {}, and budgets [{}]",
                domain,
                options.approximation.algorithm_id(),
                options.approximation.algorithm_version(),
                approximation_segments,
                options.approximation.maximum_segments(),
                budgets
            )
        }
        NegotiationAction::Drop => format!(
            "{} domain negotiated as drop: {}",
            domain,
            options.drop.reason()
        ),
        _ => format!("{} domain negotiated as {}", domain, action.as_str()),
    }
}

/// Compatibility negotiation wrapper. New code should use
/// `negotiate_export_with_options` for the report and authorization contract.
pub fn negotiate_export(
    chart: &CanonicalChart,
    target: &CapabilitySet,
) -> Result<NegotiationAction, ExportError> {
    let descriptor = target.descriptor(None);
    let options = ExportOptions::semantic(descriptor);
    Ok(negotiate_export_with_options(chart, &options)?.0.action())
}

/// Export a modern RPE JSON chart from CanonicalChart (I8.6 product surface).
pub fn export_rpe_json(chart: &CanonicalChart) -> Result<Vec<u8>, ExportError> {
    let profile = RpeProfile::PhiraLegacySpeed;
    let options = ExportOptions::semantic(
        CapabilitySet::rpe_json()
            .descriptor(Some(profile_reference(profile.id(), profile.version()))),
    );
    Ok(export_rpe_json_with_options(chart, &options)?.into_bytes())
}

pub fn export_rpe_json_with_options(
    chart: &CanonicalChart,
    options: &ExportOptions,
) -> Result<ExportOutcome, ExportError> {
    export_rpe_json_with_resource_context(chart, None, options, false)
}

fn export_rpe_json_with_resource_context(
    chart: &CanonicalChart,
    expected_resources: Option<&CanonicalResourceBundle>,
    options: &ExportOptions,
    fidelity_sink: bool,
) -> Result<ExportOutcome, ExportError> {
    let (profile, binding, rpe_version) = selected_rpe_binding(options)?;
    let (negotiation, entries) =
        negotiate_export_with_fidelity_sink(chart, options, fidelity_sink)?;
    require_rpe_chart_shape(chart, &negotiation, &binding)?;
    let offset_ms = chart
        .metadata()
        .sync()
        .map(|sync| sync.audio_offset().seconds() * 1000.0)
        .unwrap_or(0.0);
    let bpm_list: Vec<_> = chart
        .time_map()
        .segments()
        .map(|(beat, _, bpm)| {
            json!({
                "startTime": [beat.numerator(), 0, beat.denominator()],
                "bpm": bpm
            })
        })
        .collect();
    let mut ordered_lines: Vec<_> = chart.lines().lines().collect();
    ordered_lines.sort_by_key(|line| line.document_order());
    let mut judge_lines = Vec::new();
    let mut target_ids = StableIdRegistry::new();
    let mut line_alignment = Vec::with_capacity(ordered_lines.len());
    let mut note_alignment = Vec::with_capacity(chart.notes().notes().len());
    let mut target_note_order = 0u64;
    for (line_index, line) in ordered_lines.iter().copied().enumerate() {
        line_alignment.push((
            line.id().clone(),
            generated_target_id(
                &mut target_ids,
                EntityKind::Line,
                "rpeLines",
                line_index as u64,
            )?,
        ));
        let line_id = line.id().value();
        let mut line_notes: Vec<_> = chart
            .notes()
            .notes()
            .iter()
            .filter(|note| note.gameplay().line().value() == line_id)
            .collect();
        line_notes.sort_by_key(|note| note.document_order());
        let mut notes = Vec::with_capacity(line_notes.len());
        for note in line_notes {
            let start = seconds_to_rpe_beat(
                chart
                    .time_map()
                    .beat_at_time(note.gameplay().time().chart_time_seconds())
                    .map_err(|error| {
                        ExportError::new("conversion.capability-mismatch", error.to_string())
                    })?,
            );
            let end = note
                .gameplay()
                .end_time()
                .map(|time| {
                    chart
                        .time_map()
                        .beat_at_time(time.chart_time_seconds())
                        .map(seconds_to_rpe_beat)
                })
                .transpose()
                .map_err(|error| {
                    ExportError::new("conversion.capability-mismatch", error.to_string())
                })?
                .unwrap_or(start);
            let note_type = match note.kind() {
                CanonicalNoteKind::Tap => 1,
                CanonicalNoteKind::Hold => 2,
                CanonicalNoteKind::Flick => 3,
                CanonicalNoteKind::Drag => 4,
            };
            let above = match note.gameplay().side() {
                CanonicalNoteSide::Above => 1,
                CanonicalNoteSide::Below => 0,
            };
            let presentation_dropped = negotiation.drops(ConversionDomain::Presentation);
            let raw_speed = if presentation_dropped {
                4.5
            } else {
                baked_presentation_value(note.presentation().scroll_factor(), &negotiation) * 4.5
            };
            let mut payload = json!({
                "type": note_type,
                "startTime": start,
                "endTime": end,
                "positionX": if presentation_dropped {
                    0.0
                } else {
                    baked_presentation_value(note.presentation().position_x(), &negotiation)
                },
                "speed": raw_speed,
                "above": above,
                "isFake": if note.gameplay().judgment_enabled() { 0 } else { 1 }
            });
            if !presentation_dropped
                && matches!(
                    profile,
                    RpeProfile::PhiraLegacySpeed | RpeProfile::PhiraRpe170Speed
                )
            {
                let object = payload.as_object_mut().expect("Note payload is an object");
                if note.presentation().alpha() != 1.0 {
                    let alpha = note.presentation().alpha() * 255.0;
                    if alpha.round() != alpha {
                        return Err(ExportError::new(
                            "conversion.capability-mismatch",
                            "RPE Phira Note alpha requires an exact byte fraction",
                        ));
                    }
                    object.insert("alpha".into(), json!(alpha as i64));
                }
                if note.presentation().scale_x() != 1.0 {
                    object.insert("size".into(), json!(note.presentation().scale_x()));
                }
                if note.presentation().y_offset() != 0.0 {
                    if raw_speed == 0.0 {
                        return Err(ExportError::new(
                            "conversion.capability-mismatch",
                            "RPE Note yOffset cannot be inverted at zero raw speed",
                        ));
                    }
                    object.insert(
                        "yOffset".into(),
                        json!(note.presentation().y_offset() / (1.2 * raw_speed)),
                    );
                }
                if let Some(visible_from) = note.presentation().visible_from() {
                    object.insert(
                        "visibleTime".into(),
                        json!(
                            note.gameplay().time().chart_time_seconds()
                                - visible_from.chart_time_seconds()
                        ),
                    );
                }
            }
            notes.push(payload);
            note_alignment.push((
                note.id().clone(),
                generated_target_id(
                    &mut target_ids,
                    EntityKind::Note,
                    "rpeNotes",
                    target_note_order,
                )?,
            ));
            target_note_order = target_note_order.saturating_add(1);
        }
        let motion_dropped = negotiation.drops(ConversionDomain::Motion);
        let father = if motion_dropped {
            -1
        } else {
            line.parent()
                .and_then(|parent| {
                    ordered_lines
                        .iter()
                        .position(|candidate| candidate.id().value() == parent.value())
                })
                .map_or(-1, |index| index as i64)
        };
        let event_layers = if motion_dropped {
            Vec::new()
        } else {
            rpe_event_layers(chart, line, &binding)?
        };
        judge_lines.push(json!({
            "bpmfactor": 1,
            "eventLayers": event_layers,
            "notes": notes,
            "father": father,
            "rotateWithFather": if motion_dropped {
                false
            } else {
                line.inherit().rotation()
            }
        }));
    }
    if judge_lines.is_empty() {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            "RPE requires at least one canonical Line",
        ));
    }
    let root = json!({
        "META": { "RPEVersion": rpe_version, "offset": offset_ms, "name": "fcs-export" },
        "BPMList": bpm_list,
        "judgeLineList": judge_lines
    });
    let bytes = serde_json::to_vec_pretty(&root)
        .map_err(|error| ExportError::new("conversion.internal", error.to_string()))?;
    let artifact = SourceArtifact::new("export.rpe.json", ArtifactRole::Chart, bytes.clone())
        .map_err(|error| ExportError::new("conversion.source-invalid", error.to_string()))?;
    let parsed = parse_json_document(SourceFormat::Rpe, &artifact)
        .map_err(|error| ExportError::new("conversion.source-invalid", error.to_string()))?;
    let source = parse_rpe_document(&parsed, RpeLimits::default())
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let semantic = interpret_rpe_semantics(&source, &binding)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let reparsed = lower_rpe_to_canonical(&semantic, &artifact)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let alignment = entity_alignment(line_alignment, note_alignment)?;
    finish_export(
        "rpe",
        chart,
        reparsed.compilation().chart(),
        expected_resources,
        reparsed.compilation().resources(),
        &alignment,
        options,
        negotiation,
        entries,
        bytes,
    )
}

/// Export a complete canonical product through the RPE target boundary.
pub fn export_rpe_compilation_with_options(
    compilation: &CanonicalCompilation,
    options: &ExportOptions,
) -> Result<ExportOutcome, ExportError> {
    validate_compilation_resource_closure(compilation)?;
    let outcome = export_rpe_json_with_resource_context(
        compilation.chart(),
        Some(compilation.resources()),
        options,
        true,
    )?;
    record_compilation_roundtrip_context(outcome, compilation, options)
}

/// Export a Phira line-command PEC chart from CanonicalChart (I8.7 product surface).
pub fn export_pec_line(chart: &CanonicalChart) -> Result<Vec<u8>, ExportError> {
    let profile = PecProfile::Phira;
    let options = ExportOptions::semantic(
        CapabilitySet::pec_line()
            .descriptor(Some(profile_reference(profile.id(), profile.version()))),
    );
    Ok(export_pec_line_with_options(chart, &options)?.into_bytes())
}

pub fn export_pec_line_with_options(
    chart: &CanonicalChart,
    options: &ExportOptions,
) -> Result<ExportOutcome, ExportError> {
    export_pec_line_with_resource_context(chart, None, options, false)
}

fn pec_track_events(
    chart: &CanonicalChart,
    owner: &StableId,
    line_index: usize,
    profile: PecProfile,
    negotiation: &NegotiationPlan,
) -> Result<Vec<(u64, String)>, ExportError> {
    if negotiation.drops(ConversionDomain::Motion) {
        return Ok(Vec::new());
    }
    let mut events = Vec::new();
    for track in chart
        .tracks()
        .tracks()
        .iter()
        .filter(|track| track.owner() == owner)
    {
        let property = match (track.name(), track.target()) {
            ("pec.position", CanonicalTrackTarget::Position) => "position",
            ("pec.rotation", CanonicalTrackTarget::Rotation) => "rotation",
            ("pec.alpha", CanonicalTrackTarget::Alpha) => "alpha",
            ("pec.speed", CanonicalTrackTarget::ScrollSpeed) => "speed",
            _ => {
                return Err(ExportError::new(
                    "conversion.capability-mismatch",
                    format!("Track {} is not representable by PEC", track.name()),
                ));
            }
        };
        if track.blend() != CanonicalTrackBlend::Replace
            || track.priority() != 0
            || track.fill() != CanonicalTrackFill::Base
            || track.extrapolate_before() != CanonicalTrackFill::Base
            || track.extrapolate_after() != CanonicalTrackFill::Base
        {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!(
                    "Track {} has unsupported PEC blend/fill behavior",
                    track.name()
                ),
            ));
        }
        for piece in track.pieces() {
            events.push(pec_track_event(
                chart, property, line_index, profile, piece,
            )?);
        }
    }
    events.sort_by_key(|(order, _)| *order);
    Ok(events)
}

fn pec_track_event(
    chart: &CanonicalChart,
    property: &str,
    line_index: usize,
    profile: PecProfile,
    piece: &CanonicalTrackPiece,
) -> Result<(u64, String), ExportError> {
    let (order, start, end, start_value, end_value, interpolation) = match piece {
        CanonicalTrackPiece::Segment(segment) => (
            segment.document_order(),
            segment.start(),
            segment.end(),
            segment.start_value(),
            segment.end_value(),
            Some(segment.interpolation()),
        ),
        CanonicalTrackPiece::Point(point) => (
            point.document_order(),
            point.time(),
            point.time(),
            point.value(),
            point.value(),
            None,
        ),
    };
    let start_beat = pec_beat(chart, start, "PEC event start beat")?;
    let end_beat = pec_beat(chart, end, "PEC event end beat")?;
    let line = line_index.to_string();
    let output = match (property, interpolation) {
        ("position", None) => {
            let value = pec_position_value(start_value, "PEC position point")?;
            format!("cp {line} {start_beat} {} {}\n", value.0, value.1)
        }
        ("rotation", None) => {
            let value = pec_rotation_value(start_value, "PEC rotation point")?;
            format!("cd {line} {start_beat} {value}\n")
        }
        ("alpha", None) => {
            let value = pec_alpha_value(start_value, "PEC alpha point")?;
            format!("ca {line} {start_beat} {value}\n")
        }
        ("speed", None) => {
            let value = pec_speed_value(start_value, profile, "PEC speed point")?;
            format!("cv {line} {start_beat} {value}\n")
        }
        ("position", Some(interpolation)) => {
            let _ = pec_position_value(start_value, "PEC position segment")?;
            let end = pec_position_value(end_value, "PEC position segment")?;
            let easing = pec_easing_id(interpolation, "PEC position easing")?;
            format!(
                "cm {line} {start_beat} {end_beat} {} {} {easing}\n",
                end.0, end.1
            )
        }
        ("rotation", Some(interpolation)) => {
            let value = pec_rotation_value(end_value, "PEC rotation segment")?;
            let easing = pec_easing_id(interpolation, "PEC rotation easing")?;
            let _ = pec_rotation_value(start_value, "PEC rotation segment")?;
            format!("cr {line} {start_beat} {end_beat} {value} {easing}\n")
        }
        ("alpha", Some(interpolation)) => {
            if interpolation != &CanonicalTrackInterpolation::Linear {
                return Err(ExportError::new(
                    "conversion.capability-mismatch",
                    "PEC cf alpha segments require linear interpolation",
                ));
            }
            let value = pec_alpha_value(end_value, "PEC alpha segment")?;
            let _ = pec_alpha_value(start_value, "PEC alpha segment")?;
            format!("cf {line} {start_beat} {end_beat} {value}\n")
        }
        ("speed", Some(_)) => {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                "PEC cv represents point speed events only",
            ));
        }
        _ => {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                "PEC event property is not representable",
            ));
        }
    };
    Ok((order, output))
}

fn pec_beat(
    chart: &CanonicalChart,
    time: fcs_model::CanonicalTime,
    field: &str,
) -> Result<String, ExportError> {
    let beat = time
        .source_beat()
        .map_or_else(
            || chart.time_map().beat_at_time(time.chart_time_seconds()),
            |beat| Ok(beat.as_f64()),
        )
        .map_err(|error| ExportError::new("conversion.capability-mismatch", error.to_string()))?;
    finite_decimal(beat, field)
}

fn pec_position_value(
    value: CanonicalTrackValue,
    field: &str,
) -> Result<(String, String), ExportError> {
    let CanonicalTrackValue::Vec2Length(value) = value else {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("{field} has the wrong canonical value type"),
        ));
    };
    Ok((
        pec_canvas_coordinate(value.x(), 1920.0, 2048.0, field, crate::line_x_canvas_2048)?,
        pec_canvas_coordinate(value.y(), 1080.0, 1400.0, field, crate::line_y_canvas_1400)?,
    ))
}

fn pec_canvas_coordinate(
    value: f64,
    canonical_extent: f64,
    source_extent: f64,
    field: &str,
    transform: fn(&crate::ExactRational) -> Result<crate::ExactRational, crate::PecError>,
) -> Result<String, ExportError> {
    if !value.is_finite() {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("{field} must be finite"),
        ));
    }
    let source = (value / canonical_extent + 0.5) * source_extent;
    for precision in 0..=30 {
        let candidate = format!("{source:.precision$}");
        let Ok(candidate) = crate::ExactDecimal::parse(&candidate, DecimalLimits::default()) else {
            continue;
        };
        let Ok(candidate_value) = transform(candidate.exact()) else {
            continue;
        };
        let Ok(candidate_value) = candidate_value.to_f64() else {
            continue;
        };
        if candidate_value == value {
            return Ok(candidate.raw().to_owned());
        }
    }
    Err(ExportError::new(
        "conversion.capability-mismatch",
        format!("{field} cannot be serialized without changing its canonical value"),
    ))
}

fn pec_rotation_value(value: CanonicalTrackValue, field: &str) -> Result<String, ExportError> {
    let CanonicalTrackValue::Angle(value) = value else {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("{field} has the wrong canonical value type"),
        ));
    };
    finite_decimal(-value * 180.0 / std::f64::consts::PI, field)
}

fn pec_alpha_value(value: CanonicalTrackValue, field: &str) -> Result<String, ExportError> {
    let CanonicalTrackValue::Float(value) = value else {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("{field} has the wrong canonical value type"),
        ));
    };
    if !(0.0..=1.0).contains(&value) {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("{field} lies outside the PEC alpha range"),
        ));
    }
    finite_decimal(value * 255.0, field)
}

fn pec_speed_value(
    value: CanonicalTrackValue,
    profile: PecProfile,
    field: &str,
) -> Result<String, ExportError> {
    let CanonicalTrackValue::Float(value) = value else {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("{field} has the wrong canonical value type"),
        ));
    };
    let raw = match profile.cv_scale() {
        crate::PecCvScale::Div585 => value * 5.85,
        crate::PecCvScale::Div7 | crate::PecCvScale::RpeHeight900 => value * 7.0,
    };
    finite_decimal(raw, field)
}

fn pec_easing_id(
    interpolation: &CanonicalTrackInterpolation,
    field: &str,
) -> Result<i64, ExportError> {
    match interpolation {
        CanonicalTrackInterpolation::Linear => Ok(1),
        CanonicalTrackInterpolation::Easing(name) => {
            crate::rpe::rpe_easing_id(name).ok_or_else(|| {
                ExportError::new(
                    "conversion.capability-mismatch",
                    format!("{field} uses an easing outside the fixed Phira table"),
                )
            })
        }
        CanonicalTrackInterpolation::Step | CanonicalTrackInterpolation::CubicBezier(_) => {
            Err(ExportError::new(
                "conversion.capability-mismatch",
                format!("{field} uses an unsupported PEC interpolation"),
            ))
        }
    }
}

fn export_pec_line_with_resource_context(
    chart: &CanonicalChart,
    expected_resources: Option<&CanonicalResourceBundle>,
    options: &ExportOptions,
    fidelity_sink: bool,
) -> Result<ExportOutcome, ExportError> {
    let profile = selected_pec_profile(options)?;
    if !profile.strict_eligible() {
        return Err(ExportError::new(
            "conversion.profile-not-applicable",
            "the selected PEC profile is source-only and cannot be an export target",
        ));
    }
    let (negotiation, entries) =
        negotiate_export_with_fidelity_sink(chart, options, fidelity_sink)?;
    require_pec_chart_shape(chart, options, &negotiation)?;
    let offset = chart
        .metadata()
        .sync()
        .map(|sync| sync.audio_offset().seconds())
        .unwrap_or(0.0);
    let raw_offset = finite_decimal(
        offset * 1000.0 + profile.offset_bias_ms() as f64,
        "PEC offset",
    )?;
    let mut lines = String::new();
    lines.push_str(&format!("{raw_offset}\n"));
    for (beat, _, bpm) in chart.time_map().segments() {
        let beat = finite_decimal(beat.as_f64(), "PEC BPM beat")?;
        let bpm = finite_decimal(bpm, "PEC BPM value")?;
        lines.push_str(&format!("bp {beat} {bpm}\n"));
    }
    let mut ordered_lines: Vec<_> = chart.lines().lines().collect();
    ordered_lines.sort_by_key(|line| line.document_order());
    let mut target_ids = StableIdRegistry::new();
    let mut line_alignment = Vec::with_capacity(ordered_lines.len());
    for (line_index, line) in ordered_lines.iter().copied().enumerate() {
        line_alignment.push((
            line.id().clone(),
            generated_target_id(
                &mut target_ids,
                EntityKind::Line,
                "pecLines",
                line_index as u64,
            )?,
        ));
        for (_, event) in pec_track_events(chart, line.id(), line_index, profile, &negotiation)? {
            lines.push_str(&event);
        }
    }
    let mut notes: Vec<_> = chart.notes().notes().iter().collect();
    notes.sort_by_key(|note| note.document_order());
    let mut note_alignment = Vec::with_capacity(notes.len());
    for (target_note_order, note) in notes.into_iter().enumerate() {
        note_alignment.push((
            note.id().clone(),
            generated_target_id(
                &mut target_ids,
                EntityKind::Note,
                "pecNotes",
                target_note_order as u64,
            )?,
        ));
        let line_index = ordered_lines
            .iter()
            .position(|line| line.id().value() == note.gameplay().line().value())
            .ok_or_else(|| {
                ExportError::new(
                    "conversion.capability-mismatch",
                    "PEC Note references a Line outside the canonical Line graph",
                )
            })?;
        let beat = chart
            .time_map()
            .beat_at_time(note.gameplay().time().chart_time_seconds())
            .map_err(|error| {
                ExportError::new("conversion.capability-mismatch", error.to_string())
            })?;
        let beat = finite_decimal(beat, "PEC Note beat")?;
        let presentation_dropped = negotiation.drops(ConversionDomain::Presentation);
        let x = if presentation_dropped {
            0.0
        } else {
            baked_presentation_value(note.presentation().position_x(), &negotiation) * 16.0 / 15.0
        };
        let x = finite_decimal(x, "PEC Note X")?;
        let side = match note.gameplay().side() {
            CanonicalNoteSide::Above => 1,
            CanonicalNoteSide::Below => 2,
        };
        let fake = if note.gameplay().judgment_enabled() {
            0
        } else {
            1
        };
        match note.kind() {
            CanonicalNoteKind::Hold => {
                let end = note.gameplay().end_time().ok_or_else(|| {
                    ExportError::new(
                        "conversion.capability-mismatch",
                        "canonical Hold is missing its end time",
                    )
                })?;
                let end = chart
                    .time_map()
                    .beat_at_time(end.chart_time_seconds())
                    .map_err(|error| {
                        ExportError::new("conversion.capability-mismatch", error.to_string())
                    })?;
                let end = finite_decimal(end, "PEC Hold end beat")?;
                lines.push_str(&format!("n2 {line_index} {beat} {end} {x} {side} {fake}\n"));
            }
            CanonicalNoteKind::Tap => {
                lines.push_str(&format!("n1 {line_index} {beat} {x} {side} {fake}\n"));
            }
            CanonicalNoteKind::Flick => {
                lines.push_str(&format!("n3 {line_index} {beat} {x} {side} {fake}\n"));
            }
            CanonicalNoteKind::Drag => {
                lines.push_str(&format!("n4 {line_index} {beat} {x} {side} {fake}\n"));
            }
        }
        let scroll_factor = finite_decimal(
            if presentation_dropped {
                1.0
            } else {
                baked_presentation_value(note.presentation().scroll_factor(), &negotiation)
            },
            "PEC Note scroll factor",
        )?;
        let scale = finite_decimal(
            if presentation_dropped {
                1.0
            } else {
                baked_presentation_value(note.presentation().scale_x(), &negotiation)
            },
            "PEC Note scale",
        )?;
        lines.push_str(&format!("# {scroll_factor}\n& {scale}\n"));
    }
    let bytes = lines.into_bytes();
    let artifact = SourceArtifact::new("export.pec", ArtifactRole::Chart, bytes.clone())
        .map_err(|error| ExportError::new("conversion.source-invalid", error.to_string()))?;
    let source = parse_pec_document(&artifact, PecLimits::default())
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let binding = PecProfileBinding::new(profile, options.floor_scale_px.clone())
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let semantic = interpret_pec(&source, &binding)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let reparsed = lower_pec_to_canonical(&semantic, &artifact)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let alignment = entity_alignment(line_alignment, note_alignment)?;
    finish_export(
        "pec",
        chart,
        reparsed.compilation().chart(),
        expected_resources,
        reparsed.compilation().resources(),
        &alignment,
        options,
        negotiation,
        entries,
        bytes,
    )
}

/// Export a complete canonical product through the PEC target boundary.
pub fn export_pec_compilation_with_options(
    compilation: &CanonicalCompilation,
    options: &ExportOptions,
) -> Result<ExportOutcome, ExportError> {
    validate_compilation_resource_closure(compilation)?;
    let outcome = export_pec_line_with_resource_context(
        compilation.chart(),
        Some(compilation.resources()),
        options,
        true,
    )?;
    record_compilation_roundtrip_context(outcome, compilation, options)
}

fn seconds_to_rpe_beat(beats: f64) -> [i64; 3] {
    const DENOMINATOR: i64 = 1_000_000_000;
    let whole = beats.floor() as i64;
    let numerator = ((beats - whole as f64) * DENOMINATOR as f64).round() as i64;
    [whole, numerator, DENOMINATOR]
}

fn generated_target_id(
    registry: &mut StableIdRegistry,
    kind: EntityKind,
    collection: &str,
    order: u64,
) -> Result<StableId, ExportError> {
    let path = ExpansionPath::new(collection, order)
        .map_err(|error| ExportError::new("conversion.internal", error.to_string()))?;
    registry
        .insert(kind, CanonicalTextualId::generated(kind, &path, order))
        .map_err(|error| ExportError::new("conversion.internal", error.to_string()))
}

fn entity_alignment(
    lines: Vec<(StableId, StableId)>,
    notes: Vec<(StableId, StableId)>,
) -> Result<EntityAlignment, ExportError> {
    EntityAlignment::new(lines, notes).ok_or_else(|| {
        ExportError::new(
            "conversion.internal",
            "target entity provenance mapping is duplicate or ambiguous",
        )
    })
}

fn finite_decimal(value: f64, field: &str) -> Result<String, ExportError> {
    if !value.is_finite() {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("{field} must be finite"),
        ));
    }
    Ok(ryu::Buffer::new().format_finite(value).to_owned())
}

fn validate_compilation_resource_closure(
    compilation: &CanonicalCompilation,
) -> Result<(), ExportError> {
    let declared = compilation.chart().metadata().resources();
    let bundled = compilation.resources().resources();
    if declared.len() != bundled.len() {
        return Err(ExportError::new(
            "conversion.resource-missing",
            format!(
                "canonical resource declarations ({}) do not match bundled resources ({})",
                declared.len(),
                bundled.len()
            ),
        ));
    }
    for (id, resource) in declared {
        let Some(payload) = bundled.get(id) else {
            return Err(ExportError::new(
                "conversion.resource-missing",
                format!("canonical resource {id} has no bundled payload"),
            ));
        };
        if payload.resource() != resource {
            return Err(ExportError::new(
                "conversion.resource-missing",
                format!("canonical resource {id} descriptor differs from its bundled payload"),
            ));
        }
    }
    Ok(())
}

fn record_compilation_roundtrip_context(
    mut outcome: ExportOutcome,
    compilation: &CanonicalCompilation,
    options: &ExportOptions,
) -> Result<ExportOutcome, ExportError> {
    let semantic_losses = outcome
        .negotiation
        .entries()
        .iter()
        .filter(|entry| entry.action() == NegotiationAction::Preserve)
        .map(|entry| {
            SemanticLoss::new(
                entry.domain(),
                SemanticStatus::Preserved,
                entry.category(),
                None,
            )
            .map_err(|error| ExportError::new("conversion.internal", error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    outcome.distribution = Some(
        compilation
            .distribution()
            .clone()
            .with_semantic_losses(semantic_losses),
    );
    if options.policy != ConversionPolicy::Roundtrip {
        return Ok(outcome);
    }
    let stale_count = compilation
        .distribution()
        .provenance()
        .facts()
        .values()
        .filter(|fact| fact.is_stale())
        .count();
    if stale_count == 0 {
        return Ok(outcome);
    }
    let mut entries = outcome.report.entries().to_vec();
    push_report_entry(
        &mut entries,
        ConversionEntry::new(
            "roundtrip/stale-source-representation",
            "conversion.tool-rewrite",
            ConversionDomain::Profile,
            ConversionSeverity::Warning,
            SemanticStatus::Equivalent,
            ConversionPhase::Export,
            None,
            None,
            None,
            Some("source-representation".into()),
            None,
            None,
            None,
            None,
            None,
            format!(
                "rebuilt the target from canonical semantics because {stale_count} source round-trip facts are stale"
            ),
            [],
        )
        .map_err(|error| ExportError::new("conversion.report-limit", error.to_string()))?,
    )?;
    let operation_id = outcome.report.operation_id().to_owned();
    let conversion_policy = outcome.report.conversion_policy();
    let repair_mode = outcome.report.repair_mode().clone();
    let approximation_authorization = outcome.report.approximation_authorization().cloned();
    let drop_authorization = outcome.report.drop_authorization().cloned();
    let repairs = outcome.report.repairs().to_vec();
    let status = outcome.report.status();
    let output_hash = outcome.report.output_hash().map(str::to_owned);
    outcome.report = ConversionReport::new_with_authorizations(
        operation_id,
        conversion_policy,
        repair_mode,
        approximation_authorization,
        drop_authorization,
        entries,
        repairs,
        [status],
        output_hash,
    )
    .map_err(|error| ExportError::new("conversion.report-limit", error.to_string()))?;
    Ok(outcome)
}

fn profile_reference(id: &str, version: &str) -> String {
    format!("{id}@{version}")
}

fn selected_pgr_profile(options: &ExportOptions) -> Result<PgrProfile, ExportError> {
    if options.capabilities.format() != "pgr" {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            "PGR export requires a pgr capability descriptor",
        ));
    }
    let profile = options.target_profile.as_deref().ok_or_else(|| {
        ExportError::new(
            "conversion.target-profile-required",
            "PGR target profile is required",
        )
    })?;
    let profile = match profile {
        "pgr.phira.v1@1.0.0" => PgrProfile::PhiraV1,
        "pgr.phira.v3@1.0.0" => PgrProfile::PhiraV3,
        "pgr.phichain-import.v1@1.0.0" => PgrProfile::PhichainImportV1,
        "pgr.phichain-import.v3@1.0.0" => PgrProfile::PhichainImportV3,
        _ => {
            return Err(ExportError::new(
                "conversion.profile-not-found",
                format!("unknown PGR target profile {profile}"),
            ));
        }
    };
    let expected_version = if profile.format_version() == crate::PgrFormatVersion::V1 {
        "1"
    } else {
        "3"
    };
    if options.capabilities.version() != expected_version {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            "PGR capability version does not match the target profile formatVersion",
        ));
    }
    Ok(profile)
}

fn selected_rpe_binding(
    options: &ExportOptions,
) -> Result<(RpeProfile, RpeProfileBinding, i64), ExportError> {
    if options.capabilities.format() != "rpe" || options.capabilities.version() != "json" {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            "RPE export requires an rpe@json capability descriptor",
        ));
    }
    let profile = options.target_profile.as_deref().ok_or_else(|| {
        ExportError::new(
            "conversion.target-profile-required",
            "RPE target profile is required",
        )
    })?;
    let selected_profile = match profile {
        "rpe.community.divide-bpmfactor@1.0.0" => RpeProfile::CommunityDivideBpmfactor,
        "rpe.docs-example.multiply-bpmfactor@1.0.0" => RpeProfile::DocsExampleMultiplyBpmfactor,
        "rpe.phira.legacy-speed@1.0.0" => RpeProfile::PhiraLegacySpeed,
        "rpe.phira.rpe170-speed@1.0.0" => RpeProfile::PhiraRpe170Speed,
        "rpe.phichain-import@1.0.0" => {
            return Err(ExportError::new(
                "conversion.profile-not-applicable",
                "rpe.phichain-import is source-only and cannot be an export target",
            ));
        }
        _ => {
            return Err(ExportError::new(
                "conversion.profile-not-found",
                format!("unknown RPE target profile {profile}"),
            ));
        }
    };
    let binding = match options.rpe_profile_binding.as_ref() {
        Some(binding) => binding.clone(),
        None => match selected_profile {
            RpeProfile::CommunityDivideBpmfactor | RpeProfile::DocsExampleMultiplyBpmfactor => {
                return Err(ExportError::new(
                    "conversion.profile-parameter-invalid",
                    "this RPE target profile requires an explicit speedMode binding",
                ));
            }
            RpeProfile::PhiraLegacySpeed => RpeProfileBinding::phira_legacy_speed(),
            RpeProfile::PhiraRpe170Speed => {
                RpeProfileBinding::phira_rpe170_speed(Some(RpeVersionEra::AtLeast170))
            }
            RpeProfile::PhichainImport => unreachable!("source-only profile rejected above"),
        },
    };
    if binding.profile() != selected_profile {
        return Err(ExportError::new(
            "conversion.profile-parameter-invalid",
            format!(
                "RPE target binding {}@{} does not match selected profile {profile}",
                binding.profile().id(),
                binding.profile().version()
            ),
        ));
    }
    let rpe_version = rpe_target_version(&binding)?;
    Ok((selected_profile, binding, rpe_version))
}

fn rpe_target_version(binding: &RpeProfileBinding) -> Result<i64, ExportError> {
    match binding.profile() {
        RpeProfile::CommunityDivideBpmfactor | RpeProfile::DocsExampleMultiplyBpmfactor => {
            match binding.speed_mode() {
                // RPEVersion is evidence-only for these profiles. The typed
                // speedMode remains part of the external target binding.
                Some(_) => Ok(150),
                None => Err(ExportError::new(
                    "conversion.profile-parameter-invalid",
                    "this RPE target profile requires an explicit speedMode binding",
                )),
            }
        }
        RpeProfile::PhiraLegacySpeed => Ok(150),
        RpeProfile::PhiraRpe170Speed => match binding.rpe_version_era() {
            Some(RpeVersionEra::Pre170) => Ok(169),
            Some(RpeVersionEra::AtLeast170) | None => Ok(170),
        },
        RpeProfile::PhichainImport => Err(ExportError::new(
            "conversion.profile-not-applicable",
            "rpe.phichain-import is source-only and cannot be an export target",
        )),
    }
}

fn selected_pec_profile(options: &ExportOptions) -> Result<PecProfile, ExportError> {
    if options.capabilities.format() != "pec" || options.capabilities.version() != "line-command" {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            "PEC export requires a pec@line-command capability descriptor",
        ));
    }
    let profile = options.target_profile.as_deref().ok_or_else(|| {
        ExportError::new(
            "conversion.target-profile-required",
            "PEC target profile is required",
        )
    })?;
    match profile {
        "pec.phira@1.0.0" => Ok(PecProfile::Phira),
        "pec.extends@1.0.0" => Ok(PecProfile::Extends),
        "pec.phispler@1.0.0" => Ok(PecProfile::Phispler),
        _ => Err(ExportError::new(
            "conversion.profile-not-found",
            format!("unknown PEC target profile {profile}"),
        )),
    }
}

fn single_global_bpm(chart: &CanonicalChart, format: &str) -> Result<f64, ExportError> {
    let segments: Vec<_> = chart.time_map().segments().collect();
    if segments.len() != 1 || segments[0].0.numerator() != 0 {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("{format} target requires one global Beat-zero BPM for this writer"),
        ));
    }
    Ok(segments[0].2)
}

fn require_external_payload_losses(
    chart: &CanonicalChart,
    negotiation: &NegotiationPlan,
    format: &str,
) -> Result<(), ExportError> {
    let metadata = chart.metadata();
    let has_metadata = metadata.meta().is_some()
        || !metadata.contributors().is_empty()
        || !metadata.credits().is_empty()
        || metadata.artwork().is_some();
    if has_metadata && !negotiation.omits(ConversionDomain::Metadata) {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("{format} writer cannot represent canonical metadata"),
        ));
    }

    let has_resources = !metadata.resources().is_empty()
        || metadata
            .sync()
            .is_some_and(|sync| sync.primary_audio().is_some() || sync.preview().is_some());
    let resource_loss = negotiation.omits(ConversionDomain::Resource);
    let package_loss = negotiation.omits(ConversionDomain::Package);
    if has_resources && !(resource_loss && package_loss) {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("{format} writer cannot represent canonical resources or package bindings"),
        ));
    }

    if (chart.descriptors().is_some() || !chart.required_extensions().is_empty())
        && !negotiation.omits(ConversionDomain::Profile)
    {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("{format} writer cannot represent descriptors or required extensions"),
        ));
    }
    Ok(())
}

fn preserved_selectors(negotiation: &NegotiationPlan) -> Vec<String> {
    negotiation
        .entries()
        .iter()
        .filter(|entry| {
            entry.action() == NegotiationAction::Preserve
                && matches!(
                    entry.domain(),
                    ConversionDomain::Metadata
                        | ConversionDomain::Resource
                        | ConversionDomain::Package
                )
        })
        .map(|entry| entry.domain().as_str().to_owned())
        .collect()
}

fn line_base_is_default(line: &CanonicalLine, floor_scale: f64) -> bool {
    let base = line.base();
    base.position().x() == 0.0
        && base.position().y() == 0.0
        && base.rotation() == 0.0
        && base.scale().x() == 1.0
        && base.scale().y() == 1.0
        && base.alpha() == 1.0
        && base.transform_origin().x() == 0.0
        && base.transform_origin().y() == 0.0
        && base.texture_anchor().x() == 0.5
        && base.texture_anchor().y() == 0.5
        && base.floor_scale() == floor_scale
        && base.integration_origin() == 0.0
        && base.initial_floor_position() == 0.0
        && !base.allow_reverse_scroll()
        && base.z_order() == 0
}

#[derive(Clone, Copy)]
enum RpeTrackProperty {
    MoveX,
    MoveY,
    Rotation,
    Alpha,
    Speed,
}

impl RpeTrackProperty {
    const fn field(self) -> &'static str {
        match self {
            Self::MoveX => "moveXEvents",
            Self::MoveY => "moveYEvents",
            Self::Rotation => "rotateEvents",
            Self::Alpha => "alphaEvents",
            Self::Speed => "speedEvents",
        }
    }

    const fn target(self) -> CanonicalTrackTarget {
        match self {
            Self::MoveX | Self::MoveY => CanonicalTrackTarget::Position,
            Self::Rotation => CanonicalTrackTarget::Rotation,
            Self::Alpha => CanonicalTrackTarget::Alpha,
            Self::Speed => CanonicalTrackTarget::ScrollSpeed,
        }
    }
}

#[derive(Clone, Copy)]
struct RpeTrackSpec {
    layer: usize,
    property: RpeTrackProperty,
}

fn rpe_track_spec(track: &CanonicalTrack) -> Result<RpeTrackSpec, ExportError> {
    let parts = track.name().split('.').collect::<Vec<_>>();
    let ["rpe", "layer", layer, property] = parts.as_slice() else {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!(
                "Track {} is not representable by the RPE event-layer writer",
                track.name()
            ),
        ));
    };
    let layer = layer.parse::<usize>().map_err(|_| {
        ExportError::new(
            "conversion.capability-mismatch",
            format!("RPE Track {} has an invalid layer index", track.name()),
        )
    })?;
    let property = match *property {
        "moveX" => RpeTrackProperty::MoveX,
        "moveY" => RpeTrackProperty::MoveY,
        "rotate" => RpeTrackProperty::Rotation,
        "alpha" => RpeTrackProperty::Alpha,
        "speed" => RpeTrackProperty::Speed,
        _ => {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!(
                    "Track {} has an unsupported RPE event-layer property",
                    track.name()
                ),
            ));
        }
    };
    if track.target() != property.target()
        || track.blend() != CanonicalTrackBlend::Add
        || track.priority() != layer as i64
        || track.fill() != CanonicalTrackFill::Zero
        || track.extrapolate_before() != CanonicalTrackFill::Zero
        || track.extrapolate_after() != CanonicalTrackFill::Zero
    {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!(
                "Track {} has unsupported RPE layer blend, priority, fill, or target",
                track.name()
            ),
        ));
    }
    Ok(RpeTrackSpec { layer, property })
}

fn rpe_line_base_is_default(line: &CanonicalLine, alpha: f64) -> bool {
    let base = line.base();
    base.position().x() == 0.0
        && base.position().y() == 0.0
        && base.rotation() == 0.0
        && base.scale().x() == 1.0
        && base.scale().y() == 1.0
        && base.alpha() == alpha
        && base.transform_origin().x() == 0.0
        && base.transform_origin().y() == 0.0
        && base.texture_anchor().x() == 0.5
        && base.texture_anchor().y() == 0.5
        && base.floor_scale() == 1.0
        && base.integration_origin() == 0.0
        && base.initial_floor_position() == 0.0
        && !base.allow_reverse_scroll()
        && base.z_order() == 0
}

fn rpe_track_is_supported(
    track: &CanonicalTrack,
    spec: RpeTrackSpec,
    binding: &RpeProfileBinding,
) -> Result<(), ExportError> {
    let mut document_orders = BTreeSet::new();
    for piece in track.pieces() {
        if !document_orders.insert(match piece {
            CanonicalTrackPiece::Segment(segment) => segment.document_order(),
            CanonicalTrackPiece::Point(point) => point.document_order(),
        }) {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!("Track {} has ambiguous source order", track.name()),
            ));
        }
        if let CanonicalTrackPiece::Segment(segment) = piece {
            let supported = match spec.property {
                RpeTrackProperty::Speed => {
                    rpe_speed_interpolation_supported(binding, segment.interpolation())
                }
                _ => !matches!(segment.interpolation(), CanonicalTrackInterpolation::Step),
            };
            if !supported {
                return Err(ExportError::new(
                    "conversion.capability-mismatch",
                    format!(
                        "Track {} has an unsupported RPE interpolation",
                        track.name()
                    ),
                ));
            }
        }
        match (spec.property, piece) {
            (RpeTrackProperty::MoveX, CanonicalTrackPiece::Segment(segment)) => {
                require_zero_position_component(segment.start_value(), true, track.name())?;
                require_zero_position_component(segment.end_value(), true, track.name())?;
            }
            (RpeTrackProperty::MoveY, CanonicalTrackPiece::Segment(segment)) => {
                require_zero_position_component(segment.start_value(), false, track.name())?;
                require_zero_position_component(segment.end_value(), false, track.name())?;
            }
            (RpeTrackProperty::MoveX, CanonicalTrackPiece::Point(point)) => {
                require_zero_position_component(point.value(), true, track.name())?;
            }
            (RpeTrackProperty::MoveY, CanonicalTrackPiece::Point(point)) => {
                require_zero_position_component(point.value(), false, track.name())?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn rpe_speed_interpolation_supported(
    binding: &RpeProfileBinding,
    interpolation: &CanonicalTrackInterpolation,
) -> bool {
    if matches!(interpolation, CanonicalTrackInterpolation::Step) {
        return false;
    }
    match binding.profile() {
        RpeProfile::PhiraLegacySpeed => {
            matches!(interpolation, CanonicalTrackInterpolation::Linear)
        }
        RpeProfile::PhiraRpe170Speed => match binding.rpe_version_era() {
            Some(RpeVersionEra::Pre170) => {
                matches!(interpolation, CanonicalTrackInterpolation::Linear)
            }
            Some(RpeVersionEra::AtLeast170) | None => true,
        },
        RpeProfile::CommunityDivideBpmfactor | RpeProfile::DocsExampleMultiplyBpmfactor => {
            match binding.speed_mode() {
                Some(crate::RpeSpeedMode::ModernEased) => true,
                Some(crate::RpeSpeedMode::LegacyLinear | crate::RpeSpeedMode::LegacyDerivative) => {
                    matches!(interpolation, CanonicalTrackInterpolation::Linear)
                }
                None => false,
            }
        }
        RpeProfile::PhichainImport => false,
    }
}

fn require_zero_position_component(
    value: CanonicalTrackValue,
    check_y: bool,
    name: &str,
) -> Result<(), ExportError> {
    let CanonicalTrackValue::Vec2Length(value) = value else {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("Track {name} has a non-position value"),
        ));
    };
    let component = if check_y { value.y() } else { value.x() };
    if component != 0.0 {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("Track {name} has a nonzero unmapped position component"),
        ));
    }
    Ok(())
}

fn rpe_event_layers(
    chart: &CanonicalChart,
    line: &CanonicalLine,
    binding: &RpeProfileBinding,
) -> Result<Vec<Value>, ExportError> {
    let mut layers: BTreeMap<usize, BTreeMap<&'static str, Vec<Value>>> = BTreeMap::new();
    for track in chart
        .tracks()
        .tracks()
        .iter()
        .filter(|track| track.owner() == line.id())
    {
        let spec = rpe_track_spec(track)?;
        rpe_track_is_supported(track, spec, binding)?;
        let mut pieces = track.pieces().iter().collect::<Vec<_>>();
        pieces.sort_by_key(|piece| match piece {
            CanonicalTrackPiece::Segment(segment) => segment.document_order(),
            CanonicalTrackPiece::Point(point) => point.document_order(),
        });
        let events = pieces
            .into_iter()
            .map(|piece| rpe_event(chart, spec.property, piece))
            .collect::<Result<Vec<_>, _>>()?;
        if layers
            .entry(spec.layer)
            .or_default()
            .insert(spec.property.field(), events)
            .is_some()
        {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!("duplicate RPE event-layer property on layer {}", spec.layer),
            ));
        }
    }
    let Some(last_layer) = layers.keys().next_back().copied() else {
        return Ok(Vec::new());
    };
    let mut output = vec![Value::Null; last_layer + 1];
    for (layer_index, fields) in layers {
        let object = fields
            .into_iter()
            .map(|(field, events)| (field.to_owned(), Value::Array(events)))
            .collect();
        output[layer_index] = Value::Object(object);
    }
    Ok(output)
}

fn rpe_event(
    chart: &CanonicalChart,
    property: RpeTrackProperty,
    piece: &CanonicalTrackPiece,
) -> Result<Value, ExportError> {
    let (start_time, end_time, start_value, end_value, interpolation) = match piece {
        CanonicalTrackPiece::Segment(segment) => (
            segment.start().chart_time_seconds(),
            segment.end().chart_time_seconds(),
            segment.start_value(),
            segment.end_value(),
            Some(segment.interpolation()),
        ),
        CanonicalTrackPiece::Point(point) => (
            point.time().chart_time_seconds(),
            point.time().chart_time_seconds(),
            point.value(),
            point.value(),
            None,
        ),
    };
    let mut object = serde_json::Map::new();
    object.insert(
        "startTime".into(),
        json!(seconds_to_rpe_beat(chart_time_to_beat(chart, start_time)?)),
    );
    object.insert(
        "endTime".into(),
        json!(seconds_to_rpe_beat(chart_time_to_beat(chart, end_time)?)),
    );
    object.insert("start".into(), json!(rpe_value(start_value, property)?));
    object.insert("end".into(), json!(rpe_value(end_value, property)?));
    if let Some(interpolation) = interpolation {
        write_rpe_interpolation(&mut object, interpolation)?;
    }
    Ok(Value::Object(object))
}

fn chart_time_to_beat(chart: &CanonicalChart, seconds: f64) -> Result<f64, ExportError> {
    chart
        .time_map()
        .beat_at_time(seconds)
        .map_err(|error| ExportError::new("conversion.capability-mismatch", error.to_string()))
}

fn rpe_value(value: CanonicalTrackValue, property: RpeTrackProperty) -> Result<f64, ExportError> {
    let value = match (property, value) {
        (RpeTrackProperty::MoveX, CanonicalTrackValue::Vec2Length(value)) => {
            value.x() * 1350.0 / 1920.0
        }
        (RpeTrackProperty::MoveY, CanonicalTrackValue::Vec2Length(value)) => {
            value.y() * 900.0 / 1080.0
        }
        (RpeTrackProperty::Rotation, CanonicalTrackValue::Angle(value)) => {
            -value * 180.0 / std::f64::consts::PI
        }
        (RpeTrackProperty::Alpha, CanonicalTrackValue::Float(value)) => value * 255.0,
        (RpeTrackProperty::Speed, CanonicalTrackValue::Float(value)) => value * 4.5,
        _ => {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                "RPE Track value type does not match its event-layer property",
            ));
        }
    };
    value.is_finite().then_some(value).ok_or_else(|| {
        ExportError::new(
            "conversion.capability-mismatch",
            "RPE event value is not finite",
        )
    })
}

fn write_rpe_interpolation(
    object: &mut serde_json::Map<String, Value>,
    interpolation: &CanonicalTrackInterpolation,
) -> Result<(), ExportError> {
    match interpolation {
        CanonicalTrackInterpolation::Linear => {
            object.insert("easingType".into(), json!(1));
        }
        CanonicalTrackInterpolation::Easing(name) => {
            let id = crate::rpe::rpe_easing_id(name).ok_or_else(|| {
                ExportError::new(
                    "conversion.capability-mismatch",
                    format!("unsupported Core easing {name} for RPE export"),
                )
            })?;
            object.insert("easingType".into(), json!(id));
        }
        CanonicalTrackInterpolation::CubicBezier([x1, y1, x2, y2]) => {
            object.insert("bezier".into(), json!(1));
            object.insert("bezierPoints".into(), json!([x1, y1, x2, y2]));
        }
        CanonicalTrackInterpolation::Step => {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                "RPE event layers do not represent Track step interpolation",
            ));
        }
    }
    Ok(())
}

fn note_gameplay_is_external_default(chart_note: &fcs_model::CanonicalNote) -> bool {
    let gameplay = chart_note.gameplay();
    gameplay.judge_shape() == &CanonicalJudgeShape::LineDefault
        && if gameplay.judgment_enabled() {
            gameplay.sound_policy() == &CanonicalNoteSoundPolicy::Default
                && gameplay.score_policy() == &CanonicalNoteScorePolicy::Default
        } else {
            gameplay.sound_policy() == &CanonicalNoteSoundPolicy::None
                && gameplay.score_policy() == &CanonicalNoteScorePolicy::None
        }
}

fn require_rpe_chart_shape(
    chart: &CanonicalChart,
    negotiation: &NegotiationPlan,
    binding: &RpeProfileBinding,
) -> Result<(), ExportError> {
    require_external_payload_losses(chart, negotiation, "RPE")?;
    let mut alpha_tracks = BTreeSet::new();
    if !negotiation.drops(ConversionDomain::Motion) {
        for track in chart.tracks().tracks() {
            let spec = rpe_track_spec(track)?;
            rpe_track_is_supported(track, spec, binding)?;
            if matches!(spec.property, RpeTrackProperty::Alpha) {
                alpha_tracks.insert(track.owner().value());
            }
        }
    }
    for line in chart.lines().lines() {
        if !negotiation.drops(ConversionDomain::Motion)
            && (!rpe_line_base_is_default(
                line,
                if alpha_tracks.contains(&line.id().value()) {
                    0.0
                } else {
                    1.0
                },
            ) || *line.inherit()
                != CanonicalLineInherit::new(true, line.inherit().rotation(), true, true, true))
        {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!(
                    "Line {} has fields outside the RPE writer subset",
                    line.document_order()
                ),
            ));
        }
    }
    for note in chart.notes().notes() {
        let presentation = note.presentation();
        let gameplay_unsupported = !note_gameplay_is_external_default(note);
        let presentation_unsupported = presentation.x_offset() != 0.0
            || presentation.scale_x() != presentation.scale_y()
            || !(0.0..=1.0).contains(&presentation.alpha())
            || presentation.rotation() != 0.0
            || presentation.color() != CanonicalColor::rgba(255, 255, 255, 255)
            || presentation.texture().is_some()
            || !presentation.render_enabled()
            || presentation.visible_until().is_some();
        if (gameplay_unsupported && !negotiation.drops(ConversionDomain::Gameplay))
            || (presentation_unsupported
                && !negotiation.drops(ConversionDomain::Presentation)
                && !negotiation.approximates(ConversionDomain::Presentation))
        {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!(
                    "Note {} has fields outside the RPE writer subset",
                    note.document_order()
                ),
            ));
        }
    }
    Ok(())
}

fn require_pec_chart_shape(
    chart: &CanonicalChart,
    options: &ExportOptions,
    negotiation: &NegotiationPlan,
) -> Result<(), ExportError> {
    require_external_payload_losses(chart, negotiation, "PEC")?;
    let profile = selected_pec_profile(options)?;
    let floor_scale = options.floor_scale_px.to_f64().map_err(|error| {
        ExportError::new("conversion.profile-parameter-invalid", error.to_string())
    })?;
    let mut lines: Vec<_> = chart.lines().lines().collect();
    lines.sort_by_key(|line| line.document_order());
    for (index, line) in lines.iter().enumerate() {
        if !negotiation.drops(ConversionDomain::Motion)
            && (line.document_order() != index as u64
                || line.parent().is_some()
                || line.inherit() != &CanonicalLineInherit::default()
                || !line_base_is_default(line, floor_scale))
        {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!(
                    "Line {} has fields outside the PEC writer subset",
                    line.document_order()
                ),
            ));
        }
        pec_track_events(chart, line.id(), index, profile, negotiation)?;
        let has_note = chart
            .notes()
            .notes()
            .iter()
            .any(|note| note.gameplay().line().value() == line.id().value());
        let has_track = chart
            .tracks()
            .tracks()
            .iter()
            .any(|track| track.owner().value() == line.id().value());
        if index > 0 && !has_note && (!has_track || negotiation.drops(ConversionDomain::Motion)) {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!("PEC cannot encode empty line {index} without a line command"),
            ));
        }
    }
    for note in chart.notes().notes() {
        let presentation = note.presentation();
        let gameplay_unsupported = !note_gameplay_is_external_default(note);
        let presentation_unsupported = presentation.x_offset() != 0.0
            || presentation.y_offset() != 0.0
            || presentation.alpha() != 1.0
            || presentation.scale_x() != presentation.scale_y()
            || presentation.rotation() != 0.0
            || presentation.color() != CanonicalColor::rgba(255, 255, 255, 255)
            || presentation.texture().is_some()
            || !presentation.render_enabled()
            || presentation.visible_from().is_some()
            || presentation.visible_until().is_some();
        if (gameplay_unsupported && !negotiation.drops(ConversionDomain::Gameplay))
            || (presentation_unsupported
                && !negotiation.drops(ConversionDomain::Presentation)
                && !negotiation.approximates(ConversionDomain::Presentation))
        {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!(
                    "Note {} has fields outside the PEC writer subset",
                    note.document_order()
                ),
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn finish_export(
    format: &str,
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    expected_resources: Option<&CanonicalResourceBundle>,
    actual_resources: &CanonicalResourceBundle,
    alignment: &EntityAlignment,
    options: &ExportOptions,
    negotiation: NegotiationPlan,
    mut entries: Vec<ConversionEntry>,
    bytes: Vec<u8>,
) -> Result<ExportOutcome, ExportError> {
    for descriptor in options.capabilities.domains() {
        let observed_bytes = match descriptor.domain() {
            ConversionDomain::Resource => actual_resources
                .resources()
                .values()
                .map(|resource| resource.bytes().len())
                .fold(0usize, usize::saturating_add),
            ConversionDomain::Package => bytes.len(),
            _ => continue,
        };
        let limit_name = if descriptor
            .max_bytes()
            .is_some_and(|limit| observed_bytes > limit)
        {
            Some("max_bytes".to_owned())
        } else if descriptor
            .limit("byte.count")
            .is_some_and(|limit| observed_bytes as f64 > limit)
        {
            Some("byte.count".to_owned())
        } else {
            None
        };
        if let Some(limit_name) = limit_name {
            push_report_entry(
                &mut entries,
                ConversionEntry::new(
                    format!("capability/{}/limit", descriptor.domain()),
                    "conversion.capability-mismatch",
                    descriptor.domain(),
                    ConversionSeverity::Error,
                    SemanticStatus::Unsupported,
                    ConversionPhase::Export,
                    None,
                    None,
                    None,
                    Some(limit_name.clone()),
                    None,
                    None,
                    None,
                    None,
                    None,
                    format!(
                        "target bytes exceed the {} domain byte limit {}",
                        descriptor.domain(),
                        limit_name
                    ),
                    [],
                )
                .map_err(|error| ExportError::new("conversion.report-limit", error.to_string()))?,
            )?;
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!(
                    "target bytes exceed the {} domain byte limit {}",
                    descriptor.domain(),
                    limit_name
                ),
            )
            .with_entries(entries));
        }
    }
    let approximation_output_segments = ConversionDomain::ALL
        .into_iter()
        .filter(|domain| negotiation.approximates(*domain))
        .map(|domain| approximation_segment_count(actual, domain))
        .fold(0usize, usize::saturating_add);
    if approximation_output_segments > options.approximation.maximum_segments() {
        push_report_entry(
            &mut entries,
            ConversionEntry::new(
                "approximation/segment-budget",
                "conversion.approximation-budget-exceeded",
                ConversionDomain::Profile,
                ConversionSeverity::Error,
                SemanticStatus::Unsupported,
                ConversionPhase::ReparseCompare,
                None,
                None,
                None,
                Some("maximumSegments".into()),
                None,
                None,
                None,
                None,
                None,
                format!(
                    "target reparse produced {approximation_output_segments} approximation segments, exceeding maximum {}",
                    options.approximation.maximum_segments()
                ),
                [],
            )
            .map_err(|error| ExportError::new("conversion.report-limit", error.to_string()))?,
        )?;
        return Err(ExportError::new(
            "conversion.approximation-budget-exceeded",
            format!(
                "target approximation produced {approximation_output_segments} segments, exceeding maximum {}",
                options.approximation.maximum_segments()
            ),
        )
        .with_entries(entries));
    }
    let comparison_budgets = negotiated_comparison_budgets(options, &negotiation);
    let dropped_selectors = options
        .drop
        .target_selectors()
        .iter()
        .filter(|selector| {
            negotiation.entries().iter().any(|entry| {
                entry.action() == NegotiationAction::Drop
                    && selector.split('.').next() == Some(entry.domain().as_str())
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let preserved = preserved_selectors(&negotiation);
    let mut ignored_selectors = dropped_selectors.clone();
    ignored_selectors.extend(preserved.iter().cloned());
    let comparison = compare_canonical_charts_with_resources_with_budgets_and_ignored(
        expected,
        actual,
        expected_resources,
        Some(actual_resources),
        &comparison_budgets,
        crate::comparison::ComparisonFilters {
            dropped_selectors: &dropped_selectors,
            ignored_selectors: &ignored_selectors,
            ignored_structural_selectors: &preserved,
        },
        Some(alignment),
    );
    let observed_report_entries = entries
        .len()
        .saturating_add(comparison.observed_mismatch_count());
    if comparison.report_limit_exceeded() || observed_report_entries > MAX_REPORT_ENTRIES {
        return Err(report_limit_error(observed_report_entries, entries));
    }
    let unverified_metrics = comparison_budgets
        .keys()
        .filter(|metric| comparison.verified_maximum_error(metric).is_none())
        .cloned()
        .collect::<Vec<_>>();
    if !unverified_metrics.is_empty() {
        for (index, metric) in unverified_metrics.iter().enumerate() {
            push_report_entry(
                &mut entries,
                ConversionEntry::new(
                    format!("approximation/unverified/{index:06}"),
                    "conversion.approximation-budget-exceeded",
                    conversion_domain_from_str(
                        metric
                            .split_once('.')
                            .map_or(metric.as_str(), |(domain, _)| domain),
                    )?,
                    ConversionSeverity::Error,
                    SemanticStatus::Unsupported,
                    ConversionPhase::ReparseCompare,
                    None,
                    None,
                    None,
                    Some(metric.clone()),
                    None,
                    None,
                    None,
                    None,
                    None,
                    "declared approximation metric was not exercised by canonical comparison",
                    [],
                )
                .map_err(|error| ExportError::new("conversion.report-limit", error.to_string()))?,
            )?;
        }
        return Err(ExportError::new(
            "conversion.approximation-budget-exceeded",
            format!(
                "canonical comparison did not verify declared metrics: {}",
                unverified_metrics.join(", ")
            ),
        )
        .with_entries(entries));
    }
    if !comparison.mismatches().is_empty() {
        for (index, mismatch) in comparison.mismatches().iter().enumerate() {
            let category = if mismatch.error().is_some()
                && comparison_budgets.contains_key(mismatch.metric())
            {
                "conversion.approximation-budget-exceeded"
            } else {
                "conversion.roundtrip-mismatch"
            };
            push_report_entry(
                &mut entries,
                ConversionEntry::new(
                    format!("roundtrip/{index:06}"),
                    category,
                    conversion_domain_from_str(mismatch.domain())?,
                    ConversionSeverity::Error,
                    SemanticStatus::Unsupported,
                    ConversionPhase::ReparseCompare,
                    None,
                    None,
                    None,
                    Some(mismatch.field().into()),
                    None,
                    None,
                    None,
                    None,
                    None,
                    format!(
                        "{} expected {} but reparsed {}",
                        mismatch.metric(),
                        mismatch.expected(),
                        mismatch.actual()
                    ),
                    [],
                )
                .map_err(|error| ExportError::new("conversion.report-limit", error.to_string()))?,
            )?;
        }
        let category = if comparison.mismatches().iter().any(|mismatch| {
            mismatch.error().is_some() && comparison_budgets.contains_key(mismatch.metric())
        }) {
            "conversion.approximation-budget-exceeded"
        } else {
            "conversion.roundtrip-mismatch"
        };
        return Err(ExportError::new(
            category,
            format!(
                "{} canonical fields differ after same-profile reparse",
                comparison.mismatches().len()
            ),
        )
        .with_entries(entries)
        .with_failed_report(format, options, &bytes)
        .map_err(|error| ExportError::new("conversion.report-limit", error.to_string()))?);
    }
    if !comparison.unverified_selectors().is_empty() {
        for (index, selector) in comparison.unverified_selectors().iter().enumerate() {
            let domain = selector.split('.').next().unwrap_or(selector);
            push_report_entry(
                &mut entries,
                ConversionEntry::new(
                    format!("roundtrip/unverified/{index:06}"),
                    "conversion.drop-applied",
                    conversion_domain_from_str(domain)?,
                    ConversionSeverity::Warning,
                    SemanticStatus::Dropped,
                    ConversionPhase::ReparseCompare,
                    None,
                    None,
                    None,
                    Some(selector.clone()),
                    None,
                    None,
                    None,
                    None,
                    None,
                    format!(
                        "same-profile target reparse did not verify {selector} canonical semantics because the selector was authorized for drop"
                    ),
                    [],
                )
                .map_err(|error| ExportError::new("conversion.report-limit", error.to_string()))?,
            )?;
        }
    }
    let forced_boundaries = approximation_forced_boundaries(expected, actual);
    let source_descriptor_hash = source_descriptor_hash(expected);
    for (index, (metric, declared_maximum)) in comparison_budgets.iter().enumerate() {
        let verified_maximum = comparison
            .verified_maximum_error(metric)
            .expect("unverified approximation metrics fail before report construction");
        let verified_sample_count = comparison
            .verified_sample_count(metric)
            .expect("unverified approximation metrics fail before report construction");
        let metric_domain = metric
            .split_once('.')
            .map_or(metric.as_str(), |(domain, _)| domain);
        let verified_output_segments = ConversionDomain::ALL
            .into_iter()
            .find(|domain| domain.as_str() == metric_domain)
            .map_or(0, |domain| approximation_segment_count(actual, domain));
        let error_metric = ErrorMetric::new(
            conversion_domain_from_str(metric_domain)?,
            metric.clone(),
            *declared_maximum,
            verified_maximum,
            "same-profile-canonical-reparse",
            verified_sample_count,
            verified_output_segments as u64,
            forced_boundaries.iter().copied(),
            source_descriptor_hash.clone(),
        )
        .map_err(|error| ExportError::new("conversion.report-limit", error.to_string()))?;
        push_report_entry(
            &mut entries,
            ConversionEntry::new(
                format!("approximation/verified/{index:06}"),
                "conversion.approximation-verified",
                conversion_domain_from_str(metric_domain)?,
                ConversionSeverity::Warning,
                SemanticStatus::Approximated,
                ConversionPhase::ReparseCompare,
                None,
                None,
                None,
                Some(metric.clone()),
                None,
                Some(CanonicalValue::Float(*declared_maximum)),
                None,
                None,
                Some(CanonicalValue::Float(verified_maximum)),
                format!(
                    "same-profile canonical reparse verified maximum absolute error {verified_maximum} against declared maximum {declared_maximum} across {verified_output_segments} {metric_domain} target segments using {}@{}",
                    options.approximation.algorithm_id(),
                    options.approximation.algorithm_version()
                ),
                [],
            )
            .map_err(|error| ExportError::new("conversion.report-limit", error.to_string()))?
            .with_error_metric(error_metric),
        )?;
    }
    let output_hash = lower_hex(Sha256::digest(&bytes));
    let mut status_signals = vec![ConversionStatus::Equivalent];
    for entry in negotiation.entries() {
        match entry.action() {
            NegotiationAction::Bake | NegotiationAction::Drop => {
                status_signals.push(ConversionStatus::Approximate)
            }
            NegotiationAction::Preserve => status_signals.push(ConversionStatus::PreservedOnly),
            _ => {}
        }
    }
    let report = ConversionReport::new_with_authorizations(
        format!("{format}-export-{output_hash}"),
        options.policy,
        options.repair_mode.clone(),
        options
            .approximation
            .enabled()
            .then(|| options.approximation.clone()),
        options.drop.enabled().then(|| options.drop.clone()),
        entries,
        Vec::new(),
        status_signals,
        Some(output_hash),
    )
    .map_err(|error| ExportError::new("conversion.report-limit", error.to_string()))?;
    Ok(ExportOutcome {
        bytes,
        negotiation,
        comparison,
        report,
        distribution: None,
    })
}

fn negotiated_comparison_budgets(
    options: &ExportOptions,
    negotiation: &NegotiationPlan,
) -> BTreeMap<String, f64> {
    options
        .approximation
        .error_budgets()
        .iter()
        .filter(|(metric, _)| {
            ConversionDomain::ALL.into_iter().any(|domain| {
                negotiation.approximates(domain)
                    && (metric.as_str() == domain.as_str()
                        || metric
                            .strip_prefix(domain.as_str())
                            .is_some_and(|suffix| suffix.starts_with('.')))
            })
        })
        .map(|(metric, budget)| (metric.clone(), *budget))
        .collect()
}

fn conversion_domain_from_str(domain: &str) -> Result<ConversionDomain, ExportError> {
    ConversionDomain::parse(domain).ok_or_else(|| {
        ExportError::new(
            "conversion.internal",
            format!("unregistered ConversionReport domain: {domain}"),
        )
    })
}

fn lower_hex(bytes: impl AsRef<[u8]>) -> String {
    let mut output = String::with_capacity(bytes.as_ref().len() * 2);
    for byte in bytes.as_ref() {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn approximation_forced_boundaries(expected: &CanonicalChart, actual: &CanonicalChart) -> Vec<f64> {
    let mut boundaries = vec![0.0];
    for chart in [expected, actual] {
        boundaries.extend(chart.time_map().segments().map(|segment| segment.1));
        for note in chart.notes().notes() {
            boundaries.push(note.gameplay().time().chart_time_seconds());
            if let Some(end_time) = note.gameplay().end_time() {
                boundaries.push(end_time.chart_time_seconds());
            }
        }
        for track in chart.tracks().tracks() {
            for piece in track.pieces() {
                match piece {
                    CanonicalTrackPiece::Segment(segment) => {
                        boundaries.push(segment.start().chart_time_seconds());
                        boundaries.push(segment.end().chart_time_seconds());
                    }
                    CanonicalTrackPiece::Point(point) => {
                        boundaries.push(point.time().chart_time_seconds());
                    }
                }
            }
        }
        for line in chart.scroll().lines() {
            boundaries.push(line.integration_origin());
            boundaries.extend(
                line.coordinate()
                    .points()
                    .iter()
                    .map(|point| point.chart_time()),
            );
        }
    }
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup_by(|left, right| *left == *right);
    boundaries
}

fn source_descriptor_hash(chart: &CanonicalChart) -> String {
    let material = chart.descriptors().map_or_else(
        || "fcs-source-descriptor:none:v1".to_owned(),
        |table| format!("{table:?}"),
    );
    lower_hex(Sha256::digest(material.as_bytes()))
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
