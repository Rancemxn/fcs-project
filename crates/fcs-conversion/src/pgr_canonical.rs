//! I6.2b lowering from the profile-bound PGR semantic IR.
//!
//! This module only assembles existing source-free model types. It does not
//! select a profile, repair source data, resolve package resources, or retain
//! the source parse tree.

use std::fmt::Write as _;

use fcs_model::{
    AudioOffset, Beat, CanonicalChart, CanonicalColor, CanonicalCompilation, CanonicalLine,
    CanonicalLineBase, CanonicalLineGraph, CanonicalLineInherit, CanonicalMetadata, CanonicalNote,
    CanonicalNoteGameplay, CanonicalNoteKind, CanonicalNotePresentation, CanonicalNoteScorePolicy,
    CanonicalNoteSet, CanonicalNoteSide, CanonicalNoteSoundPolicy, CanonicalObject,
    CanonicalObjectEntry, CanonicalProfile, CanonicalResourceBundle, CanonicalScrollLine,
    CanonicalScrollSet, CanonicalScrollTempo, CanonicalScrollTempoMap, CanonicalScrollTempoPoint,
    CanonicalSourceVersion, CanonicalSync, CanonicalTime, CanonicalTrack, CanonicalTrackBlend,
    CanonicalTrackFill, CanonicalTrackInterpolation, CanonicalTrackPiece, CanonicalTrackPoint,
    CanonicalTrackSegment, CanonicalTrackSet, CanonicalTrackTarget, CanonicalTrackValue,
    CanonicalValue, CanonicalVec2, ConversionDomain, ConversionEntry, ConversionPhase,
    ConversionPolicy, ConversionReport, ConversionSeverity, ConversionStatus, DistributionMetadata,
    EntityKind, ExpansionPath, InputContentHash, LogicalSourceLocator, MappingRuleRef, OriginState,
    ProvenanceGraph, RepairMode, RestrictedProvenanceFact, ScrollTempoKey, SemanticStatus,
    StableId, StableIdRegistry, TempoPoint,
};
use sha2::{Digest, Sha256};

use crate::{
    ArtifactRole, ExactRational, PROFILE_NOT_APPLICABLE, PgrError, PgrProfile, PgrSemanticDocument,
    PgrSemanticMoveEvent, PgrSemanticScalarEvent, PgrSemanticSpeedEvent, SourceArtifact,
};

const CANONICAL_SOURCE_VERSION: &str = "5.0.0";
const CANONICAL_INVALID: &str = "conversion.source-invalid";
const UNSUPPORTED_SEMANTIC: &str = "conversion.capability-mismatch";

/// The products emitted by the PGR canonical assembly boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct PgrCanonicalImport {
    compilation: CanonicalCompilation,
    report: ConversionReport,
}

impl PgrCanonicalImport {
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

/// Assemble one validated profile-bound PGR document into the canonical model.
///
/// The source artifact is used only for identity and hash facts. Its bytes are
/// not copied into the canonical chart or distribution metadata.
pub fn lower_pgr_to_canonical(
    semantic: &PgrSemanticDocument,
    artifact: &SourceArtifact,
) -> Result<PgrCanonicalImport, PgrError> {
    if artifact.role() != ArtifactRole::Chart {
        return Err(PgrError::new(
            CANONICAL_INVALID,
            "sourceArtifact.role",
            "PGR canonical lowering requires a chart artifact",
        ));
    }
    if artifact.logical_id() != semantic.artifact_id() {
        return Err(PgrError::new(
            CANONICAL_INVALID,
            "sourceArtifact.logicalId",
            "source artifact identity does not match the semantic document",
        ));
    }

    let profile_ref = format!(
        "{}@{}",
        semantic.profile().id(),
        semantic.profile().version()
    );
    let artifact_hash = lower_hex(&artifact.content_sha256());
    let operation_id = operation_id(semantic, artifact, &profile_ref);
    let source_locator = artifact.logical_id().clone();

    let mut registry = StableIdRegistry::new();
    let line_ids = semantic
        .lines()
        .iter()
        .enumerate()
        .map(|(index, _)| generated_id(&mut registry, EntityKind::Line, "pgrLines", index, index))
        .collect::<Result<Vec<_>, _>>()?;

    let global_bpm = canonical_global_bpm(semantic)?;
    let time_map = fcs_model::ChartTimeMap::new([TempoPoint {
        beat: Beat::zero(),
        bpm: global_bpm,
    }])
    .map_err(|error| canonical_error("chart.timeMap", error))?;
    let floor_scale = exact_f64(semantic.floor_scale_px(), "profile.floorScalePx")?;
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
    let mut lines = Vec::with_capacity(semantic.lines().len());
    let mut scroll_lines = Vec::with_capacity(semantic.lines().len());
    let mut tracks = Vec::new();
    let mut notes = Vec::new();
    let mut facts = Vec::new();
    let mut entries = Vec::new();
    let artifact_fact = "pgr/artifact".to_owned();
    let profile_fact = "pgr/profile".to_owned();

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
        [artifact_fact],
    )?);
    facts.push(fact(
        "pgr/offset",
        artifact,
        Some(locator("offset")?),
        Some(semantic.audio_offset_seconds().to_string()),
        None,
        Some("pgr.offset.seconds@1.0.0"),
        OriginState::Imported,
        Some(SemanticStatus::Mapped),
        [profile_fact.clone()],
    )?);

    let mut note_order = 0u64;
    for (line_index, (line, line_id)) in semantic.lines().iter().zip(&line_ids).enumerate() {
        let line_fact = format!("pgr/line/{line_index}");
        facts.push(fact(
            &line_fact,
            artifact,
            Some(locator(format!("judgeLineList/{line_index}"))?),
            Some(line.source_bpm().to_string()),
            Some(line_index as u64),
            Some(if semantic.profile().is_phira() {
                "pgr.time.per-line-bpm@1.0.0"
            } else {
                "pgr.time.first-line-bpm@1.0.0"
            }),
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
            floor_scale,
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
                floor_scale,
                0.0,
                0.0,
            )
            .map_err(|error| canonical_error("line.scroll", error))?,
        );
        lines.push(canonical_line);

        add_track(
            &mut tracks,
            line_id,
            "pgr.position",
            CanonicalTrackTarget::Position,
            move_pieces(line.move_events(), line_index)?,
        )?;
        add_track(
            &mut tracks,
            line_id,
            "pgr.rotation",
            CanonicalTrackTarget::Rotation,
            scalar_pieces(line.rotate_events(), line_index, ScalarKind::Rotation)?,
        )?;
        add_track(
            &mut tracks,
            line_id,
            "pgr.alpha",
            CanonicalTrackTarget::Alpha,
            scalar_pieces(line.disappear_events(), line_index, ScalarKind::Alpha)?,
        )?;
        add_track_with_fills(
            &mut tracks,
            line_id,
            "pgr.speed",
            CanonicalTrackTarget::ScrollSpeed,
            speed_pieces(line.speed_events(), line_index)?,
            CanonicalTrackFill::Error,
            CanonicalTrackFill::HoldBefore,
            CanonicalTrackFill::HoldAfter,
        )?;

        for (side, side_name, side_notes) in [
            (CanonicalNoteSide::Above, "notesAbove", line.notes_above()),
            (CanonicalNoteSide::Below, "notesBelow", line.notes_below()),
        ] {
            for (source_index, note) in side_notes.iter().enumerate() {
                let note_id = generated_id(
                    &mut registry,
                    EntityKind::Note,
                    "pgrNotes",
                    note_order as usize,
                    note_order,
                )?;
                let note_path = format!("judgeLineList/{line_index}/{side_name}/{source_index}");
                let start = canonical_time(note.start_time().chart_time_seconds(), &note_path)?;
                let end = note
                    .end_time()
                    .map(|value| canonical_time(value.chart_time_seconds(), &note_path))
                    .transpose()?;
                let kind = canonical_note_kind(note.kind());
                let gameplay = CanonicalNoteGameplay::new(
                    kind,
                    line_id.clone(),
                    start,
                    end,
                    side,
                    true,
                    fcs_model::CanonicalJudgeShape::LineDefault,
                    CanonicalNoteSoundPolicy::Default,
                    CanonicalNoteScorePolicy::Default,
                )
                .map_err(|error| canonical_error(&note_path, error))?;
                let presentation = CanonicalNotePresentation::new(
                    exact_f64(note.position_x_px(), &format!("{note_path}.positionX"))?,
                    exact_f64(note.scroll_factor(), &format!("{note_path}.scrollFactor"))?,
                    0.0,
                    0.0,
                    1.0,
                    1.0,
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
                    CanonicalNote::new(note_id, kind, note_order, gameplay, presentation)
                        .map_err(|error| canonical_error(&note_path, error))?,
                );
                facts.push(fact(
                    &format!("pgr/note/{note_order}"),
                    artifact,
                    Some(locator(&note_path)?),
                    Some(note.position_x_px().to_string()),
                    Some(note_order),
                    Some(note_rule(semantic.profile())),
                    OriginState::Imported,
                    Some(SemanticStatus::Mapped),
                    [line_fact.clone()],
                )?);
                note_order = note_order.saturating_add(1);
            }
        }
    }

    if semantic.profile().strict_eligible()
        && !semantic
            .lines()
            .iter()
            .all(|line| line.source_bpm() == semantic.lines()[0].source_bpm())
    {
        entries.push(generated_tempo_entry(semantic, global_bpm)?);
    }
    let compatibility = !semantic.profile().strict_eligible();
    if compatibility {
        entries.push(compatibility_entry(semantic)?);
    }

    let line_graph =
        CanonicalLineGraph::new(lines).map_err(|error| canonical_error("chart.lines", error))?;
    let note_set =
        CanonicalNoteSet::new(notes).map_err(|error| canonical_error("chart.notes", error))?;
    let track_set =
        CanonicalTrackSet::new(tracks).map_err(|error| canonical_error("chart.tracks", error))?;
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
            "formatVersion",
            CanonicalValue::Int(i64::from(semantic.format_version().as_u8())),
        ),
        CanonicalObjectEntry::new(
            "floorScalePx",
            CanonicalValue::String(semantic.floor_scale_px().to_string()),
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
    Ok(PgrCanonicalImport {
        compilation,
        report,
    })
}

fn canonical_global_bpm(semantic: &PgrSemanticDocument) -> Result<f64, PgrError> {
    let first = exact_f64(semantic.lines()[0].source_bpm(), "judgeLineList/0/bpm")?;
    if !semantic.profile().is_phira() {
        return Ok(first);
    }
    let all_equal = semantic
        .lines()
        .iter()
        .all(|line| line.source_bpm() == semantic.lines()[0].source_bpm());
    if all_equal { Ok(first) } else { Ok(60.0) }
}

fn operation_id(
    semantic: &PgrSemanticDocument,
    artifact: &SourceArtifact,
    profile_ref: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(artifact.content_sha256());
    hasher.update([0]);
    hasher.update(profile_ref.as_bytes());
    hasher.update([0]);
    hasher.update(semantic.floor_scale_px().to_string().as_bytes());
    format!("pgr-import-{}", lower_hex(&hasher.finalize()))
}

fn generated_id(
    registry: &mut StableIdRegistry,
    kind: EntityKind,
    collection: &str,
    item_order: usize,
    output_order: u64,
) -> Result<StableId, PgrError> {
    let path = ExpansionPath::new(collection, item_order as u64)
        .map_err(|error| canonical_error("canonical.id", error))?;
    registry
        .insert(
            kind,
            fcs_model::CanonicalTextualId::generated(kind, &path, output_order),
        )
        .map_err(|error| canonical_error("canonical.id", error))
}

fn add_track(
    tracks: &mut Vec<CanonicalTrack>,
    owner: &StableId,
    name: &str,
    target: CanonicalTrackTarget,
    pieces: Vec<CanonicalTrackPiece>,
) -> Result<(), PgrError> {
    add_track_with_fills(
        tracks,
        owner,
        name,
        target,
        pieces,
        CanonicalTrackFill::HoldAfter,
        CanonicalTrackFill::Base,
        CanonicalTrackFill::HoldAfter,
    )
}

fn add_track_with_fills(
    tracks: &mut Vec<CanonicalTrack>,
    owner: &StableId,
    name: &str,
    target: CanonicalTrackTarget,
    pieces: Vec<CanonicalTrackPiece>,
    fill: CanonicalTrackFill,
    before: CanonicalTrackFill,
    after: CanonicalTrackFill,
) -> Result<(), PgrError> {
    if pieces.is_empty() {
        return Ok(());
    }
    tracks.push(
        CanonicalTrack::new(
            owner.clone(),
            name,
            target,
            CanonicalTrackBlend::Replace,
            0,
            fill,
            before,
            after,
            pieces,
        )
        .map_err(|error| canonical_error("chart.tracks", error))?,
    );
    Ok(())
}

fn move_pieces(
    events: &[PgrSemanticMoveEvent],
    line_index: usize,
) -> Result<Vec<CanonicalTrackPiece>, PgrError> {
    let mut pieces = Vec::new();
    let mut previous_end = None;
    for (order, event) in events.iter().enumerate() {
        let path = format!("judgeLineList/{line_index}/moveEvents/{order}");
        let (start, end) = event_interval(
            event.start_time().chart_time_seconds(),
            event.end_time().chart_time_seconds(),
            &path,
        )?;
        let start_value = CanonicalTrackValue::Vec2Length(
            CanonicalVec2::new(
                exact_f64(event.start_x_px(), &path)?,
                exact_f64(event.start_y_px(), &path)?,
            )
            .map_err(|error| canonical_error(&path, error))?,
        );
        let end_value = CanonicalTrackValue::Vec2Length(
            CanonicalVec2::new(
                exact_f64(event.end_x_px(), &path)?,
                exact_f64(event.end_y_px(), &path)?,
            )
            .map_err(|error| canonical_error(&path, error))?,
        );
        require_contiguous(previous_end, start, &path)?;
        pieces.push(piece(
            start,
            end,
            start_value,
            end_value,
            order as u64,
            &path,
        )?);
        previous_end = Some(end);
    }
    Ok(pieces)
}

#[derive(Clone, Copy)]
enum ScalarKind {
    Rotation,
    Alpha,
}

fn scalar_pieces(
    events: &[PgrSemanticScalarEvent],
    line_index: usize,
    kind: ScalarKind,
) -> Result<Vec<CanonicalTrackPiece>, PgrError> {
    let mut pieces = Vec::new();
    let mut previous_end = None;
    for (order, event) in events.iter().enumerate() {
        let field = match kind {
            ScalarKind::Rotation => "rotateEvents",
            ScalarKind::Alpha => "disappearEvents",
        };
        let path = format!("judgeLineList/{line_index}/{field}/{order}");
        let (start, end) = event_interval(
            event.start_time().chart_time_seconds(),
            event.end_time().chart_time_seconds(),
            &path,
        )?;
        let start_value = scalar_track_value(event.start_value(), kind, &path)?;
        let end_value = scalar_track_value(event.end_value(), kind, &path)?;
        require_contiguous(previous_end, start, &path)?;
        pieces.push(piece(
            start,
            end,
            start_value,
            end_value,
            order as u64,
            &path,
        )?);
        previous_end = Some(end);
    }
    Ok(pieces)
}

fn speed_pieces(
    events: &[PgrSemanticSpeedEvent],
    line_index: usize,
) -> Result<Vec<CanonicalTrackPiece>, PgrError> {
    let mut pieces = Vec::with_capacity(events.len());
    let mut previous_end: Option<CanonicalTime> = None;
    for (order, event) in events.iter().enumerate() {
        let path = format!("judgeLineList/{line_index}/speedEvents/{order}");
        let (start, end) = event_interval(
            event.start_time().chart_time_seconds(),
            event.end_time().chart_time_seconds(),
            &path,
        )?;
        require_contiguous(previous_end, start, &path)?;
        let value = exact_f64(event.value(), &path)?;
        pieces.push(CanonicalTrackPiece::Segment(
            CanonicalTrackSegment::new(
                start,
                end,
                CanonicalTrackValue::Float(value),
                CanonicalTrackValue::Float(value),
                CanonicalTrackInterpolation::Step,
                order as u64,
            )
            .map_err(|error| canonical_error(&path, error))?,
        ));
        previous_end = Some(end);
    }
    Ok(pieces)
}

fn require_contiguous(
    previous_end: Option<CanonicalTime>,
    start: CanonicalTime,
    path: &str,
) -> Result<(), PgrError> {
    let expected = previous_end.map_or(0.0, CanonicalTime::chart_time_seconds);
    if start.chart_time_seconds() != expected {
        return Err(PgrError::new(
            PROFILE_NOT_APPLICABLE,
            path,
            "the selected PGR profile does not define a nonzero first event or event gap",
        ));
    }
    Ok(())
}

fn piece(
    start: CanonicalTime,
    end: CanonicalTime,
    start_value: CanonicalTrackValue,
    end_value: CanonicalTrackValue,
    order: u64,
    path: &str,
) -> Result<CanonicalTrackPiece, PgrError> {
    if start.chart_time_seconds() == end.chart_time_seconds() {
        if start_value != end_value {
            return Err(PgrError::new(
                UNSUPPORTED_SEMANTIC,
                path,
                "zero-duration PGR event has distinct endpoints",
            ));
        }
        return Ok(CanonicalTrackPiece::Point(
            CanonicalTrackPoint::new(start, start_value, order)
                .map_err(|error| canonical_error(path, error))?,
        ));
    }
    if end.chart_time_seconds() < start.chart_time_seconds() {
        return Err(PgrError::new(
            UNSUPPORTED_SEMANTIC,
            path,
            "canonical Float64 conversion reversed an exact PGR interval",
        ));
    }
    Ok(CanonicalTrackPiece::Segment(
        CanonicalTrackSegment::new(
            start,
            end,
            start_value,
            end_value,
            CanonicalTrackInterpolation::Linear,
            order,
        )
        .map_err(|error| canonical_error(path, error))?,
    ))
}

fn scalar_value(value: &ExactRational, kind: ScalarKind, path: &str) -> Result<f64, PgrError> {
    let value = exact_f64(value, path)?;
    let value = match kind {
        ScalarKind::Rotation => value * std::f64::consts::PI,
        ScalarKind::Alpha => value,
    };
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| PgrError::new(CANONICAL_INVALID, path, "canonical scalar is not finite"))
}

fn scalar_track_value(
    value: &ExactRational,
    kind: ScalarKind,
    path: &str,
) -> Result<CanonicalTrackValue, PgrError> {
    let value = scalar_value(value, kind, path)?;
    Ok(match kind {
        ScalarKind::Rotation => CanonicalTrackValue::Angle(value),
        ScalarKind::Alpha => CanonicalTrackValue::Float(value),
    })
}

fn canonical_note_kind(kind: crate::PgrNoteKind) -> CanonicalNoteKind {
    match kind {
        crate::PgrNoteKind::Tap => CanonicalNoteKind::Tap,
        crate::PgrNoteKind::Drag => CanonicalNoteKind::Drag,
        crate::PgrNoteKind::Hold => CanonicalNoteKind::Hold,
        crate::PgrNoteKind::Flick => CanonicalNoteKind::Flick,
    }
}

fn note_rule(profile: PgrProfile) -> &'static str {
    if profile.is_phira() {
        "pgr.note-x.unit108@1.0.0"
    } else {
        "pgr.note-x.unit320_3@1.0.0"
    }
}

fn canonical_time(value: &ExactRational, path: &str) -> Result<CanonicalTime, PgrError> {
    CanonicalTime::from_chart_time_seconds(exact_f64(value, path)?)
        .map_err(|error| canonical_error(path, error))
}

fn event_interval(
    start: &ExactRational,
    end: &ExactRational,
    path: &str,
) -> Result<(CanonicalTime, CanonicalTime), PgrError> {
    let canonical_start = canonical_time(start, path)?;
    let canonical_end = canonical_time(end, path)?;
    if start < end && canonical_start.chart_time_seconds() >= canonical_end.chart_time_seconds() {
        return Err(PgrError::new(
            CANONICAL_INVALID,
            path,
            "exact PGR interval collapses during canonical Float64 conversion",
        ));
    }
    Ok((canonical_start, canonical_end))
}

fn exact_f64(value: &ExactRational, path: &str) -> Result<f64, PgrError> {
    value
        .to_f64()
        .map_err(|error| PgrError::new(CANONICAL_INVALID, path, error.to_string()))
}

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
) -> Result<RestrictedProvenanceFact, PgrError> {
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

fn locator(path: impl Into<String>) -> Result<LogicalSourceLocator, PgrError> {
    LogicalSourceLocator::new(path).map_err(|error| canonical_error("sourceLocator", error))
}

fn generated_tempo_entry(
    semantic: &PgrSemanticDocument,
    global_bpm: f64,
) -> Result<ConversionEntry, PgrError> {
    ConversionEntry::new(
        "pgr/generated-canonical-tempo",
        "conversion.generated-canonical-tempo",
        ConversionDomain::Timing,
        ConversionSeverity::Info,
        SemanticStatus::Mapped,
        ConversionPhase::Lowering,
        Some(locator("judgeLineList")?),
        Some(locator("canonical/timeMap")?),
        None,
        Some("tempo".into()),
        Some(mapping_rule("pgr.tempo.per-line-canonical-anchor@1.0.0")),
        Some(CanonicalValue::String(semantic.profile().id().into())),
        None,
        Some(CanonicalValue::Float(global_bpm)),
        None,
        "different PGR Line BPM values use the specified identity canonical tempo anchor",
        [],
    )
    .map_err(|error| canonical_error("report.entries", error))
}

fn compatibility_entry(semantic: &PgrSemanticDocument) -> Result<ConversionEntry, PgrError> {
    ConversionEntry::new(
        "pgr/compatibility-characterization",
        "conversion.compatibility-characterization",
        ConversionDomain::Profile,
        ConversionSeverity::Warning,
        SemanticStatus::Preserved,
        ConversionPhase::ProfileSelection,
        Some(locator("formatVersion")?),
        None,
        None,
        Some("profile".into()),
        None,
        Some(CanonicalValue::String(semantic.profile().id().into())),
        None,
        None,
        None,
        "the selected Phichain import profile is compatibility-characterized and not strict eligible",
        [],
    )
    .map_err(|error| canonical_error("report.entries", error))
}

fn canonical_error(path: &str, error: impl std::fmt::Display) -> PgrError {
    PgrError::new(CANONICAL_INVALID, path, error.to_string())
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use fcs_model::{CanonicalNoteKind, CanonicalNoteSide, ConversionStatus, OriginState};
    use serde::Deserialize;
    use serde_json::json;

    use super::*;
    use crate::{
        DecimalLimits, ExactDecimal, PgrLimits, PgrProfileBinding, SourceFormat,
        parse_json_document, parse_pgr_document,
    };

    #[derive(Deserialize)]
    struct VectorFile {
        schema_version: u8,
        vector: Vec<Vector>,
    }

    #[derive(Deserialize)]
    struct Vector {
        id: String,
        profile: String,
        expected_status: String,
        expected_lines: usize,
        expected_notes: usize,
        generated_tempo: bool,
    }

    fn chart(second_line_bpm: Option<u64>) -> Vec<u8> {
        let mut lines = vec![json!({
            "bpm": 120,
            "judgeLineMoveEvents": [
                {"startTime": 0, "endTime": 32, "start": 440260, "end": 440500}
            ],
            "judgeLineRotateEvents": [
                {"startTime": 0, "endTime": 32, "start": 0, "end": 90}
            ],
            "judgeLineDisappearEvents": [
                {"startTime": 0, "endTime": 32, "start": 0, "end": 1}
            ],
            "speedEvents": [
                {"startTime": 0, "endTime": 64, "value": 2, "floorPosition": 0}
            ],
            "notesAbove": [
                {"type": 3, "time": 32, "holdTime": 32, "positionX": 1, "speed": 4, "floorPosition": 1}
            ],
            "notesBelow": [
                {"type": 1, "time": 0, "holdTime": 0, "positionX": -1, "speed": 2, "floorPosition": 0}
            ]
        })];
        if let Some(bpm) = second_line_bpm {
            lines.push(json!({
                "bpm": bpm,
                "judgeLineMoveEvents": [
                    {"startTime": 0, "endTime": 32, "start": 440260, "end": 440260}
                ],
                "judgeLineRotateEvents": [],
                "judgeLineDisappearEvents": [],
                "speedEvents": [
                    {"startTime": 0, "endTime": 64, "value": 1, "floorPosition": 0}
                ],
                "notesAbove": [],
                "notesBelow": []
            }));
        }
        serde_json::to_vec(&json!({
            "formatVersion": 1,
            "offset": 0.125,
            "judgeLineList": lines
        }))
        .unwrap()
    }

    fn semantic(profile: PgrProfile, bytes: Vec<u8>) -> (SourceArtifact, PgrSemanticDocument) {
        let artifact = SourceArtifact::new("chart.json", ArtifactRole::Chart, bytes).unwrap();
        let parsed = parse_json_document(SourceFormat::Pgr, &artifact).unwrap();
        let source = parse_pgr_document(&parsed, PgrLimits::default()).unwrap();
        let floor_scale = ExactDecimal::parse("120", DecimalLimits::default()).unwrap();
        let binding = PgrProfileBinding::new(profile, floor_scale).unwrap();
        let semantic = crate::interpret_pgr(&source, &binding).unwrap();
        (artifact, semantic)
    }

    fn lower(
        profile: PgrProfile,
        second_line_bpm: Option<u64>,
    ) -> (SourceArtifact, PgrSemanticDocument, PgrCanonicalImport) {
        let (artifact, semantic) = semantic(profile, chart(second_line_bpm));
        let lowered = lower_pgr_to_canonical(&semantic, &artifact).unwrap();
        (artifact, semantic, lowered)
    }

    #[test]
    fn checked_in_vectors_bind_canonical_profile_order_provenance_and_report() {
        let vectors: VectorFile = toml::from_str(include_str!(
            "../../../docs/conformance/conversion/pgr-canonical-vectors.toml"
        ))
        .unwrap();
        assert_eq!(vectors.schema_version, 1);

        for vector in vectors.vector {
            let profile = match vector.profile.as_str() {
                "pgr.phira.v1" => PgrProfile::PhiraV1,
                "pgr.phichain-import.v1" => PgrProfile::PhichainImportV1,
                other => panic!("unexpected checked-in profile {other}"),
            };
            let second_line_bpm = (vector.id == "per-line-bpm-anchor").then_some(60);
            let (artifact, semantic, output) = lower(profile, second_line_bpm);
            let chart = output.compilation().chart();
            assert_eq!(chart.lines().lines().count(), vector.expected_lines);
            assert_eq!(chart.notes().notes().len(), vector.expected_notes);
            assert_eq!(
                output.report().status(),
                ConversionStatus::parse(&vector.expected_status).unwrap()
            );
            assert_eq!(
                output
                    .report()
                    .entries()
                    .iter()
                    .any(|entry| entry.category() == "conversion.generated-canonical-tempo"),
                vector.generated_tempo
            );
            assert_eq!(
                chart.metadata().sync().unwrap().audio_offset().seconds(),
                0.125
            );
            assert!(output.compilation().resources().is_empty());
            assert!(
                chart
                    .lines()
                    .line_by_textual_id("generated/line/collection/pgrLines/item/0/order/0")
                    .is_some()
            );
            let hold = chart
                .notes()
                .note_by_textual_id("generated/note/collection/pgrNotes/item/0/order/0")
                .unwrap();
            assert_eq!(hold.kind(), CanonicalNoteKind::Hold);
            assert_eq!(hold.gameplay().side(), CanonicalNoteSide::Above);
            assert!(hold.gameplay().end_time().is_some());
            let below = chart
                .notes()
                .note_by_textual_id("generated/note/collection/pgrNotes/item/1/order/1")
                .unwrap();
            assert_eq!(below.gameplay().side(), CanonicalNoteSide::Below);
            for expected_track in ["pgr.position", "pgr.rotation", "pgr.alpha", "pgr.speed"] {
                assert!(
                    chart
                        .tracks()
                        .tracks()
                        .iter()
                        .any(|track| track.name() == expected_track)
                );
            }

            let provenance = output.compilation().distribution().provenance();
            let artifact_fact = provenance.get("pgr/artifact").unwrap();
            assert_eq!(artifact_fact.source_artifact_id(), Some("chart.json"));
            assert_eq!(artifact_fact.origin_state(), OriginState::Imported);
            assert_eq!(artifact_fact.source_value().unwrap().len(), 64);
            provenance.validate_dependency_closure().unwrap();
            assert_eq!(
                lower_pgr_to_canonical(&semantic, &artifact).unwrap(),
                output,
                "{} must assemble deterministically",
                vector.id
            );
        }
    }

    #[test]
    fn canonical_lowering_rejects_mismatched_artifact_identity() {
        let (_, semantic, _) = lower(PgrProfile::PhiraV1, None);
        let other = SourceArtifact::new("other.json", ArtifactRole::Chart, chart(None)).unwrap();
        let error = lower_pgr_to_canonical(&semantic, &other).unwrap_err();
        assert_eq!(error.category(), CANONICAL_INVALID);
        assert_eq!(error.path(), "sourceArtifact.logicalId");
    }

    #[test]
    fn zero_duration_event_with_distinct_endpoints_is_an_explicit_capability_failure() {
        let mut source_json: serde_json::Value = serde_json::from_slice(&chart(None)).unwrap();
        source_json["judgeLineList"][0]["judgeLineRotateEvents"][0]["endTime"] = json!(0);
        let (artifact, semantic) = semantic(
            PgrProfile::PhiraV1,
            serde_json::to_vec(&source_json).unwrap(),
        );
        let error = lower_pgr_to_canonical(&semantic, &artifact).unwrap_err();
        assert_eq!(error.category(), UNSUPPORTED_SEMANTIC);
        assert_eq!(error.path(), "judgeLineList/0/rotateEvents/0");
    }

    #[test]
    fn ordinary_event_gap_without_profile_semantics_is_rejected() {
        let mut source_json: serde_json::Value = serde_json::from_slice(&chart(None)).unwrap();
        source_json["judgeLineList"][0]["judgeLineRotateEvents"][0]["startTime"] = json!(1);
        let (artifact, semantic) = semantic(
            PgrProfile::PhiraV1,
            serde_json::to_vec(&source_json).unwrap(),
        );
        let error = lower_pgr_to_canonical(&semantic, &artifact).unwrap_err();
        assert_eq!(error.category(), PROFILE_NOT_APPLICABLE);
        assert_eq!(error.path(), "judgeLineList/0/rotateEvents/0");
    }

    #[test]
    fn exact_interval_that_collapses_to_one_float_is_rejected() {
        let mut source_json: serde_json::Value = serde_json::from_slice(&chart(None)).unwrap();
        let event = &mut source_json["judgeLineList"][0]["judgeLineRotateEvents"][0];
        event["startTime"] = json!(9_007_199_254_740_992_u64);
        event["endTime"] = json!(9_007_199_254_740_993_u64);
        source_json["judgeLineList"][0]["speedEvents"][0]["endTime"] =
            json!(9_007_199_254_740_993_u64);
        let (artifact, semantic) = semantic(
            PgrProfile::PhiraV1,
            serde_json::to_vec(&source_json).unwrap(),
        );
        let error = lower_pgr_to_canonical(&semantic, &artifact).unwrap_err();
        assert_eq!(error.category(), CANONICAL_INVALID);
        assert_eq!(error.path(), "judgeLineList/0/rotateEvents/0");
    }
}
