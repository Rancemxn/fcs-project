//! I8 product export and FCS format surfaces.
//!
//! `format_fcs_source` validates source then rewrites UTF-8 without inventing
//! semantics. `export_pgr_v3` emits a formatVersion-3 PGR chart from a product
//! CanonicalChart so target reparse can run through the existing importer.

use std::collections::BTreeMap;
use std::fmt;

use fcs_model::{
    CanonicalChart, CanonicalColor, CanonicalCompilation, CanonicalJudgeShape, CanonicalLine,
    CanonicalLineInherit, CanonicalNoteKind, CanonicalNoteScorePolicy, CanonicalNoteSide,
    CanonicalNoteSoundPolicy, CanonicalTrack, CanonicalTrackBlend, CanonicalTrackFill,
    CanonicalTrackInterpolation, CanonicalTrackPiece, CanonicalTrackTarget, CanonicalTrackValue,
    ConversionDomain, ConversionEntry, ConversionPhase, ConversionPolicy, ConversionReport,
    ConversionSeverity, ConversionStatus, RepairMode, SemanticStatus,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    ApproximationAuthorization, ArtifactRole, CapabilityDescriptor, CapabilityDomain,
    CapabilityDomainDescriptor, DecimalLimits, DropAuthorization, ExactDecimal, PecLimits,
    PecProfile, PecProfileBinding, PgrLimits, PgrProfile, PgrProfileBinding, RpeLimits, RpeProfile,
    RpeProfileBinding, RpeVersionEra, SourceArtifact, SourceFormat,
    compare_canonical_charts_with_budgets, interpret_pec, interpret_pgr, interpret_rpe_semantics,
    lower_pec_to_canonical, lower_pgr_to_canonical, lower_rpe_to_canonical, parse_json_document,
    parse_pec_document, parse_pgr_document, parse_rpe_document,
};

/// Stable formatter / exporter diagnostic category.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportError {
    category: &'static str,
    message: String,
    entries: Vec<ConversionEntry>,
}

#[derive(Default)]
struct PgrLineTracks<'a> {
    position: Option<&'a CanonicalTrack>,
    rotation: Option<&'a CanonicalTrack>,
    alpha: Option<&'a CanonicalTrack>,
    speed: Option<&'a CanonicalTrack>,
}

fn pgr_line_tracks(
    chart: &CanonicalChart,
    owner: u64,
    negotiation: &NegotiationPlan,
) -> Result<PgrLineTracks<'_>, ExportError> {
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
            _ if negotiation.drops(CapabilityDomain::Motion) => continue,
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
        if !negotiation.drops(CapabilityDomain::Motion)
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
        if (gameplay_unsupported && !negotiation.drops(CapabilityDomain::Gameplay))
            || (presentation_unsupported && !negotiation.drops(CapabilityDomain::Presentation))
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
}

/// One domain decision made before target bytes are written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiationEntry {
    domain: CapabilityDomain,
    action: NegotiationAction,
    category: &'static str,
    status: SemanticStatus,
}

impl NegotiationEntry {
    pub const fn domain(&self) -> CapabilityDomain {
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

    pub fn action_for(&self, domain: CapabilityDomain) -> Option<NegotiationAction> {
        self.entries
            .iter()
            .find(|entry| entry.domain == domain)
            .map(|entry| entry.action)
    }

    pub fn drops(&self, domain: CapabilityDomain) -> bool {
        self.action_for(domain) == Some(NegotiationAction::Drop)
    }

    pub fn approximates(&self, domain: CapabilityDomain) -> bool {
        self.action_for(domain) == Some(NegotiationAction::Bake)
    }

    fn has_unsupported(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.action == NegotiationAction::Unsupported)
    }
}

impl ExportError {
    pub fn new(category: &'static str, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
            entries: Vec::new(),
        }
    }

    fn with_entries(mut self, entries: Vec<ConversionEntry>) -> Self {
        self.entries = entries;
        self
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
}

impl fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.category, self.message)
    }
}

impl std::error::Error for ExportError {}

/// Validate FCS source and apply the fixed text policy: LF line endings, no
/// trailing horizontal whitespace, no trailing blank lines, one final LF.
pub fn format_fcs_source(source: &str) -> Result<String, ExportError> {
    fcs_source::parser::parse_document(source)
        .into_result()
        .map_err(|diagnostics| {
            let message = diagnostics
                .first()
                .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
                .unwrap_or_else(|| "source invalid".into());
            ExportError::new("source.invalid", message)
        })?;
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<_> = normalized
        .split('\n')
        .map(|line| line.trim_end_matches(|character| matches!(character, ' ' | '\t')))
        .collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    let formatted = format!("{}\n", lines.join("\n"));
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
    let outcome = export_pgr_with_options(compilation.chart(), options)?;
    record_compilation_roundtrip_context(outcome, compilation, options)
}

/// Export PGR v1 or v3 according to the explicit target profile.
pub fn export_pgr_with_options(
    chart: &CanonicalChart,
    options: &ExportOptions,
) -> Result<ExportOutcome, ExportError> {
    let profile = selected_pgr_profile(options)?;
    if !profile.strict_eligible() {
        return Err(ExportError::new(
            "conversion.profile-not-applicable",
            "the selected PGR profile is source-only and cannot be an export target",
        ));
    }
    let (negotiation, entries) = negotiate_export_with_options(chart, options)?;
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
    for line in ordered_lines {
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
            let position_x = if negotiation.drops(CapabilityDomain::Presentation) {
                0.0
            } else {
                note.presentation().position_x() / 108.0
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
                "speed": if negotiation.drops(CapabilityDomain::Presentation) {
                    1.0
                } else {
                    note.presentation().scroll_factor()
                },
                "floorPosition": floor_position
            });
            match note.gameplay().side() {
                CanonicalNoteSide::Above => notes_above.push(payload),
                CanonicalNoteSide::Below => notes_below.push(payload),
            }
        }
        let (move_events, rotate_events, alpha_events) =
            if negotiation.drops(CapabilityDomain::Motion) {
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
    finish_export(
        "pgr",
        chart,
        reparsed.compilation().chart(),
        options,
        negotiation,
        entries,
        bytes,
    )
}

/// Import → export → re-import a PGR chart and compare line/note counts.
pub fn roundtrip_pgr_v3_public_bytes(
    bytes: &[u8],
) -> Result<(usize, usize, usize, usize), ExportError> {
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
    let exported = export_pgr_v3(first.compilation().chart())?;
    let artifact2 = SourceArtifact::new("chart-reexport.json", ArtifactRole::Chart, exported)
        .map_err(|error| ExportError::new("conversion.source-invalid", error.to_string()))?;
    let parsed2 = parse_json_document(SourceFormat::Pgr, &artifact2)
        .map_err(|error| ExportError::new("conversion.source-invalid", error.to_string()))?;
    let source2 = parse_pgr_document(&parsed2, PgrLimits::default())
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let semantic2 = interpret_pgr(&source2, &binding)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let second = lower_pgr_to_canonical(&semantic2, &artifact2)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let first_chart = first.compilation().chart();
    let second_chart = second.compilation().chart();
    Ok((
        first_chart.lines().lines().count(),
        first_chart.notes().notes().len(),
        second_chart.lines().lines().count(),
        second_chart.notes().notes().len(),
    ))
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
            tracks: false,
            expressions: false,
            resources: false,
        }
    }

    pub fn descriptor(&self, profile: Option<String>) -> CapabilityDescriptor {
        let exact = |domain, supported| {
            CapabilityDomainDescriptor::new(
                domain, supported, false, false, false, false, None, None,
            )
        };
        CapabilityDescriptor::new(
            self.format,
            self.version,
            profile,
            vec![
                exact(CapabilityDomain::Timing, self.time),
                exact(CapabilityDomain::Gameplay, self.notes),
                exact(CapabilityDomain::Motion, self.tracks),
                exact(CapabilityDomain::Scroll, self.time),
                exact(CapabilityDomain::Presentation, self.notes),
                exact(CapabilityDomain::Resource, self.resources),
                exact(CapabilityDomain::Metadata, false),
                exact(CapabilityDomain::Numeric, true),
                exact(CapabilityDomain::Entity, true),
                exact(CapabilityDomain::Limits, true),
                exact(CapabilityDomain::Expression, self.expressions),
                exact(CapabilityDomain::Package, self.resources),
            ],
        )
        .expect("static compatibility capability descriptor")
    }
}

/// Build a deterministic per-domain plan and its report entries before writing.
pub fn negotiate_export_with_options(
    chart: &CanonicalChart,
    options: &ExportOptions,
) -> Result<(NegotiationPlan, Vec<ConversionEntry>), ExportError> {
    if options.policy == ConversionPolicy::Strict
        && options.target_profile.as_deref().is_none_or(str::is_empty)
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
    for domain in CapabilityDomain::ALL {
        let descriptor = options.capabilities.domain(domain);
        let needed = match domain {
            CapabilityDomain::Timing => true,
            CapabilityDomain::Gameplay => !chart.notes().notes().is_empty(),
            CapabilityDomain::Motion => !chart.tracks().tracks().is_empty(),
            CapabilityDomain::Scroll => !chart.scroll().lines().is_empty(),
            CapabilityDomain::Presentation => !chart.notes().notes().is_empty(),
            CapabilityDomain::Resource => {
                !chart.metadata().resources().is_empty()
                    || chart.metadata().sync().is_some_and(|sync| {
                        sync.primary_audio().is_some() || sync.preview().is_some()
                    })
            }
            CapabilityDomain::Metadata => {
                chart.metadata().meta().is_some()
                    || !chart.metadata().contributors().is_empty()
                    || !chart.metadata().credits().is_empty()
                    || chart.metadata().artwork().is_some()
            }
            CapabilityDomain::Numeric => true,
            CapabilityDomain::Entity => chart.lines().lines().next().is_some(),
            CapabilityDomain::Limits => true,
            CapabilityDomain::Expression => {
                chart.descriptors().is_some() || !chart.required_extensions().is_empty()
            }
            CapabilityDomain::Package => {
                !chart.metadata().resources().is_empty()
                    || chart.metadata().sync().is_some_and(|sync| {
                        sync.primary_audio().is_some() || sync.preview().is_some()
                    })
            }
        };
        if !needed {
            continue;
        }
        let limit_exceeded = descriptor
            .and_then(CapabilityDomainDescriptor::max_entities)
            .is_some_and(|limit| capability_entity_count(chart, domain) > limit);
        let requested_approximation_segments = descriptor
            .filter(|descriptor| {
                descriptor.approximation() && options.approximation.allows(domain.as_str())
            })
            .map_or(0, |_| approximation_segment_count(chart, domain));
        let action = match descriptor {
            _ if limit_exceeded => NegotiationAction::Unsupported,
            Some(descriptor) if descriptor.exact() => NegotiationAction::Direct,
            Some(descriptor) if descriptor.equivalent() => NegotiationAction::Equivalent,
            Some(descriptor)
                if descriptor.approximation() && options.approximation.allows(domain.as_str()) =>
            {
                NegotiationAction::Bake
            }
            Some(descriptor) if descriptor.preserve() => NegotiationAction::Preserve,
            Some(descriptor) if descriptor.drop() && options.drop.allows(domain.as_str()) => {
                NegotiationAction::Drop
            }
            _ => NegotiationAction::Unsupported,
        };
        let category = match action {
            NegotiationAction::Unsupported if limit_exceeded => "conversion.capability-mismatch",
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
        entries.push(
            ConversionEntry::new(
                format!("capability/{}", domain.as_str()),
                category,
                conversion_domain(domain),
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
                Some(domain.as_str().into()),
                None,
                None,
                None,
                None,
                None,
                negotiation_message(domain, action, options, requested_approximation_segments),
                [],
            )
            .map_err(|error| ExportError::new("conversion.report", error.to_string()))?,
        );
    }
    let plan = NegotiationPlan { entries: plan };
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

fn capability_entity_count(chart: &CanonicalChart, domain: CapabilityDomain) -> usize {
    match domain {
        CapabilityDomain::Timing => chart.time_map().segments().count(),
        CapabilityDomain::Gameplay | CapabilityDomain::Presentation => chart.notes().notes().len(),
        CapabilityDomain::Motion => chart.tracks().tracks().len(),
        CapabilityDomain::Scroll => chart.scroll().lines().len(),
        CapabilityDomain::Resource | CapabilityDomain::Package => {
            chart.metadata().resources().len()
        }
        CapabilityDomain::Metadata => {
            chart.metadata().meta().map_or(0, BTreeMap::len)
                + chart.metadata().contributors().len()
                + chart.metadata().credits().len()
                + usize::from(chart.metadata().artwork().is_some())
        }
        CapabilityDomain::Entity => {
            chart.lines().lines().count()
                + chart.notes().notes().len()
                + chart.tracks().tracks().len()
        }
        CapabilityDomain::Expression => {
            chart
                .descriptors()
                .map_or(0, |table| table.descriptors().len())
                + chart.required_extensions().len()
        }
        CapabilityDomain::Numeric | CapabilityDomain::Limits => 0,
    }
}

fn approximation_segment_count(chart: &CanonicalChart, domain: CapabilityDomain) -> usize {
    match domain {
        CapabilityDomain::Timing => chart.time_map().segments().count(),
        CapabilityDomain::Gameplay | CapabilityDomain::Presentation => chart.notes().notes().len(),
        CapabilityDomain::Motion => chart
            .tracks()
            .tracks()
            .iter()
            .map(|track| track.pieces().len())
            .sum(),
        CapabilityDomain::Scroll => chart
            .scroll()
            .lines()
            .iter()
            .map(|line| line.coordinate().points().len())
            .sum(),
        CapabilityDomain::Resource | CapabilityDomain::Package => {
            chart.metadata().resources().len()
        }
        CapabilityDomain::Metadata => capability_entity_count(chart, domain),
        CapabilityDomain::Numeric | CapabilityDomain::Limits => 0,
        CapabilityDomain::Entity => capability_entity_count(chart, domain),
        CapabilityDomain::Expression => capability_entity_count(chart, domain),
    }
}

fn negotiation_message(
    domain: CapabilityDomain,
    action: NegotiationAction,
    options: &ExportOptions,
    approximation_segments: usize,
) -> String {
    match action {
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

fn conversion_domain(domain: CapabilityDomain) -> ConversionDomain {
    match domain {
        CapabilityDomain::Timing => ConversionDomain::Timing,
        CapabilityDomain::Gameplay => ConversionDomain::Gameplay,
        CapabilityDomain::Motion => ConversionDomain::Motion,
        CapabilityDomain::Scroll => ConversionDomain::Scroll,
        CapabilityDomain::Presentation => ConversionDomain::Presentation,
        CapabilityDomain::Resource => ConversionDomain::Resource,
        CapabilityDomain::Metadata => ConversionDomain::Metadata,
        CapabilityDomain::Numeric | CapabilityDomain::Entity | CapabilityDomain::Expression => {
            ConversionDomain::Profile
        }
        CapabilityDomain::Limits | CapabilityDomain::Package => ConversionDomain::Package,
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
    let (profile, binding, rpe_version) = selected_rpe_binding(options)?;
    let (negotiation, entries) = negotiate_export_with_options(chart, options)?;
    require_rpe_chart_shape(chart, &negotiation)?;
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
    for line in &ordered_lines {
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
            let presentation_dropped = negotiation.drops(CapabilityDomain::Presentation);
            let raw_speed = if presentation_dropped {
                4.5
            } else {
                note.presentation().scroll_factor() * 4.5
            };
            let mut payload = json!({
                "type": note_type,
                "startTime": start,
                "endTime": end,
                "positionX": if presentation_dropped {
                    0.0
                } else {
                    note.presentation().position_x()
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
        }
        let father = line
            .parent()
            .and_then(|parent| {
                ordered_lines
                    .iter()
                    .position(|candidate| candidate.id().value() == parent.value())
            })
            .map_or(-1, |index| index as i64);
        judge_lines.push(json!({
            "bpmfactor": 1,
            "eventLayers": [],
            "notes": notes,
            "father": father,
            "rotateWithFather": line.inherit().rotation()
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
    finish_export(
        "rpe",
        chart,
        reparsed.compilation().chart(),
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
    let outcome = export_rpe_json_with_options(compilation.chart(), options)?;
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
    let profile = selected_pec_profile(options)?;
    if !profile.strict_eligible() {
        return Err(ExportError::new(
            "conversion.profile-not-applicable",
            "the selected PEC profile is source-only and cannot be an export target",
        ));
    }
    let (negotiation, entries) = negotiate_export_with_options(chart, options)?;
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
    if ordered_lines.len() > 1 {
        lines.push_str(&format!("cp {} 0 1024 700\n", ordered_lines.len() - 1));
    }
    let mut notes: Vec<_> = chart.notes().notes().iter().collect();
    notes.sort_by_key(|note| note.document_order());
    for note in notes {
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
        let presentation_dropped = negotiation.drops(CapabilityDomain::Presentation);
        let x = if presentation_dropped {
            0.0
        } else {
            note.presentation().position_x() * 16.0 / 15.0
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
                note.presentation().scroll_factor()
            },
            "PEC Note scroll factor",
        )?;
        let scale = finite_decimal(
            if presentation_dropped {
                1.0
            } else {
                note.presentation().scale_x()
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
    finish_export(
        "pec",
        chart,
        reparsed.compilation().chart(),
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
    let outcome = export_pec_line_with_options(compilation.chart(), options)?;
    record_compilation_roundtrip_context(outcome, compilation, options)
}

fn seconds_to_rpe_beat(beats: f64) -> [i64; 3] {
    const DENOMINATOR: i64 = 1_000_000_000;
    let whole = beats.floor() as i64;
    let numerator = ((beats - whole as f64) * DENOMINATOR as f64).round() as i64;
    [whole, numerator, DENOMINATOR]
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
    entries.push(
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
        .map_err(|error| ExportError::new("conversion.report", error.to_string()))?,
    );
    let operation_id = outcome.report.operation_id().to_owned();
    let conversion_policy = outcome.report.conversion_policy();
    let repair_mode = outcome.report.repair_mode().clone();
    let repairs = outcome.report.repairs().to_vec();
    let status = outcome.report.status();
    let output_hash = outcome.report.output_hash().map(str::to_owned);
    outcome.report = ConversionReport::new(
        operation_id,
        conversion_policy,
        repair_mode,
        entries,
        repairs,
        [status],
        output_hash,
    )
    .map_err(|error| ExportError::new("conversion.report", error.to_string()))?;
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
    match profile {
        "rpe.phira.legacy-speed@1.0.0" => Ok((
            RpeProfile::PhiraLegacySpeed,
            RpeProfileBinding::phira_legacy_speed(),
            150,
        )),
        "rpe.phira.rpe170-speed@1.0.0" => Ok((
            RpeProfile::PhiraRpe170Speed,
            RpeProfileBinding::phira_rpe170_speed(Some(RpeVersionEra::AtLeast170)),
            170,
        )),
        "rpe.community.divide-bpmfactor@1.0.0" | "rpe.docs-example.multiply-bpmfactor@1.0.0" => {
            Err(ExportError::new(
                "conversion.profile-parameter-invalid",
                "this RPE target profile requires an explicit speedMode binding",
            ))
        }
        "rpe.phichain-import@1.0.0" => Err(ExportError::new(
            "conversion.profile-not-applicable",
            "rpe.phichain-import is source-only and cannot be an export target",
        )),
        _ => Err(ExportError::new(
            "conversion.profile-not-found",
            format!("unknown RPE target profile {profile}"),
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
    if has_metadata && !negotiation.drops(CapabilityDomain::Metadata) {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("{format} writer cannot represent canonical metadata"),
        ));
    }

    let has_resources = !metadata.resources().is_empty()
        || metadata
            .sync()
            .is_some_and(|sync| sync.primary_audio().is_some() || sync.preview().is_some());
    if has_resources
        && !(negotiation.drops(CapabilityDomain::Resource)
            && negotiation.drops(CapabilityDomain::Package))
    {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("{format} writer cannot represent canonical resources or package bindings"),
        ));
    }

    if (chart.descriptors().is_some() || !chart.required_extensions().is_empty())
        && !negotiation.drops(CapabilityDomain::Expression)
    {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!("{format} writer cannot represent descriptors or required extensions"),
        ));
    }
    Ok(())
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
) -> Result<(), ExportError> {
    require_external_payload_losses(chart, negotiation, "RPE")?;
    if !chart.tracks().tracks().is_empty() && !negotiation.drops(CapabilityDomain::Motion) {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            "RPE writer cannot represent canonical Tracks",
        ));
    }
    for line in chart.lines().lines() {
        if !negotiation.drops(CapabilityDomain::Motion)
            && (!line_base_is_default(line, 1.0)
                || *line.inherit()
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
        if (gameplay_unsupported && !negotiation.drops(CapabilityDomain::Gameplay))
            || (presentation_unsupported && !negotiation.drops(CapabilityDomain::Presentation))
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
    if !chart.tracks().tracks().is_empty() && !negotiation.drops(CapabilityDomain::Motion) {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            "PEC writer cannot represent canonical Tracks",
        ));
    }
    let floor_scale = options.floor_scale_px.to_f64().map_err(|error| {
        ExportError::new("conversion.profile-parameter-invalid", error.to_string())
    })?;
    let mut lines: Vec<_> = chart.lines().lines().collect();
    lines.sort_by_key(|line| line.document_order());
    for (index, line) in lines.iter().enumerate() {
        if !negotiation.drops(CapabilityDomain::Motion)
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
        if (gameplay_unsupported && !negotiation.drops(CapabilityDomain::Gameplay))
            || (presentation_unsupported && !negotiation.drops(CapabilityDomain::Presentation))
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
    options: &ExportOptions,
    negotiation: NegotiationPlan,
    mut entries: Vec<ConversionEntry>,
    bytes: Vec<u8>,
) -> Result<ExportOutcome, ExportError> {
    for descriptor in options.capabilities.domains() {
        if descriptor
            .max_bytes()
            .is_some_and(|limit| bytes.len() > limit)
        {
            return Err(ExportError::new(
                "conversion.capability-mismatch",
                format!(
                    "target bytes exceed the {} domain byte limit",
                    descriptor.domain()
                ),
            )
            .with_entries(entries));
        }
    }
    let approximation_output_segments = CapabilityDomain::ALL
        .into_iter()
        .filter(|domain| negotiation.approximates(*domain))
        .map(|domain| approximation_segment_count(actual, domain))
        .fold(0usize, usize::saturating_add);
    if approximation_output_segments > options.approximation.maximum_segments() {
        entries.push(
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
            .map_err(|error| ExportError::new("conversion.report", error.to_string()))?,
        );
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
    let dropped_domains = negotiation
        .entries()
        .iter()
        .filter(|entry| entry.action() == NegotiationAction::Drop)
        .map(|entry| entry.domain().as_str().to_owned())
        .collect::<Vec<_>>();
    let comparison = compare_canonical_charts_with_budgets(
        expected,
        actual,
        &comparison_budgets,
        &dropped_domains,
    );
    if !comparison.is_equivalent() {
        for (index, mismatch) in comparison.mismatches().iter().enumerate() {
            let category = if mismatch.error().is_some()
                && comparison_budgets.contains_key(mismatch.metric())
            {
                "conversion.approximation-budget-exceeded"
            } else {
                "conversion.roundtrip-mismatch"
            };
            entries.push(
                ConversionEntry::new(
                    format!("roundtrip/{index:06}"),
                    category,
                    conversion_domain_from_str(mismatch.domain()),
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
                .map_err(|error| ExportError::new("conversion.report", error.to_string()))?,
            );
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
        .with_entries(entries));
    }
    entries.push(
        ConversionEntry::new(
            "roundtrip/equivalent",
            "conversion.capability-negotiated",
            ConversionDomain::Profile,
            ConversionSeverity::Info,
            SemanticStatus::Equivalent,
            ConversionPhase::ReparseCompare,
            None,
            None,
            None,
            Some("canonical".into()),
            None,
            None,
            None,
            None,
            None,
            "same-profile target reparse is canonically equivalent",
            [],
        )
        .map_err(|error| ExportError::new("conversion.report", error.to_string()))?,
    );
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
    let report = ConversionReport::new(
        format!("{format}-export-{output_hash}"),
        options.policy,
        options.repair_mode.clone(),
        entries,
        Vec::new(),
        status_signals,
        Some(output_hash),
    )
    .map_err(|error| ExportError::new("conversion.report", error.to_string()))?;
    Ok(ExportOutcome {
        bytes,
        negotiation,
        comparison,
        report,
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
            CapabilityDomain::ALL.into_iter().any(|domain| {
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

fn conversion_domain_from_str(domain: &str) -> ConversionDomain {
    match domain {
        "timing" => ConversionDomain::Timing,
        "gameplay" => ConversionDomain::Gameplay,
        "motion" => ConversionDomain::Motion,
        "scroll" => ConversionDomain::Scroll,
        "presentation" => ConversionDomain::Presentation,
        "resource" => ConversionDomain::Resource,
        "metadata" => ConversionDomain::Metadata,
        "package" => ConversionDomain::Package,
        _ => ConversionDomain::Profile,
    }
}

fn lower_hex(bytes: impl AsRef<[u8]>) -> String {
    let mut output = String::with_capacity(bytes.as_ref().len() * 2);
    for byte in bytes.as_ref() {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use fcs_model::{
        CanonicalChart, CanonicalMetadata, CanonicalObject, CanonicalResourceBundle,
        CanonicalSourceVersion, CanonicalValue, DistributionMetadata, OriginState, ProvenanceGraph,
        RestrictedProvenanceFact,
    };
    use std::fs;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn pgr_chart(name: &str, profile: PgrProfile) -> CanonicalChart {
        let bytes = fs::read(root().join(format!(
            "docs/conformance/conversion/public-fixtures/sources/{name}"
        )))
        .unwrap();
        let artifact = SourceArtifact::new(name, ArtifactRole::Chart, bytes).unwrap();
        let parsed = parse_json_document(SourceFormat::Pgr, &artifact).unwrap();
        let source = parse_pgr_document(&parsed, PgrLimits::default()).unwrap();
        let floor = ExactDecimal::parse("120", DecimalLimits::default()).unwrap();
        let binding = PgrProfileBinding::new(profile, floor).unwrap();
        let semantic = interpret_pgr(&source, &binding).unwrap();
        lower_pgr_to_canonical(&semantic, &artifact)
            .unwrap()
            .compilation()
            .chart()
            .clone()
    }

    fn rpe_chart() -> CanonicalChart {
        let name = "rpe-minimal.rpe.json";
        let bytes = fs::read(root().join(format!(
            "docs/conformance/conversion/public-fixtures/sources/{name}"
        )))
        .unwrap();
        let artifact = SourceArtifact::new(name, ArtifactRole::Chart, bytes).unwrap();
        let parsed = parse_json_document(SourceFormat::Rpe, &artifact).unwrap();
        let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
        let binding = RpeProfileBinding::phira_legacy_speed();
        let semantic = interpret_rpe_semantics(&source, &binding).unwrap();
        lower_rpe_to_canonical(&semantic, &artifact)
            .unwrap()
            .compilation()
            .chart()
            .clone()
    }

    fn pec_chart() -> CanonicalChart {
        let name = "pec-minimal.pec";
        let bytes = fs::read(root().join(format!(
            "docs/conformance/conversion/public-fixtures/sources/{name}"
        )))
        .unwrap();
        let artifact = SourceArtifact::new(name, ArtifactRole::Chart, bytes).unwrap();
        let source = parse_pec_document(&artifact, PecLimits::default()).unwrap();
        let floor = ExactDecimal::parse("120", DecimalLimits::default()).unwrap();
        let binding = PecProfileBinding::new(PecProfile::Phira, floor).unwrap();
        let semantic = interpret_pec(&source, &binding).unwrap();
        lower_pec_to_canonical(&semantic, &artifact)
            .unwrap()
            .compilation()
            .chart()
            .clone()
    }

    fn profile_options(set: CapabilitySet, id: &str, version: &str) -> ExportOptions {
        ExportOptions::semantic(set.descriptor(Some(profile_reference(id, version))))
    }

    fn loss_descriptor(profile: &str, approximation: bool, drop: bool) -> CapabilityDescriptor {
        CapabilityDescriptor::new(
            "pec",
            "line-command",
            Some(profile.into()),
            CapabilityDomain::ALL
                .map(|domain| {
                    if domain == CapabilityDomain::Motion {
                        CapabilityDomainDescriptor::new(
                            domain,
                            false,
                            false,
                            approximation,
                            false,
                            drop,
                            None,
                            None,
                        )
                    } else {
                        CapabilityDomainDescriptor::new(
                            domain, true, false, false, false, false, None, None,
                        )
                    }
                })
                .into(),
        )
        .unwrap()
    }

    #[allow(clippy::too_many_arguments)]
    fn descriptor_with_domain(
        set: CapabilitySet,
        profile: &str,
        target: CapabilityDomain,
        exact: bool,
        approximation: bool,
        drop: bool,
        max_entities: Option<usize>,
        max_bytes: Option<usize>,
    ) -> CapabilityDescriptor {
        let base = set.descriptor(Some(profile.into()));
        let domains = base
            .domains()
            .iter()
            .map(|descriptor| {
                if descriptor.domain() == target {
                    CapabilityDomainDescriptor::new(
                        target,
                        exact,
                        false,
                        approximation,
                        false,
                        drop,
                        max_entities,
                        max_bytes,
                    )
                } else {
                    descriptor.clone()
                }
            })
            .collect();
        CapabilityDescriptor::new(
            base.format(),
            base.version(),
            base.profile().map(str::to_owned),
            domains,
        )
        .unwrap()
    }

    fn with_source_version(chart: &CanonicalChart, version: &str) -> CanonicalChart {
        let mut changed = CanonicalChart::new(
            CanonicalSourceVersion::new(version).unwrap(),
            chart.profile(),
            chart.features().iter().copied(),
            chart.time_map().clone(),
            chart.metadata().clone(),
            chart.lines().clone(),
            chart.notes().clone(),
            chart.tracks().clone(),
            chart.scroll().clone(),
            chart.required_extensions().iter().cloned(),
        );
        if let Some(descriptors) = chart.descriptors() {
            changed = changed.with_descriptors(descriptors.clone());
        }
        changed
    }

    fn with_metadata_fact(chart: &CanonicalChart) -> CanonicalChart {
        let mut meta = BTreeMap::new();
        meta.insert(
            "title".into(),
            CanonicalValue::String("dropped title".into()),
        );
        let metadata = CanonicalMetadata::new(
            Some(meta),
            chart.metadata().contributors().clone(),
            chart.metadata().credits().to_vec(),
            chart.metadata().resources().clone(),
            chart.metadata().artwork().cloned(),
            chart.metadata().sync().cloned(),
        );
        let mut changed = CanonicalChart::new(
            chart.source_version().clone(),
            chart.profile(),
            chart.features().iter().copied(),
            chart.time_map().clone(),
            metadata,
            chart.lines().clone(),
            chart.notes().clone(),
            chart.tracks().clone(),
            chart.scroll().clone(),
            chart.required_extensions().iter().cloned(),
        );
        if let Some(descriptors) = chart.descriptors() {
            changed = changed.with_descriptors(descriptors.clone());
        }
        changed
    }

    fn compilation_with_stale_roundtrip_fact(chart: &CanonicalChart) -> CanonicalCompilation {
        let root = RestrictedProvenanceFact::new(
            "canonical-edit",
            None,
            None,
            Some("old canonical value".into()),
            Some(0),
            None,
            OriginState::Imported,
            Some(SemanticStatus::Mapped),
            std::iter::empty(),
        )
        .unwrap();
        let dependent = RestrictedProvenanceFact::new(
            "source-roundtrip-handle",
            None,
            None,
            Some("stale source representation".into()),
            Some(1),
            None,
            OriginState::Imported,
            Some(SemanticStatus::Preserved),
            ["canonical-edit".into()],
        )
        .unwrap();
        let mut provenance = ProvenanceGraph::new([root, dependent]).unwrap();
        let stale = provenance
            .mark_user_modified_and_stale_dependents("canonical-edit")
            .unwrap();
        assert_eq!(
            stale.into_iter().collect::<Vec<_>>(),
            vec!["source-roundtrip-handle".to_owned()]
        );
        let distribution = DistributionMetadata::new(
            provenance,
            Vec::new(),
            Vec::new(),
            CanonicalObject::new(Vec::new()).unwrap(),
        )
        .unwrap();
        CanonicalCompilation::new(
            chart.clone(),
            CanonicalResourceBundle::new(Vec::new()).unwrap(),
            distribution,
        )
    }

    #[test]
    fn formatter_applies_one_idempotent_text_policy() {
        let source =
            fs::read_to_string(root().join("docs/conformance/fcs5/source/valid/minimal-chart.fcs"))
                .unwrap();
        let noisy = source.replace('\n', "  \r\n");
        let formatted = format_fcs_source(&noisy).unwrap();
        assert_eq!(formatted, format_fcs_source(&formatted).unwrap());
        assert!(!formatted.contains('\r'));
        assert!(formatted.ends_with('\n'));
        assert!(
            !formatted
                .lines()
                .any(|line| line.ends_with(' ') || line.ends_with('\t'))
        );
    }

    #[test]
    fn format_fcs_source_rejects_invalid() {
        let error = format_fcs_source("not a chart").unwrap_err();
        assert_eq!(error.category(), "source.invalid");
    }

    #[test]
    fn public_pgr_feature_fixture_roundtrips_through_export() {
        let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
        let options = profile_options(
            CapabilitySet::pgr_v3(),
            PgrProfile::PhiraV3.id(),
            PgrProfile::PhiraV3.version(),
        );
        let outcome = export_pgr_v3_with_options(&chart, &options).unwrap();
        assert!(outcome.comparison().is_equivalent());
        assert_eq!(outcome.report().status(), ConversionStatus::Equivalent);
        assert!(outcome.report().output_hash().is_some());
    }

    #[test]
    fn pgr_v1_uses_the_selected_packed_coordinate_profile() {
        let chart = pgr_chart("pgr-minimal.pgr.json", PgrProfile::PhiraV1);
        let options = profile_options(
            CapabilitySet::pgr_v1(),
            PgrProfile::PhiraV1.id(),
            PgrProfile::PhiraV1.version(),
        );
        assert!(
            export_pgr_with_options(&chart, &options)
                .unwrap()
                .comparison()
                .is_equivalent()
        );
    }

    #[test]
    fn rpe_and_pec_export_reparse_compare_full_canonical_semantics() {
        let rpe = rpe_chart();
        let rpe_options = profile_options(
            CapabilitySet::rpe_json(),
            RpeProfile::PhiraLegacySpeed.id(),
            RpeProfile::PhiraLegacySpeed.version(),
        );
        assert!(
            export_rpe_json_with_options(&rpe, &rpe_options)
                .unwrap()
                .comparison()
                .is_equivalent()
        );

        let pec = pec_chart();
        let pec_options = profile_options(
            CapabilitySet::pec_line(),
            PecProfile::Phira.id(),
            PecProfile::Phira.version(),
        );
        assert!(
            export_pec_line_with_options(&pec, &pec_options)
                .unwrap()
                .comparison()
                .is_equivalent()
        );
    }

    #[test]
    fn strict_profile_choice_is_not_repair() {
        let chart = pec_chart();
        let descriptor = CapabilitySet::pec_line().descriptor(Some(profile_reference(
            PecProfile::Phira.id(),
            PecProfile::Phira.version(),
        )));
        let options = ExportOptions::strict(descriptor)
            .with_repair_mode(RepairMode::new(true, std::iter::empty()));
        let error = negotiate_export_with_options(&chart, &options).unwrap_err();
        assert_eq!(error.category(), "conversion.target-profile-required");
    }

    #[test]
    fn approximation_and_drop_need_independent_typed_authorization() {
        let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
        let profile = profile_reference(PecProfile::Phira.id(), PecProfile::Phira.version());

        let approximation = ExportOptions::semantic(loss_descriptor(&profile, true, false));
        let error = negotiate_export_with_options(&chart, &approximation).unwrap_err();
        assert_eq!(error.category(), "conversion.approximation-not-authorized");
        let authorization = ApproximationAuthorization::new(
            ["motion".into()],
            [("motion.track_value".into(), 0.001)],
            1024,
            "linear-segment",
            "1.0.0",
        )
        .unwrap();
        let (plan, _) =
            negotiate_export_with_options(&chart, &approximation.with_approximation(authorization))
                .unwrap();
        assert_eq!(plan.action(), NegotiationAction::Bake);

        let drop = ExportOptions::semantic(loss_descriptor(&profile, false, true));
        let error = negotiate_export_with_options(&chart, &drop).unwrap_err();
        assert_eq!(error.category(), "conversion.drop-not-authorized");
        let authorization =
            DropAuthorization::new(["motion".into()], "explicit target loss").unwrap();
        let (plan, _) =
            negotiate_export_with_options(&chart, &drop.with_drop(authorization)).unwrap();
        assert_eq!(plan.action(), NegotiationAction::Drop);
    }

    #[test]
    fn approximation_segment_limit_is_a_hard_reparse_budget() {
        let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
        let profile = profile_reference(PgrProfile::PhiraV3.id(), PgrProfile::PhiraV3.version());
        let descriptor = descriptor_with_domain(
            CapabilitySet::pgr_v3(),
            &profile,
            CapabilityDomain::Presentation,
            false,
            true,
            false,
            None,
            None,
        );
        let authorization = ApproximationAuthorization::new(
            ["presentation".into()],
            [("presentation.value".into(), 0.001)],
            1,
            "linear-segment",
            "1.0.0",
        )
        .unwrap();
        let error = export_pgr_v3_with_options(
            &chart,
            &ExportOptions::semantic(descriptor).with_approximation(authorization),
        )
        .unwrap_err();
        assert_eq!(error.category(), "conversion.approximation-budget-exceeded");
    }

    #[test]
    fn authorized_metadata_drop_is_applied_by_the_writer_and_reported() {
        let chart = with_metadata_fact(&rpe_chart());
        let profile = profile_reference(
            RpeProfile::PhiraLegacySpeed.id(),
            RpeProfile::PhiraLegacySpeed.version(),
        );
        let descriptor = descriptor_with_domain(
            CapabilitySet::rpe_json(),
            &profile,
            CapabilityDomain::Metadata,
            false,
            false,
            true,
            None,
            None,
        );
        let authorization =
            DropAuthorization::new(["metadata".into()], "remove target-inexpressible metadata")
                .unwrap();
        let outcome = export_rpe_json_with_options(
            &chart,
            &ExportOptions::semantic(descriptor).with_drop(authorization),
        )
        .unwrap();
        assert!(outcome.negotiation().drops(CapabilityDomain::Metadata));
        assert_eq!(outcome.report().status(), ConversionStatus::Approximate);
        assert_eq!(outcome.report().summary().drop_count(), 1);
    }

    #[test]
    fn unused_drop_authorization_cannot_mask_a_direct_roundtrip_mismatch() {
        let chart = with_source_version(&rpe_chart(), "5.0.1");
        let options = profile_options(
            CapabilitySet::rpe_json(),
            RpeProfile::PhiraLegacySpeed.id(),
            RpeProfile::PhiraLegacySpeed.version(),
        )
        .with_drop(DropAuthorization::new(["entity".into()], "not negotiated").unwrap());
        let error = export_rpe_json_with_options(&chart, &options).unwrap_err();
        assert_eq!(error.category(), "conversion.roundtrip-mismatch");
    }

    #[test]
    fn roundtrip_policy_rebuilds_from_canonical_when_source_fidelity_is_stale() {
        let chart = rpe_chart();
        let compilation = compilation_with_stale_roundtrip_fact(&chart);
        let mut options = profile_options(
            CapabilitySet::rpe_json(),
            RpeProfile::PhiraLegacySpeed.id(),
            RpeProfile::PhiraLegacySpeed.version(),
        );
        options.policy = ConversionPolicy::Roundtrip;
        let outcome = export_rpe_compilation_with_options(&compilation, &options).unwrap();
        assert!(outcome.comparison().is_equivalent());
        assert_eq!(outcome.report().status(), ConversionStatus::Equivalent);
        assert!(outcome.report().entries().iter().any(|entry| {
            entry.id() == "roundtrip/stale-source-representation"
                && entry.category() == "conversion.tool-rewrite"
        }));
    }

    #[test]
    fn serialized_target_bytes_obey_the_declared_hard_limit() {
        let chart = rpe_chart();
        let profile = profile_reference(
            RpeProfile::PhiraLegacySpeed.id(),
            RpeProfile::PhiraLegacySpeed.version(),
        );
        let descriptor = descriptor_with_domain(
            CapabilitySet::rpe_json(),
            &profile,
            CapabilityDomain::Limits,
            true,
            false,
            false,
            None,
            Some(1),
        );
        let error =
            export_rpe_json_with_options(&chart, &ExportOptions::semantic(descriptor)).unwrap_err();
        assert_eq!(error.category(), "conversion.capability-mismatch");
        assert!(error.message().contains("byte limit"));
    }

    #[test]
    fn unsupported_required_domain_fails_before_target_write() {
        let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
        let profile = profile_reference(PecProfile::Phira.id(), PecProfile::Phira.version());
        let options = ExportOptions::semantic(loss_descriptor(&profile, false, false));
        let error = negotiate_export_with_options(&chart, &options).unwrap_err();
        assert_eq!(error.category(), "conversion.capability-mismatch");
        assert!(
            error
                .entries()
                .iter()
                .any(|entry| entry.semantic_status() == SemanticStatus::Unsupported)
        );
    }

    #[test]
    fn negotiation_report_order_is_deterministic() {
        let chart = pgr_chart("pgr-feature.pgr.json", PgrProfile::PhiraV3);
        let options = profile_options(
            CapabilitySet::pgr_v3(),
            PgrProfile::PhiraV3.id(),
            PgrProfile::PhiraV3.version(),
        );
        let (_, first) = negotiate_export_with_options(&chart, &options).unwrap();
        let (_, second) = negotiate_export_with_options(&chart, &options).unwrap();
        assert_eq!(
            first.iter().map(ConversionEntry::id).collect::<Vec<_>>(),
            second.iter().map(ConversionEntry::id).collect::<Vec<_>>()
        );
        assert!(
            first
                .iter()
                .all(|entry| entry.category() == "conversion.capability-negotiated")
        );
    }

    #[test]
    fn successful_writer_fails_if_same_profile_reparse_changes_canonical_identity() {
        let chart = with_source_version(&rpe_chart(), "5.0.1");
        let options = profile_options(
            CapabilitySet::rpe_json(),
            RpeProfile::PhiraLegacySpeed.id(),
            RpeProfile::PhiraLegacySpeed.version(),
        );
        let error = export_rpe_json_with_options(&chart, &options).unwrap_err();
        assert_eq!(error.category(), "conversion.roundtrip-mismatch");
        assert!(
            error
                .entries()
                .iter()
                .any(|entry| entry.category() == "conversion.roundtrip-mismatch")
        );
    }
}
