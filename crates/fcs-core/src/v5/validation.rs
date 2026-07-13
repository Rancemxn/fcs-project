use crate::v5::ast::{Beat, DocumentProfile, TempoMap};

use super::parser::ParseError;

pub fn validate_profile(
    profile: DocumentProfile,
    tempo_map: Option<&TempoMap>,
) -> Result<(), ParseError> {
    if matches!(
        profile,
        DocumentProfile::Chart | DocumentProfile::Playable | DocumentProfile::Publishable
    ) && tempo_map.is_none()
    {
        return Err(ParseError::MissingRequiredBlock("tempoMap"));
    }
    if let Some(tempo_map) = tempo_map {
        validate_tempo_map(tempo_map)?;
    }
    Ok(())
}

fn validate_tempo_map(tempo_map: &TempoMap) -> Result<(), ParseError> {
    let zero = Beat::new(0, 1).expect("constant zero beat is valid");
    if tempo_map.points.first().map(|point| point.beat) != Some(zero) {
        return Err(ParseError::InvalidTempoMap("first beat must be zero"));
    }
    if tempo_map
        .points
        .windows(2)
        .any(|points| points[0].beat > points[1].beat)
    {
        return Err(ParseError::InvalidTempoMap("beats must be non-decreasing"));
    }
    Ok(())
}
