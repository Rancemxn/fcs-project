use fcs_source::ResourceLimits;
use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;
use tempfile::tempdir;

#[test]
fn native_canonical_compilation_has_empty_distribution_metadata() {
    let workspace = tempdir().expect("temp workspace");
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
"#;
    let document = parse_document(source)
        .into_result()
        .expect("source should parse");
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| panic!("canonical compilation failed: {diagnostics:?}"));

    assert_eq!(compilation.chart().source_version().as_str(), "5.0.0");
    assert!(compilation.resources().is_empty());
    assert!(compilation.distribution().is_empty());
    let (chart, resources) = compilation.without_distribution();
    assert_eq!(chart.source_version().as_str(), "5.0.0");
    assert!(resources.is_empty());
}
