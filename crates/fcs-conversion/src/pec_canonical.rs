//! I6.4 PEC canonical assembly from profile-bound semantic IR.

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

use crate::pec::{
    PecError, PecNoteKind, PecNoteSide, PecProfile, PecSemanticDocument, SOURCE_INVALID,
};
use crate::{ArtifactRole, ExactRational, SourceArtifact};

const CANONICAL_SOURCE_VERSION: &str = "5.0.0";
const CANONICAL_INVALID: &str = "conversion.source-invalid";

#[derive(Debug, Clone, PartialEq)]
pub struct PecCanonicalImport {
    compilation: CanonicalCompilation,
    report: ConversionReport,
}

impl PecCanonicalImport {
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

pub fn lower_pec_to_canonical(
    semantic: &PecSemanticDocument,
    artifact: &SourceArtifact,
) -> Result<PecCanonicalImport, PecError> {
    if artifact.role() != ArtifactRole::Chart {
        return Err(PecError::new(
            CANONICAL_INVALID,
            "sourceArtifact.role",
            "PEC canonical lowering requires a chart artifact",
        ));
    }
    if artifact.logical_id() != semantic.artifact_id() {
        return Err(PecError::new(
            CANONICAL_INVALID,
            "sourceArtifact.logicalId",
            "source artifact identity does not match the semantic document",
        ));
    }
    if artifact.content_sha256() != semantic.artifact_content_sha256() {
        return Err(PecError::new(
            CANONICAL_INVALID,
            "sourceArtifact.contentSha256",
            "source artifact content does not match the semantic document",
        ));
    }
    if semantic.bpm_points().is_empty() {
        return Err(PecError::new(
            SOURCE_INVALID,
            "bp",
            "PEC canonical lowering requires at least one bp point",
        ));
    }

    let profile = semantic.profile();
    let profile_ref = format!("{}@{}", profile.id(), profile.version());
    let artifact_hash = lower_hex(artifact.content_sha256());
    let operation_id = operation_id(semantic.artifact_content_sha256(), &profile_ref);
    let source_locator = artifact.logical_id().clone();

    let line_count = semantic.max_line_index().saturating_add(1);
    let mut registry = StableIdRegistry::new();
    let line_ids = (0..line_count)
        .map(|index| {
            generated_id(
                &mut registry,
                EntityKind::Line,
                "pecLines",
                index,
                index as u64,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let time_map = build_time_map(semantic.bpm_points())?;
    let offset = exact_f64(semantic.audio_offset_seconds(), "offset")?;
    let sync = CanonicalSync::new(
        None,
        AudioOffset::new(offset).map_err(|error| canonical_error("offset", error))?,
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

    let mut lines = Vec::with_capacity(line_count);
    let mut scroll_lines = Vec::with_capacity(line_count);
    let mut notes = Vec::new();
    let mut facts = Vec::new();
    let mut entries = Vec::new();
    let artifact_fact = "pec/artifact".to_owned();
    let profile_fact = "pec/profile".to_owned();

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

    for (line_index, line_id) in line_ids.iter().enumerate() {
        let line_fact = format!("pec/line/{line_index}");
        facts.push(fact(
            &line_fact,
            artifact,
            Some(locator(format!("line/{line_index}"))?),
            Some(line_index.to_string()),
            Some(line_index as u64),
            Some("pec.time.direct-beat@1.0.0"),
            OriginState::Imported,
            Some(SemanticStatus::Mapped),
            [profile_fact.clone()],
        )?);
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
            exact_f64(semantic.floor_scale_px(), "floorScalePx")?,
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
            None,
            line_index as u64,
            base,
            CanonicalLineInherit::default(),
            scroll_tempo.clone(),
        )
        .map_err(|error| canonical_error("line", error))?;
        let coordinate = fcs_model::coordinate_for_tempo(&scroll_tempo, &time_map)
            .map_err(|error| canonical_error("line.scrollCoordinate", error))?;
        scroll_lines.push(
            CanonicalScrollLine::new(
                line_id.clone(),
                coordinate,
                1.0,
                false,
                exact_f64(semantic.floor_scale_px(), "floorScalePx")?,
                0.0,
                0.0,
            )
            .map_err(|error| canonical_error("line.scroll", error))?,
        );
        lines.push(canonical_line);
    }

    for (note_order, note) in semantic.notes().iter().enumerate() {
        let line_id = line_ids.get(note.line_index()).cloned().ok_or_else(|| {
            PecError::new(
                SOURCE_INVALID,
                format!("note/{note_order}"),
                "Note line index is out of range",
            )
        })?;
        let note_id = generated_id(
            &mut registry,
            EntityKind::Note,
            "pecNotes",
            note_order,
            note_order as u64,
        )?;
        let note_path = format!("note/{note_order}");
        let start = canonical_time(note.start_time().chart_time_seconds(), &note_path)?;
        let end = note
            .end_time()
            .map(|value| canonical_time(value.chart_time_seconds(), &note_path))
            .transpose()?;
        let kind = match note.kind() {
            PecNoteKind::Tap => CanonicalNoteKind::Tap,
            PecNoteKind::Hold => CanonicalNoteKind::Hold,
            PecNoteKind::Flick => CanonicalNoteKind::Flick,
            PecNoteKind::Drag => CanonicalNoteKind::Drag,
        };
        let side = match note.side() {
            PecNoteSide::Above => CanonicalNoteSide::Above,
            PecNoteSide::Below => CanonicalNoteSide::Below,
        };
        let gameplay = CanonicalNoteGameplay::new(
            kind,
            line_id,
            start,
            end,
            side,
            note.judgment_enabled(),
            fcs_model::CanonicalJudgeShape::LineDefault,
            CanonicalNoteSoundPolicy::Default,
            CanonicalNoteScorePolicy::Default,
        )
        .map_err(|error| canonical_error(&note_path, error))?;
        let presentation = CanonicalNotePresentation::new(
            exact_f64(note.position_x_px(), &format!("{note_path}.x"))?,
            exact_f64(note.speed_factor(), &format!("{note_path}.speed"))?,
            0.0,
            0.0,
            exact_f64(note.width_factor(), &format!("{note_path}.width"))?,
            exact_f64(note.width_factor(), &format!("{note_path}.width"))?,
            1.0,
            0.0,
            CanonicalColor::rgba(255, 255, 255, 255),
            None,
            true,
            None,
            None,
        )
        .map_err(|error| canonical_error(&note_path, error))?;
        notes.push(
            CanonicalNote::new(note_id, kind, note_order as u64, gameplay, presentation)
                .map_err(|error| canonical_error(&note_path, error))?,
        );
        facts.push(fact(
            &format!("pec/note/{note_order}"),
            artifact,
            Some(locator(&note_path)?),
            Some(note.position_x_px().to_string()),
            Some(note_order as u64),
            Some("pec.note-x.relative2048@1.0.0"),
            OriginState::Imported,
            Some(SemanticStatus::Mapped),
            [format!("pec/line/{}", note.line_index())],
        )?);
    }

    let compatibility = !profile.strict_eligible();
    if compatibility {
        entries.push(
            ConversionEntry::new(
                "pec/compatibility-characterization",
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
                "the selected PEC profile is compatibility-characterized and not strict eligible",
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
    let custom = CanonicalObject::new(vec![CanonicalObjectEntry::new(
        "profile",
        CanonicalValue::String(profile_ref.clone()),
    )])
    .map_err(|error| canonical_error("distribution.custom", error))?;
    let provenance = ProvenanceGraph::new(facts)
        .map_err(|error| canonical_error("distribution.provenance", error))?;
    let input_hash = InputContentHash::sha256_lower_hex(artifact_hash, Some(source_locator))
        .map_err(|error| canonical_error("distribution.inputHash", error))?;
    let distribution = DistributionMetadata::new(provenance, Vec::new(), vec![input_hash], custom)
        .map_err(|error| canonical_error("distribution", error))?;
    let compilation = CanonicalCompilation::new(
        chart,
        CanonicalResourceBundle::new(Vec::new()).expect("empty resource bundle is valid"),
        distribution,
    );
    let status = if compatibility {
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
    Ok(PecCanonicalImport {
        compilation,
        report,
    })
}

fn build_time_map(
    points: &[crate::pec::PecSemanticBpm],
) -> Result<fcs_model::ChartTimeMap, PecError> {
    let mut tempo_points = Vec::with_capacity(points.len());
    for (index, point) in points.iter().enumerate() {
        let beat = beat_from_exact(point.start_beat(), &format!("bp[{index}].beat"))?;
        let bpm = exact_f64(point.bpm(), &format!("bp[{index}].bpm"))?;
        tempo_points.push(TempoPoint { beat, bpm });
    }
    fcs_model::ChartTimeMap::new(tempo_points)
        .map_err(|error| canonical_error("chart.timeMap", error))
}

fn beat_from_exact(value: &ExactRational, path: &str) -> Result<Beat, PecError> {
    let numerator = value.numerator().parse::<i64>().map_err(|_| {
        PecError::new(
            SOURCE_INVALID,
            path,
            "Beat numerator is not a bounded integer",
        )
    })?;
    let denominator = value.denominator().parse::<i64>().map_err(|_| {
        PecError::new(
            SOURCE_INVALID,
            path,
            "Beat denominator is not a bounded integer",
        )
    })?;
    Beat::new(numerator, denominator).map_err(|error| canonical_error(path, error))
}

fn canonical_time(seconds: &ExactRational, path: &str) -> Result<CanonicalTime, PecError> {
    CanonicalTime::from_chart_time_seconds(exact_f64(seconds, path)?)
        .map_err(|error| canonical_error(path, error))
}

fn exact_f64(value: &ExactRational, path: &str) -> Result<f64, PecError> {
    value
        .to_f64()
        .map_err(|error| PecError::new(CANONICAL_INVALID, path, error.to_string()))
}

fn generated_id(
    registry: &mut StableIdRegistry,
    kind: EntityKind,
    collection: &str,
    item_order: usize,
    output_order: u64,
) -> Result<StableId, PecError> {
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
    format!("pec-import-{}", lower_hex(hasher.finalize()))
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
) -> Result<RestrictedProvenanceFact, PecError> {
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

fn locator(path: impl Into<String>) -> Result<LogicalSourceLocator, PecError> {
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

fn canonical_error(path: &str, error: impl std::fmt::Display) -> PecError {
    PecError::new(CANONICAL_INVALID, path, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pec::{PecLimits, PecProfile, PecProfileBinding, interpret_pec, parse_pec_document};
    use crate::{ArtifactRole, DecimalLimits, ExactDecimal, SourceArtifact};

    const SIMPLE: &str =
        "0\nbp 0.00 120\nn1 0 1.00 1024 1 0\n# 1.000\n& 1.000\nn2 0 2.00 3.00 0 1 0\n";

    fn artifact(bytes: &str) -> SourceArtifact {
        SourceArtifact::new("charts/main.pec", ArtifactRole::Chart, bytes.as_bytes()).unwrap()
    }

    fn semantic(bytes: &str) -> (PecSemanticDocument, SourceArtifact) {
        let art = artifact(bytes);
        let source = parse_pec_document(&art, PecLimits::default()).unwrap();
        let binding = PecProfileBinding::new(
            PecProfile::Phira,
            ExactDecimal::parse("100", DecimalLimits::default()).unwrap(),
        )
        .unwrap();
        (interpret_pec(&source, &binding).unwrap(), art)
    }

    #[test]
    fn lowers_simple_pec_with_notes() {
        let (semantic_doc, art) = semantic(SIMPLE);
        let import = lower_pec_to_canonical(&semantic_doc, &art).unwrap();
        let chart = import.compilation().chart();
        assert_eq!(chart.lines().lines().count(), 1);
        assert_eq!(chart.notes().notes().len(), 2);
        assert_eq!(import.report().status(), ConversionStatus::Equivalent);
        let tap = chart
            .notes()
            .notes()
            .iter()
            .find(|note| note.kind() == CanonicalNoteKind::Tap)
            .unwrap();
        assert!((tap.gameplay().time().chart_time_seconds() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn rejects_identity_mismatch() {
        let (semantic_doc, _) = semantic(SIMPLE);
        let other = SourceArtifact::new("charts/other.pec", ArtifactRole::Chart, SIMPLE.as_bytes())
            .unwrap();
        assert_eq!(
            lower_pec_to_canonical(&semantic_doc, &other)
                .unwrap_err()
                .path(),
            "sourceArtifact.logicalId"
        );
    }

    #[test]
    fn reassembly_is_stable() {
        let (semantic_doc, art) = semantic(SIMPLE);
        let first = lower_pec_to_canonical(&semantic_doc, &art).unwrap();
        let second = lower_pec_to_canonical(&semantic_doc, &art).unwrap();
        assert_eq!(first.compilation(), second.compilation());
        assert_eq!(
            first.report().operation_id(),
            second.report().operation_id()
        );
    }
}
