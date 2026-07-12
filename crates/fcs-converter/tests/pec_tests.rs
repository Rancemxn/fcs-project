//! PEC format: parse + round-trip tests.

mod common;

use fcs_converter::from_fcs::pec_writer::fcs_to_pec;
use fcs_converter::ir::IrChart;
use fcs_converter::pec::parse_pec;
use fcs_converter::to_fcs::ir_to_fcs;

fn load_pec(name: &str) -> IrChart {
    let path = common::manifest_path(&format!("examples/pec/{name}"));
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {name}: {e}"));
    parse_pec(&src).unwrap_or_else(|e| panic!("failed to parse {name}: {e}"))
}

fn roundtrip_pec(chart: &IrChart) -> IrChart {
    let doc = ir_to_fcs(chart);
    let out = fcs_to_pec(&doc);
    parse_pec(&out).unwrap()
}

#[test]
fn test_parse_pec_simple() {
    let chart = load_pec("simple.pec");
    assert_eq!(chart.lines.len(), 1);
    assert!((chart.lines[0].bpm - 120.0).abs() < 1e-6);
    use fcs_converter::ir::IrNoteKind;
    assert_eq!(chart.lines[0].notes_above[0].kind, IrNoteKind::Tap);
    assert_eq!(chart.lines[0].notes_above[1].kind, IrNoteKind::Hold);
    assert_eq!(chart.lines[0].notes_above[2].kind, IrNoteKind::Drag);
}

#[test]
fn test_parse_pec_all_notes() {
    let chart = load_pec("all-notes.pec");
    use fcs_converter::ir::IrNoteKind;
    assert_eq!(chart.lines[0].notes_above[0].kind, IrNoteKind::Tap);
    assert_eq!(chart.lines[0].notes_above[1].kind, IrNoteKind::Hold);
    assert_eq!(chart.lines[0].notes_above[2].kind, IrNoteKind::Flick);
    assert_eq!(chart.lines[0].notes_above[3].kind, IrNoteKind::Drag);
    // Fake note has above=0, so it goes to notes_below
    assert_eq!(chart.lines[0].notes_below.len(), 1);
    assert!(chart.lines[0].notes_below[0].is_fake);
}

#[test]
fn test_pec_roundtrip_simple() {
    let orig = load_pec("simple.pec");
    let rt = roundtrip_pec(&orig);
    assert_eq!(orig.lines.len(), rt.lines.len());
    common::compare_events_sampled(
        &orig,
        &rt,
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

#[test]
fn test_pec_roundtrip_all_notes() {
    let orig = load_pec("all-notes.pec");
    let rt = roundtrip_pec(&orig);
    assert_eq!(
        orig.lines[0].notes_above.len(),
        rt.lines[0].notes_above.len()
    );
}
