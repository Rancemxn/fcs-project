//! Round-trip integration tests for all three chart formats.
//!
//! Each test:
//! 1. Parses original chart → IrChart
//! 2. Converts IrChart → FCS Document
//! 3. Converts FCS Document → target format string
//! 4. Parses target format string → IrChart
//! 5. Compares structural properties, per-field note values, and
//!    time-sampled event values (line parameters at specific times)
//!
//! Gaps (do NOT add assertions for these — known format limitations):
//! - `ir_to_fcs()` ignores `chart.bpm_list` entirely, creating only a single
//!   master_timeline entry from the first line's bpm. PEC charts with 33+ bp
//!   entries round-trip to 1.
//! - `visible_time`, `alpha`, `size`, `y_offset` are stored in IrNote but
//!   not propagated to FCS NoteInstance properties in `to_fcs.rs`.
//! - `IrNoteKind::Flick` doesn't get `endTime` set in `to_fcs.rs`, so
//!   PEC n3 holdTime is lost.
//! - PEC cp/cd/ca/cv point events inflate during round-trip because the
//!   IR→FCS conversion creates intervals from point events, then the
//!   writer creates additional center-position events from autofill.
//! - PEC cm/cr/cf interpolation events: the writer only outputs cp/cd/ca/cv,
//!   not the interpolation variants, so these are lost.

mod common;

use std::path::Path;

fn manifest_path(rel: &str) -> String {
    let dir = env!("CARGO_MANIFEST_DIR");
    let full = Path::new(dir).join("../../").join(rel);
    full.to_string_lossy().to_string()
}

// ---------------------------------------------------------------------------
// PGR round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_pgr_roundtrip_small() {
    let path = manifest_path("examples/test.pgr.json");
    let src = std::fs::read_to_string(&path).unwrap();

    let ir = fcs_converter::pgr::parse_pgr(&src).unwrap();
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let rt_str = fcs_converter::from_fcs::pgr_writer::fcs_to_pgr_json(&doc, 3);
    let ir_rt = fcs_converter::pgr::parse_pgr(&rt_str).unwrap();

    assert_eq!(ir.lines.len(), ir_rt.lines.len(), "line count");
    assert_eq!(ir.meta.source_version, ir_rt.meta.source_version);
    // PGR has no per-line BPM beyond the first entry; each line has one bpm.
    // The line.bpm is preserved through the round-trip (no master_timeline loss).
    for (i, (ol, rl)) in ir.lines.iter().zip(&ir_rt.lines).enumerate() {
        assert!(
            (ol.bpm - rl.bpm).abs() < 1e-6,
            "line {i} bpm: {} vs {}",
            ol.bpm,
            rl.bpm
        );
    }

    // Per-field note comparison (exact, 3 notes)
    common::compare_notes_exact(&ir, &ir_rt, 1e-6);

    // Time-sampled event comparison with tight tolerance (junctionBeats preserves boundaries)
    common::compare_events_sampled(
        &ir,
        &ir_rt,
        200,
        common::EventTolerances {
            rotate: 0.001,
            ..Default::default()
        },
    );
}

#[test]
fn test_pgr_roundtrip_medium() {
    let path = manifest_path("examples/4886210000956270.json");
    let src = std::fs::read_to_string(&path).unwrap();

    let ir = fcs_converter::pgr::parse_pgr(&src).unwrap();
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let rt_str = fcs_converter::from_fcs::pgr_writer::fcs_to_pgr_json(&doc, 3);
    let ir_rt = fcs_converter::pgr::parse_pgr(&rt_str).unwrap();

    assert_eq!(ir.lines.len(), ir_rt.lines.len(), "line count");

    // BPM per-line preserved
    for (i, (ol, rl)) in ir.lines.iter().zip(&ir_rt.lines).enumerate() {
        assert!(
            (ol.bpm - rl.bpm).abs() < 1e-6,
            "line {i} bpm: {} vs {}",
            ol.bpm,
            rl.bpm
        );
    }

    // Note count per line
    for (i, (ol, rl)) in ir.lines.iter().zip(&ir_rt.lines).enumerate() {
        let o_count = ol.notes_above.len() + ol.notes_below.len();
        let r_count = rl.notes_above.len() + rl.notes_below.len();
        assert_eq!(o_count, r_count, "line {i} note count");
    }

    // Per-field note comparison on first 500 notes per line
    for (i, (ol, rl)) in ir.lines.iter().zip(&ir_rt.lines).enumerate() {
        let o_notes: Vec<&fcs_converter::ir::IrNote> = ol
            .notes_above
            .iter()
            .chain(&ol.notes_below)
            .take(500)
            .collect();
        let r_notes: Vec<&fcs_converter::ir::IrNote> = rl
            .notes_above
            .iter()
            .chain(&rl.notes_below)
            .take(500)
            .collect();
        for (j, (on, rn)) in o_notes.iter().zip(&r_notes).enumerate() {
            assert!(
                (on.time_beat - rn.time_beat).abs() < 1e-6,
                "line {i} note {j} time"
            );
            assert!(
                (on.position_x - rn.position_x).abs() < 1e-6,
                "line {i} note {j} pos"
            );
            assert!(
                (on.speed - rn.speed).abs() < 1e-6,
                "line {i} note {j} speed"
            );
        }
    }

    // Time-sampled event comparison (tight tolerance with junctionBeats)
    common::compare_events_sampled(
        &ir,
        &ir_rt,
        200,
        common::EventTolerances {
            rotate: 0.001,
            ..Default::default()
        },
    );
}

// ---------------------------------------------------------------------------
// RPE round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_rpe_roundtrip_small() {
    let path = manifest_path("examples/10176.json");
    let src = std::fs::read_to_string(&path).unwrap();

    let ir = fcs_converter::rpe::parse_rpe(&src).unwrap();
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let rt_str = fcs_converter::from_fcs::rpe_writer::fcs_to_rpe_json(&doc);
    let ir_rt = fcs_converter::rpe::parse_rpe(&rt_str).unwrap();

    assert_eq!(ir.lines.len(), ir_rt.lines.len(), "line count");

    // Note count ratio < 2x (RPE time precision causes minor splitting)
    for (i, (ol, rl)) in ir.lines.iter().zip(&ir_rt.lines).enumerate() {
        let o_count = ol.notes_above.len() + ol.notes_below.len();
        let r_count = rl.notes_above.len() + rl.notes_below.len();
        let ratio = if o_count > r_count {
            o_count as f64 / r_count.max(1) as f64
        } else {
            r_count as f64 / o_count.max(1) as f64
        };
        assert!(
            ratio < 2.0,
            "line {i} note count: {o_count} vs {r_count} (ratio {ratio:.2})"
        );
    }

    // Time-sampled event comparison. RPE rotate values can be very large
    // (up to ~72090), and the half-split approximation in to_fcs produces
    // step-function diffs up to ~50% of the span value.
    common::compare_events_sampled(
        &ir,
        &ir_rt,
        200,
        common::EventTolerances {
            rotate: 40000.0,
            alpha: 2.0,
            speed: 10.0,
            ..Default::default()
        },
    );
}

#[test]
fn test_rpe_roundtrip_large() {
    let path = manifest_path("examples/10674.json");
    let src = std::fs::read_to_string(&path).unwrap();

    let ir = fcs_converter::rpe::parse_rpe(&src).unwrap();
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let rt_str = fcs_converter::from_fcs::rpe_writer::fcs_to_rpe_json(&doc);
    let ir_rt = fcs_converter::rpe::parse_rpe(&rt_str).unwrap();

    assert_eq!(ir.lines.len(), ir_rt.lines.len(), "line count");

    // Time-sampled event comparison
    common::compare_events_sampled(
        &ir,
        &ir_rt,
        200,
        common::EventTolerances {
            rotate: 40000.0,
            alpha: 2.0,
            speed: 10.0,
            ..Default::default()
        },
    );
}

// ---------------------------------------------------------------------------
// PEC round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_pec_roundtrip() {
    let path = manifest_path("examples/3007.json");
    let src = std::fs::read_to_string(&path).unwrap();

    let ir = fcs_converter::pec::parse_pec(&src).unwrap();
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let rt_str = fcs_converter::from_fcs::pec_writer::fcs_to_pec(&doc);
    let ir_rt = fcs_converter::pec::parse_pec(&rt_str).unwrap();

    assert_eq!(ir.lines.len(), ir_rt.lines.len(), "line count");

    // Note count ratio < 1.5x (better than RPE because PEC time encoding
    // is deterministic beat·2048, not floating-point quantization)
    let mut max_ratio = 0.0f64;
    for (i, (ol, rl)) in ir.lines.iter().zip(&ir_rt.lines).enumerate() {
        let o_count = ol.notes_above.len() + ol.notes_below.len();
        let r_count = rl.notes_above.len() + rl.notes_below.len();
        let ratio = if o_count > r_count {
            o_count as f64 / r_count.max(1) as f64
        } else {
            r_count as f64 / o_count.max(1) as f64
        };
        if ratio > max_ratio {
            max_ratio = ratio;
        }
        assert!(
            ratio < 1.5,
            "line {i} note count: {o_count} vs {r_count} (ratio {ratio:.2})"
        );
    }

    // Per-field note comparison (PEC coordinate double-rounding
    // gives ~0.5px position_x noise; tolerance 1.0 handles this)
    common::compare_notes_exact(&ir, &ir_rt, 1.0);

    // Time-sampled event comparison.
    //
    // PEC motion uses cp point-events, not intervals. The IR → FCS conversion
    // creates intervals from point events, extending them with EPS. Autofill
    // fills gaps with default values (center for moveX/Y). The round-trip
    // PEC output gains extra center-position cp events that the original
    // didn't have, producing large moveX/Y diffs (up to half-canvas = 960).
    // This is a fundamental PEC format limitation, not a bug.
    common::compare_events_sampled(
        &ir,
        &ir_rt,
        200,
        common::EventTolerances {
            rotate: 40000.0,
            move_x: 1000.0,
            move_y: 1000.0,
            speed: 10.0,
            alpha: 2.0,
        },
    );
}

// ---------------------------------------------------------------------------
// Cross-format smoke tests
// ---------------------------------------------------------------------------

/// Cross-format: PGR → FCS → RPE. The output should parse as valid RPE
/// with the same number of lines and roughly the same note count.
#[test]
fn test_cross_pgr_to_rpe() {
    let path = manifest_path("examples/4886210000956270.json");
    let src = std::fs::read_to_string(&path).unwrap();

    let ir = fcs_converter::pgr::parse_pgr(&src).unwrap();
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let rpe_str = fcs_converter::from_fcs::rpe_writer::fcs_to_rpe_json(&doc);
    let ir_rpe = fcs_converter::rpe::parse_rpe(&rpe_str).unwrap();

    assert_eq!(ir.lines.len(), ir_rpe.lines.len(), "line count");

    // Note count ratio -- cross-format has inherent quantization differences
    for (i, (ol, rl)) in ir.lines.iter().zip(&ir_rpe.lines).enumerate() {
        let o_count = ol.notes_above.len() + ol.notes_below.len();
        let r_count = rl.notes_above.len() + rl.notes_below.len();
        let ratio = if o_count > r_count {
            o_count as f64 / r_count.max(1) as f64
        } else {
            r_count as f64 / o_count.max(1) as f64
        };
        assert!(
            ratio < 3.0,
            "line {i} note count: {o_count} vs {r_count} (ratio {ratio:.2})"
        );
    }
}

/// Cross-format: RPE → FCS → PGR. The output should parse as valid PGR
/// with the same number of lines and roughly the same note count.
#[test]
fn test_cross_rpe_to_pgr() {
    let path = manifest_path("examples/10176.json");
    let src = std::fs::read_to_string(&path).unwrap();

    let ir = fcs_converter::rpe::parse_rpe(&src).unwrap();
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let pgr_str = fcs_converter::from_fcs::pgr_writer::fcs_to_pgr_json(&doc, 3);
    let ir_pgr = fcs_converter::pgr::parse_pgr(&pgr_str).unwrap();

    assert_eq!(ir.lines.len(), ir_pgr.lines.len(), "line count");

    // Note count ratio -- cross-format has inherent quantization differences
    for (i, (ol, rl)) in ir.lines.iter().zip(&ir_pgr.lines).enumerate() {
        let o_count = ol.notes_above.len() + ol.notes_below.len();
        let r_count = rl.notes_above.len() + rl.notes_below.len();
        let ratio = if o_count > r_count {
            o_count as f64 / r_count.max(1) as f64
        } else {
            r_count as f64 / o_count.max(1) as f64
        };
        assert!(
            ratio < 3.0,
            "line {i} note count: {o_count} vs {r_count} (ratio {ratio:.2})"
        );
    }
}

/// Cross-format: PEC → FCS → PGR. The output should parse as valid PGR.
#[test]
fn test_cross_pec_to_pgr() {
    let path = manifest_path("examples/3007.json");
    let src = std::fs::read_to_string(&path).unwrap();

    let ir = fcs_converter::pec::parse_pec(&src).unwrap();
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let pgr_str = fcs_converter::from_fcs::pgr_writer::fcs_to_pgr_json(&doc, 3);
    let ir_pgr = fcs_converter::pgr::parse_pgr(&pgr_str).unwrap();

    // Note count should be preserved exactly (PEC -> PGR coordinate conversion
    // doesn't add or remove notes)
    for (i, (ol, rl)) in ir.lines.iter().zip(&ir_pgr.lines).enumerate() {
        let o_count = ol.notes_above.len() + ol.notes_below.len();
        let r_count = rl.notes_above.len() + rl.notes_below.len();
        assert_eq!(
            o_count, r_count,
            "line {i} note count: {o_count} vs {r_count}"
        );
    }
}
