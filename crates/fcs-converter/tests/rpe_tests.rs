//! RPE format: parse + round-trip tests.

#[path = "common/paths.rs"]
mod paths;
#[path = "common/roundtrip.rs"]
mod roundtrip;

use fcs_converter::from_fcs::rpe_writer::fcs_to_rpe_json;
use fcs_converter::ir::IrChart;
use fcs_converter::rpe::parse_rpe;
use fcs_converter::to_fcs::ir_to_fcs;

fn load_rpe(name: &str) -> IrChart {
    let path = paths::manifest_path(&format!("examples/rpe/{name}"));
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {name}: {e}"));
    parse_rpe(&src).unwrap_or_else(|e| panic!("failed to parse {name}: {e}"))
}

fn roundtrip_rpe(chart: &IrChart) -> IrChart {
    let doc = ir_to_fcs(chart);
    let out = fcs_to_rpe_json(&doc);
    parse_rpe(&out).unwrap()
}

#[test]
fn test_parse_rpe_simple() {
    let chart = load_rpe("simple.rpe.json");
    assert_eq!(chart.lines.len(), 1);
    assert!((chart.lines[0].bpm - 120.0).abs() < 1e-6);
    assert_eq!(chart.lines[0].notes_above.len(), 4);
}

#[test]
fn test_parse_rpe_extremes() {
    let chart = load_rpe("extremes.rpe.json");
    // RPE position_x is on 1350-wide canvas → FCS 1920-wide: x_fcs = x_rpe / 1350 * 1920
    let fcs_neg = -675.0 / 1350.0 * 1920.0;
    let fcs_pos = 675.0 / 1350.0 * 1920.0;
    assert!((chart.lines[0].notes_above[0].position_x - fcs_neg).abs() < 1.0);
    assert!((chart.lines[0].notes_above[1].position_x - fcs_pos).abs() < 1.0);
}

#[test]
fn test_rpe_roundtrip_simple() {
    let orig = load_rpe("simple.rpe.json");
    let rt = roundtrip_rpe(&orig);
    assert_eq!(orig.lines.len(), rt.lines.len());
    roundtrip::compare_events_sampled(
        &orig,
        &rt,
        200,
        roundtrip::EventTolerances {
            rotate: 0.001,
            move_x: 0.1,
            move_y: 0.1,
            speed: 0.01,
            alpha: 0.01,
        },
    );
}

#[test]
fn test_rpe_roundtrip_extremes() {
    let orig = load_rpe("extremes.rpe.json");
    let rt = roundtrip_rpe(&orig);
    assert_eq!(orig.lines.len(), rt.lines.len());
    roundtrip::compare_events_sampled(
        &orig,
        &rt,
        200,
        roundtrip::EventTolerances {
            rotate: 0.001,
            move_x: 0.1,
            move_y: 0.1,
            speed: 0.01,
            alpha: 0.01,
        },
    );
}
