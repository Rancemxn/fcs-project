//! PGR format: parse + round-trip tests.

mod common;

use fcs_converter::from_fcs::pgr_writer::fcs_to_pgr_json;
use fcs_converter::ir::IrChart;
use fcs_converter::pgr::parse_pgr;
use fcs_converter::to_fcs::ir_to_fcs;

fn load_pgr(name: &str) -> IrChart {
    let path = common::manifest_path(&format!("examples/pgr/{name}"));
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {name}: {e}"));
    parse_pgr(&src).unwrap_or_else(|e| panic!("failed to parse {name}: {e}"))
}

fn roundtrip_pgr(chart: &IrChart) -> IrChart {
    let doc = ir_to_fcs(chart);
    let out = fcs_to_pgr_json(&doc, 3);
    parse_pgr(&out).unwrap()
}

#[test]
fn test_parse_pgr_simple() {
    let chart = load_pgr("simple.pgr.json");
    assert_eq!(chart.lines.len(), 1);
    assert!((chart.lines[0].bpm - 120.0).abs() < 1e-6);
    assert_eq!(chart.lines[0].notes_above.len(), 3);
    assert_eq!(chart.lines[0].events.speed.len(), 1);
    assert_eq!(chart.lines[0].events.move_x.len(), 1);
    assert_eq!(chart.lines[0].events.rotate.len(), 2);
    assert_eq!(chart.lines[0].events.alpha.len(), 1);
}

#[test]
fn test_parse_pgr_features() {
    let chart = load_pgr("features.pgr.json");
    assert_eq!(chart.lines.len(), 2);
    assert!((chart.lines[0].bpm - 120.0).abs() < 1e-6);
    assert!((chart.lines[1].bpm - 160.0).abs() < 1e-6);
    assert_eq!(chart.lines[0].events.move_y.len(), 1);
}

#[test]
fn test_pgr_roundtrip_simple() {
    let orig = load_pgr("simple.pgr.json");
    let rt = roundtrip_pgr(&orig);
    assert_eq!(orig.lines.len(), rt.lines.len());
    common::compare_events_sampled(
        &orig,
        &rt,
        200,
        common::EventTolerances {
            rotate: 0.001,
            ..Default::default()
        },
    );
}

#[test]
fn test_pgr_roundtrip_features() {
    let orig = load_pgr("features.pgr.json");
    let rt = roundtrip_pgr(&orig);
    assert_eq!(orig.lines.len(), rt.lines.len());
    common::compare_events_sampled(&orig, &rt, 200, common::EventTolerances::default());
}
