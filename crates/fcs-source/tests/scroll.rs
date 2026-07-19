use fcs_model::{Beat, ChartTimeMap, TempoPoint};
use fcs_source::parser::parse_document;

const HEADER: &str = "#fcs 5.0.0\nformat { profile: chart; }\n";

fn time_map() -> ChartTimeMap {
    ChartTimeMap::new([
        TempoPoint {
            beat: Beat::zero(),
            bpm: 120.0,
        },
        TempoPoint {
            beat: Beat::new(4, 1).unwrap(),
            bpm: 240.0,
        },
    ])
    .unwrap()
}

fn scroll(source: &str) -> fcs_model::CanonicalScrollSet {
    let document = parse_document(source).into_result().unwrap();
    document.canonical_scroll_set(&time_map()).unwrap()
}

#[test]
fn global_scroll_uses_chart_time_and_line_defaults() {
    let set = scroll(&format!(
        "{HEADER}tempoMap {{ 0beat -> 120bpm; 4beat -> 240bpm; }} lines {{ line main {{}} }}"
    ));
    let line = &set.lines()[0];
    assert_eq!(line.coordinate().coordinate(-1.0).unwrap(), -2.0);
    assert_eq!(line.coordinate().coordinate(1.0).unwrap(), 2.0);
    assert_eq!(line.scroll_bpm(1.0).unwrap(), 120.0);
    assert_eq!(line.floor_position(1.0).unwrap(), 2.0);
}

#[test]
fn beat_scroll_override_is_normalized_through_global_chart_time() {
    let set = scroll(&format!(
        "{HEADER}tempoMap {{ 0beat -> 120bpm; 4beat -> 240bpm; }} lines {{ line main {{ scrollTempoMap {{ 0beat -> 60bpm; 4beat -> 120bpm; }} }} }}"
    ));
    let line = &set.lines()[0];
    assert_eq!(line.coordinate().coordinate(1.0).unwrap(), 1.0);
    assert_eq!(line.scroll_bpm(1.0).unwrap(), 60.0);
    assert_eq!(line.coordinate().points()[1].chart_time(), 2.0);
}

#[test]
fn line_ids_are_stable_and_set_order_is_independent_of_source_order() {
    let first = scroll(&format!(
        "{HEADER}tempoMap {{ 0beat -> 120bpm; }} lines {{ line z {{}} line a {{}} }}"
    ));
    let second = scroll(&format!(
        "{HEADER}tempoMap {{ 0beat -> 120bpm; }} lines {{ line a {{}} line z {{}} }}"
    ));
    let first_ids: Vec<_> = first
        .lines()
        .iter()
        .map(|line| line.line_id().value())
        .collect();
    let second_ids: Vec<_> = second
        .lines()
        .iter()
        .map(|line| line.line_id().value())
        .collect();
    assert_eq!(first_ids, second_ids);
    assert_eq!(
        first.lines()[0].coordinate(),
        second.lines()[0].coordinate()
    );
}

#[test]
fn scroll_model_keeps_existing_line_policy_values() {
    let set = scroll(&format!(
        "{HEADER}tempoMap {{ 0beat -> 120bpm; }} lines {{ line main {{ floorScale: 240px; integrationOrigin: -1s; initialFloorPosition: 12.0; allowReverseScroll: true; }} }}"
    ));
    let line = &set.lines()[0];
    assert_eq!(line.floor_scale(), 240.0);
    assert_eq!(line.integration_origin(), -1.0);
    assert_eq!(line.initial_floor_position(), 12.0);
    assert!(line.allow_reverse_scroll());
    assert_eq!(line.floor_position(-1.0).unwrap(), 12.0);
}
