//! I3.6 expanded-source-to-canonical Track lowering.

use fcs_model::{
    Beat as CanonicalBeat, CanonicalLineGraph, CanonicalTime, CanonicalTrack, CanonicalTrackBlend,
    CanonicalTrackError, CanonicalTrackFill, CanonicalTrackInterpolation, CanonicalTrackPiece,
    CanonicalTrackPoint, CanonicalTrackSegment, CanonicalTrackSet, CanonicalTrackTarget,
    CanonicalTrackValue, CanonicalVec2, ChartTimeMap,
};

use crate::ast::{
    ExpandedField, ExpandedSourceDocument, ExpandedTrack, ExpandedTrackInterpolation,
    ExpandedTrackPiece, Type, TypedValue,
};
use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage};

impl ExpandedSourceDocument {
    pub fn canonical_tracks(
        &self,
        time_map: &ChartTimeMap,
        lines: &CanonicalLineGraph,
    ) -> Result<CanonicalTrackSet, Vec<Diagnostic>> {
        lower_tracks(self, time_map, lines)
    }
}

fn lower_tracks(
    document: &ExpandedSourceDocument,
    time_map: &ChartTimeMap,
    lines: &CanonicalLineGraph,
) -> Result<CanonicalTrackSet, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut tracks = Vec::new();
    for track in document.tracks() {
        if let Some(track) = lower_track(track, time_map, lines, &mut diagnostics) {
            tracks.push(track);
        }
    }
    if !diagnostics.is_empty() {
        sort_diagnostics(&mut diagnostics);
        return Err(diagnostics);
    }
    CanonicalTrackSet::new(tracks).map_err(|error| {
        vec![track_diagnostic(
            error,
            document
                .tracks()
                .first()
                .map(ExpandedTrack::name_span)
                .unwrap_or(crate::ast::SourceSpan::new(0, 0)),
        )]
    })
}

fn lower_track(
    track: &ExpandedTrack,
    time_map: &ChartTimeMap,
    lines: &CanonicalLineGraph,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalTrack> {
    let owner = match lines.line_by_textual_id(track.owner()) {
        Some(line) => line.id().clone(),
        None => {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::GRAPH_UNKNOWN_PARENT,
                DiagnosticStage::Canonical,
                format!("Track owner {} is not a canonical Line", track.owner()),
                track.name_span(),
            ));
            return None;
        }
    };
    let target = target(track, diagnostics)?;
    let blend = enum_setting(
        track,
        "blend",
        "replace",
        &["replace", "add", "multiply"],
        diagnostics,
    )?;
    let blend = match blend.as_str() {
        "replace" => CanonicalTrackBlend::Replace,
        "add" => CanonicalTrackBlend::Add,
        "multiply" => CanonicalTrackBlend::Multiply,
        _ => unreachable!(),
    };
    let priority = int_setting(track, "priority", 0, diagnostics)?;
    let default_fill = match blend {
        CanonicalTrackBlend::Replace => "base",
        CanonicalTrackBlend::Add => "zero",
        CanonicalTrackBlend::Multiply => "one",
    };
    let fill = fill_setting(track, "fill", default_fill, diagnostics)?;
    let extrapolate_before =
        fill_setting(track, "extrapolateBefore", fill_name(fill), diagnostics)?;
    let extrapolate_after = fill_setting(track, "extrapolateAfter", fill_name(fill), diagnostics)?;
    let diagnostic_count = diagnostics.len();
    let mut pieces = Vec::new();
    for (document_order, piece) in track.pieces().iter().enumerate() {
        let lowered = match piece {
            ExpandedTrackPiece::Segment(segment) => {
                let start = lower_time(segment.start(), time_map, segment.span(), diagnostics)?;
                let end = lower_time(segment.end(), time_map, segment.span(), diagnostics)?;
                if segment.start().ty() != segment.end().ty() {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::TRACK_INVALID_INTERVAL,
                        DiagnosticStage::Canonical,
                        "Track segment start and end must use the same source time type",
                        segment.span(),
                    ));
                    continue;
                }
                let start_value =
                    lower_value(segment.start_value(), target, segment.span(), diagnostics)?;
                let end_value =
                    lower_value(segment.end_value(), target, segment.span(), diagnostics)?;
                let interpolation =
                    lower_interpolation(segment.interpolation(), segment.span(), diagnostics)?;
                match CanonicalTrackSegment::new(
                    start,
                    end,
                    start_value,
                    end_value,
                    interpolation,
                    document_order as u64,
                ) {
                    Ok(segment) => CanonicalTrackPiece::Segment(segment),
                    Err(error) => {
                        diagnostics.push(track_diagnostic(error, segment.span()));
                        continue;
                    }
                }
            }
            ExpandedTrackPiece::Point(point) => {
                let time = lower_time(point.time(), time_map, point.span(), diagnostics)?;
                let value = lower_value(point.value(), target, point.span(), diagnostics)?;
                match CanonicalTrackPoint::new(time, value, document_order as u64) {
                    Ok(point) => CanonicalTrackPiece::Point(point),
                    Err(error) => {
                        diagnostics.push(track_diagnostic(error, point.span()));
                        continue;
                    }
                }
            }
        };
        pieces.push(lowered);
    }
    if diagnostics.len() != diagnostic_count {
        return None;
    }
    match CanonicalTrack::new(
        owner,
        track.name(),
        target,
        blend,
        priority,
        fill,
        extrapolate_before,
        extrapolate_after,
        pieces,
    ) {
        Ok(track) => Some(track),
        Err(error) => {
            diagnostics.push(track_diagnostic(error, track.span()));
            None
        }
    }
}

fn target(
    track: &ExpandedTrack,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalTrackTarget> {
    let expected = match track.target() {
        "position" => (
            CanonicalTrackTarget::Position,
            Type::Vec2(Box::new(Type::Length)),
        ),
        "rotation" => (CanonicalTrackTarget::Rotation, Type::Angle),
        "scale" => (
            CanonicalTrackTarget::Scale,
            Type::Vec2(Box::new(Type::Float)),
        ),
        "alpha" => (CanonicalTrackTarget::Alpha, Type::Float),
        "scrollSpeed" => (CanonicalTrackTarget::ScrollSpeed, Type::Float),
        _ => {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::SCHEMA_DYNAMIC_FIELD_FORBIDDEN,
                DiagnosticStage::Canonical,
                format!("Line field {} is not a Track target", track.target()),
                track.target_span(),
            ));
            return None;
        }
    };
    if track.value_type() != &expected.1 {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::TYPE_MISMATCH,
            DiagnosticStage::Canonical,
            format!(
                "Track target {} requires {}, found {}",
                track.target(),
                expected.1,
                track.value_type()
            ),
            track.target_span(),
        ));
        None
    } else {
        Some(expected.0)
    }
}

fn lower_time(
    value: &TypedValue,
    time_map: &ChartTimeMap,
    span: crate::ast::SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalTime> {
    match value {
        TypedValue::Beat(value) => {
            let beat = CanonicalBeat::new(value.numerator(), value.denominator()).ok()?;
            time_map
                .chart_time(beat)
                .map_err(|error| {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::TRACK_INVALID_INTERVAL,
                        DiagnosticStage::Canonical,
                        error.to_string(),
                        span,
                    ));
                })
                .ok()
        }
        TypedValue::Time(value) => CanonicalTime::from_chart_time_seconds(*value)
            .map_err(|error| {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::TRACK_INVALID_INTERVAL,
                    DiagnosticStage::Canonical,
                    error.to_string(),
                    span,
                ));
            })
            .ok(),
        _ => {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::TYPE_MISMATCH,
                DiagnosticStage::Canonical,
                format!("Track time must be beat or time, found {}", value.ty()),
                span,
            ));
            None
        }
    }
}

fn lower_value(
    value: &TypedValue,
    target: CanonicalTrackTarget,
    span: crate::ast::SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalTrackValue> {
    let lowered = match (target, value) {
        (CanonicalTrackTarget::Alpha, TypedValue::Float(value)) => {
            Some(CanonicalTrackValue::Float(*value))
        }
        (CanonicalTrackTarget::ScrollSpeed, TypedValue::Float(value)) => {
            Some(CanonicalTrackValue::Float(*value))
        }
        (CanonicalTrackTarget::Rotation, TypedValue::Angle(value)) => {
            Some(CanonicalTrackValue::Angle(*value))
        }
        (CanonicalTrackTarget::Position, TypedValue::Vec2(x, y)) => {
            let (TypedValue::Length(x), TypedValue::Length(y)) = (&**x, &**y) else {
                return value_mismatch(value, span, diagnostics);
            };
            CanonicalVec2::new(*x, *y)
                .ok()
                .map(CanonicalTrackValue::Vec2Length)
        }
        (CanonicalTrackTarget::Scale, TypedValue::Vec2(x, y)) => {
            let (TypedValue::Float(x), TypedValue::Float(y)) = (&**x, &**y) else {
                return value_mismatch(value, span, diagnostics);
            };
            CanonicalVec2::new(*x, *y)
                .ok()
                .map(CanonicalTrackValue::Vec2Float)
        }
        _ => None,
    };
    lowered.or_else(|| value_mismatch(value, span, diagnostics))
}

fn value_mismatch(
    value: &TypedValue,
    span: crate::ast::SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalTrackValue> {
    diagnostics.push(Diagnostic::new(
        DiagnosticCode::TYPE_MISMATCH,
        DiagnosticStage::Canonical,
        format!("Track value type {} does not match its target", value.ty()),
        span,
    ));
    None
}

fn lower_interpolation(
    interpolation: &ExpandedTrackInterpolation,
    span: crate::ast::SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalTrackInterpolation> {
    let lowered = match interpolation {
        ExpandedTrackInterpolation::Value(TypedValue::String(name)) => match name.as_str() {
            "step" => CanonicalTrackInterpolation::Step,
            "linear" => CanonicalTrackInterpolation::Linear,
            _ => CanonicalTrackInterpolation::Easing(name.clone()),
        },
        ExpandedTrackInterpolation::CubicBezier(values) => {
            let mut floats = [0.0; 4];
            for (slot, value) in floats.iter_mut().zip(values) {
                let TypedValue::Float(value) = value else {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::TRACK_INVALID_EASING,
                        DiagnosticStage::Canonical,
                        "cubic Bezier controls must be float",
                        span,
                    ));
                    return None;
                };
                *slot = *value;
            }
            CanonicalTrackInterpolation::CubicBezier(floats)
        }
        ExpandedTrackInterpolation::Value(value) => {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::TYPE_MISMATCH,
                DiagnosticStage::Canonical,
                format!("Track interpolation must be string, found {}", value.ty()),
                span,
            ));
            return None;
        }
    };
    Some(lowered)
}

fn enum_setting(
    track: &ExpandedTrack,
    name: &str,
    default: &str,
    allowed: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    match track.setting(name) {
        Some(field) => match field.value() {
            TypedValue::String(value) if allowed.contains(&value.as_str()) => Some(value.clone()),
            TypedValue::String(_) => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::SCHEMA_NON_CONSTRUCTIBLE,
                    DiagnosticStage::Canonical,
                    format!("invalid Track {name} value"),
                    field.span(),
                ));
                None
            }
            value => {
                setting_type_mismatch(name, "string", value, field, diagnostics);
                None
            }
        },
        None => Some(default.to_owned()),
    }
}

fn fill_setting(
    track: &ExpandedTrack,
    name: &str,
    default: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalTrackFill> {
    match enum_setting(
        track,
        name,
        default,
        &["base", "zero", "one", "holdBefore", "holdAfter", "error"],
        diagnostics,
    )?
    .as_str()
    {
        "base" => Some(CanonicalTrackFill::Base),
        "zero" => Some(CanonicalTrackFill::Zero),
        "one" => Some(CanonicalTrackFill::One),
        "holdBefore" => Some(CanonicalTrackFill::HoldBefore),
        "holdAfter" => Some(CanonicalTrackFill::HoldAfter),
        "error" => Some(CanonicalTrackFill::Error),
        _ => unreachable!(),
    }
}

fn int_setting(
    track: &ExpandedTrack,
    name: &str,
    default: i64,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<i64> {
    match track.setting(name) {
        Some(field) => match field.value() {
            TypedValue::Int(value) => Some(*value),
            value => {
                setting_type_mismatch(name, "int", value, field, diagnostics);
                None
            }
        },
        None => Some(default),
    }
}

fn setting_type_mismatch(
    name: &str,
    expected: &str,
    value: &TypedValue,
    field: &ExpandedField,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(Diagnostic::new(
        DiagnosticCode::TYPE_MISMATCH,
        DiagnosticStage::Canonical,
        format!("Track {name} must be {expected}, found {}", value.ty()),
        field.span(),
    ));
}

fn fill_name(fill: CanonicalTrackFill) -> &'static str {
    match fill {
        CanonicalTrackFill::Base => "base",
        CanonicalTrackFill::Zero => "zero",
        CanonicalTrackFill::One => "one",
        CanonicalTrackFill::HoldBefore => "holdBefore",
        CanonicalTrackFill::HoldAfter => "holdAfter",
        CanonicalTrackFill::Error => "error",
    }
}

fn track_diagnostic(error: CanonicalTrackError, span: crate::ast::SourceSpan) -> Diagnostic {
    let code = match &error {
        CanonicalTrackError::InvalidInterval => DiagnosticCode::TRACK_INVALID_INTERVAL,
        CanonicalTrackError::Overlap => DiagnosticCode::TRACK_OVERLAP,
        CanonicalTrackError::ReplaceConflict => DiagnosticCode::TRACK_REPLACE_CONFLICT,
        CanonicalTrackError::Gap => DiagnosticCode::TRACK_GAP,
        CanonicalTrackError::InvalidEasing { .. } | CanonicalTrackError::InvalidBezier => {
            DiagnosticCode::TRACK_INVALID_EASING
        }
        CanonicalTrackError::DuplicateIdentity { .. } => DiagnosticCode::NAME_DUPLICATE,
        CanonicalTrackError::TargetTypeMismatch => DiagnosticCode::TYPE_MISMATCH,
        CanonicalTrackError::NonFiniteValue => DiagnosticCode::NUMERIC_NON_FINITE,
        CanonicalTrackError::WrongOwnerNamespace { .. }
        | CanonicalTrackError::EmptyName
        | CanonicalTrackError::Empty => DiagnosticCode::SCHEMA_NON_CONSTRUCTIBLE,
    };
    Diagnostic::new(code, DiagnosticStage::Canonical, error.to_string(), span)
}

fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by_key(|diagnostic| {
        (
            diagnostic.primary_span().start,
            diagnostic.primary_span().end,
            diagnostic.code().as_str(),
        )
    });
}
