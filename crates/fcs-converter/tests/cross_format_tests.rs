//! Cross-format conversion smoke tests.
//!
//! These tests verify that converting between formats produces parsable output.
//! Known limitations: PGR→FCS→RPE/PEC round-trip may produce output with
//! format-specific differences (BPM timeline, motion encoding, etc.) that
//! don't re-parse cleanly. These tests validate the conversion pipeline
//! without requiring reverse-parse correctness.

#[path = "common/paths.rs"]
mod paths;

fn load_pgr(name: &str) -> fcs_converter::ir::IrChart {
    let path = paths::manifest_path(&format!("examples/pgr/{name}"));
    let src = std::fs::read_to_string(&path).unwrap();
    fcs_converter::pgr::parse_pgr(&src).unwrap()
}

fn load_rpe(name: &str) -> fcs_converter::ir::IrChart {
    let path = paths::manifest_path(&format!("examples/rpe/{name}"));
    let src = std::fs::read_to_string(&path).unwrap();
    fcs_converter::rpe::parse_rpe(&src).unwrap()
}

fn load_pec(name: &str) -> fcs_converter::ir::IrChart {
    let path = paths::manifest_path(&format!("examples/pec/{name}"));
    let src = std::fs::read_to_string(&path).unwrap();
    fcs_converter::pec::parse_pec(&src).unwrap()
}

#[test]
fn test_cross_pgr_to_rpe() {
    let ir = load_pgr("simple.pgr.json");
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let rpe_str = fcs_converter::from_fcs::rpe_writer::fcs_to_rpe_json(&doc);
    assert!(!rpe_str.is_empty(), "RPE output should not be empty");
    assert_eq!(ir.lines.len(), doc.judgelines.lines.len());
}

#[test]
fn test_cross_pgr_to_pec() {
    let ir = load_pgr("simple.pgr.json");
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let pec_str = fcs_converter::from_fcs::pec_writer::fcs_to_pec(&doc);
    assert!(!pec_str.is_empty(), "PEC output should not be empty");
    assert_eq!(ir.lines.len(), doc.judgelines.lines.len());
}

#[test]
fn test_cross_rpe_to_pgr() {
    let ir = load_rpe("simple.rpe.json");
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let pgr_str = fcs_converter::from_fcs::pgr_writer::fcs_to_pgr_json(&doc, 3);
    let ir_pgr = fcs_converter::pgr::parse_pgr(&pgr_str).unwrap();
    assert_eq!(ir.lines.len(), ir_pgr.lines.len());
}

#[test]
fn test_cross_pec_to_pgr() {
    let ir = load_pec("simple.pec");
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let pgr_str = fcs_converter::from_fcs::pgr_writer::fcs_to_pgr_json(&doc, 3);
    let ir_pgr = fcs_converter::pgr::parse_pgr(&pgr_str).unwrap();
    assert_eq!(ir.lines.len(), ir_pgr.lines.len());
}
