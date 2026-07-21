//! Typed RPE source parsing and profile-bound exact timing (I6.3a).
//!
//! Event-curve, speed-distance, parent graph, Note presentation, and canonical
//! assembly remain later I6.3 units. This module preserves those fields as typed
//! source data without evaluating them.

// Retained optional source fields are intentionally unread until I6.3b/I6.3c.
#![allow(dead_code)]

use std::fmt;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

use crate::{
    DecimalLimits, ExactDecimal, ExactNumberError, ExactRational, LogicalSourceLocator,
    LosslessJsonMember, LosslessJsonString, LosslessJsonValue, ParsedSourceDocument, SourceFormat,
};

pub const SOURCE_INVALID: &str = "conversion.source-invalid";
pub const PROFILE_PARAMETER_INVALID: &str = "conversion.profile-parameter-invalid";
pub const PROFILE_NOT_APPLICABLE: &str = "conversion.profile-not-applicable";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RpeLimits {
    pub decimal: DecimalLimits,
    pub max_bpm_points: usize,
    pub max_lines: usize,
    pub max_layers_per_line: usize,
    pub max_events_per_layer_field: usize,
    pub max_notes_per_line: usize,
}

impl Default for RpeLimits {
    fn default() -> Self {
        Self {
            decimal: DecimalLimits::default(),
            max_bpm_points: 65_536,
            max_lines: 4096,
            max_layers_per_line: 256,
            max_events_per_layer_field: 262_144,
            max_notes_per_line: 262_144,
        }
    }
}

/// Raw `META.RPEVersion` evidence: JSON type and spelling, never a invented default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpeVersionEvidence {
    Number { raw: String, value: ExactDecimal },
    String(LosslessJsonString),
}

impl RpeVersionEvidence {
    pub const fn is_number(&self) -> bool {
        matches!(self, Self::Number { .. })
    }

    pub const fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    pub fn raw_spelling(&self) -> &str {
        match self {
            Self::Number { raw, .. } => raw,
            Self::String(value) => value.raw(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeBeat {
    whole: ExactDecimal,
    numerator: ExactDecimal,
    denominator: ExactDecimal,
}

impl RpeBeat {
    pub fn whole(&self) -> &ExactDecimal {
        &self.whole
    }

    pub fn numerator(&self) -> &ExactDecimal {
        &self.numerator
    }

    pub fn denominator(&self) -> &ExactDecimal {
        &self.denominator
    }

    pub fn components(&self) -> [&ExactDecimal; 3] {
        [&self.whole, &self.numerator, &self.denominator]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSourceMeta {
    rpe_version: Option<RpeVersionEvidence>,
    offset: ExactDecimal,
    name: Option<LosslessJsonString>,
    level: Option<LosslessJsonString>,
    charter: Option<LosslessJsonString>,
    composer: Option<LosslessJsonString>,
    illustration: Option<LosslessJsonString>,
    song: Option<LosslessJsonString>,
    background: Option<LosslessJsonString>,
    id: Option<LosslessJsonString>,
    unknown_fields: Vec<LosslessJsonMember>,
}

impl RpeSourceMeta {
    pub fn rpe_version(&self) -> Option<&RpeVersionEvidence> {
        self.rpe_version.as_ref()
    }

    pub fn offset(&self) -> &ExactDecimal {
        &self.offset
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_ref().map(LosslessJsonString::value)
    }

    pub fn song(&self) -> Option<&str> {
        self.song.as_ref().map(LosslessJsonString::value)
    }

    pub fn background(&self) -> Option<&str> {
        self.background.as_ref().map(LosslessJsonString::value)
    }

    pub fn unknown_fields(&self) -> &[LosslessJsonMember] {
        &self.unknown_fields
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSourceBpmPoint {
    start_time: RpeBeat,
    bpm: ExactDecimal,
    unknown_fields: Vec<LosslessJsonMember>,
}

impl RpeSourceBpmPoint {
    pub fn start_time(&self) -> &RpeBeat {
        &self.start_time
    }

    pub fn bpm(&self) -> &ExactDecimal {
        &self.bpm
    }

    pub fn unknown_fields(&self) -> &[LosslessJsonMember] {
        &self.unknown_fields
    }
}

/// Presence model for `eventLayers`: missing, JSON null, or sparse slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpeEventLayersField {
    Missing,
    Null,
    Present(Vec<RpeEventLayerSlot>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpeEventLayerSlot {
    Null,
    Layer(RpeSourceEventLayer),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpeOptionalEventList {
    Missing,
    Null,
    Present(Vec<RpeSourceCommonEvent>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpeOptionalSpeedList {
    Missing,
    Null,
    Present(Vec<RpeSourceSpeedEvent>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSourceCommonEvent {
    start_time: RpeBeat,
    end_time: RpeBeat,
    start: ExactDecimal,
    end: ExactDecimal,
    easing_type: Option<ExactDecimal>,
    easing_left: Option<ExactDecimal>,
    easing_right: Option<ExactDecimal>,
    bezier: Option<ExactDecimal>,
    bezier_points: Option<Vec<ExactDecimal>>,
    linkgroup: Option<ExactDecimal>,
    unknown_fields: Vec<LosslessJsonMember>,
}

impl RpeSourceCommonEvent {
    pub fn start_time(&self) -> &RpeBeat {
        &self.start_time
    }

    pub fn end_time(&self) -> &RpeBeat {
        &self.end_time
    }

    pub fn start(&self) -> &ExactDecimal {
        &self.start
    }

    pub fn end(&self) -> &ExactDecimal {
        &self.end
    }

    pub fn easing_type(&self) -> Option<&ExactDecimal> {
        self.easing_type.as_ref()
    }

    pub fn unknown_fields(&self) -> &[LosslessJsonMember] {
        &self.unknown_fields
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSourceSpeedEvent {
    start_time: RpeBeat,
    end_time: RpeBeat,
    start: ExactDecimal,
    end: ExactDecimal,
    easing_type: Option<ExactDecimal>,
    easing_left: Option<ExactDecimal>,
    easing_right: Option<ExactDecimal>,
    bezier: Option<ExactDecimal>,
    bezier_points: Option<Vec<ExactDecimal>>,
    linkgroup: Option<ExactDecimal>,
    unknown_fields: Vec<LosslessJsonMember>,
}

impl RpeSourceSpeedEvent {
    pub fn start_time(&self) -> &RpeBeat {
        &self.start_time
    }

    pub fn end_time(&self) -> &RpeBeat {
        &self.end_time
    }

    pub fn start(&self) -> &ExactDecimal {
        &self.start
    }

    pub fn end(&self) -> &ExactDecimal {
        &self.end
    }

    pub fn unknown_fields(&self) -> &[LosslessJsonMember] {
        &self.unknown_fields
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSourceEventLayer {
    move_x_events: RpeOptionalEventList,
    move_y_events: RpeOptionalEventList,
    rotate_events: RpeOptionalEventList,
    alpha_events: RpeOptionalEventList,
    speed_events: RpeOptionalSpeedList,
    unknown_fields: Vec<LosslessJsonMember>,
}

impl RpeSourceEventLayer {
    pub fn move_x_events(&self) -> &RpeOptionalEventList {
        &self.move_x_events
    }

    pub fn move_y_events(&self) -> &RpeOptionalEventList {
        &self.move_y_events
    }

    pub fn rotate_events(&self) -> &RpeOptionalEventList {
        &self.rotate_events
    }

    pub fn alpha_events(&self) -> &RpeOptionalEventList {
        &self.alpha_events
    }

    pub fn speed_events(&self) -> &RpeOptionalSpeedList {
        &self.speed_events
    }

    pub fn unknown_fields(&self) -> &[LosslessJsonMember] {
        &self.unknown_fields
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSourceNote {
    kind: ExactDecimal,
    start_time: RpeBeat,
    end_time: RpeBeat,
    position_x: ExactDecimal,
    speed: ExactDecimal,
    above: Option<ExactDecimal>,
    is_fake: Option<ExactDecimal>,
    alpha: Option<ExactDecimal>,
    size: Option<ExactDecimal>,
    visible_time: Option<ExactDecimal>,
    y_offset: Option<ExactDecimal>,
    hitsound: Option<LosslessJsonString>,
    unknown_fields: Vec<LosslessJsonMember>,
}

impl RpeSourceNote {
    pub fn kind(&self) -> &ExactDecimal {
        &self.kind
    }

    pub fn start_time(&self) -> &RpeBeat {
        &self.start_time
    }

    pub fn end_time(&self) -> &RpeBeat {
        &self.end_time
    }

    pub fn position_x(&self) -> &ExactDecimal {
        &self.position_x
    }

    pub fn speed(&self) -> &ExactDecimal {
        &self.speed
    }

    pub fn above(&self) -> Option<&ExactDecimal> {
        self.above.as_ref()
    }

    pub fn is_fake(&self) -> Option<&ExactDecimal> {
        self.is_fake.as_ref()
    }

    pub fn hitsound(&self) -> Option<&str> {
        self.hitsound.as_ref().map(LosslessJsonString::value)
    }

    pub fn unknown_fields(&self) -> &[LosslessJsonMember] {
        &self.unknown_fields
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSourceLine {
    bpmfactor: Option<ExactDecimal>,
    event_layers: RpeEventLayersField,
    notes: Vec<RpeSourceNote>,
    father: Option<ExactDecimal>,
    rotate_with_father: Option<bool>,
    texture: Option<LosslessJsonString>,
    name: Option<LosslessJsonString>,
    z_order: Option<ExactDecimal>,
    is_cover: Option<ExactDecimal>,
    attach_ui: Option<LosslessJsonValue>,
    is_gif: Option<bool>,
    extended: Option<LosslessJsonValue>,
    unknown_fields: Vec<LosslessJsonMember>,
}

impl RpeSourceLine {
    pub fn bpmfactor(&self) -> Option<&ExactDecimal> {
        self.bpmfactor.as_ref()
    }

    pub fn event_layers(&self) -> &RpeEventLayersField {
        &self.event_layers
    }

    pub fn notes(&self) -> &[RpeSourceNote] {
        &self.notes
    }

    pub fn father(&self) -> Option<&ExactDecimal> {
        self.father.as_ref()
    }

    pub fn rotate_with_father(&self) -> Option<bool> {
        self.rotate_with_father
    }

    pub fn texture(&self) -> Option<&str> {
        self.texture.as_ref().map(LosslessJsonString::value)
    }

    pub fn unknown_fields(&self) -> &[LosslessJsonMember] {
        &self.unknown_fields
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSourceDocument {
    artifact_id: LogicalSourceLocator,
    artifact_content_sha256: [u8; 32],
    meta: RpeSourceMeta,
    bpm_list: Vec<RpeSourceBpmPoint>,
    lines: Vec<RpeSourceLine>,
    unknown_fields: Vec<LosslessJsonMember>,
}

impl RpeSourceDocument {
    pub fn artifact_id(&self) -> &LogicalSourceLocator {
        &self.artifact_id
    }

    pub(crate) const fn artifact_content_sha256(&self) -> [u8; 32] {
        self.artifact_content_sha256
    }

    pub fn meta(&self) -> &RpeSourceMeta {
        &self.meta
    }

    pub fn bpm_list(&self) -> &[RpeSourceBpmPoint] {
        &self.bpm_list
    }

    pub fn lines(&self) -> &[RpeSourceLine] {
        &self.lines
    }

    pub fn unknown_fields(&self) -> &[LosslessJsonMember] {
        &self.unknown_fields
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RpeProfile {
    CommunityDivideBpmfactor,
    DocsExampleMultiplyBpmfactor,
    PhichainImport,
    PhiraLegacySpeed,
    PhiraRpe170Speed,
}

impl RpeProfile {
    pub const fn id(self) -> &'static str {
        match self {
            Self::CommunityDivideBpmfactor => "rpe.community.divide-bpmfactor",
            Self::DocsExampleMultiplyBpmfactor => "rpe.docs-example.multiply-bpmfactor",
            Self::PhichainImport => "rpe.phichain-import",
            Self::PhiraLegacySpeed => "rpe.phira.legacy-speed",
            Self::PhiraRpe170Speed => "rpe.phira.rpe170-speed",
        }
    }

    pub const fn version(self) -> &'static str {
        "1.0.0"
    }

    pub const fn factor_mode(self) -> RpeFactorMode {
        match self {
            Self::CommunityDivideBpmfactor => RpeFactorMode::Divide,
            Self::DocsExampleMultiplyBpmfactor => RpeFactorMode::Multiply,
            Self::PhichainImport | Self::PhiraLegacySpeed | Self::PhiraRpe170Speed => {
                RpeFactorMode::Ignore
            }
        }
    }

    pub const fn accepts_zero_zero_integer_beat(self) -> bool {
        matches!(self, Self::PhichainImport)
    }

    pub const fn missing_rotate_with_father_default(self) -> bool {
        matches!(
            self,
            Self::DocsExampleMultiplyBpmfactor | Self::PhichainImport
        )
    }

    pub const fn requires_speed_mode(self) -> bool {
        matches!(
            self,
            Self::CommunityDivideBpmfactor | Self::DocsExampleMultiplyBpmfactor
        )
    }

    pub const fn requires_rpe_version_era_when_absent(self) -> bool {
        matches!(self, Self::PhiraRpe170Speed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RpeFactorMode {
    Divide,
    Multiply,
    Ignore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RpeSpeedMode {
    LegacyLinear,
    LegacyDerivative,
    ModernEased,
}

impl RpeSpeedMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LegacyLinear => "legacy-linear",
            Self::LegacyDerivative => "legacy-derivative",
            Self::ModernEased => "modern-eased",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RpeVersionEra {
    Pre170,
    AtLeast170,
}

impl RpeVersionEra {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pre170 => "pre170",
            Self::AtLeast170 => "at-least-170",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeProfileBinding {
    profile: RpeProfile,
    speed_mode: Option<RpeSpeedMode>,
    rpe_version_era: Option<RpeVersionEra>,
}

impl RpeProfileBinding {
    pub fn community_divide(speed_mode: RpeSpeedMode) -> Self {
        Self {
            profile: RpeProfile::CommunityDivideBpmfactor,
            speed_mode: Some(speed_mode),
            rpe_version_era: None,
        }
    }

    pub fn docs_example_multiply(speed_mode: RpeSpeedMode) -> Self {
        Self {
            profile: RpeProfile::DocsExampleMultiplyBpmfactor,
            speed_mode: Some(speed_mode),
            rpe_version_era: None,
        }
    }

    pub fn phichain_import() -> Self {
        Self {
            profile: RpeProfile::PhichainImport,
            speed_mode: None,
            rpe_version_era: None,
        }
    }

    pub fn phira_legacy_speed() -> Self {
        Self {
            profile: RpeProfile::PhiraLegacySpeed,
            speed_mode: None,
            rpe_version_era: None,
        }
    }

    pub fn phira_rpe170_speed(rpe_version_era: Option<RpeVersionEra>) -> Self {
        Self {
            profile: RpeProfile::PhiraRpe170Speed,
            speed_mode: None,
            rpe_version_era,
        }
    }

    pub const fn profile(&self) -> RpeProfile {
        self.profile
    }

    pub const fn speed_mode(&self) -> Option<RpeSpeedMode> {
        self.speed_mode
    }

    pub const fn rpe_version_era(&self) -> Option<RpeVersionEra> {
        self.rpe_version_era
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSemanticTime {
    source_beat: ExactRational,
    chart_time_seconds: ExactRational,
}

impl RpeSemanticTime {
    pub fn source_beat(&self) -> &ExactRational {
        &self.source_beat
    }

    pub fn chart_time_seconds(&self) -> &ExactRational {
        &self.chart_time_seconds
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSemanticBpmPoint {
    start_beat: ExactRational,
    bpm: ExactRational,
}

impl RpeSemanticBpmPoint {
    pub fn start_beat(&self) -> &ExactRational {
        &self.start_beat
    }

    pub fn bpm(&self) -> &ExactRational {
        &self.bpm
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSemanticCommonEvent {
    start_time: RpeSemanticTime,
    end_time: RpeSemanticTime,
    start: ExactRational,
    end: ExactRational,
}

impl RpeSemanticCommonEvent {
    pub fn start_time(&self) -> &RpeSemanticTime {
        &self.start_time
    }

    pub fn end_time(&self) -> &RpeSemanticTime {
        &self.end_time
    }

    pub fn start(&self) -> &ExactRational {
        &self.start
    }

    pub fn end(&self) -> &ExactRational {
        &self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSemanticSpeedEvent {
    start_time: RpeSemanticTime,
    end_time: RpeSemanticTime,
    start: ExactRational,
    end: ExactRational,
}

impl RpeSemanticSpeedEvent {
    pub fn start_time(&self) -> &RpeSemanticTime {
        &self.start_time
    }

    pub fn end_time(&self) -> &RpeSemanticTime {
        &self.end_time
    }

    pub fn start(&self) -> &ExactRational {
        &self.start
    }

    pub fn end(&self) -> &ExactRational {
        &self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSemanticEventLayer {
    move_x_events: Vec<RpeSemanticCommonEvent>,
    move_y_events: Vec<RpeSemanticCommonEvent>,
    rotate_events: Vec<RpeSemanticCommonEvent>,
    alpha_events: Vec<RpeSemanticCommonEvent>,
    speed_events: Vec<RpeSemanticSpeedEvent>,
}

impl RpeSemanticEventLayer {
    pub fn move_x_events(&self) -> &[RpeSemanticCommonEvent] {
        &self.move_x_events
    }

    pub fn move_y_events(&self) -> &[RpeSemanticCommonEvent] {
        &self.move_y_events
    }

    pub fn rotate_events(&self) -> &[RpeSemanticCommonEvent] {
        &self.rotate_events
    }

    pub fn alpha_events(&self) -> &[RpeSemanticCommonEvent] {
        &self.alpha_events
    }

    pub fn speed_events(&self) -> &[RpeSemanticSpeedEvent] {
        &self.speed_events
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSemanticNote {
    kind: ExactRational,
    start_time: RpeSemanticTime,
    end_time: RpeSemanticTime,
    position_x: ExactRational,
    speed: ExactRational,
}

impl RpeSemanticNote {
    pub fn kind(&self) -> &ExactRational {
        &self.kind
    }

    pub fn start_time(&self) -> &RpeSemanticTime {
        &self.start_time
    }

    pub fn end_time(&self) -> &RpeSemanticTime {
        &self.end_time
    }

    pub fn position_x(&self) -> &ExactRational {
        &self.position_x
    }

    pub fn speed(&self) -> &ExactRational {
        &self.speed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSemanticLine {
    bpmfactor: ExactRational,
    raw_bpmfactor: Option<ExactRational>,
    rotate_with_father: bool,
    rotate_with_father_was_present: bool,
    event_layers: Vec<Option<RpeSemanticEventLayer>>,
    notes: Vec<RpeSemanticNote>,
}

impl RpeSemanticLine {
    pub fn bpmfactor(&self) -> &ExactRational {
        &self.bpmfactor
    }

    pub fn raw_bpmfactor(&self) -> Option<&ExactRational> {
        self.raw_bpmfactor.as_ref()
    }

    pub const fn rotate_with_father(&self) -> bool {
        self.rotate_with_father
    }

    pub const fn rotate_with_father_was_present(&self) -> bool {
        self.rotate_with_father_was_present
    }

    pub fn event_layers(&self) -> &[Option<RpeSemanticEventLayer>] {
        &self.event_layers
    }

    pub fn notes(&self) -> &[RpeSemanticNote] {
        &self.notes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeSemanticDocument {
    artifact_id: LogicalSourceLocator,
    artifact_content_sha256: [u8; 32],
    profile: RpeProfile,
    speed_mode: Option<RpeSpeedMode>,
    rpe_version_era: Option<RpeVersionEra>,
    audio_offset_milliseconds: ExactRational,
    bpm_points: Vec<RpeSemanticBpmPoint>,
    lines: Vec<RpeSemanticLine>,
}

impl RpeSemanticDocument {
    pub fn artifact_id(&self) -> &LogicalSourceLocator {
        &self.artifact_id
    }

    pub(crate) const fn artifact_content_sha256(&self) -> [u8; 32] {
        self.artifact_content_sha256
    }

    pub const fn profile(&self) -> RpeProfile {
        self.profile
    }

    pub const fn speed_mode(&self) -> Option<RpeSpeedMode> {
        self.speed_mode
    }

    pub const fn rpe_version_era(&self) -> Option<RpeVersionEra> {
        self.rpe_version_era
    }

    pub fn audio_offset_milliseconds(&self) -> &ExactRational {
        &self.audio_offset_milliseconds
    }

    pub fn bpm_points(&self) -> &[RpeSemanticBpmPoint] {
        &self.bpm_points
    }

    pub fn lines(&self) -> &[RpeSemanticLine] {
        &self.lines
    }
}

pub fn parse_rpe_document(
    document: &ParsedSourceDocument,
    limits: RpeLimits,
) -> Result<RpeSourceDocument, RpeError> {
    if document.format() != SourceFormat::Rpe {
        return Err(RpeError::new(
            "conversion.unsupported-format",
            "$",
            "typed RPE parsing requires an RPE ParsedSourceDocument",
        ));
    }
    let root = object(document.body(), "$")?;
    let meta = parse_meta(required(root, "META", "$")?, limits)?;
    let bpm_values = array(required(root, "BPMList", "$")?, "$.BPMList")?;
    enforce_count(
        "$.BPMList",
        "max_bpm_points",
        bpm_values.len(),
        limits.max_bpm_points,
    )?;
    let mut bpm_list = Vec::with_capacity(bpm_values.len());
    for (index, value) in bpm_values.iter().enumerate() {
        bpm_list.push(parse_bpm_point(value, index, limits)?);
    }
    let line_values = array(required(root, "judgeLineList", "$")?, "$.judgeLineList")?;
    enforce_count(
        "$.judgeLineList",
        "max_lines",
        line_values.len(),
        limits.max_lines,
    )?;
    let mut lines = Vec::with_capacity(line_values.len());
    for (index, value) in line_values.iter().enumerate() {
        lines.push(parse_line(value, index, limits)?);
    }
    Ok(RpeSourceDocument {
        artifact_id: document.artifact_id().clone(),
        artifact_content_sha256: document.artifact_content_sha256(),
        meta,
        bpm_list,
        lines,
        unknown_fields: unknown(root, &["META", "BPMList", "judgeLineList"]),
    })
}

pub fn interpret_rpe_timing(
    source: &RpeSourceDocument,
    binding: &RpeProfileBinding,
) -> Result<RpeSemanticDocument, RpeError> {
    validate_binding(source, binding)?;
    let profile = binding.profile();
    if source.bpm_list.is_empty() {
        return Err(RpeError::new(
            SOURCE_INVALID,
            "$.BPMList",
            "RPE requires at least one BPMList point",
        ));
    }

    let mut bpm_points = Vec::with_capacity(source.bpm_list.len());
    let mut previous_beat: Option<ExactRational> = None;
    for (index, point) in source.bpm_list.iter().enumerate() {
        let path = format!("$.BPMList[{index}]");
        let start_beat = resolve_beat(&point.start_time, profile, &format!("{path}.startTime"))?;
        validate_positive_finite(point.bpm.exact(), &format!("{path}.bpm"), "BPM")?;
        if let Some(previous) = &previous_beat {
            if start_beat.value() < previous.value() {
                return Err(RpeError::new(
                    SOURCE_INVALID,
                    format!("{path}.startTime"),
                    "BPMList startTime must be non-decreasing in source order",
                ));
            }
        }
        previous_beat = Some(start_beat.clone());
        bpm_points.push(RpeSemanticBpmPoint {
            start_beat,
            bpm: point.bpm.exact().clone(),
        });
    }

    let mut lines = Vec::with_capacity(source.lines.len());
    for (index, line) in source.lines.iter().enumerate() {
        lines.push(interpret_line(
            line,
            &format!("$.judgeLineList[{index}]"),
            profile,
            &bpm_points,
        )?);
    }

    Ok(RpeSemanticDocument {
        artifact_id: source.artifact_id.clone(),
        artifact_content_sha256: source.artifact_content_sha256(),
        profile,
        speed_mode: binding.speed_mode,
        rpe_version_era: resolved_version_era(source, binding)?,
        audio_offset_milliseconds: source.meta.offset.exact().clone(),
        bpm_points,
        lines,
    })
}

/// Map a beat delta under one BPM segment using the profile factor rule.
pub fn chart_time_delta_seconds(
    beat_delta: &ExactRational,
    list_bpm: &ExactRational,
    bpmfactor: &ExactRational,
    mode: RpeFactorMode,
) -> Result<ExactRational, RpeError> {
    if !beat_delta.is_nonnegative() {
        return Err(RpeError::new(
            SOURCE_INVALID,
            "beat_delta",
            "beat delta must not be negative",
        ));
    }
    validate_positive_finite(list_bpm, "bpm", "BPM")?;
    validate_positive_finite(bpmfactor, "bpmfactor", "bpmfactor")?;
    let sixty = integer(60);
    let seconds = match mode {
        RpeFactorMode::Divide => beat_delta.value() * &sixty * bpmfactor.value() / list_bpm.value(),
        RpeFactorMode::Multiply => {
            beat_delta.value() * &sixty / (list_bpm.value() * bpmfactor.value())
        }
        RpeFactorMode::Ignore => beat_delta.value() * &sixty / list_bpm.value(),
    };
    Ok(ExactRational(seconds))
}

pub fn resolve_beat(
    beat: &RpeBeat,
    profile: RpeProfile,
    path: &str,
) -> Result<ExactRational, RpeError> {
    let whole = require_integer(&beat.whole, &format!("{path}[0]"), "Beat whole")?;
    let numerator = require_integer(&beat.numerator, &format!("{path}[1]"), "Beat numerator")?;
    let denominator =
        require_integer(&beat.denominator, &format!("{path}[2]"), "Beat denominator")?;
    if denominator.is_zero() {
        if profile.accepts_zero_zero_integer_beat() && numerator.is_zero() && whole.is_integer() {
            return Ok(ExactRational::from_integer(
                whole.to_i64().map(BigInt::from).ok_or_else(|| {
                    RpeError::new(SOURCE_INVALID, path, "Beat whole is out of integer range")
                })?,
            ));
        }
        return Err(RpeError::new(
            SOURCE_INVALID,
            path,
            "RPE Beat denominator must be greater than zero for this profile",
        ));
    }
    if denominator.is_negative() {
        return Err(RpeError::new(
            SOURCE_INVALID,
            path,
            "RPE Beat denominator must be positive",
        ));
    }
    let whole_i = whole
        .to_i64()
        .ok_or_else(|| RpeError::new(SOURCE_INVALID, path, "Beat whole is out of integer range"))?;
    let num_i = numerator.to_i64().ok_or_else(|| {
        RpeError::new(
            SOURCE_INVALID,
            path,
            "Beat numerator is out of integer range",
        )
    })?;
    let den_i = denominator.to_i64().ok_or_else(|| {
        RpeError::new(
            SOURCE_INVALID,
            path,
            "Beat denominator is out of integer range",
        )
    })?;
    Ok(ExactRational(
        BigRational::from_integer(BigInt::from(whole_i))
            + BigRational::new(BigInt::from(num_i), BigInt::from(den_i)),
    ))
}

fn interpret_line(
    line: &RpeSourceLine,
    path: &str,
    profile: RpeProfile,
    bpm_points: &[RpeSemanticBpmPoint],
) -> Result<RpeSemanticLine, RpeError> {
    let (bpmfactor, raw_bpmfactor) = match &line.bpmfactor {
        Some(value) => {
            validate_positive_finite(value.exact(), &format!("{path}.bpmfactor"), "bpmfactor")?;
            (value.exact().clone(), Some(value.exact().clone()))
        }
        None => (ExactRational::from_integer(BigInt::one()), None),
    };
    let rotate_with_father_was_present = line.rotate_with_father.is_some();
    let rotate_with_father = line
        .rotate_with_father
        .unwrap_or_else(|| profile.missing_rotate_with_father_default());

    let event_layers = match &line.event_layers {
        RpeEventLayersField::Missing | RpeEventLayersField::Null => Vec::new(),
        RpeEventLayersField::Present(slots) => slots
            .iter()
            .enumerate()
            .map(|(index, slot)| match slot {
                RpeEventLayerSlot::Null => Ok(None),
                RpeEventLayerSlot::Layer(layer) => Ok(Some(interpret_event_layer(
                    layer,
                    &format!("{path}.eventLayers[{index}]"),
                    profile,
                    bpm_points,
                    &bpmfactor,
                )?)),
            })
            .collect::<Result<Vec<_>, RpeError>>()?,
    };

    let notes = line
        .notes
        .iter()
        .enumerate()
        .map(|(index, note)| {
            let note_path = format!("{path}.notes[{index}]");
            Ok(RpeSemanticNote {
                kind: note.kind.exact().clone(),
                start_time: semantic_time(
                    &note.start_time,
                    profile,
                    bpm_points,
                    &bpmfactor,
                    &format!("{note_path}.startTime"),
                )?,
                end_time: semantic_time(
                    &note.end_time,
                    profile,
                    bpm_points,
                    &bpmfactor,
                    &format!("{note_path}.endTime"),
                )?,
                position_x: note.position_x.exact().clone(),
                speed: note.speed.exact().clone(),
            })
        })
        .collect::<Result<Vec<_>, RpeError>>()?;

    Ok(RpeSemanticLine {
        bpmfactor,
        raw_bpmfactor,
        rotate_with_father,
        rotate_with_father_was_present,
        event_layers,
        notes,
    })
}

fn interpret_event_layer(
    layer: &RpeSourceEventLayer,
    path: &str,
    profile: RpeProfile,
    bpm_points: &[RpeSemanticBpmPoint],
    bpmfactor: &ExactRational,
) -> Result<RpeSemanticEventLayer, RpeError> {
    Ok(RpeSemanticEventLayer {
        move_x_events: interpret_common_list(
            &layer.move_x_events,
            &format!("{path}.moveXEvents"),
            profile,
            bpm_points,
            bpmfactor,
        )?,
        move_y_events: interpret_common_list(
            &layer.move_y_events,
            &format!("{path}.moveYEvents"),
            profile,
            bpm_points,
            bpmfactor,
        )?,
        rotate_events: interpret_common_list(
            &layer.rotate_events,
            &format!("{path}.rotateEvents"),
            profile,
            bpm_points,
            bpmfactor,
        )?,
        alpha_events: interpret_common_list(
            &layer.alpha_events,
            &format!("{path}.alphaEvents"),
            profile,
            bpm_points,
            bpmfactor,
        )?,
        speed_events: interpret_speed_list(
            &layer.speed_events,
            &format!("{path}.speedEvents"),
            profile,
            bpm_points,
            bpmfactor,
        )?,
    })
}

fn interpret_common_list(
    list: &RpeOptionalEventList,
    path: &str,
    profile: RpeProfile,
    bpm_points: &[RpeSemanticBpmPoint],
    bpmfactor: &ExactRational,
) -> Result<Vec<RpeSemanticCommonEvent>, RpeError> {
    match list {
        RpeOptionalEventList::Missing | RpeOptionalEventList::Null => Ok(Vec::new()),
        RpeOptionalEventList::Present(events) => events
            .iter()
            .enumerate()
            .map(|(index, event)| {
                let event_path = format!("{path}[{index}]");
                Ok(RpeSemanticCommonEvent {
                    start_time: semantic_time(
                        &event.start_time,
                        profile,
                        bpm_points,
                        bpmfactor,
                        &format!("{event_path}.startTime"),
                    )?,
                    end_time: semantic_time(
                        &event.end_time,
                        profile,
                        bpm_points,
                        bpmfactor,
                        &format!("{event_path}.endTime"),
                    )?,
                    start: event.start.exact().clone(),
                    end: event.end.exact().clone(),
                })
            })
            .collect(),
    }
}

fn interpret_speed_list(
    list: &RpeOptionalSpeedList,
    path: &str,
    profile: RpeProfile,
    bpm_points: &[RpeSemanticBpmPoint],
    bpmfactor: &ExactRational,
) -> Result<Vec<RpeSemanticSpeedEvent>, RpeError> {
    match list {
        RpeOptionalSpeedList::Missing | RpeOptionalSpeedList::Null => Ok(Vec::new()),
        RpeOptionalSpeedList::Present(events) => events
            .iter()
            .enumerate()
            .map(|(index, event)| {
                let event_path = format!("{path}[{index}]");
                Ok(RpeSemanticSpeedEvent {
                    start_time: semantic_time(
                        &event.start_time,
                        profile,
                        bpm_points,
                        bpmfactor,
                        &format!("{event_path}.startTime"),
                    )?,
                    end_time: semantic_time(
                        &event.end_time,
                        profile,
                        bpm_points,
                        bpmfactor,
                        &format!("{event_path}.endTime"),
                    )?,
                    start: event.start.exact().clone(),
                    end: event.end.exact().clone(),
                })
            })
            .collect(),
    }
}

fn semantic_time(
    beat: &RpeBeat,
    profile: RpeProfile,
    bpm_points: &[RpeSemanticBpmPoint],
    bpmfactor: &ExactRational,
    path: &str,
) -> Result<RpeSemanticTime, RpeError> {
    let source_beat = resolve_beat(beat, profile, path)?;
    let chart_time_seconds =
        chart_time_at(&source_beat, bpm_points, bpmfactor, profile.factor_mode())?;
    Ok(RpeSemanticTime {
        source_beat,
        chart_time_seconds,
    })
}

fn chart_time_at(
    target: &ExactRational,
    bpm_points: &[RpeSemanticBpmPoint],
    bpmfactor: &ExactRational,
    mode: RpeFactorMode,
) -> Result<ExactRational, RpeError> {
    // Collapse same-beat points so the final source-order BPM at that beat is active.
    let mut segments: Vec<(ExactRational, ExactRational)> = Vec::new();
    for point in bpm_points {
        if let Some((beat, bpm)) = segments.last_mut() {
            if beat == &point.start_beat {
                *bpm = point.bpm.clone();
                continue;
            }
        }
        segments.push((point.start_beat.clone(), point.bpm.clone()));
    }

    let mut time = BigRational::zero();
    let first_beat = &segments[0].0;
    if target.value() < first_beat.value() {
        let delta = ExactRational(first_beat.value() - target.value());
        // Extrapolate the first BPM leftward without inserting a synthetic Beat 0 point.
        let left = chart_time_delta_seconds(&delta, &segments[0].1, bpmfactor, mode)?;
        return Ok(ExactRational(-left.value().clone()));
    }

    for index in 0..segments.len() {
        let (start, bpm) = &segments[index];
        if target.value() <= start.value() {
            break;
        }
        let end = if index + 1 < segments.len() {
            let next = &segments[index + 1].0;
            if target.value() < next.value() {
                target.clone()
            } else {
                next.clone()
            }
        } else {
            target.clone()
        };
        if end.value() <= start.value() {
            continue;
        }
        let delta = ExactRational(end.value() - start.value());
        let step = chart_time_delta_seconds(&delta, bpm, bpmfactor, mode)?;
        time += step.value();
        if end.value() == target.value() {
            break;
        }
    }
    Ok(ExactRational(time))
}

fn validate_binding(
    source: &RpeSourceDocument,
    binding: &RpeProfileBinding,
) -> Result<(), RpeError> {
    let profile = binding.profile();
    if profile.requires_speed_mode() && binding.speed_mode.is_none() {
        return Err(RpeError::new(
            PROFILE_PARAMETER_INVALID,
            "profile.speedMode",
            format!(
                "profile {}@{} requires speedMode",
                profile.id(),
                profile.version()
            ),
        ));
    }
    if !profile.requires_speed_mode() && binding.speed_mode.is_some() {
        return Err(RpeError::new(
            PROFILE_PARAMETER_INVALID,
            "profile.speedMode",
            format!(
                "profile {}@{} does not accept speedMode",
                profile.id(),
                profile.version()
            ),
        ));
    }
    if profile.requires_rpe_version_era_when_absent()
        && source.meta.rpe_version.is_none()
        && binding.rpe_version_era.is_none()
    {
        return Err(RpeError::new(
            PROFILE_PARAMETER_INVALID,
            "profile.rpeVersionEra",
            "profile rpe.phira.rpe170-speed requires rpeVersionEra when META.RPEVersion is absent",
        ));
    }
    Ok(())
}

fn resolved_version_era(
    source: &RpeSourceDocument,
    binding: &RpeProfileBinding,
) -> Result<Option<RpeVersionEra>, RpeError> {
    if binding.profile() != RpeProfile::PhiraRpe170Speed {
        return Ok(binding.rpe_version_era);
    }
    if let Some(era) = binding.rpe_version_era {
        return Ok(Some(era));
    }
    match source.meta.rpe_version.as_ref() {
        Some(RpeVersionEvidence::Number { value, .. }) => {
            let version = value.exact().to_i64().ok_or_else(|| {
                RpeError::new(
                    SOURCE_INVALID,
                    "$.META.RPEVersion",
                    "RPEVersion number must be an exact integer for era selection",
                )
            })?;
            Ok(Some(if version >= 170 {
                RpeVersionEra::AtLeast170
            } else {
                RpeVersionEra::Pre170
            }))
        }
        Some(RpeVersionEvidence::String(value)) => {
            let version = value.value().parse::<i64>().map_err(|_| {
                RpeError::new(
                    SOURCE_INVALID,
                    "$.META.RPEVersion",
                    "RPEVersion string must parse as an integer for era selection",
                )
            })?;
            Ok(Some(if version >= 170 {
                RpeVersionEra::AtLeast170
            } else {
                RpeVersionEra::Pre170
            }))
        }
        None => Ok(None),
    }
}

fn parse_meta(value: &LosslessJsonValue, limits: RpeLimits) -> Result<RpeSourceMeta, RpeError> {
    let members = object(value, "$.META")?;
    let rpe_version = match optional(members, "RPEVersion", "$.META")? {
        None => None,
        Some(LosslessJsonValue::Number(raw)) => Some(RpeVersionEvidence::Number {
            value: ExactDecimal::parse(raw, limits.decimal)
                .map_err(|error| RpeError::from_exact("$.META.RPEVersion", error))?,
            raw: raw.clone(),
        }),
        Some(LosslessJsonValue::String(value)) => Some(RpeVersionEvidence::String(value.clone())),
        Some(_) => {
            return Err(RpeError::new(
                SOURCE_INVALID,
                "$.META.RPEVersion",
                "RPEVersion must be a JSON number or string when present",
            ));
        }
    };
    Ok(RpeSourceMeta {
        rpe_version,
        offset: number(
            required(members, "offset", "$.META")?,
            "$.META.offset",
            limits,
        )?,
        name: optional_string(members, "name", "$.META")?,
        level: optional_string(members, "level", "$.META")?,
        charter: optional_string(members, "charter", "$.META")?,
        composer: optional_string(members, "composer", "$.META")?,
        illustration: optional_string(members, "illustration", "$.META")?,
        song: optional_string(members, "song", "$.META")?,
        background: optional_string(members, "background", "$.META")?,
        id: optional_string(members, "id", "$.META")?,
        unknown_fields: unknown(
            members,
            &[
                "RPEVersion",
                "offset",
                "name",
                "level",
                "charter",
                "composer",
                "illustration",
                "song",
                "background",
                "id",
            ],
        ),
    })
}

fn parse_bpm_point(
    value: &LosslessJsonValue,
    index: usize,
    limits: RpeLimits,
) -> Result<RpeSourceBpmPoint, RpeError> {
    let path = format!("$.BPMList[{index}]");
    let members = object(value, &path)?;
    Ok(RpeSourceBpmPoint {
        start_time: parse_beat(
            required(members, "startTime", &path)?,
            &format!("{path}.startTime"),
            limits,
        )?,
        bpm: number(
            required(members, "bpm", &path)?,
            &format!("{path}.bpm"),
            limits,
        )?,
        unknown_fields: unknown(members, &["startTime", "bpm"]),
    })
}

fn parse_line(
    value: &LosslessJsonValue,
    index: usize,
    limits: RpeLimits,
) -> Result<RpeSourceLine, RpeError> {
    let path = format!("$.judgeLineList[{index}]");
    let members = object(value, &path)?;
    let notes = match optional(members, "notes", &path)? {
        None | Some(LosslessJsonValue::Null) => Vec::new(),
        Some(value) => {
            let values = array(value, &format!("{path}.notes"))?;
            enforce_count(
                &format!("{path}.notes"),
                "max_notes_per_line",
                values.len(),
                limits.max_notes_per_line,
            )?;
            values
                .iter()
                .enumerate()
                .map(|(note_index, note)| {
                    parse_note(note, &format!("{path}.notes[{note_index}]"), limits)
                })
                .collect::<Result<Vec<_>, RpeError>>()?
        }
    };
    Ok(RpeSourceLine {
        bpmfactor: optional_number(members, "bpmfactor", &path, limits)?,
        event_layers: parse_event_layers(members, &path, limits)?,
        notes,
        father: optional_number(members, "father", &path, limits)?,
        rotate_with_father: optional_bool(members, "rotateWithFather", &path)?,
        texture: optional_string(members, "Texture", &path)?
            .or(optional_string(members, "texture", &path)?),
        name: optional_string(members, "Name", &path)?.or(optional_string(members, "name", &path)?),
        z_order: optional_number(members, "zOrder", &path, limits)?,
        is_cover: optional_number(members, "isCover", &path, limits)?,
        attach_ui: optional(members, "attachUI", &path)?.cloned(),
        is_gif: optional_bool(members, "isGif", &path)?,
        extended: optional(members, "extended", &path)?.cloned(),
        unknown_fields: unknown(
            members,
            &[
                "bpmfactor",
                "eventLayers",
                "notes",
                "father",
                "rotateWithFather",
                "Texture",
                "texture",
                "Name",
                "name",
                "zOrder",
                "isCover",
                "attachUI",
                "isGif",
                "extended",
            ],
        ),
    })
}

fn parse_event_layers(
    members: &[LosslessJsonMember],
    path: &str,
    limits: RpeLimits,
) -> Result<RpeEventLayersField, RpeError> {
    match optional(members, "eventLayers", path)? {
        None => Ok(RpeEventLayersField::Missing),
        Some(LosslessJsonValue::Null) => Ok(RpeEventLayersField::Null),
        Some(value) => {
            let values = array(value, &format!("{path}.eventLayers"))?;
            enforce_count(
                &format!("{path}.eventLayers"),
                "max_layers_per_line",
                values.len(),
                limits.max_layers_per_line,
            )?;
            let mut slots = Vec::with_capacity(values.len());
            for (index, slot) in values.iter().enumerate() {
                let slot_path = format!("{path}.eventLayers[{index}]");
                match slot {
                    LosslessJsonValue::Null => slots.push(RpeEventLayerSlot::Null),
                    other => slots.push(RpeEventLayerSlot::Layer(parse_event_layer(
                        other, &slot_path, limits,
                    )?)),
                }
            }
            Ok(RpeEventLayersField::Present(slots))
        }
    }
}

fn parse_event_layer(
    value: &LosslessJsonValue,
    path: &str,
    limits: RpeLimits,
) -> Result<RpeSourceEventLayer, RpeError> {
    let members = object(value, path)?;
    Ok(RpeSourceEventLayer {
        move_x_events: parse_optional_common_list(members, "moveXEvents", path, limits)?,
        move_y_events: parse_optional_common_list(members, "moveYEvents", path, limits)?,
        rotate_events: parse_optional_common_list(members, "rotateEvents", path, limits)?,
        alpha_events: parse_optional_common_list(members, "alphaEvents", path, limits)?,
        speed_events: parse_optional_speed_list(members, "speedEvents", path, limits)?,
        unknown_fields: unknown(
            members,
            &[
                "moveXEvents",
                "moveYEvents",
                "rotateEvents",
                "alphaEvents",
                "speedEvents",
            ],
        ),
    })
}

fn parse_optional_common_list(
    members: &[LosslessJsonMember],
    field: &str,
    parent: &str,
    limits: RpeLimits,
) -> Result<RpeOptionalEventList, RpeError> {
    match optional(members, field, parent)? {
        None => Ok(RpeOptionalEventList::Missing),
        Some(LosslessJsonValue::Null) => Ok(RpeOptionalEventList::Null),
        Some(value) => {
            let path = format!("{parent}.{field}");
            let values = array(value, &path)?;
            enforce_count(
                &path,
                "max_events_per_layer_field",
                values.len(),
                limits.max_events_per_layer_field,
            )?;
            let events = values
                .iter()
                .enumerate()
                .map(|(index, event)| {
                    parse_common_event(event, &format!("{path}[{index}]"), limits)
                })
                .collect::<Result<Vec<_>, RpeError>>()?;
            Ok(RpeOptionalEventList::Present(events))
        }
    }
}

fn parse_optional_speed_list(
    members: &[LosslessJsonMember],
    field: &str,
    parent: &str,
    limits: RpeLimits,
) -> Result<RpeOptionalSpeedList, RpeError> {
    match optional(members, field, parent)? {
        None => Ok(RpeOptionalSpeedList::Missing),
        Some(LosslessJsonValue::Null) => Ok(RpeOptionalSpeedList::Null),
        Some(value) => {
            let path = format!("{parent}.{field}");
            let values = array(value, &path)?;
            enforce_count(
                &path,
                "max_events_per_layer_field",
                values.len(),
                limits.max_events_per_layer_field,
            )?;
            let events = values
                .iter()
                .enumerate()
                .map(|(index, event)| parse_speed_event(event, &format!("{path}[{index}]"), limits))
                .collect::<Result<Vec<_>, RpeError>>()?;
            Ok(RpeOptionalSpeedList::Present(events))
        }
    }
}

fn parse_common_event(
    value: &LosslessJsonValue,
    path: &str,
    limits: RpeLimits,
) -> Result<RpeSourceCommonEvent, RpeError> {
    let members = object(value, path)?;
    Ok(RpeSourceCommonEvent {
        start_time: parse_beat(
            required(members, "startTime", path)?,
            &format!("{path}.startTime"),
            limits,
        )?,
        end_time: parse_beat(
            required(members, "endTime", path)?,
            &format!("{path}.endTime"),
            limits,
        )?,
        start: number(
            required(members, "start", path)?,
            &format!("{path}.start"),
            limits,
        )?,
        end: number(
            required(members, "end", path)?,
            &format!("{path}.end"),
            limits,
        )?,
        easing_type: optional_number(members, "easingType", path, limits)?,
        easing_left: optional_number(members, "easingLeft", path, limits)?,
        easing_right: optional_number(members, "easingRight", path, limits)?,
        bezier: optional_number(members, "bezier", path, limits)?,
        bezier_points: optional_number_array(members, "bezierPoints", path, limits)?,
        linkgroup: optional_number(members, "linkgroup", path, limits)?,
        unknown_fields: unknown(
            members,
            &[
                "startTime",
                "endTime",
                "start",
                "end",
                "easingType",
                "easingLeft",
                "easingRight",
                "bezier",
                "bezierPoints",
                "linkgroup",
            ],
        ),
    })
}

fn parse_speed_event(
    value: &LosslessJsonValue,
    path: &str,
    limits: RpeLimits,
) -> Result<RpeSourceSpeedEvent, RpeError> {
    let members = object(value, path)?;
    Ok(RpeSourceSpeedEvent {
        start_time: parse_beat(
            required(members, "startTime", path)?,
            &format!("{path}.startTime"),
            limits,
        )?,
        end_time: parse_beat(
            required(members, "endTime", path)?,
            &format!("{path}.endTime"),
            limits,
        )?,
        start: number(
            required(members, "start", path)?,
            &format!("{path}.start"),
            limits,
        )?,
        end: number(
            required(members, "end", path)?,
            &format!("{path}.end"),
            limits,
        )?,
        easing_type: optional_number(members, "easingType", path, limits)?,
        easing_left: optional_number(members, "easingLeft", path, limits)?,
        easing_right: optional_number(members, "easingRight", path, limits)?,
        bezier: optional_number(members, "bezier", path, limits)?,
        bezier_points: optional_number_array(members, "bezierPoints", path, limits)?,
        linkgroup: optional_number(members, "linkgroup", path, limits)?,
        unknown_fields: unknown(
            members,
            &[
                "startTime",
                "endTime",
                "start",
                "end",
                "easingType",
                "easingLeft",
                "easingRight",
                "bezier",
                "bezierPoints",
                "linkgroup",
            ],
        ),
    })
}

fn parse_note(
    value: &LosslessJsonValue,
    path: &str,
    limits: RpeLimits,
) -> Result<RpeSourceNote, RpeError> {
    let members = object(value, path)?;
    Ok(RpeSourceNote {
        kind: number(
            required(members, "type", path)?,
            &format!("{path}.type"),
            limits,
        )?,
        start_time: parse_beat(
            required(members, "startTime", path)?,
            &format!("{path}.startTime"),
            limits,
        )?,
        end_time: parse_beat(
            required(members, "endTime", path)?,
            &format!("{path}.endTime"),
            limits,
        )?,
        position_x: number(
            required(members, "positionX", path)?,
            &format!("{path}.positionX"),
            limits,
        )?,
        speed: number(
            required(members, "speed", path)?,
            &format!("{path}.speed"),
            limits,
        )?,
        above: optional_number(members, "above", path, limits)?,
        is_fake: optional_number(members, "isFake", path, limits)?,
        alpha: optional_number(members, "alpha", path, limits)?,
        size: optional_number(members, "size", path, limits)?,
        visible_time: optional_number(members, "visibleTime", path, limits)?,
        y_offset: optional_number(members, "yOffset", path, limits)?,
        hitsound: optional_string(members, "hitsound", path)?,
        unknown_fields: unknown(
            members,
            &[
                "type",
                "startTime",
                "endTime",
                "positionX",
                "speed",
                "above",
                "isFake",
                "alpha",
                "size",
                "visibleTime",
                "yOffset",
                "hitsound",
            ],
        ),
    })
}

fn parse_beat(
    value: &LosslessJsonValue,
    path: &str,
    limits: RpeLimits,
) -> Result<RpeBeat, RpeError> {
    let values = array(value, path)?;
    if values.len() != 3 {
        return Err(RpeError::new(
            SOURCE_INVALID,
            path,
            "RPE Beat must be a three-element array [a, b, c]",
        ));
    }
    Ok(RpeBeat {
        whole: number(&values[0], &format!("{path}[0]"), limits)?,
        numerator: number(&values[1], &format!("{path}[1]"), limits)?,
        denominator: number(&values[2], &format!("{path}[2]"), limits)?,
    })
}

fn object<'a>(
    value: &'a LosslessJsonValue,
    path: &str,
) -> Result<&'a [LosslessJsonMember], RpeError> {
    value
        .as_object()
        .ok_or_else(|| RpeError::new(SOURCE_INVALID, path, "expected a JSON object"))
}

fn array<'a>(
    value: &'a LosslessJsonValue,
    path: &str,
) -> Result<&'a [LosslessJsonValue], RpeError> {
    value
        .as_array()
        .ok_or_else(|| RpeError::new(SOURCE_INVALID, path, "expected a JSON array"))
}

fn required<'a>(
    members: &'a [LosslessJsonMember],
    key: &str,
    parent: &str,
) -> Result<&'a LosslessJsonValue, RpeError> {
    let mut matches = members.iter().filter(|member| member.key() == key);
    let value = matches.next().ok_or_else(|| {
        RpeError::new(
            SOURCE_INVALID,
            format!("{parent}.{key}"),
            "required RPE field is missing",
        )
    })?;
    if matches.next().is_some() {
        return Err(RpeError::new(
            SOURCE_INVALID,
            format!("{parent}.{key}"),
            "known RPE field is duplicated",
        ));
    }
    Ok(value.value())
}

fn optional<'a>(
    members: &'a [LosslessJsonMember],
    key: &str,
    parent: &str,
) -> Result<Option<&'a LosslessJsonValue>, RpeError> {
    let mut matches = members.iter().filter(|member| member.key() == key);
    let value = matches.next();
    if matches.next().is_some() {
        return Err(RpeError::new(
            SOURCE_INVALID,
            format!("{parent}.{key}"),
            "known RPE field is duplicated",
        ));
    }
    Ok(value.map(LosslessJsonMember::value))
}

fn number(
    value: &LosslessJsonValue,
    path: &str,
    limits: RpeLimits,
) -> Result<ExactDecimal, RpeError> {
    let raw = value
        .as_number_lexeme()
        .ok_or_else(|| RpeError::new(SOURCE_INVALID, path, "expected an exact JSON number"))?;
    ExactDecimal::parse(raw, limits.decimal).map_err(|error| RpeError::from_exact(path, error))
}

fn optional_number(
    members: &[LosslessJsonMember],
    key: &str,
    parent: &str,
    limits: RpeLimits,
) -> Result<Option<ExactDecimal>, RpeError> {
    match optional(members, key, parent)? {
        None | Some(LosslessJsonValue::Null) => Ok(None),
        Some(value) => Ok(Some(number(value, &format!("{parent}.{key}"), limits)?)),
    }
}

fn optional_string(
    members: &[LosslessJsonMember],
    key: &str,
    parent: &str,
) -> Result<Option<LosslessJsonString>, RpeError> {
    match optional(members, key, parent)? {
        None | Some(LosslessJsonValue::Null) => Ok(None),
        Some(LosslessJsonValue::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(RpeError::new(
            SOURCE_INVALID,
            format!("{parent}.{key}"),
            "expected a JSON string",
        )),
    }
}

fn optional_bool(
    members: &[LosslessJsonMember],
    key: &str,
    parent: &str,
) -> Result<Option<bool>, RpeError> {
    match optional(members, key, parent)? {
        None | Some(LosslessJsonValue::Null) => Ok(None),
        Some(LosslessJsonValue::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(RpeError::new(
            SOURCE_INVALID,
            format!("{parent}.{key}"),
            "expected a JSON boolean",
        )),
    }
}

fn optional_number_array(
    members: &[LosslessJsonMember],
    key: &str,
    parent: &str,
    limits: RpeLimits,
) -> Result<Option<Vec<ExactDecimal>>, RpeError> {
    match optional(members, key, parent)? {
        None | Some(LosslessJsonValue::Null) => Ok(None),
        Some(value) => {
            let path = format!("{parent}.{key}");
            let values = array(value, &path)?;
            values
                .iter()
                .enumerate()
                .map(|(index, item)| number(item, &format!("{path}[{index}]"), limits))
                .collect::<Result<Vec<_>, RpeError>>()
                .map(Some)
        }
    }
}

fn unknown(members: &[LosslessJsonMember], known: &[&str]) -> Vec<LosslessJsonMember> {
    members
        .iter()
        .filter(|member| !known.contains(&member.key()))
        .cloned()
        .collect()
}

fn enforce_count(
    path: &str,
    kind: &'static str,
    observed: usize,
    limit: usize,
) -> Result<(), RpeError> {
    if observed > limit {
        Err(RpeError::new(
            SOURCE_INVALID,
            path,
            format!("RPE limit {kind} exceeded: limit {limit}, observed {observed}"),
        ))
    } else {
        Ok(())
    }
}

fn require_integer(
    value: &ExactDecimal,
    path: &str,
    label: &str,
) -> Result<ExactRational, RpeError> {
    if !value.exact().is_integer() {
        return Err(RpeError::new(
            SOURCE_INVALID,
            path,
            format!("{label} must be an exact integer"),
        ));
    }
    Ok(value.exact().clone())
}

fn validate_positive_finite(
    value: &ExactRational,
    path: &str,
    label: &str,
) -> Result<(), RpeError> {
    if !value.is_positive() || value.to_f64().is_err() {
        Err(RpeError::new(
            SOURCE_INVALID,
            path,
            format!("{label} must be finite and positive"),
        ))
    } else {
        Ok(())
    }
}

fn integer(value: i64) -> BigRational {
    BigRational::from_integer(BigInt::from(value))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpeError {
    category: &'static str,
    path: String,
    message: String,
}

impl RpeError {
    pub(crate) fn new(
        category: &'static str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            category,
            path: path.into(),
            message: message.into(),
        }
    }

    fn from_exact(path: &str, error: ExactNumberError) -> Self {
        Self::new(SOURCE_INVALID, path, error.to_string())
    }

    pub const fn category(&self) -> &'static str {
        self.category
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for RpeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at {}: {}",
            self.category, self.path, self.message
        )
    }
}

impl std::error::Error for RpeError {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde::Deserialize;

    use super::*;
    use crate::{ArtifactRole, SourceArtifact, parse_json_document};

    const MINIMAL_CHART: &str = r#"{
        "META": {
            "RPEVersion": 150,
            "offset": 12,
            "name": "timing",
            "song": "a.ogg",
            "background": "b.png",
            "metaUnknown": true
        },
        "BPMList": [
            {"startTime": [0, 0, 1], "bpm": 120, "bpmUnknown": 1},
            {"startTime": [4, 0, 1], "bpm": 180}
        ],
        "judgeLineList": [
            {
                "bpmfactor": 2,
                "eventLayers": [
                    null,
                    {
                        "moveXEvents": [
                            {
                                "startTime": [0, 0, 1],
                                "endTime": [1, 0, 1],
                                "start": 0,
                                "end": 100,
                                "easingType": 1,
                                "moveUnknown": 9
                            }
                        ],
                        "speedEvents": [
                            {
                                "startTime": [0, 0, 1],
                                "endTime": [2, 0, 1],
                                "start": 1,
                                "end": 1
                            }
                        ]
                    }
                ],
                "notes": [
                    {
                        "type": 1,
                        "startTime": [1, 0, 1],
                        "endTime": [1, 0, 1],
                        "positionX": 0,
                        "speed": 1,
                        "above": 1,
                        "isFake": 0,
                        "hitsound": "click.ogg"
                    }
                ],
                "father": -1,
                "Texture": "line.png",
                "lineUnknownA": 1,
                "lineUnknownB": 2
            },
            {
                "eventLayers": null,
                "notes": [],
                "rotateWithFather": true
            }
        ],
        "chartTime": 0
    }"#;

    fn artifact(bytes: &str) -> SourceArtifact {
        SourceArtifact::new(
            "charts/main.rpe.json",
            ArtifactRole::Chart,
            bytes.as_bytes(),
        )
        .unwrap()
    }

    fn parse_minimal() -> RpeSourceDocument {
        let parsed = parse_json_document(SourceFormat::Rpe, &artifact(MINIMAL_CHART)).unwrap();
        parse_rpe_document(&parsed, RpeLimits::default()).unwrap()
    }

    fn exact(value: &str) -> ExactRational {
        ExactDecimal::parse(value, DecimalLimits::default())
            .unwrap()
            .exact()
            .clone()
    }

    fn beat(a: i64, b: i64, c: i64) -> RpeBeat {
        RpeBeat {
            whole: ExactDecimal::parse(&a.to_string(), DecimalLimits::default()).unwrap(),
            numerator: ExactDecimal::parse(&b.to_string(), DecimalLimits::default()).unwrap(),
            denominator: ExactDecimal::parse(&c.to_string(), DecimalLimits::default()).unwrap(),
        }
    }

    #[test]
    fn parse_retains_identity_version_layers_and_unknown_order() {
        let source = parse_minimal();
        assert_eq!(source.artifact_id().as_str(), "charts/main.rpe.json");
        assert_eq!(
            source.artifact_content_sha256(),
            artifact(MINIMAL_CHART).content_sha256()
        );
        let version = source.meta().rpe_version().unwrap();
        assert!(version.is_number());
        assert_eq!(version.raw_spelling(), "150");
        assert_eq!(source.meta().offset().raw(), "12");
        assert_eq!(source.meta().song(), Some("a.ogg"));
        assert_eq!(source.meta().unknown_fields()[0].key(), "metaUnknown");
        assert_eq!(source.unknown_fields()[0].key(), "chartTime");
        assert!(matches!(
            source.lines()[0].event_layers(),
            RpeEventLayersField::Present(_)
        ));
        if let RpeEventLayersField::Present(slots) = source.lines()[0].event_layers() {
            assert!(matches!(slots[0], RpeEventLayerSlot::Null));
            assert!(matches!(slots[1], RpeEventLayerSlot::Layer(_)));
        }
        assert!(matches!(
            source.lines()[1].event_layers(),
            RpeEventLayersField::Null
        ));
        assert_eq!(source.lines()[0].rotate_with_father(), None);
        assert_eq!(source.lines()[1].rotate_with_father(), Some(true));
        assert_eq!(source.lines()[0].notes()[0].hitsound(), Some("click.ogg"));
        assert_eq!(source.lines()[0].unknown_fields()[0].key(), "lineUnknownA");
        assert_eq!(source.lines()[0].unknown_fields()[1].key(), "lineUnknownB");
    }

    #[test]
    fn string_rpe_version_is_preserved_as_string_evidence() {
        let chart = MINIMAL_CHART.replace("\"RPEVersion\": 150", "\"RPEVersion\": \"170\"");
        let parsed = parse_json_document(SourceFormat::Rpe, &artifact(&chart)).unwrap();
        let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
        let version = source.meta().rpe_version().unwrap();
        assert!(version.is_string());
        assert!(version.raw_spelling().contains("170"));
    }

    #[test]
    fn missing_event_layers_and_sparse_fields_are_observable() {
        let chart = r#"{
            "META": {"offset": 0},
            "BPMList": [{"startTime": [0,0,1], "bpm": 120}],
            "judgeLineList": [{
                "notes": [],
                "eventLayers": [{
                    "moveXEvents": null
                }]
            }]
        }"#;
        let parsed = parse_json_document(SourceFormat::Rpe, &artifact(chart)).unwrap();
        let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
        match source.lines()[0].event_layers() {
            RpeEventLayersField::Present(slots) => match &slots[0] {
                RpeEventLayerSlot::Layer(layer) => {
                    assert!(matches!(layer.move_x_events(), RpeOptionalEventList::Null));
                    assert!(matches!(
                        layer.speed_events(),
                        RpeOptionalSpeedList::Missing
                    ));
                }
                _ => panic!("expected layer object"),
            },
            other => panic!("unexpected layers {other:?}"),
        }
    }

    #[test]
    fn five_profile_bindings_resolve_factor_and_rotate_defaults() {
        let source = parse_minimal();
        let divide = interpret_rpe_timing(
            &source,
            &RpeProfileBinding::community_divide(RpeSpeedMode::LegacyLinear),
        )
        .unwrap();
        assert_eq!(divide.profile().id(), "rpe.community.divide-bpmfactor");
        assert_eq!(divide.lines()[0].bpmfactor(), &exact("2"));
        assert!(!divide.lines()[0].rotate_with_father());
        assert!(!divide.lines()[0].rotate_with_father_was_present());
        assert!(divide.lines()[1].rotate_with_father());
        assert!(divide.lines()[1].rotate_with_father_was_present());

        let multiply = interpret_rpe_timing(
            &source,
            &RpeProfileBinding::docs_example_multiply(RpeSpeedMode::ModernEased),
        )
        .unwrap();
        assert_eq!(multiply.profile().factor_mode(), RpeFactorMode::Multiply);
        assert!(multiply.lines()[0].rotate_with_father());

        let phichain =
            interpret_rpe_timing(&source, &RpeProfileBinding::phichain_import()).unwrap();
        assert_eq!(phichain.profile().factor_mode(), RpeFactorMode::Ignore);
        assert!(phichain.lines()[0].rotate_with_father());

        let legacy =
            interpret_rpe_timing(&source, &RpeProfileBinding::phira_legacy_speed()).unwrap();
        assert!(!legacy.lines()[0].rotate_with_father());

        let modern =
            interpret_rpe_timing(&source, &RpeProfileBinding::phira_rpe170_speed(None)).unwrap();
        assert_eq!(modern.rpe_version_era(), Some(RpeVersionEra::Pre170));
    }

    #[test]
    fn profile_parameter_requirements_are_strict() {
        let source = parse_minimal();
        let missing_speed = interpret_rpe_timing(
            &source,
            &RpeProfileBinding {
                profile: RpeProfile::CommunityDivideBpmfactor,
                speed_mode: None,
                rpe_version_era: None,
            },
        )
        .unwrap_err();
        assert_eq!(missing_speed.category(), PROFILE_PARAMETER_INVALID);

        let chart = MINIMAL_CHART.replace("\"RPEVersion\": 150,\n            ", "");
        let parsed = parse_json_document(SourceFormat::Rpe, &artifact(&chart)).unwrap();
        let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
        assert!(source.meta().rpe_version().is_none());
        let missing_era =
            interpret_rpe_timing(&source, &RpeProfileBinding::phira_rpe170_speed(None))
                .unwrap_err();
        assert_eq!(missing_era.category(), PROFILE_PARAMETER_INVALID);
        let with_era = interpret_rpe_timing(
            &source,
            &RpeProfileBinding::phira_rpe170_speed(Some(RpeVersionEra::AtLeast170)),
        )
        .unwrap();
        assert_eq!(with_era.rpe_version_era(), Some(RpeVersionEra::AtLeast170));
    }

    #[test]
    fn beat_rules_follow_profile_and_reject_invalid_denominators() {
        assert_eq!(
            resolve_beat(&beat(4, 1, 2), RpeProfile::PhiraLegacySpeed, "beat").unwrap(),
            exact("9/2")
        );
        assert_eq!(
            resolve_beat(&beat(4, 0, 0), RpeProfile::PhichainImport, "beat").unwrap(),
            exact("4")
        );
        assert_eq!(
            resolve_beat(&beat(1, 0, 0), RpeProfile::PhiraLegacySpeed, "beat")
                .unwrap_err()
                .category(),
            SOURCE_INVALID
        );
        assert_eq!(
            resolve_beat(&beat(1, 1, 0), RpeProfile::PhichainImport, "beat")
                .unwrap_err()
                .category(),
            SOURCE_INVALID
        );
    }

    #[test]
    fn factor_modes_diverge_exactly_on_same_inputs() {
        let delta = exact("1");
        let bpm = exact("120");
        let factor = exact("2");
        assert_eq!(
            chart_time_delta_seconds(&delta, &bpm, &factor, RpeFactorMode::Divide).unwrap(),
            exact("1")
        );
        assert_eq!(
            chart_time_delta_seconds(&delta, &bpm, &factor, RpeFactorMode::Multiply).unwrap(),
            exact("1/4")
        );
        assert_eq!(
            chart_time_delta_seconds(&delta, &bpm, &factor, RpeFactorMode::Ignore).unwrap(),
            exact("1/2")
        );
    }

    #[test]
    fn interpret_maps_note_and_event_boundaries_through_bpmlist() {
        let source = parse_minimal();
        let divide = interpret_rpe_timing(
            &source,
            &RpeProfileBinding::community_divide(RpeSpeedMode::LegacyLinear),
        )
        .unwrap();
        // beat 1 with bpmfactor=2 and first BPM 120: dt = 1 * 60 * 2 / 120 = 1
        assert_eq!(
            divide.lines()[0].notes()[0]
                .start_time()
                .chart_time_seconds(),
            &exact("1")
        );
        let ignore =
            interpret_rpe_timing(&source, &RpeProfileBinding::phira_legacy_speed()).unwrap();
        // same beat with ignore factor: dt = 1 * 60 / 120 = 1/2
        assert_eq!(
            ignore.lines()[0].notes()[0]
                .start_time()
                .chart_time_seconds(),
            &exact("1/2")
        );
        let layer = ignore.lines()[0].event_layers()[1].as_ref().unwrap();
        assert_eq!(
            layer.move_x_events()[0].end_time().chart_time_seconds(),
            &exact("1/2")
        );
    }

    #[test]
    fn same_beat_bpm_points_keep_source_order_last_active() {
        let chart = r#"{
            "META": {"offset": 0},
            "BPMList": [
                {"startTime": [0,0,1], "bpm": 60},
                {"startTime": [0,0,1], "bpm": 120}
            ],
            "judgeLineList": [{
                "notes": [{
                    "type": 1,
                    "startTime": [1,0,1],
                    "endTime": [1,0,1],
                    "positionX": 0,
                    "speed": 1
                }]
            }]
        }"#;
        let parsed = parse_json_document(SourceFormat::Rpe, &artifact(chart)).unwrap();
        let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
        let semantic =
            interpret_rpe_timing(&source, &RpeProfileBinding::phira_legacy_speed()).unwrap();
        assert_eq!(semantic.bpm_points().len(), 2);
        assert_eq!(
            semantic.lines()[0].notes()[0]
                .start_time()
                .chart_time_seconds(),
            &exact("1/2")
        );
    }

    #[test]
    fn limits_and_duplicate_known_fields_fail_strictly() {
        let limits = RpeLimits {
            max_lines: 0,
            ..RpeLimits::default()
        };
        let parsed = parse_json_document(SourceFormat::Rpe, &artifact(MINIMAL_CHART)).unwrap();
        assert_eq!(
            parse_rpe_document(&parsed, limits).unwrap_err().category(),
            SOURCE_INVALID
        );

        let duplicate = r#"{
            "META": {"offset": 0, "offset": 1},
            "BPMList": [{"startTime": [0,0,1], "bpm": 120}],
            "judgeLineList": []
        }"#;
        let parsed = parse_json_document(SourceFormat::Rpe, &artifact(duplicate)).unwrap();
        assert_eq!(
            parse_rpe_document(&parsed, RpeLimits::default())
                .unwrap_err()
                .path(),
            "$.META.offset"
        );
    }

    #[test]
    fn decreasing_bpmlist_is_rejected_without_sorting() {
        let chart = r#"{
            "META": {"offset": 0},
            "BPMList": [
                {"startTime": [2,0,1], "bpm": 120},
                {"startTime": [1,0,1], "bpm": 120}
            ],
            "judgeLineList": []
        }"#;
        let parsed = parse_json_document(SourceFormat::Rpe, &artifact(chart)).unwrap();
        let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
        assert_eq!(
            interpret_rpe_timing(&source, &RpeProfileBinding::phira_legacy_speed())
                .unwrap_err()
                .category(),
            SOURCE_INVALID
        );
    }

    #[derive(Debug, Deserialize)]
    struct MappingCorpus {
        vector: Vec<MappingVector>,
        invalid: Vec<InvalidVector>,
    }

    #[derive(Debug, Deserialize)]
    struct MappingVector {
        id: String,
        rule_id: String,
        source: BTreeMap<String, toml::Value>,
        expected: String,
    }

    #[derive(Debug, Deserialize)]
    struct InvalidVector {
        id: String,
        rule_id: String,
        source: BTreeMap<String, toml::Value>,
        diagnostic: String,
    }

    #[test]
    fn checked_in_rpe_beat_and_bpmfactor_vectors_execute_exactly() {
        let corpus: MappingCorpus = toml::from_str(include_str!(
            "../../../docs/conformance/conversion/mapping-vectors.toml"
        ))
        .unwrap();
        let mut executed = 0;
        for vector in &corpus.vector {
            let actual = match vector.rule_id.as_str() {
                "rpe.beat.abc-strict" => resolve_beat(
                    &beat(
                        vector.source["a"].as_integer().unwrap(),
                        vector.source["b"].as_integer().unwrap(),
                        vector.source["c"].as_integer().unwrap(),
                    ),
                    RpeProfile::PhiraLegacySpeed,
                    &vector.id,
                )
                .unwrap(),
                "rpe.beat.abc-zero-zero-integer" => resolve_beat(
                    &beat(
                        vector.source["a"].as_integer().unwrap(),
                        vector.source["b"].as_integer().unwrap(),
                        vector.source["c"].as_integer().unwrap(),
                    ),
                    RpeProfile::PhichainImport,
                    &vector.id,
                )
                .unwrap(),
                "rpe.time.bpmfactor-divide" => chart_time_delta_seconds(
                    &exact(vector.source["beat_delta"].as_str().unwrap()),
                    &exact(vector.source["bpm"].as_str().unwrap()),
                    &exact(vector.source["bpmfactor"].as_str().unwrap()),
                    RpeFactorMode::Divide,
                )
                .unwrap(),
                "rpe.time.bpmfactor-multiply" => chart_time_delta_seconds(
                    &exact(vector.source["beat_delta"].as_str().unwrap()),
                    &exact(vector.source["bpm"].as_str().unwrap()),
                    &exact(vector.source["bpmfactor"].as_str().unwrap()),
                    RpeFactorMode::Multiply,
                )
                .unwrap(),
                "rpe.time.bpmfactor-ignore" => chart_time_delta_seconds(
                    &exact(vector.source["beat_delta"].as_str().unwrap()),
                    &exact(vector.source["bpm"].as_str().unwrap()),
                    &exact(vector.source["bpmfactor"].as_str().unwrap()),
                    RpeFactorMode::Ignore,
                )
                .unwrap(),
                _ => continue,
            };
            assert_eq!(actual, exact(&vector.expected), "{}", vector.id);
            executed += 1;
        }
        assert_eq!(executed, 5);

        let mut invalid_executed = 0;
        for vector in &corpus.invalid {
            let error = match vector.rule_id.as_str() {
                "rpe.beat.abc-strict" => resolve_beat(
                    &beat(
                        vector.source["a"].as_integer().unwrap(),
                        vector.source["b"].as_integer().unwrap(),
                        vector.source["c"].as_integer().unwrap(),
                    ),
                    RpeProfile::PhiraLegacySpeed,
                    &vector.id,
                )
                .unwrap_err(),
                "rpe.beat.abc-zero-zero-integer" => resolve_beat(
                    &beat(
                        vector.source["a"].as_integer().unwrap(),
                        vector.source["b"].as_integer().unwrap(),
                        vector.source["c"].as_integer().unwrap(),
                    ),
                    RpeProfile::PhichainImport,
                    &vector.id,
                )
                .unwrap_err(),
                _ => continue,
            };
            assert_eq!(
                error.category(),
                vector.diagnostic.as_str(),
                "{}",
                vector.id
            );
            invalid_executed += 1;
        }
        assert_eq!(invalid_executed, 2);
    }
}
