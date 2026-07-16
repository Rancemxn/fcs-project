use crate::ast::{Beat, DocumentProfile, SourceSpan, TempoMap};
use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage};

pub(crate) fn validate_profile(
    profile: DocumentProfile,
    tempo_map: Option<&TempoMap>,
    document_span: SourceSpan,
) -> Result<(), Diagnostic> {
    if matches!(
        profile,
        DocumentProfile::Chart | DocumentProfile::Playable | DocumentProfile::Publishable
    ) && tempo_map.is_none()
    {
        return Err(Diagnostic::new(
            DiagnosticCode::PROFILE_REQUIREMENT_MISSING,
            DiagnosticStage::Parse,
            "chart, playable, and publishable profiles require a tempoMap block",
            document_span,
        ));
    }
    if let Some(tempo_map) = tempo_map {
        validate_tempo_map(tempo_map, document_span)?;
    }
    Ok(())
}

fn validate_tempo_map(tempo_map: &TempoMap, span: SourceSpan) -> Result<(), Diagnostic> {
    let zero = Beat::new(0, 1).expect("constant zero beat is valid");
    if tempo_map.points.first().map(|point| point.beat) != Some(zero) {
        return Err(Diagnostic::new(
            DiagnosticCode::TEMPO_INVALID,
            DiagnosticStage::Parse,
            "tempoMap must start at beat zero",
            span,
        ));
    }
    if tempo_map
        .points
        .iter()
        .any(|point| !point.bpm.get().is_finite() || point.bpm.get() <= 0.0)
    {
        return Err(Diagnostic::new(
            DiagnosticCode::TEMPO_INVALID,
            DiagnosticStage::Parse,
            "tempoMap BPM must be finite and greater than zero",
            span,
        ));
    }
    if tempo_map
        .points
        .windows(2)
        .any(|points| points[0].beat > points[1].beat)
    {
        return Err(Diagnostic::new(
            DiagnosticCode::TEMPO_NON_MONOTONIC,
            DiagnosticStage::Parse,
            "tempoMap beats must be non-decreasing",
            span,
        ));
    }
    Ok(())
}
