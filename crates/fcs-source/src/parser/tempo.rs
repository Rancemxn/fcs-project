use crate::ast::{Beat, Bpm, TempoMap, TempoPoint};

use super::{ParseError, document::strip_comments};

pub fn parse_tempo_map(input: &str) -> Result<TempoMap, ParseError> {
    let input = strip_comments(input)?;
    let mut points = Vec::new();

    for entry in input.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (beat, bpm) = entry
            .split_once("->")
            .ok_or(ParseError::InvalidSyntax("tempo map"))?;
        points.push(TempoPoint {
            beat: parse_beat(beat.trim())?,
            bpm: parse_bpm(bpm.trim())?,
        });
    }

    Ok(TempoMap { points })
}

fn parse_beat(input: &str) -> Result<Beat, ParseError> {
    let value = input
        .strip_suffix("beat")
        .ok_or(ParseError::InvalidSyntax("tempo beat"))?
        .trim();

    if let Some(value) = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        let mut parts = value.split(',').map(str::trim);
        let whole = parse_i128(parts.next())?;
        let numerator = parse_i128(parts.next())?;
        let denominator = parse_i128(parts.next())?;
        if parts.next().is_some() {
            return Err(ParseError::InvalidSyntax("tempo beat"));
        }
        let whole = beat_from_i128(whole, 1)?;
        let fraction = beat_from_i128(numerator, denominator)?;
        return whole
            .checked_add(fraction)
            .map_err(|_| ParseError::InvalidSyntax("tempo beat"));
    }

    let negative = value.starts_with('-');
    let value = value.strip_prefix('-').unwrap_or(value);
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if fraction.is_empty() && value.contains('.') {
        return Err(ParseError::InvalidSyntax("tempo beat"));
    }
    if !fraction.chars().all(|character| character.is_ascii_digit()) {
        return Err(ParseError::InvalidSyntax("tempo beat"));
    }

    let whole = parse_i128_text(whole)?;
    if fraction.is_empty() {
        let whole = if negative {
            whole
                .checked_neg()
                .ok_or(ParseError::InvalidSyntax("tempo beat"))?
        } else {
            whole
        };
        return beat_from_i128(whole, 1);
    }
    let denominator = 10_i128
        .checked_pow(fraction.len() as u32)
        .ok_or(ParseError::InvalidSyntax("tempo beat"))?;
    let fractional = fraction
        .parse::<i128>()
        .map_err(|_| ParseError::InvalidSyntax("tempo beat"))?;
    let numerator = whole
        .checked_mul(denominator)
        .and_then(|value| value.checked_add(fractional))
        .ok_or(ParseError::InvalidSyntax("tempo beat"))?;
    let numerator = if negative {
        numerator
            .checked_neg()
            .ok_or(ParseError::InvalidSyntax("tempo beat"))?
    } else {
        numerator
    };
    beat_from_i128(numerator, denominator)
}

fn parse_bpm(input: &str) -> Result<Bpm, ParseError> {
    let value = input
        .strip_suffix("bpm")
        .ok_or(ParseError::InvalidSyntax("tempo bpm"))?
        .trim()
        .parse::<f64>()
        .map_err(|_| ParseError::InvalidSyntax("tempo bpm"))?;
    Bpm::new(value).map_err(|_| ParseError::InvalidSyntax("tempo bpm"))
}

fn parse_i128(value: Option<&str>) -> Result<i128, ParseError> {
    value
        .ok_or(ParseError::InvalidSyntax("tempo beat"))
        .and_then(parse_i128_text)
}

fn parse_i128_text(value: &str) -> Result<i128, ParseError> {
    value
        .parse::<i128>()
        .map_err(|_| ParseError::InvalidSyntax("tempo beat"))
}

fn beat_from_i128(numerator: i128, denominator: i128) -> Result<Beat, ParseError> {
    let numerator =
        i64::try_from(numerator).map_err(|_| ParseError::InvalidSyntax("tempo beat"))?;
    let denominator =
        i64::try_from(denominator).map_err(|_| ParseError::InvalidSyntax("tempo beat"))?;
    Beat::new(numerator, denominator).map_err(|_| ParseError::InvalidSyntax("tempo beat"))
}
