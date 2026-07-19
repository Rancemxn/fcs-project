use fcs_model::{CanonicalProfile, CanonicalProfileFeature, CanonicalValue};
use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;

fn canonical(source: &str) -> fcs_model::CanonicalChart {
    parse_document(source)
        .into_result()
        .expect("source should parse")
        .canonical_chart(CompileTimeLimits::default())
        .unwrap_or_else(|diagnostics| panic!("canonical chart lowering failed: {diagnostics:?}"))
}

#[test]
fn canonical_chart_aggregates_current_i3_products_and_identity() {
    let chart = canonical(
        r#"#fcs 5.0.0
format { profile: chart; features: [playable,]; }
meta { title: "Aggregate"; }
resources {
    audio song {
        source: "assets/song.ogg";
        hash: "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        mediaType: "audio/ogg";
    }
}
sync { primaryAudio: @song; audioOffset: 0s; }
extensions { extension("org.test.chart", 1.0.0) required { "mode": "test", } }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        tracks {
            track fade -> alpha: float {
                segments { [0s, 1s): 1.0 -> 0.5 using "linear"; }
            }
        }
    }
}
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
"#,
    );

    assert_eq!(chart.source_version().as_str(), "5.0.0");
    assert_eq!(chart.profile(), CanonicalProfile::Chart);
    assert!(
        chart
            .features()
            .contains(&CanonicalProfileFeature::Playable)
    );
    assert_eq!(chart.time_map().segments().count(), 1);
    assert_eq!(
        chart.metadata().meta().unwrap().get("title"),
        Some(&CanonicalValue::String("Aggregate".into()))
    );
    assert_eq!(chart.lines().lines().count(), 1);
    assert_eq!(chart.notes().notes().len(), 1);
    assert_eq!(chart.tracks().tracks().len(), 1);
    assert_eq!(chart.scroll().lines().len(), 1);
    assert_eq!(chart.required_extensions().len(), 1);
    let extension = chart.required_extensions().first().unwrap();
    assert_eq!(extension.namespace(), "org.test.chart");
    assert_eq!(extension.version(), "1.0.0");
}

#[test]
fn canonical_chart_is_stable_when_top_level_declarations_are_reordered() {
    let first = canonical(
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
"#,
    );
    let reordered = canonical(
        r#"#fcs 5.0.0
format { profile: chart; }
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
lines { line main {} }
tempoMap { 0beat -> 120bpm; }
"#,
    );

    assert_eq!(first, reordered);
}

#[test]
fn canonical_chart_includes_lines_emitted_by_judgelines() {
    let chart = canonical(
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
collections {
    judgelines { Line { id: "judge"; }; }
    notes { tap { id: "tap"; line: @judge; gameplay.time: 1s; }; }
}
"#,
    );

    assert_eq!(chart.lines().lines().count(), 1);
    assert_eq!(chart.notes().notes().len(), 1);
    assert_eq!(
        chart
            .lines()
            .line_by_textual_id("judge")
            .expect("emitted Line should enter the canonical graph")
            .id()
            .textual()
            .as_str(),
        "judge"
    );
}
