use fcs_source::ResourceLimits;
use fcs_source::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage};
use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;

const NODE: &str = r#"
circle animated {
    center: vec2(0px, 0px);
    radius: 1px;
    fill: solid(#FFFFFFFF);
    tracks { track fade -> opacity: float {
        segments { [0s, 1s): 0.25 -> 0.75 using "linear"; }
    } }
}
"#;

fn scene(core: &str, nodes: &str) -> String {
    format!(
        r#"#fcs 5.0.0
format {{ profile: renderable; }}
tempoMap {{ 0beat -> 120bpm; }}
{core}
render profile 1.0.0 {{
    viewport {{ width: 4px; height: 4px; }}
    layer main {{ pass: "overlay"; children {{ {nodes} }} }}
}}
"#
    )
}

fn assert_budget(diagnostic: &Diagnostic, kind: &str, limit: usize) {
    assert_eq!(
        diagnostic.code(),
        DiagnosticCode::COMPILE_TIME_BUDGET_EXCEEDED
    );
    assert_eq!(diagnostic.stage(), DiagnosticStage::Elaborate);
    let budget = diagnostic.budget().expect("structured budget");
    assert_eq!(budget.kind(), kind);
    assert_eq!(budget.limit(), limit);
    assert_eq!(budget.observed(), limit + 1);
}

#[test]
fn source_aware_boundaries_share_generated_node_budget_with_render_tracks() {
    let line_track = r#"lines { line line1 { tracks { track alpha -> alpha: float {
        segments { [0s, 1s): 0.0 -> 1.0 using "linear"; }
    } } } }"#;
    let note = r#"lines { line line1 {} }
collections { notes { tap { id: "n"; line: @line1; gameplay.time: 0s; }; } }"#;
    let two_nodes = format!("{NODE}{}", NODE.replace("animated", "second"));
    for (core, nodes, limit) in [
        ("", NODE, 0),
        ("", two_nodes.as_str(), 1),
        (line_track, NODE, 1),
        (note, NODE, 1),
    ] {
        let source = scene(core, nodes);
        let document = parse_document(&source).into_result().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let limits = CompileTimeLimits {
            max_generated_nodes: limit,
            ..CompileTimeLimits::default()
        };
        let chart_error = document
            .canonical_chart_with_source(&source, limits)
            .unwrap_err();
        let compilation_error = document
            .canonical_compilation_with_source(
                &source,
                limits,
                workspace.path(),
                ResourceLimits::default(),
            )
            .unwrap_err();
        assert_eq!(chart_error, compilation_error);
        let diagnostic = &chart_error[0];
        assert_budget(diagnostic, "max_generated_nodes", limit);
        let span = diagnostic.primary_span();
        assert_eq!(span.start, source.rfind("[0s, 1s)").unwrap());
        assert!(source[span.start..span.end].contains("0.25 -> 0.75"));
        // These are direct declarations, so no generator/template is active.
        assert!(diagnostic.expansion_trace().is_empty());
        let accepted = CompileTimeLimits {
            max_generated_nodes: limit + 1,
            ..limits
        };
        document
            .canonical_chart_with_source(&source, accepted)
            .unwrap();
        document
            .canonical_compilation_with_source(
                &source,
                accepted,
                workspace.path(),
                ResourceLimits::default(),
            )
            .unwrap();
    }
}

#[test]
fn render_expression_helpers_use_the_configured_shared_budget() {
    let source = scene("", NODE);
    let document = parse_document(&source).into_result().unwrap();
    for (kind, limits) in [
        (
            "max_compile_time_operations",
            CompileTimeLimits {
                max_compile_time_operations: 0,
                ..CompileTimeLimits::default()
            },
        ),
        (
            "max_expression_nodes",
            CompileTimeLimits {
                max_expression_nodes: 0,
                ..CompileTimeLimits::default()
            },
        ),
    ] {
        let error = document
            .canonical_chart_with_source(&source, limits)
            .unwrap_err();
        assert_budget(&error[0], kind, 0);
        let span = error[0].primary_span();
        assert_eq!(&source[span.start..span.end], "4px");
    }
}

#[test]
fn source_aware_core_expression_helpers_use_the_same_budget() {
    for core in [
        "meta { level: 1.0 + 2.0; }",
        "lines { line main { alpha: 0.5 + 0.25; } }",
    ] {
        let source = scene(core, NODE);
        let document = parse_document(&source).into_result().unwrap();
        let errors = document
            .canonical_chart_with_source(
                &source,
                CompileTimeLimits {
                    max_expression_nodes: 0,
                    ..CompileTimeLimits::default()
                },
            )
            .unwrap_err();
        assert_budget(&errors[0], "max_expression_nodes", 0);
        assert!(errors[0].primary_span().start < source.find("render profile").unwrap());
    }
}

#[test]
fn runtime_render_fallback_cannot_swallow_a_budget_error() {
    let node = r#"circle animated {
        center: vec2(0px, 0px); radius: 1px; fill: solid(#FFFFFFFF);
        opacity: q;
    }"#;
    let source = scene("", node);
    let document = parse_document(&source).into_result().unwrap();
    // Every insufficient limit must retain its budget diagnostic, including
    // limits reached while deciding whether opacity is a runtime expression.
    let mut accepted = false;
    for limit in 0..100 {
        match document.canonical_chart_with_source(
            &source,
            CompileTimeLimits {
                max_expression_nodes: limit,
                ..CompileTimeLimits::default()
            },
        ) {
            Ok(_) => {
                accepted = true;
                break;
            }
            Err(errors) => assert_budget(&errors[0], "max_expression_nodes", limit),
        }
    }
    assert!(
        accepted,
        "the bounded scene must compile with a sufficient budget"
    );
}
