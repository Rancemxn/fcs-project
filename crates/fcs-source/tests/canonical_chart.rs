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
fn canonical_chart_includes_direct_template_and_generator_judgelines() {
    let chart = canonical(
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
definitions {
    template Line makeJudge() {
        return Line { id: "made"; };
    }
}
collections {
    judgelines {
        Line { id: "direct"; zOrder: 7; };
        makeJudge();
        generate i: int in 0..=0 step 1 {
            emit Line { id: "generator"; };
        }
    }
    notes {
        tap { id: "direct-note"; line: @direct; gameplay.time: 1s; };
        tap { id: "template-note"; line: @made; gameplay.time: 2s; };
        tap { id: "generator-note"; line: @generator; gameplay.time: 3s; };
    }
}
"#,
    );

    assert_eq!(chart.lines().lines().count(), 3);
    assert_eq!(chart.notes().notes().len(), 3);
    assert_eq!(chart.scroll().lines().len(), 3);
    let direct = chart
        .lines()
        .line_by_textual_id("direct")
        .expect("direct emitted Line should enter the canonical graph");
    assert_eq!(direct.base().z_order(), 7);
    assert_eq!(direct.base().position().x(), 0.0);
    assert_eq!(direct.base().position().y(), 0.0);
    assert!(direct.inherit().position());
    assert!(!direct.inherit().scroll());
    assert_eq!(
        chart
            .lines()
            .line_by_textual_id("made")
            .expect("template-produced Line should enter the canonical graph")
            .id()
            .textual()
            .as_str(),
        "made"
    );
    assert!(chart.lines().line_by_textual_id("generator").is_some());
}
