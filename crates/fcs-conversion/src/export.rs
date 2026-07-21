//! I8 product export and FCS format surfaces.
//!
//! `format_fcs_source` validates source then rewrites UTF-8 without inventing
//! semantics. `export_pgr_v3` emits a formatVersion-3 PGR chart from a product
//! CanonicalChart so target reparse can run through the existing importer.

use std::fmt;

use fcs_model::{CanonicalChart, CanonicalNoteKind, CanonicalNoteSide};
use serde_json::{Value, json};

use crate::{
    ArtifactRole, DecimalLimits, ExactDecimal, PgrLimits, PgrProfile, PgrProfileBinding,
    SourceArtifact, SourceFormat, interpret_pgr, lower_pgr_to_canonical, parse_json_document,
    parse_pgr_document,
};

/// Stable formatter / exporter diagnostic category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportError {
    category: &'static str,
    message: String,
}

impl ExportError {
    pub fn new(category: &'static str, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
        }
    }

    pub const fn category(&self) -> &'static str {
        self.category
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.category, self.message)
    }
}

impl std::error::Error for ExportError {}

/// Validate FCS source and return a product-stable rewrite.
///
/// This RC unit keeps a parse-validated identity formatter: whitespace and
/// authoring layout are preserved after UTF-8/parse acceptance. A full
/// pretty-printer that normalizes whitespace remains a later refinement and
/// must not invent semantics.
pub fn format_fcs_source(source: &str) -> Result<String, ExportError> {
    let parsed = fcs_source::parser::parse_document(source)
        .into_result()
        .map_err(|diagnostics| {
            let message = diagnostics
                .first()
                .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
                .unwrap_or_else(|| "source invalid".into());
            ExportError::new("source.invalid", message)
        })?;
    let _ = parsed;
    Ok(source.to_owned())
}

/// Export one CanonicalChart as PGR formatVersion 3 JSON bytes.
///
/// Mapping uses the portable Phira v3 coordinate/time conventions already owned
/// by the importer. This is a product writer boundary for round-trip tests, not
/// a claim that every canonical construct is representable in PGR.
pub fn export_pgr_v3(chart: &CanonicalChart) -> Result<Vec<u8>, ExportError> {
    let offset = chart
        .metadata()
        .sync()
        .map(|sync| sync.audio_offset().seconds())
        .unwrap_or(0.0);
    let mut lines = Vec::new();
    for line in chart.lines().lines() {
        let line_id = line.id().value();
        let mut notes_above = Vec::new();
        let mut notes_below = Vec::new();
        for note in chart.notes().notes() {
            if note.gameplay().line().value() != line_id {
                continue;
            }
            let time_t = chart_time_to_pgr_t(note.gameplay().time().chart_time_seconds(), 120.0);
            let hold_time = note
                .gameplay()
                .end_time()
                .map(|end| chart_time_to_pgr_t(end.chart_time_seconds(), 120.0) - time_t)
                .unwrap_or(0.0)
                .max(0.0);
            let position_x = note.presentation().position_x() / 108.0;
            // Constant speed value=1 from 0: reconstructed floor at T is T * 60 / (32 * bpm).
            let floor_position = time_t * 60.0 / (32.0 * 120.0);
            let note_type = match note.kind() {
                CanonicalNoteKind::Tap => 1,
                CanonicalNoteKind::Drag => 2,
                CanonicalNoteKind::Hold => 3,
                CanonicalNoteKind::Flick => 4,
            };
            let payload = json!({
                "type": note_type,
                "time": time_t,
                "holdTime": hold_time,
                "positionX": position_x,
                "speed": note.presentation().scroll_factor().max(0.0),
                "floorPosition": floor_position
            });
            match note.gameplay().side() {
                CanonicalNoteSide::Above => notes_above.push(payload),
                CanonicalNoteSide::Below => notes_below.push(payload),
            }
        }
        // Cover [0, required] with constant speed 1 so importer floor caches match 0.
        let max_t = notes_above
            .iter()
            .chain(notes_below.iter())
            .map(|note| {
                note.get("time").and_then(Value::as_f64).unwrap_or(0.0)
                    + note.get("holdTime").and_then(Value::as_f64).unwrap_or(0.0)
            })
            .fold(32.0_f64, f64::max)
            .ceil()
            .max(32.0);
        lines.push(json!({
            "bpm": 120,
            "judgeLineMoveEvents": [{
                "startTime": 0,
                "endTime": max_t,
                "start": 0.5,
                "end": 0.5,
                "start2": 0.5,
                "end2": 0.5
            }],
            "judgeLineRotateEvents": [],
            "judgeLineDisappearEvents": [],
            "speedEvents": [{
                "startTime": 0,
                "endTime": max_t,
                "value": 1,
                "floorPosition": 0
            }],
            "notesAbove": notes_above,
            "notesBelow": notes_below
        }));
    }
    if lines.is_empty() {
        lines.push(json!({
            "bpm": 120,
            "judgeLineMoveEvents": [{
                "startTime": 0,
                "endTime": 32,
                "start": 0.5,
                "end": 0.5,
                "start2": 0.5,
                "end2": 0.5
            }],
            "judgeLineRotateEvents": [],
            "judgeLineDisappearEvents": [],
            "speedEvents": [{
                "startTime": 0,
                "endTime": 32,
                "value": 1,
                "floorPosition": 0
            }],
            "notesAbove": [],
            "notesBelow": []
        }));
    }
    let root = json!({
        "formatVersion": 3,
        "offset": offset,
        "judgeLineList": lines
    });
    serde_json::to_vec_pretty(&root).map_err(|error| {
        ExportError::new(
            "conversion.internal",
            format!("failed to serialize PGR JSON: {error}"),
        )
    })
}

/// Import → export → re-import a PGR chart and compare line/note counts.
pub fn roundtrip_pgr_v3_public_bytes(
    bytes: &[u8],
) -> Result<(usize, usize, usize, usize), ExportError> {
    let artifact = SourceArtifact::new("chart.json", ArtifactRole::Chart, bytes.to_vec())
        .map_err(|error| ExportError::new("conversion.source-invalid", error.to_string()))?;
    let parsed = parse_json_document(SourceFormat::Pgr, &artifact)
        .map_err(|error| ExportError::new("conversion.source-invalid", error.to_string()))?;
    let source = parse_pgr_document(&parsed, PgrLimits::default())
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let floor = ExactDecimal::parse("120", DecimalLimits::default()).map_err(|error| {
        ExportError::new("conversion.profile-parameter-invalid", error.to_string())
    })?;
    let binding = PgrProfileBinding::new(PgrProfile::PhiraV3, floor)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let semantic = interpret_pgr(&source, &binding)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let first = lower_pgr_to_canonical(&semantic, &artifact)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let exported = export_pgr_v3(first.compilation().chart())?;
    let artifact2 = SourceArtifact::new("chart-reexport.json", ArtifactRole::Chart, exported)
        .map_err(|error| ExportError::new("conversion.source-invalid", error.to_string()))?;
    let parsed2 = parse_json_document(SourceFormat::Pgr, &artifact2)
        .map_err(|error| ExportError::new("conversion.source-invalid", error.to_string()))?;
    let source2 = parse_pgr_document(&parsed2, PgrLimits::default())
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let semantic2 = interpret_pgr(&source2, &binding)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let second = lower_pgr_to_canonical(&semantic2, &artifact2)
        .map_err(|error| ExportError::new(error.category(), error.to_string()))?;
    let first_chart = first.compilation().chart();
    let second_chart = second.compilation().chart();
    Ok((
        first_chart.lines().lines().count(),
        first_chart.notes().notes().len(),
        second_chart.lines().lines().count(),
        second_chart.notes().notes().len(),
    ))
}

fn chart_time_to_pgr_t(chart_time_seconds: f64, bpm: f64) -> f64 {
    // Inverse of importer: chart_time = T * 60 / (32 * bpm)
    chart_time_seconds * 32.0 * bpm / 60.0
}

/// Declared target capability surface used before export (I8.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySet {
    pub format: &'static str,
    pub version: &'static str,
    pub time: bool,
    pub notes: bool,
    pub tracks: bool,
    pub expressions: bool,
    pub resources: bool,
}

impl CapabilitySet {
    pub const fn pgr_v3() -> Self {
        Self {
            format: "pgr",
            version: "3",
            time: true,
            notes: true,
            tracks: true,
            expressions: false,
            resources: false,
        }
    }

    pub const fn rpe_json() -> Self {
        Self {
            format: "rpe",
            version: "json",
            time: true,
            notes: true,
            tracks: true,
            expressions: false,
            resources: false,
        }
    }

    pub const fn pec_line() -> Self {
        Self {
            format: "pec",
            version: "line-command",
            time: true,
            notes: true,
            tracks: false,
            expressions: false,
            resources: false,
        }
    }
}

/// Negotiation outcome before writing a target format (I8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegotiationAction {
    Direct,
    Equivalent,
    Bake,
    Preserve,
    Drop,
    Unsupported,
}

impl NegotiationAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Equivalent => "equivalent",
            Self::Bake => "bake",
            Self::Preserve => "preserve",
            Self::Drop => "drop",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Negotiate chart features against a target capability set.
pub fn negotiate_export(
    chart: &CanonicalChart,
    target: &CapabilitySet,
) -> Result<NegotiationAction, ExportError> {
    if !target.time || !target.notes {
        return Err(ExportError::new(
            "conversion.capability-mismatch",
            format!(
                "target {}@{} lacks required time/note capability",
                target.format, target.version
            ),
        ));
    }
    let has_tracks = !chart.tracks().tracks().is_empty();
    if has_tracks && !target.tracks {
        return Ok(NegotiationAction::Bake);
    }
    if chart.notes().notes().is_empty() && chart.lines().lines().count() <= 1 {
        return Ok(NegotiationAction::Direct);
    }
    Ok(NegotiationAction::Equivalent)
}

/// Export a modern RPE JSON chart from CanonicalChart (I8.6 product surface).
pub fn export_rpe_json(chart: &CanonicalChart) -> Result<Vec<u8>, ExportError> {
    let _ = negotiate_export(chart, &CapabilitySet::rpe_json())?;
    let offset_ms = chart
        .metadata()
        .sync()
        .map(|sync| (sync.audio_offset().seconds() * 1000.0).round() as i64)
        .unwrap_or(0);
    let bpm = chart
        .time_map()
        .segments()
        .next()
        .map(|(_, _, bpm)| bpm)
        .unwrap_or(120.0);
    let mut judge_lines = Vec::new();
    for line in chart.lines().lines() {
        let line_id = line.id().value();
        let mut notes = Vec::new();
        for note in chart.notes().notes() {
            if note.gameplay().line().value() != line_id {
                continue;
            }
            let start = seconds_to_rpe_beat(note.gameplay().time().chart_time_seconds(), bpm);
            let end = note
                .gameplay()
                .end_time()
                .map(|time| seconds_to_rpe_beat(time.chart_time_seconds(), bpm))
                .unwrap_or(start);
            let note_type = match note.kind() {
                CanonicalNoteKind::Tap => 1,
                CanonicalNoteKind::Hold => 2,
                CanonicalNoteKind::Flick => 3,
                CanonicalNoteKind::Drag => 4,
            };
            let above = match note.gameplay().side() {
                CanonicalNoteSide::Above => 1,
                CanonicalNoteSide::Below => 0,
            };
            notes.push(json!({
                "type": note_type,
                "startTime": start,
                "endTime": end,
                "positionX": note.presentation().position_x(),
                "speed": 4.5,
                "above": above,
                "isFake": if note.gameplay().judgment_enabled() { 0 } else { 1 }
            }));
        }
        judge_lines.push(json!({
            "bpmfactor": 1,
            "eventLayers": [{"speedEvents": [{
                "startTime": [0, 0, 1],
                "endTime": [4, 0, 1],
                "start": 1,
                "end": 1
            }]}],
            "notes": notes,
            "father": -1
        }));
    }
    if judge_lines.is_empty() {
        judge_lines.push(json!({
            "bpmfactor": 1,
            "eventLayers": [{"speedEvents": [{
                "startTime": [0, 0, 1],
                "endTime": [4, 0, 1],
                "start": 1,
                "end": 1
            }]}],
            "notes": [],
            "father": -1
        }));
    }
    let root = json!({
        "META": { "RPEVersion": 150, "offset": offset_ms, "name": "fcs-export" },
        "BPMList": [{ "startTime": [0, 0, 1], "bpm": bpm }],
        "judgeLineList": judge_lines
    });
    serde_json::to_vec_pretty(&root)
        .map_err(|error| ExportError::new("conversion.internal", error.to_string()))
}

/// Export a Phira line-command PEC chart from CanonicalChart (I8.7 product surface).
pub fn export_pec_line(chart: &CanonicalChart) -> Result<Vec<u8>, ExportError> {
    let _ = negotiate_export(chart, &CapabilitySet::pec_line())?;
    let offset = chart
        .metadata()
        .sync()
        .map(|sync| sync.audio_offset().seconds())
        .unwrap_or(0.0);
    // Phira PEC offset uses 150ms bias: raw = seconds*1000 - 150.
    let raw_offset = (offset * 1000.0 - 150.0).round() as i64;
    let bpm = chart
        .time_map()
        .segments()
        .next()
        .map(|(_, _, bpm)| bpm)
        .unwrap_or(120.0);
    let mut lines = String::new();
    lines.push_str(&format!("{raw_offset}\n"));
    lines.push_str(&format!("bp 0.00 {bpm}\n"));
    for note in chart.notes().notes() {
        let beat = note.gameplay().time().chart_time_seconds() * bpm / 60.0;
        let x = ((note.presentation().position_x() / 2048.0) + 0.5) * 2048.0;
        let side = match note.gameplay().side() {
            CanonicalNoteSide::Above => 1,
            CanonicalNoteSide::Below => 2,
        };
        let fake = if note.gameplay().judgment_enabled() {
            0
        } else {
            1
        };
        match note.kind() {
            CanonicalNoteKind::Hold => {
                let end = note
                    .gameplay()
                    .end_time()
                    .map(|time| time.chart_time_seconds() * bpm / 60.0)
                    .unwrap_or(beat + 1.0);
                lines.push_str(&format!("n2 0 {beat:.2} {end:.2} {x:.0} {side} {fake}\n"));
            }
            CanonicalNoteKind::Tap => {
                lines.push_str(&format!("n1 0 {beat:.2} {x:.0} {side} {fake}\n"));
            }
            CanonicalNoteKind::Flick => {
                lines.push_str(&format!("n3 0 {beat:.2} {x:.0} {side} {fake}\n"));
            }
            CanonicalNoteKind::Drag => {
                lines.push_str(&format!("n4 0 {beat:.2} {x:.0} {side} {fake}\n"));
            }
        }
        lines.push_str("# 1.000\n& 1.000\n");
    }
    Ok(lines.into_bytes())
}

fn seconds_to_rpe_beat(seconds: f64, bpm: f64) -> [i64; 3] {
    let beats = seconds * bpm / 60.0;
    let whole = beats.floor() as i64;
    let frac = beats - whole as f64;
    let numerator = (frac * 1000.0).round() as i64;
    [whole, numerator, 1000]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn format_fcs_source_accepts_minimal_chart() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source =
            fs::read_to_string(root.join("docs/conformance/fcs5/source/valid/minimal-chart.fcs"))
                .unwrap();
        let formatted = format_fcs_source(&source).unwrap();
        assert_eq!(formatted, source);
    }

    #[test]
    fn format_fcs_source_rejects_invalid() {
        let error = format_fcs_source("not a chart").unwrap_err();
        assert_eq!(error.category(), "source.invalid");
    }

    #[test]
    fn public_pgr_feature_fixture_roundtrips_through_export() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let bytes = fs::read(
            root.join("docs/conformance/conversion/public-fixtures/sources/pgr-feature.pgr.json"),
        )
        .unwrap();
        let (lines_a, notes_a, lines_b, notes_b) = roundtrip_pgr_v3_public_bytes(&bytes).unwrap();
        assert_eq!(lines_a, lines_b);
        assert_eq!(notes_a, notes_b);
        assert!(lines_a >= 1);
        assert!(notes_a >= 1);
    }

    #[test]
    fn capability_negotiation_and_rpe_pec_export_emit_bytes() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let bytes = fs::read(
            root.join("docs/conformance/conversion/public-fixtures/sources/pgr-minimal.pgr.json"),
        )
        .unwrap();
        let artifact = SourceArtifact::new("chart.json", ArtifactRole::Chart, bytes).unwrap();
        let parsed = parse_json_document(SourceFormat::Pgr, &artifact).unwrap();
        let source = parse_pgr_document(&parsed, PgrLimits::default()).unwrap();
        let floor = ExactDecimal::parse("120", DecimalLimits::default()).unwrap();
        let binding = PgrProfileBinding::new(PgrProfile::PhiraV1, floor).unwrap();
        let semantic = interpret_pgr(&source, &binding).unwrap();
        let import = lower_pgr_to_canonical(&semantic, &artifact).unwrap();
        let chart = import.compilation().chart();
        assert_eq!(
            negotiate_export(chart, &CapabilitySet::pgr_v3()).unwrap(),
            NegotiationAction::Equivalent
        );
        assert!(!export_rpe_json(chart).unwrap().is_empty());
        assert!(!export_pec_line(chart).unwrap().is_empty());
    }
}
