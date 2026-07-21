//! I6.3c lowering from profile-bound RPE semantic interpretation.
//!
//! Assembles existing source-free model types only. Does not select profiles,
//! repair source, resolve package resources, or retain the source parse tree.

use std::fmt::Write as _;

use fcs_model::{
    AudioOffset, Beat, CanonicalChart, CanonicalColor, CanonicalCompilation, CanonicalLine,
    CanonicalLineBase, CanonicalLineGraph, CanonicalLineInherit, CanonicalMetadata, CanonicalNote,
    CanonicalNoteGameplay, CanonicalNoteKind, CanonicalNotePresentation, CanonicalNoteScorePolicy,
    CanonicalNoteSet, CanonicalNoteSide, CanonicalNoteSoundPolicy, CanonicalObject,
    CanonicalObjectEntry, CanonicalProfile, CanonicalResourceBundle, CanonicalScrollLine,
    CanonicalScrollSet, CanonicalScrollTempo, CanonicalScrollTempoMap, CanonicalScrollTempoPoint,
    CanonicalSourceVersion, CanonicalSync, CanonicalTime, CanonicalTrackSet, CanonicalValue,
    CanonicalVec2, ConversionDomain, ConversionEntry, ConversionPhase, ConversionPolicy,
    ConversionReport, ConversionSeverity, ConversionStatus, DistributionMetadata, EntityKind,
    ExpansionPath, InputContentHash, LogicalSourceLocator, MappingRuleRef, OriginState,
    ProvenanceGraph, RepairMode, RestrictedProvenanceFact, ScrollTempoKey, SemanticStatus,
    StableId, StableIdRegistry, TempoPoint,
};
use sha2::{Digest, Sha256};

use crate::rpe::LAYER_LOSS;
use crate::{
    ArtifactRole, ExactRational, RpeError, RpeNoteKind, RpeNoteSide, RpeProfile,
    RpeSemanticInterpretation, SOURCE_INVALID, SourceArtifact,
};

const CANONICAL_SOURCE_VERSION: &str = "5.0.0";
const CANONICAL_INVALID: &str = "conversion.source-invalid";

/// Products of the RPE canonical assembly boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct RpeCanonicalImport {
    compilation: CanonicalCompilation,
    report: ConversionReport,
}

impl RpeCanonicalImport {
    pub fn compilation(&self) -> &CanonicalCompilation {
        &self.compilation
    }

    pub fn report(&self) -> &ConversionReport {
        &self.report
    }

    pub fn into_parts(self) -> (CanonicalCompilation, ConversionReport) {
        (self.compilation, self.report)
    }
}

/// Assemble one validated RPE semantic interpretation into the canonical model.
pub fn lower_rpe_to_canonical(
    semantic: &RpeSemanticInterpretation,
    artifact: &SourceArtifact,
) -> Result<RpeCanonicalImport, RpeError> {
    if artifact.role() != ArtifactRole::Chart {
        return Err(RpeError::new(
            CANONICAL_INVALID,
            "sourceArtifact.role",
            "RPE canonical lowering requires a chart artifact",
        ));
    }
    let timing = semantic.timing();
    if artifact.logical_id() != timing.artifact_id() {
        return Err(RpeError::new(
            CANONICAL_INVALID,
            "sourceArtifact.logicalId",
            "source artifact identity does not match the semantic document",
        ));
    }
    if artifact.content_sha256() != timing.artifact_content_sha256() {
        return Err(RpeError::new(
            CANONICAL_INVALID,
            "sourceArtifact.contentSha256",
            "source artifact content does not match the semantic document",
        ));
    }
    if timing.bpm_points().is_empty() {
        return Err(RpeError::new(
            SOURCE_INVALID,
            "$.BPMList",
            "RPE canonical lowering requires at least one BPMList point",
        ));
    }

    let profile = timing.profile();
    let profile_ref = format!("{}@{}", profile.id(), profile.version());
    let artifact_hash = lower_hex(artifact.content_sha256());
    let operation_id = operation_id(timing.artifact_content_sha256(), &profile_ref);
    let source_locator = artifact.logical_id().clone();

    let mut registry = StableIdRegistry::new();
    let line_ids = semantic
        .lines()
        .iter()
        .enumerate()
        .map(|(index, _)| {
            generated_id(
                &mut registry,
                EntityKind::Line,
                "rpeLines",
                index,
                index as u64,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let time_map = build_time_map(timing.bpm_points())?;
    let offset_ms = exact_f64(timing.audio_offset_milliseconds(), "META.offset")?;
    let offset_seconds = offset_ms / 1000.0;
    let sync = CanonicalSync::new(
        None,
        AudioOffset::new(offset_seconds).map_err(|error| canonical_error("offset", error))?,
        None,
    )
    .map_err(|error| canonical_error("sync", error))?;
    let metadata = CanonicalMetadata::new(
        None,
        Default::default(),
        Vec::new(),
        Default::default(),
        None,
        Some(sync),
    );

    let mut lines = Vec::with_capacity(semantic.lines().len());
    let mut scroll_lines = Vec::with_capacity(semantic.lines().len());
    let mut notes = Vec::new();
    let mut facts = Vec::new();
    let mut entries = Vec::new();
    let artifact_fact = "rpe/artifact".to_owned();
    let profile_fact = "rpe/profile".to_owned();

    facts.push(fact(
        &artifact_fact,
        artifact,
        Some(source_locator.clone()),
        Some(artifact_hash.clone()),
        None,
        None,
        OriginState::Imported,
        Some(SemanticStatus::Native),
        [],
    )?);
    facts.push(fact(
        &profile_fact,
        artifact,
        Some(source_locator.clone()),
        Some(profile_ref.clone()),
        None,
        None,
        OriginState::Imported,
        Some(SemanticStatus::Mapped),
        [artifact_fact.clone()],
    )?);

    let mut note_order = 0u64;
    for (line_index, (line, line_id)) in semantic.lines().iter().zip(&line_ids).enumerate() {
        let line_fact = format!("rpe/line/{line_index}");
        facts.push(fact(
            &line_fact,
            artifact,
            Some(locator(format!("judgeLineList/{line_index}"))?),
            Some(line.bpmfactor().to_string()),
            Some(line_index as u64),
            Some(factor_rule(profile)),
            OriginState::Imported,
            Some(SemanticStatus::Mapped),
            [profile_fact.clone()],
        )?);

        let parent = line
            .father()
            .map(|parent_index| {
                line_ids.get(parent_index).cloned().ok_or_else(|| {
                    RpeError::new(
                        SOURCE_INVALID,
                        format!("$.judgeLineList[{line_index}].father"),
                        "father index is out of range after semantic interpretation",
                    )
                })
            })
            .transpose()?;
        let inherit = CanonicalLineInherit::new(true, line.rotate_with_father(), true, true, true);
        let base = CanonicalLineBase::new(
            CanonicalVec2::new(0.0, 0.0)
                .map_err(|error| canonical_error("line.position", error))?,
            0.0,
            CanonicalVec2::new(1.0, 1.0).map_err(|error| canonical_error("line.scale", error))?,
            1.0,
            CanonicalVec2::new(0.0, 0.0)
                .map_err(|error| canonical_error("line.transformOrigin", error))?,
            CanonicalVec2::new(0.5, 0.5)
                .map_err(|error| canonical_error("line.textureAnchor", error))?,
            1.0,
            0.0,
            0.0,
            false,
            0,
        )
        .map_err(|error| canonical_error("line.base", error))?;
        let scroll_tempo = CanonicalScrollTempo::Override(
            CanonicalScrollTempoMap::new([CanonicalScrollTempoPoint::new(
                ScrollTempoKey::Time(0.0),
                60.0,
            )
            .map_err(|error| canonical_error("line.scrollTempo", error))?])
            .map_err(|error| canonical_error("line.scrollTempo", error))?,
        );
        let canonical_line = CanonicalLine::new(
            line_id.clone(),
            parent,
            line_index as u64,
            base,
            inherit,
            scroll_tempo.clone(),
        )
        .map_err(|error| canonical_error("line", error))?;
        let coordinate = fcs_model::coordinate_for_tempo(&scroll_tempo, &time_map)
            .map_err(|error| canonical_error("line.scrollCoordinate", error))?;
        scroll_lines.push(
            CanonicalScrollLine::new(line_id.clone(), coordinate, 1.0, false, 1.0, 0.0, 0.0)
                .map_err(|error| canonical_error("line.scroll", error))?,
        );
        lines.push(canonical_line);

        for note in line.notes() {
            let note_id = generated_id(
                &mut registry,
                EntityKind::Note,
                "rpeNotes",
                note_order as usize,
                note_order,
            )?;
            let note_path = format!("judgeLineList/{line_index}/notes/{note_order}");
            let start = canonical_time(note.start_time().chart_time_seconds(), &note_path)?;
            let end = if note.kind() == RpeNoteKind::Hold {
                Some(canonical_time(
                    note.end_time().chart_time_seconds(),
                    &note_path,
                )?)
            } else {
                None
            };
            let kind = canonical_note_kind(note.kind());
            let side = match note.side() {
                RpeNoteSide::Above => CanonicalNoteSide::Above,
                RpeNoteSide::Below => CanonicalNoteSide::Below,
            };
            let gameplay = CanonicalNoteGameplay::new(
                kind,
                line_id.clone(),
                start,
                end,
                side,
                note.judgment_enabled(),
                fcs_model::CanonicalJudgeShape::LineDefault,
                CanonicalNoteSoundPolicy::Default,
                CanonicalNoteScorePolicy::Default,
            )
            .map_err(|error| canonical_error(&note_path, error))?;
            let alpha = note
                .linear_alpha()
                .map(|value| exact_f64(value, &format!("{note_path}.alpha")))
                .transpose()?
                .unwrap_or(1.0);
            let scale = note
                .scale()
                .map(|(x, y)| {
                    Ok((
                        exact_f64(x, &format!("{note_path}.size"))?,
                        exact_f64(y, &format!("{note_path}.size"))?,
                    ))
                })
                .transpose()?
                .unwrap_or((1.0, 1.0));
            let offset_y = note
                .offset_y_logical_px()
                .map(|value| exact_f64(value, &format!("{note_path}.yOffset")))
                .transpose()?
                .unwrap_or(0.0);
            let presentation = CanonicalNotePresentation::new(
                exact_f64(note.position_x(), &format!("{note_path}.positionX"))?,
                exact_f64(note.canonical_speed(), &format!("{note_path}.speed"))?,
                0.0,
                offset_y,
                scale.0,
                scale.1,
                alpha,
                0.0,
                CanonicalColor::rgba(255, 255, 255, 255),
                None,
                true,
                None,
                None,
            )
            .map_err(|error| canonical_error(&note_path, error))?;
            notes.push(
                CanonicalNote::new(note_id, kind, note_order, gameplay, presentation)
                    .map_err(|error| canonical_error(&note_path, error))?,
            );
            facts.push(fact(
                &format!("rpe/note/{note_order}"),
                artifact,
                Some(locator(&note_path)?),
                Some(note.position_x().to_string()),
                Some(note_order),
                Some("rpe.x.canvas1350@1.0.0"),
                OriginState::Imported,
                Some(SemanticStatus::Mapped),
                [line_fact.clone()],
            )?);
            note_order = note_order.saturating_add(1);
        }

        if line.dropped_layer_count() > 0 {
            entries.push(
                ConversionEntry::new(
                    format!("rpe/layer-loss/{line_index}"),
                    LAYER_LOSS,
                    ConversionDomain::Presentation,
                    ConversionSeverity::Warning,
                    SemanticStatus::Preserved,
                    ConversionPhase::Lowering,
                    Some(locator(format!("judgeLineList/{line_index}/eventLayers"))?),
                    None,
                    None,
                    Some("eventLayers".into()),
                    Some(mapping_rule("rpe.layers.first-only@1.0.0")),
                    None,
                    None,
                    None,
                    None,
                    format!(
                        "retained {} of {} present event layers under first-only projection",
                        line.retained_layer_count(),
                        line.retained_layer_count() + line.dropped_layer_count()
                    ),
                    [],
                )
                .map_err(|error| canonical_error("report.layer-loss", error))?,
            );
        }
    }

    let compatibility = matches!(profile, RpeProfile::PhichainImport);
    if compatibility {
        entries.push(
            ConversionEntry::new(
                "rpe/compatibility-characterization",
                "conversion.compatibility-characterization",
                ConversionDomain::Profile,
                ConversionSeverity::Warning,
                SemanticStatus::Preserved,
                ConversionPhase::ProfileSelection,
                Some(source_locator.clone()),
                None,
                None,
                Some("profile".into()),
                None,
                Some(CanonicalValue::String(profile.id().into())),
                None,
                None,
                None,
                "the selected RPE profile is compatibility-characterized and not strict eligible",
                [],
            )
            .map_err(|error| canonical_error("report.compatibility", error))?,
        );
    }

    let line_graph =
        CanonicalLineGraph::new(lines).map_err(|error| canonical_error("chart.lines", error))?;
    let note_set =
        CanonicalNoteSet::new(notes).map_err(|error| canonical_error("chart.notes", error))?;
    let track_set = CanonicalTrackSet::new(Vec::new())
        .map_err(|error| canonical_error("chart.tracks", error))?;
    let scroll_set = CanonicalScrollSet::new(scroll_lines)
        .map_err(|error| canonical_error("chart.scroll", error))?;
    let chart = CanonicalChart::new(
        CanonicalSourceVersion::new(CANONICAL_SOURCE_VERSION)
            .map_err(|error| canonical_error("chart.sourceVersion", error))?,
        CanonicalProfile::Chart,
        [],
        time_map,
        metadata,
        line_graph,
        note_set,
        track_set,
        scroll_set,
        [],
    );
    let custom = CanonicalObject::new(vec![
        CanonicalObjectEntry::new("profile", CanonicalValue::String(profile_ref.clone())),
        CanonicalObjectEntry::new(
            "layerPolicy",
            CanonicalValue::String(format!("{:?}", semantic.layer_policy())),
        ),
    ])
    .map_err(|error| canonical_error("distribution.custom", error))?;
    let provenance = ProvenanceGraph::new(facts)
        .map_err(|error| canonical_error("distribution.provenance", error))?;
    let input_hash = InputContentHash::sha256_lower_hex(artifact_hash, Some(source_locator))
        .map_err(|error| canonical_error("distribution.inputHash", error))?;
    let distribution = DistributionMetadata::new(provenance, Vec::new(), vec![input_hash], custom)
        .map_err(|error| canonical_error("distribution", error))?;
    let compilation = CanonicalCompilation::new(
        chart,
        CanonicalResourceBundle::new(Vec::new()).expect("an empty resource bundle is valid"),
        distribution,
    );

    let status = if compatibility || semantic.layer_loss_reported() {
        ConversionStatus::PreservedOnly
    } else {
        ConversionStatus::Equivalent
    };
    let report = ConversionReport::new(
        operation_id,
        ConversionPolicy::Semantic,
        RepairMode::disabled(),
        entries,
        Vec::new(),
        [status],
        None,
    )
    .map_err(|error| canonical_error("report", error))?;
    Ok(RpeCanonicalImport {
        compilation,
        report,
    })
}

fn build_time_map(
    points: &[crate::RpeSemanticBpmPoint],
) -> Result<fcs_model::ChartTimeMap, RpeError> {
    let mut tempo_points = Vec::with_capacity(points.len());
    for (index, point) in points.iter().enumerate() {
        let beat = beat_from_exact(point.start_beat(), &format!("BPMList[{index}].startTime"))?;
        let bpm = exact_f64(point.bpm(), &format!("BPMList[{index}].bpm"))?;
        tempo_points.push(TempoPoint { beat, bpm });
    }
    fcs_model::ChartTimeMap::new(tempo_points)
        .map_err(|error| canonical_error("chart.timeMap", error))
}

fn beat_from_exact(value: &ExactRational, path: &str) -> Result<Beat, RpeError> {
    let numerator = value.numerator().parse::<i64>().map_err(|_| {
        RpeError::new(
            SOURCE_INVALID,
            path,
            "Beat numerator is not a bounded integer",
        )
    })?;
    let denominator = value.denominator().parse::<i64>().map_err(|_| {
        RpeError::new(
            SOURCE_INVALID,
            path,
            "Beat denominator is not a bounded integer",
        )
    })?;
    Beat::new(numerator, denominator).map_err(|error| canonical_error(path, error))
}

fn factor_rule(profile: RpeProfile) -> &'static str {
    match profile.factor_mode() {
        crate::RpeFactorMode::Divide => "rpe.time.bpmfactor-divide@1.0.0",
        crate::RpeFactorMode::Multiply => "rpe.time.bpmfactor-multiply@1.0.0",
        crate::RpeFactorMode::Ignore => "rpe.time.bpmfactor-ignore@1.0.0",
    }
}

fn canonical_note_kind(kind: RpeNoteKind) -> CanonicalNoteKind {
    match kind {
        RpeNoteKind::Tap => CanonicalNoteKind::Tap,
        RpeNoteKind::Hold => CanonicalNoteKind::Hold,
        RpeNoteKind::Flick => CanonicalNoteKind::Flick,
        RpeNoteKind::Drag => CanonicalNoteKind::Drag,
    }
}

fn canonical_time(seconds: &ExactRational, path: &str) -> Result<CanonicalTime, RpeError> {
    CanonicalTime::from_chart_time_seconds(exact_f64(seconds, path)?)
        .map_err(|error| canonical_error(path, error))
}

fn exact_f64(value: &ExactRational, path: &str) -> Result<f64, RpeError> {
    value
        .to_f64()
        .map_err(|error| RpeError::new(CANONICAL_INVALID, path, error.to_string()))
}

fn generated_id(
    registry: &mut StableIdRegistry,
    kind: EntityKind,
    collection: &str,
    item_order: usize,
    output_order: u64,
) -> Result<StableId, RpeError> {
    let path = ExpansionPath::new(collection, item_order as u64)
        .map_err(|error| canonical_error("canonical.id", error))?;
    registry
        .insert(
            kind,
            fcs_model::CanonicalTextualId::generated(kind, &path, output_order),
        )
        .map_err(|error| canonical_error("canonical.id", error))
}

fn operation_id(content_sha256: [u8; 32], profile_ref: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content_sha256);
    hasher.update([0]);
    hasher.update(profile_ref.as_bytes());
    format!("rpe-import-{}", lower_hex(hasher.finalize()))
}

#[allow(clippy::too_many_arguments)]
fn fact(
    id: &str,
    artifact: &SourceArtifact,
    locator: Option<LogicalSourceLocator>,
    value: Option<String>,
    order: Option<u64>,
    rule: Option<&str>,
    origin: OriginState,
    status: Option<SemanticStatus>,
    dependencies: impl IntoIterator<Item = String>,
) -> Result<RestrictedProvenanceFact, RpeError> {
    RestrictedProvenanceFact::new(
        id,
        Some(artifact.logical_id().as_str().to_owned()),
        locator,
        value,
        order,
        rule.map(mapping_rule),
        origin,
        status,
        dependencies,
    )
    .map_err(|error| canonical_error("distribution.provenance", error))
}

fn mapping_rule(id: &str) -> MappingRuleRef {
    MappingRuleRef::new(id).expect("checked-in mapping rule IDs are valid")
}

fn locator(path: impl Into<String>) -> Result<LogicalSourceLocator, RpeError> {
    LogicalSourceLocator::new(path).map_err(|error| canonical_error("sourceLocator", error))
}

fn lower_hex(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn canonical_error(path: &str, error: impl std::fmt::Display) -> RpeError {
    RpeError::new(CANONICAL_INVALID, path, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ArtifactRole, RpeLimits, RpeProfileBinding, SourceArtifact, SourceFormat,
        interpret_rpe_semantics, parse_json_document, parse_rpe_document,
    };

    const MINIMAL: &str = r#"{
        "META": {"RPEVersion": 150, "offset": 0, "name": "c"},
        "BPMList": [
            {"startTime": [0, 0, 1], "bpm": 120},
            {"startTime": [4, 0, 1], "bpm": 180}
        ],
        "judgeLineList": [
            {
                "bpmfactor": 1,
                "eventLayers": [{"speedEvents": [
                    {"startTime": [0,0,1], "endTime": [4,0,1], "start": 1, "end": 1}
                ]}],
                "notes": [
                    {
                        "type": 1,
                        "startTime": [1, 0, 1],
                        "endTime": [1, 0, 1],
                        "positionX": 0,
                        "speed": 4.5,
                        "above": 1,
                        "isFake": 0
                    },
                    {
                        "type": 2,
                        "startTime": [2, 0, 1],
                        "endTime": [3, 0, 1],
                        "positionX": 100,
                        "speed": 4.5,
                        "above": 0
                    }
                ],
                "father": -1
            },
            {
                "father": 0,
                "rotateWithFather": true,
                "notes": []
            }
        ]
    }"#;

    fn artifact(bytes: &str) -> SourceArtifact {
        SourceArtifact::new(
            "charts/main.rpe.json",
            ArtifactRole::Chart,
            bytes.as_bytes(),
        )
        .unwrap()
    }

    fn semantic(bytes: &str) -> (RpeSemanticInterpretation, SourceArtifact) {
        let art = artifact(bytes);
        let parsed = parse_json_document(SourceFormat::Rpe, &art).unwrap();
        let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
        let semantic =
            interpret_rpe_semantics(&source, &RpeProfileBinding::phira_legacy_speed()).unwrap();
        (semantic, art)
    }

    #[test]
    fn lowers_minimal_rpe_chart_with_parent_and_notes() {
        let (semantic_doc, art) = semantic(MINIMAL);
        let import = lower_rpe_to_canonical(&semantic_doc, &art).unwrap();
        let chart = import.compilation().chart();
        assert_eq!(chart.lines().lines().count(), 2);
        assert_eq!(chart.notes().notes().len(), 2);
        let lines: Vec<_> = chart.lines().lines().collect();
        let child = lines
            .iter()
            .find(|line| line.parent().is_some())
            .expect("one Line should reference a parent");
        let parent_id = child.parent().unwrap().value();
        assert!(
            lines
                .iter()
                .any(|line| line.id().value() == parent_id && line.parent().is_none())
        );
        assert!(child.inherit().rotation());
        assert_eq!(import.report().status(), ConversionStatus::Equivalent);
        let notes = chart.notes().notes();
        let tap = notes
            .iter()
            .find(|note| note.kind() == CanonicalNoteKind::Tap)
            .unwrap();
        let hold = notes
            .iter()
            .find(|note| note.kind() == CanonicalNoteKind::Hold)
            .unwrap();
        assert!((tap.gameplay().time().chart_time_seconds() - 0.5).abs() < 1e-12);
        assert!(hold.gameplay().end_time().is_some());
    }

    #[test]
    fn rejects_artifact_identity_mismatch() {
        let (semantic_doc, _) = semantic(MINIMAL);
        let other = SourceArtifact::new(
            "charts/other.rpe.json",
            ArtifactRole::Chart,
            MINIMAL.as_bytes(),
        )
        .unwrap();
        assert_eq!(
            lower_rpe_to_canonical(&semantic_doc, &other)
                .unwrap_err()
                .path(),
            "sourceArtifact.logicalId"
        );
    }

    #[test]
    fn reassembly_is_stable_for_same_inputs() {
        let (semantic_doc, art) = semantic(MINIMAL);
        let first = lower_rpe_to_canonical(&semantic_doc, &art).unwrap();
        let second = lower_rpe_to_canonical(&semantic_doc, &art).unwrap();
        assert_eq!(first.compilation(), second.compilation());
        assert_eq!(
            first.report().operation_id(),
            second.report().operation_id()
        );
    }

    #[test]
    fn phichain_layer_loss_is_preserved_only() {
        let chart = r#"{
            "META": {"offset": 0},
            "BPMList": [{"startTime": [0,0,1], "bpm": 120}],
            "judgeLineList": [{
                "eventLayers": [
                    {"moveXEvents": []},
                    {"moveXEvents": []}
                ],
                "notes": []
            }]
        }"#;
        let art = artifact(chart);
        let parsed = parse_json_document(SourceFormat::Rpe, &art).unwrap();
        let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
        let semantic =
            interpret_rpe_semantics(&source, &RpeProfileBinding::phichain_import()).unwrap();
        assert!(semantic.layer_loss_reported());
        let import = lower_rpe_to_canonical(&semantic, &art).unwrap();
        assert_eq!(import.report().status(), ConversionStatus::PreservedOnly);
        assert!(
            import
                .report()
                .entries()
                .iter()
                .any(|entry| entry.category() == LAYER_LOSS)
        );
    }
}
