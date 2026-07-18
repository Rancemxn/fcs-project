//! I3.5 expanded-source-to-canonical Note lowering and deterministic ordering.

use fcs_model::{
    Beat as CanonicalBeat, CanonicalColor, CanonicalJudgeShape, CanonicalLineGraph, CanonicalNote,
    CanonicalNoteError, CanonicalNoteGameplay, CanonicalNoteKind, CanonicalNotePresentation,
    CanonicalNoteScorePolicy, CanonicalNoteSet, CanonicalNoteSide, CanonicalNoteSoundPolicy,
    CanonicalTime, CanonicalVec2, ChartTimeMap, StableId,
};

use crate::ast::{ExpandedEntity, ExpandedSourceDocument, NoteVariant, Type, TypedValue};
use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage};

impl ExpandedSourceDocument {
    /// Lowers expanded Notes into immutable canonical values and the normative sort order.
    pub fn canonical_notes(
        &self,
        time_map: &ChartTimeMap,
        lines: &CanonicalLineGraph,
    ) -> Result<CanonicalNoteSet, Vec<Diagnostic>> {
        lower_notes(self, time_map, lines)
    }
}

fn lower_notes(
    document: &ExpandedSourceDocument,
    time_map: &ChartTimeMap,
    lines: &CanonicalLineGraph,
) -> Result<CanonicalNoteSet, Vec<Diagnostic>> {
    let ids = match document.canonical_note_ids_with_spans() {
        Ok(ids) => ids,
        Err((error, span)) => return Err(vec![identity_diagnostic(error, span)]),
    };
    let mut diagnostics = Vec::new();
    let mut notes = Vec::new();
    let mut note_index = 0;
    let mut document_order = 0_u64;

    for collection in document.collections() {
        for entity in collection.entities() {
            if entity.entity_type() != &Type::Note {
                document_order += 1;
                continue;
            }
            let (id, _id_span) = ids
                .get(note_index)
                .cloned()
                .expect("canonical_note_ids and expanded Notes have equal cardinality");
            note_index += 1;
            if let Some(note) = lower_note(
                document,
                entity,
                id,
                document_order,
                time_map,
                lines,
                &mut diagnostics,
            ) {
                notes.push(note);
            }
            document_order += 1;
        }
    }

    if !diagnostics.is_empty() {
        sort_diagnostics(&mut diagnostics);
        return Err(diagnostics);
    }
    CanonicalNoteSet::new(notes).map_err(|error| {
        let span = match &error {
            CanonicalNoteError::DuplicateId { id } => ids
                .iter()
                .find_map(|(stable_id, span)| (stable_id.value() == *id).then_some(*span))
                .expect("duplicate canonical Note ID must have an identity span"),
            _ => ids
                .first()
                .map(|(_, span)| *span)
                .expect("canonical Note error requires at least one Note"),
        };
        vec![note_diagnostic(error, span)]
    })
}

fn lower_note(
    document: &ExpandedSourceDocument,
    entity: &ExpandedEntity,
    id: StableId,
    document_order: u64,
    time_map: &ChartTimeMap,
    lines: &CanonicalLineGraph,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalNote> {
    let kind = match entity.variant() {
        Some(NoteVariant::Tap) => CanonicalNoteKind::Tap,
        Some(NoteVariant::Hold) => CanonicalNoteKind::Hold,
        Some(NoteVariant::Flick) => CanonicalNoteKind::Flick,
        Some(NoteVariant::Drag) => CanonicalNoteKind::Drag,
        None => {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::SCHEMA_NON_CONSTRUCTIBLE,
                DiagnosticStage::Canonical,
                "Note is missing its Core kind",
                entity.span(),
            ));
            return None;
        }
    };

    let line_name = match required_value(entity, "line", diagnostics) {
        Some(TypedValue::Line(name)) => name,
        Some(value) => {
            type_mismatch(
                "line",
                "Line",
                value,
                field_span(entity, "line"),
                diagnostics,
            );
            return None;
        }
        None => return None,
    };
    let line = match lines.line_by_textual_id(line_name) {
        Some(line) => line,
        None => {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::GRAPH_UNKNOWN_PARENT,
                DiagnosticStage::Canonical,
                format!("Note refers to unknown Line {line_name}"),
                field_span(entity, "line"),
            ));
            return None;
        }
    };

    let time = time_from_field(entity, "gameplay.time", time_map, diagnostics, true)?;
    let end_time = time_from_field(entity, "gameplay.endTime", time_map, diagnostics, false);
    let side =
        string_field(entity, "gameplay.side", "above", diagnostics).and_then(
            |value| match value.as_str() {
                "above" => Some(CanonicalNoteSide::Above),
                "below" => Some(CanonicalNoteSide::Below),
                _ => {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::TYPE_INVALID_OPERATION,
                        DiagnosticStage::Canonical,
                        "gameplay.side must be above or below",
                        field_span(entity, "gameplay.side"),
                    ));
                    None
                }
            },
        )?;
    let judgment_enabled = bool_field(entity, "gameplay.judgment.enabled", true, diagnostics)?;
    let judge_shape = lower_shape(entity, diagnostics)?;
    let sound_policy = lower_sound_policy(entity, judgment_enabled, document, diagnostics)?;
    let score_policy = lower_score_policy(entity, judgment_enabled, document, diagnostics)?;
    let gameplay = match CanonicalNoteGameplay::new(
        kind,
        line.id().clone(),
        time,
        end_time,
        side,
        judgment_enabled,
        judge_shape,
        sound_policy,
        score_policy,
    ) {
        Ok(gameplay) => gameplay,
        Err(error) => {
            diagnostics.push(note_diagnostic(error, entity.span()));
            return None;
        }
    };

    let presentation = lower_presentation(entity, time_map, diagnostics)?;
    CanonicalNote::new(id, kind, document_order, gameplay, presentation)
        .map_err(|error| {
            diagnostics.push(note_diagnostic(error, entity.span()));
        })
        .ok()
}

fn lower_shape(
    entity: &ExpandedEntity,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalJudgeShape> {
    let kind = string_field(
        entity,
        "gameplay.judgeShape.kind",
        "lineDefault",
        diagnostics,
    )?;
    let center_present = entity.field("gameplay.judgeShape.center").is_some();
    let half_extents_present = entity.field("gameplay.judgeShape.halfExtents").is_some();
    let radius_present = entity.field("gameplay.judgeShape.radius").is_some();
    match kind.as_str() {
        "lineDefault" => {
            if center_present || half_extents_present || radius_present {
                diagnostics.push(non_constructible(
                    "lineDefault judgeShape does not allow geometry fields",
                    entity.span(),
                ));
                None
            } else {
                Some(CanonicalJudgeShape::LineDefault)
            }
        }
        "rectangle" => {
            if radius_present {
                diagnostics.push(non_constructible(
                    "rectangle judgeShape does not allow radius",
                    field_span(entity, "gameplay.judgeShape.radius"),
                ));
                return None;
            }
            let center = vec2_length_field(
                entity,
                "gameplay.judgeShape.center",
                (0.0, 0.0),
                diagnostics,
            )?;
            let half_extents = match vec2_length_field(
                entity,
                "gameplay.judgeShape.halfExtents",
                (0.0, 0.0),
                diagnostics,
            ) {
                Some(value) if half_extents_present => value,
                _ => {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::SCHEMA_MISSING_REQUIRED_FIELD,
                        DiagnosticStage::Canonical,
                        "rectangle judgeShape requires halfExtents",
                        entity.span(),
                    ));
                    return None;
                }
            };
            Some(CanonicalJudgeShape::Rectangle {
                center,
                half_extents,
            })
        }
        "circle" => {
            if half_extents_present {
                diagnostics.push(non_constructible(
                    "circle judgeShape does not allow halfExtents",
                    field_span(entity, "gameplay.judgeShape.halfExtents"),
                ));
                return None;
            }
            let center = vec2_length_field(
                entity,
                "gameplay.judgeShape.center",
                (0.0, 0.0),
                diagnostics,
            )?;
            let radius = match entity.field("gameplay.judgeShape.radius") {
                Some(field) => match field.value() {
                    TypedValue::Length(value) => *value,
                    value => {
                        type_mismatch(
                            "gameplay.judgeShape.radius",
                            "length",
                            value,
                            field.span(),
                            diagnostics,
                        );
                        return None;
                    }
                },
                None => {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::SCHEMA_MISSING_REQUIRED_FIELD,
                        DiagnosticStage::Canonical,
                        "circle judgeShape requires radius",
                        entity.span(),
                    ));
                    return None;
                }
            };
            Some(CanonicalJudgeShape::Circle { center, radius })
        }
        _ => {
            diagnostics.push(non_constructible(
                "unknown judgeShape kind",
                field_span(entity, "gameplay.judgeShape.kind"),
            ));
            None
        }
    }
}

fn lower_sound_policy(
    entity: &ExpandedEntity,
    judgment_enabled: bool,
    document: &ExpandedSourceDocument,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalNoteSoundPolicy> {
    let default = if judgment_enabled { "default" } else { "none" };
    let policy = string_field(entity, "gameplay.soundPolicy", default, diagnostics)?;
    let resource = optional_string(entity, "gameplay.soundResource", diagnostics)?;
    match policy.as_str() {
        "default" => {
            if resource.is_some() {
                diagnostics.push(non_constructible(
                    "soundResource requires resource soundPolicy",
                    field_span(entity, "gameplay.soundResource"),
                ));
                None
            } else {
                Some(CanonicalNoteSoundPolicy::Default)
            }
        }
        "none" => {
            if resource.is_some() {
                diagnostics.push(non_constructible(
                    "soundResource is forbidden by none soundPolicy",
                    field_span(entity, "gameplay.soundResource"),
                ));
                None
            } else {
                Some(CanonicalNoteSoundPolicy::None)
            }
        }
        "resource" => match resource {
            Some(resource) if !resource.is_empty() => match document.resource_kind(&resource) {
                Some(crate::ast::ResourceKind::Audio) => {
                    Some(CanonicalNoteSoundPolicy::Resource(resource))
                }
                Some(_) => {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::RESOURCE_TYPE_MISMATCH,
                        DiagnosticStage::Canonical,
                        format!("soundResource {resource} must reference an audio resource"),
                        field_span(entity, "gameplay.soundResource"),
                    ));
                    None
                }
                None => {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE,
                        DiagnosticStage::Canonical,
                        format!("unknown soundResource {resource}"),
                        field_span(entity, "gameplay.soundResource"),
                    ));
                    None
                }
            },
            _ => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::SCHEMA_MISSING_REQUIRED_FIELD,
                    DiagnosticStage::Canonical,
                    "resource soundPolicy requires soundResource",
                    entity.span(),
                ));
                None
            }
        },
        _ => None,
    }
}

fn lower_score_policy(
    entity: &ExpandedEntity,
    judgment_enabled: bool,
    document: &ExpandedSourceDocument,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalNoteScorePolicy> {
    let default = if judgment_enabled { "default" } else { "none" };
    let policy = string_field(entity, "gameplay.scorePolicy", default, diagnostics)?;
    let extension = optional_string(entity, "gameplay.scoreExtension", diagnostics)?;
    match policy.as_str() {
        "default" => {
            if extension.is_some() {
                diagnostics.push(non_constructible(
                    "scoreExtension requires custom scorePolicy",
                    field_span(entity, "gameplay.scoreExtension"),
                ));
                None
            } else {
                Some(CanonicalNoteScorePolicy::Default)
            }
        }
        "none" => {
            if extension.is_some() {
                diagnostics.push(non_constructible(
                    "scoreExtension is forbidden by none scorePolicy",
                    field_span(entity, "gameplay.scoreExtension"),
                ));
                None
            } else {
                Some(CanonicalNoteScorePolicy::None)
            }
        }
        "custom" => match extension {
            Some(extension) if !extension.is_empty() => {
                if document.has_required_extension(&extension) {
                    Some(CanonicalNoteScorePolicy::Custom(extension))
                } else {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::EXTENSION_UNSUPPORTED_REQUIRED,
                        DiagnosticStage::Canonical,
                        format!("score extension {extension} is not required by the document"),
                        field_span(entity, "gameplay.scoreExtension"),
                    ));
                    None
                }
            }
            _ => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::SCHEMA_MISSING_REQUIRED_FIELD,
                    DiagnosticStage::Canonical,
                    "custom scorePolicy requires scoreExtension",
                    entity.span(),
                ));
                None
            }
        },
        _ => None,
    }
}

fn lower_presentation(
    entity: &ExpandedEntity,
    time_map: &ChartTimeMap,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalNotePresentation> {
    let position_x = length_field(entity, "presentation.positionX", 0.0, diagnostics)?;
    let scroll_factor = float_field(entity, "presentation.scrollFactor", 1.0, diagnostics)?;
    let x_offset = length_field(entity, "presentation.xOffset", 0.0, diagnostics)?;
    let y_offset = length_field(entity, "presentation.yOffset", 0.0, diagnostics)?;
    let alpha = float_field(entity, "presentation.alpha", 1.0, diagnostics)?;
    let scale_x = float_field(entity, "presentation.scaleX", 1.0, diagnostics)?;
    let scale_y = float_field(entity, "presentation.scaleY", 1.0, diagnostics)?;
    let rotation = angle_field(entity, "presentation.rotation", 0.0, diagnostics)?;
    let color = match entity.field("presentation.color") {
        Some(field) => match field.value() {
            TypedValue::Color(value) => CanonicalColor::rgba(value.r, value.g, value.b, value.a),
            value => {
                type_mismatch(
                    "presentation.color",
                    "color",
                    value,
                    field.span(),
                    diagnostics,
                );
                return None;
            }
        },
        None => CanonicalColor::rgba(255, 255, 255, 255),
    };
    let texture = optional_string(entity, "presentation.texture", diagnostics)?;
    let render_enabled = bool_field(entity, "render.enabled", true, diagnostics)?;
    let visible_from = time_from_field(
        entity,
        "presentation.visibleFrom",
        time_map,
        diagnostics,
        false,
    );
    let visible_until = time_from_field(
        entity,
        "presentation.visibleUntil",
        time_map,
        diagnostics,
        false,
    );
    match CanonicalNotePresentation::new(
        position_x,
        scroll_factor,
        x_offset,
        y_offset,
        alpha,
        scale_x,
        scale_y,
        rotation,
        color,
        texture,
        render_enabled,
        visible_from,
        visible_until,
    ) {
        Ok(presentation) => Some(presentation),
        Err(error) => {
            diagnostics.push(note_diagnostic(error, entity.span()));
            None
        }
    }
}

fn required_value<'a>(
    entity: &'a ExpandedEntity,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<&'a TypedValue> {
    match entity.field(path) {
        Some(field) => Some(field.value()),
        None => {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::SCHEMA_MISSING_REQUIRED_FIELD,
                DiagnosticStage::Canonical,
                format!("Note requires {path}"),
                entity.span(),
            ));
            None
        }
    }
}

fn string_field(
    entity: &ExpandedEntity,
    path: &str,
    default: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    match entity.field(path) {
        Some(field) => match field.value() {
            TypedValue::String(value) => Some(value.clone()),
            value => {
                type_mismatch(path, "string", value, field.span(), diagnostics);
                None
            }
        },
        None => Some(default.to_owned()),
    }
}

fn optional_string(
    entity: &ExpandedEntity,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Option<String>> {
    match entity.field(path) {
        Some(field) => match field.value() {
            TypedValue::String(value) => Some(Some(value.clone())),
            value => {
                type_mismatch(path, "string", value, field.span(), diagnostics);
                None
            }
        },
        None => Some(None),
    }
}

fn bool_field(
    entity: &ExpandedEntity,
    path: &str,
    default: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<bool> {
    match entity.field(path) {
        Some(field) => match field.value() {
            TypedValue::Bool(value) => Some(*value),
            value => {
                type_mismatch(path, "bool", value, field.span(), diagnostics);
                None
            }
        },
        None => Some(default),
    }
}

fn float_field(
    entity: &ExpandedEntity,
    path: &str,
    default: f64,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<f64> {
    match entity.field(path) {
        Some(field) => match field.value() {
            TypedValue::Float(value) => Some(*value),
            value => {
                type_mismatch(path, "float", value, field.span(), diagnostics);
                None
            }
        },
        None => Some(default),
    }
}

fn length_field(
    entity: &ExpandedEntity,
    path: &str,
    default: f64,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<f64> {
    match entity.field(path) {
        Some(field) => match field.value() {
            TypedValue::Length(value) => Some(*value),
            value => {
                type_mismatch(path, "length", value, field.span(), diagnostics);
                None
            }
        },
        None => Some(default),
    }
}

fn angle_field(
    entity: &ExpandedEntity,
    path: &str,
    default: f64,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<f64> {
    match entity.field(path) {
        Some(field) => match field.value() {
            TypedValue::Angle(value) => Some(*value),
            value => {
                type_mismatch(path, "angle", value, field.span(), diagnostics);
                None
            }
        },
        None => Some(default),
    }
}

fn vec2_length_field(
    entity: &ExpandedEntity,
    path: &str,
    default: (f64, f64),
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalVec2> {
    let Some(field) = entity.field(path) else {
        return CanonicalVec2::new(default.0, default.1).ok();
    };
    let TypedValue::Vec2(x, y) = field.value() else {
        type_mismatch(
            path,
            "vec2<length>",
            field.value(),
            field.span(),
            diagnostics,
        );
        return None;
    };
    let (TypedValue::Length(x), TypedValue::Length(y)) = (&**x, &**y) else {
        type_mismatch(
            path,
            "vec2<length>",
            field.value(),
            field.span(),
            diagnostics,
        );
        return None;
    };
    match CanonicalVec2::new(*x, *y) {
        Ok(value) => Some(value),
        Err(_) => {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::NUMERIC_NON_FINITE,
                DiagnosticStage::Canonical,
                format!("{path} must be finite"),
                field.span(),
            ));
            None
        }
    }
}

fn time_from_field(
    entity: &ExpandedEntity,
    path: &str,
    time_map: &ChartTimeMap,
    diagnostics: &mut Vec<Diagnostic>,
    required: bool,
) -> Option<CanonicalTime> {
    let Some(field) = entity.field(path) else {
        if required {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::SCHEMA_MISSING_REQUIRED_FIELD,
                DiagnosticStage::Canonical,
                format!("Note requires {path}"),
                entity.span(),
            ));
        }
        return None;
    };
    let TypedValue::Beat(value) = field.value() else {
        type_mismatch(path, "beat", field.value(), field.span(), diagnostics);
        return None;
    };
    let beat = match CanonicalBeat::new(value.numerator(), value.denominator()) {
        Ok(beat) => beat,
        Err(_) => {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::TEMPO_INVALID,
                DiagnosticStage::Canonical,
                "Note beat is not representable canonically",
                field.span(),
            ));
            return None;
        }
    };
    match time_map.chart_time(beat) {
        Ok(time) => Some(time),
        Err(error) => {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::TEMPO_INVALID,
                DiagnosticStage::Canonical,
                error.to_string(),
                field.span(),
            ));
            None
        }
    }
}

fn field_span(entity: &ExpandedEntity, path: &str) -> crate::ast::SourceSpan {
    entity
        .field(path)
        .map(|field| field.span())
        .unwrap_or_else(|| entity.span())
}

fn type_mismatch(
    path: &str,
    expected: &str,
    value: &TypedValue,
    span: crate::ast::SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(Diagnostic::new(
        DiagnosticCode::TYPE_MISMATCH,
        DiagnosticStage::Canonical,
        format!("{path} must have type {expected}, found {}", value.ty()),
        span,
    ));
}

fn non_constructible(message: &'static str, span: crate::ast::SourceSpan) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::SCHEMA_NON_CONSTRUCTIBLE,
        DiagnosticStage::Canonical,
        message,
        span,
    )
}

fn note_diagnostic(error: CanonicalNoteError, span: crate::ast::SourceSpan) -> Diagnostic {
    let code = match error {
        CanonicalNoteError::MissingHoldEndTime
        | CanonicalNoteError::InvalidHoldInterval
        | CanonicalNoteError::EndTimeOnNonHold => DiagnosticCode::NOTE_INVALID_HOLD,
        CanonicalNoteError::WrongLineNamespace { .. } => DiagnosticCode::TYPE_MISMATCH,
        CanonicalNoteError::WrongNoteNamespace { .. } | CanonicalNoteError::DuplicateId { .. } => {
            DiagnosticCode::NAME_DUPLICATE
        }
        CanonicalNoteError::NonPositiveShape
        | CanonicalNoteError::KindMismatch { .. }
        | CanonicalNoteError::DisabledJudgmentPolicy
        | CanonicalNoteError::EmptyPolicyReference => DiagnosticCode::SCHEMA_NON_CONSTRUCTIBLE,
        CanonicalNoteError::NonFinitePresentation { .. } => DiagnosticCode::NUMERIC_NON_FINITE,
        CanonicalNoteError::PresentationOutOfRange { .. }
        | CanonicalNoteError::InvalidVisibilityInterval => DiagnosticCode::NUMERIC_DOMAIN,
    };
    Diagnostic::new(code, DiagnosticStage::Canonical, error.to_string(), span)
}

fn identity_diagnostic(error: fcs_model::IdError, span: crate::ast::SourceSpan) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::NAME_DUPLICATE,
        DiagnosticStage::Canonical,
        error.to_string(),
        span,
    )
}

fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|left, right| {
        left.primary_span()
            .start
            .cmp(&right.primary_span().start)
            .then_with(|| left.primary_span().end.cmp(&right.primary_span().end))
            .then_with(|| left.code().cmp(&right.code()))
    });
}
