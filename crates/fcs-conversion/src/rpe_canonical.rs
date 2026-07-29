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

use crate::rpe::{LAYER_LOSS, RpeEventInterpolation};
use crate::{
    ArtifactRole, ExactRational, RpeError, RpeLayerPolicy, RpeNoteKind, RpeNoteSide, RpeProfile,
    RpeSemanticCommonEvent, RpeSemanticEventLayer, RpeSemanticInterpretation, RpeSemanticLine,
    RpeSemanticSpeedEvent, RpeSpeedEra, SOURCE_INVALID, SourceArtifact,
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
    let mut tracks = Vec::new();
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
        let timing_line = timing.lines().get(line_index).ok_or_else(|| {
            RpeError::new(
                CANONICAL_INVALID,
                format!("judgeLineList[{line_index}]"),
                "semantic line and timing line counts differ",
            )
        })?;
        let event_layers = retained_event_layers(timing_line, semantic.layer_policy());
        let has_alpha_events = event_layers
            .iter()
            .any(|(_, layer)| !layer.alpha_events().is_empty());
        let has_speed_events = event_layers
            .iter()
            .any(|(_, layer)| !layer.speed_events().is_empty());
        lower_event_layers(
            &mut tracks,
            line_id,
            line_index,
            &event_layers,
            line.speed_era(),
        )?;
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
            if has_alpha_events { 0.0 } else { 1.0 },
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
            CanonicalScrollLine::new(
                line_id.clone(),
                coordinate,
                if has_speed_events { 0.0 } else { 1.0 },
                false,
                1.0,
                0.0,
                0.0,
            )
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
            let judgment_enabled = note.judgment_enabled();
            let (sound_policy, score_policy) = if judgment_enabled {
                (
                    CanonicalNoteSoundPolicy::Default,
                    CanonicalNoteScorePolicy::Default,
                )
            } else {
                (
                    CanonicalNoteSoundPolicy::None,
                    CanonicalNoteScorePolicy::None,
                )
            };
            let gameplay = CanonicalNoteGameplay::new(
                kind,
                line_id.clone(),
                start,
                end,
                side,
                judgment_enabled,
                fcs_model::CanonicalJudgeShape::LineDefault,
                sound_policy,
                score_policy,
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

#[derive(Clone, Copy)]
enum RpeTrackProperty {
    MoveX,
    MoveY,
    Rotation,
    Alpha,
    Speed,
}

impl RpeTrackProperty {
    const fn name(self) -> &'static str {
        match self {
            Self::MoveX => "moveX",
            Self::MoveY => "moveY",
            Self::Rotation => "rotate",
            Self::Alpha => "alpha",
            Self::Speed => "speed",
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

fn retained_event_layers(
    line: &RpeSemanticLine,
    policy: RpeLayerPolicy,
) -> Vec<(usize, &RpeSemanticEventLayer)> {
    let layers = line
        .event_layers()
        .iter()
        .enumerate()
        .filter_map(|(index, layer)| layer.as_ref().map(|layer| (index, layer)));
    match policy {
        RpeLayerPolicy::Additive => layers.collect(),
        RpeLayerPolicy::FirstOnly => layers.take(1).collect(),
    }
}

fn lower_event_layers(
    tracks: &mut Vec<CanonicalTrack>,
    owner: &StableId,
    line_index: usize,
    layers: &[(usize, &RpeSemanticEventLayer)],
    speed_era: RpeSpeedEra,
) -> Result<(), RpeError> {
    for (layer_index, layer) in layers {
        add_common_track(
            tracks,
            owner,
            line_index,
            *layer_index,
            RpeTrackProperty::MoveX,
            layer.move_x_events(),
        )?;
        add_common_track(
            tracks,
            owner,
            line_index,
            *layer_index,
            RpeTrackProperty::MoveY,
            layer.move_y_events(),
        )?;
        add_common_track(
            tracks,
            owner,
            line_index,
            *layer_index,
            RpeTrackProperty::Rotation,
            layer.rotate_events(),
        )?;
        add_common_track(
            tracks,
            owner,
            line_index,
            *layer_index,
            RpeTrackProperty::Alpha,
            layer.alpha_events(),
        )?;
        add_speed_track(
            tracks,
            owner,
            line_index,
            *layer_index,
            layer.speed_events(),
            speed_era,
        )?;
    }
    Ok(())
}

fn add_common_track(
    tracks: &mut Vec<CanonicalTrack>,
    owner: &StableId,
    line_index: usize,
    layer_index: usize,
    property: RpeTrackProperty,
    events: &[RpeSemanticCommonEvent],
) -> Result<(), RpeError> {
    if events.is_empty() {
        return Ok(());
    }
    let pieces = events
        .iter()
        .enumerate()
        .map(|(order, event)| {
            let path = format!(
                "judgeLineList[{line_index}].eventLayers[{layer_index}].{}Events[{order}]",
                property.name()
            );
            event_piece(
                event.start_time().chart_time_seconds(),
                event.end_time().chart_time_seconds(),
                common_value(event.start(), property, &path)?,
                common_value(event.end(), property, &path)?,
                canonical_interpolation(event.interpolation(), &path)?,
                order as u64,
                &path,
            )
        })
        .collect::<Result<Vec<_>, RpeError>>()?;
    tracks.push(rpe_track(owner, layer_index, property, pieces, line_index)?);
    Ok(())
}

fn add_speed_track(
    tracks: &mut Vec<CanonicalTrack>,
    owner: &StableId,
    line_index: usize,
    layer_index: usize,
    events: &[RpeSemanticSpeedEvent],
    speed_era: RpeSpeedEra,
) -> Result<(), RpeError> {
    if events.is_empty() {
        return Ok(());
    }
    let pieces = events
        .iter()
        .enumerate()
        .map(|(order, event)| {
            let path = format!(
                "judgeLineList[{line_index}].eventLayers[{layer_index}].speedEvents[{order}]"
            );
            let interpolation = match speed_era {
                RpeSpeedEra::LegacyLinear => CanonicalTrackInterpolation::Linear,
                RpeSpeedEra::ModernEased => {
                    canonical_interpolation(event.interpolation(), &path)?
                }
                RpeSpeedEra::LegacyDerivative if event.interpolation() == &RpeEventInterpolation::Linear => {
                    CanonicalTrackInterpolation::Linear
                }
                RpeSpeedEra::LegacyDerivative => {
                    return Err(RpeError::new(
                        crate::PROFILE_NOT_APPLICABLE,
                        path,
                        "RPE legacy derivative speed easing is not representable by a canonical Track",
                    ));
                }
            };
            event_piece(
                event.start_time().chart_time_seconds(),
                event.end_time().chart_time_seconds(),
                speed_value(event.start(), &path)?,
                speed_value(event.end(), &path)?,
                interpolation,
                order as u64,
                &path,
            )
        })
        .collect::<Result<Vec<_>, RpeError>>()?;
    tracks.push(rpe_track(
        owner,
        layer_index,
        RpeTrackProperty::Speed,
        pieces,
        line_index,
    )?);
    Ok(())
}

fn rpe_track(
    owner: &StableId,
    layer_index: usize,
    property: RpeTrackProperty,
    pieces: Vec<CanonicalTrackPiece>,
    line_index: usize,
) -> Result<CanonicalTrack, RpeError> {
    CanonicalTrack::new(
        owner.clone(),
        format!("rpe.layer.{layer_index}.{}", property.name()),
        property.target(),
        CanonicalTrackBlend::Add,
        layer_index as i64,
        CanonicalTrackFill::Zero,
        CanonicalTrackFill::Zero,
        CanonicalTrackFill::Zero,
        pieces,
    )
    .map_err(|error| {
        canonical_error(
            &format!("judgeLineList[{line_index}].eventLayers[{layer_index}]"),
            error,
        )
    })
}

fn common_value(
    value: &ExactRational,
    property: RpeTrackProperty,
    path: &str,
) -> Result<CanonicalTrackValue, RpeError> {
    let value = exact_f64(value, path)?;
    match property {
        RpeTrackProperty::MoveX => CanonicalVec2::new(value * 1920.0 / 1350.0, 0.0)
            .map(CanonicalTrackValue::Vec2Length)
            .map_err(|error| canonical_error(path, error)),
        RpeTrackProperty::MoveY => CanonicalVec2::new(0.0, value * 1080.0 / 900.0)
            .map(CanonicalTrackValue::Vec2Length)
            .map_err(|error| canonical_error(path, error)),
        RpeTrackProperty::Rotation => Ok(CanonicalTrackValue::Angle(
            -value * std::f64::consts::PI / 180.0,
        )),
        RpeTrackProperty::Alpha => Ok(CanonicalTrackValue::Float(value / 255.0)),
        RpeTrackProperty::Speed => unreachable!("speed events use speed_value"),
    }
}

fn speed_value(value: &ExactRational, path: &str) -> Result<CanonicalTrackValue, RpeError> {
    Ok(CanonicalTrackValue::Float(exact_f64(value, path)? / 4.5))
}

fn canonical_interpolation(
    interpolation: &RpeEventInterpolation,
    path: &str,
) -> Result<CanonicalTrackInterpolation, RpeError> {
    match interpolation {
        RpeEventInterpolation::Linear => Ok(CanonicalTrackInterpolation::Linear),
        RpeEventInterpolation::Core(name) => Ok(CanonicalTrackInterpolation::Easing(name.clone())),
        RpeEventInterpolation::CubicBezier(controls) => {
            let values = controls
                .iter()
                .map(|value| exact_f64(value, path))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(CanonicalTrackInterpolation::CubicBezier([
                values[0], values[1], values[2], values[3],
            ]))
        }
    }
}

fn event_piece(
    start: &ExactRational,
    end: &ExactRational,
    start_value: CanonicalTrackValue,
    end_value: CanonicalTrackValue,
    interpolation: CanonicalTrackInterpolation,
    order: u64,
    path: &str,
) -> Result<CanonicalTrackPiece, RpeError> {
    if end < start {
        return Err(RpeError::new(
            SOURCE_INVALID,
            path,
            "RPE event endTime must not precede startTime",
        ));
    }
    let canonical_start = canonical_time(start, path)?;
    let canonical_end = canonical_time(end, path)?;
    if start == end {
        if start_value != end_value {
            return Err(RpeError::new(
                crate::PROFILE_NOT_APPLICABLE,
                path,
                "zero-duration RPE event with distinct endpoints is unsupported",
            ));
        }
        return CanonicalTrackPoint::new(canonical_start, start_value, order)
            .map(CanonicalTrackPiece::Point)
            .map_err(|error| canonical_error(path, error));
    }
    if canonical_end.chart_time_seconds() <= canonical_start.chart_time_seconds() {
        return Err(RpeError::new(
            CANONICAL_INVALID,
            path,
            "exact RPE event interval collapses during canonical Float64 conversion",
        ));
    }
    CanonicalTrackSegment::new(
        canonical_start,
        canonical_end,
        start_value,
        end_value,
        interpolation,
        order,
    )
    .map(CanonicalTrackPiece::Segment)
    .map_err(|error| canonical_error(path, error))
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
        assert_eq!(chart.tracks().tracks().len(), 1);
        assert_eq!(chart.tracks().tracks()[0].name(), "rpe.layer.0.speed");
        assert_eq!(chart.tracks().tracks()[0].blend(), CanonicalTrackBlend::Add);
        assert_eq!(chart.tracks().tracks()[0].fill(), CanonicalTrackFill::Zero);
        assert_eq!(chart.scroll().lines()[0].speed(), 0.0);
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
    fn lowers_sparse_motion_layers_with_supported_shapes() {
        let chart = r#"{
            "META": {"RPEVersion": 170, "offset": 0},
            "BPMList": [{"startTime": [0,0,1], "bpm": 120}],
            "judgeLineList": [{
                "eventLayers": [null, {
                    "moveXEvents": [{"startTime": [0,0,1], "endTime": [2,0,1], "start": -675, "end": 675, "easingType": 2}],
                    "moveYEvents": [{"startTime": [0,0,1], "endTime": [2,0,1], "start": -450, "end": 450, "bezier": 1, "bezierPoints": [0.25, 0, 0.75, 1]}],
                    "rotateEvents": [{"startTime": [0,0,1], "endTime": [2,0,1], "start": 0, "end": 90}],
                    "alphaEvents": [{"startTime": [0,0,1], "endTime": [2,0,1], "start": 0, "end": 255}],
                    "speedEvents": [{"startTime": [0,0,1], "endTime": [2,0,1], "start": 1, "end": 2, "easingType": 2}]
                }],
                "notes": []
            }]
        }"#;
        let (semantic_doc, art) = {
            let art = artifact(chart);
            let parsed = parse_json_document(SourceFormat::Rpe, &art).unwrap();
            let source = parse_rpe_document(&parsed, RpeLimits::default()).unwrap();
            let semantic = interpret_rpe_semantics(
                &source,
                &RpeProfileBinding::phira_rpe170_speed(Some(crate::RpeVersionEra::AtLeast170)),
            )
            .unwrap();
            (semantic, art)
        };
        let import = lower_rpe_to_canonical(&semantic_doc, &art).unwrap();
        let tracks = import.compilation().chart().tracks().tracks();
        assert_eq!(tracks.len(), 5);
        assert_eq!(tracks[0].name(), "rpe.layer.1.moveX");
        assert!(tracks.iter().any(|track| {
            track.name() == "rpe.layer.1.moveY"
                && track.pieces().iter().any(|piece| {
                    matches!(
                        piece,
                        CanonicalTrackPiece::Segment(segment)
                            if matches!(segment.interpolation(), CanonicalTrackInterpolation::CubicBezier(_))
                    )
                })
        }));
        assert_eq!(
            import
                .compilation()
                .chart()
                .lines()
                .lines()
                .next()
                .unwrap()
                .base()
                .alpha(),
            0.0
        );
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
