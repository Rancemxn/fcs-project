use fcs_model::{EntityKind, derive_stable_id};
use fcs_source::ast::LineBodyItem;
use fcs_source::diagnostic::DiagnosticCode;
use fcs_source::elaborator::{CompileTimeLimits, elaborate};
use fcs_source::parser::parse_document;
use fcs_source::schema::phase2_schema;

fn canonical(source: &str) -> fcs_model::CanonicalLineGraph {
    parse_document(source)
        .into_result()
        .expect("source should parse")
        .canonical_line_graph()
        .unwrap_or_else(|diagnostics| panic!("canonical line graph failed: {diagnostics:?}"))
}

fn line<'a>(graph: &'a fcs_model::CanonicalLineGraph, name: &str) -> &'a fcs_model::CanonicalLine {
    graph
        .line_by_textual_id(name)
        .unwrap_or_else(|| panic!("missing line {name}"))
}

const HEADER: &str = "#fcs 5.0.0\nformat { profile: chart; }\n";

#[test]
fn identity_line_defaults_and_explicit_base_values_are_canonical() {
    let graph = canonical(&format!(
        "{HEADER}lines {{\
            line main {{\
                position: vec2(10px, 20px);\
                rotation: 90deg;\
                scale: vec2(2.0, 0.5);\
                alpha: 0.75;\
                transformOrigin: vec2(3px, 4px);\
                textureAnchor: vec2(0.25, 0.75);\
                floorScale: 120px;\
                integrationOrigin: -2s;\
                initialFloorPosition: 4.5;\
                allowReverseScroll: true;\
                zOrder: -3;\
            }}\
            line identity {{}}\
        }}"
    ));

    let main = line(&graph, "main");
    assert_eq!(
        main.id().value(),
        derive_stable_id(EntityKind::Line, "main")
    );
    assert_eq!(main.base().position().x(), 10.0);
    assert_eq!(main.base().position().y(), 20.0);
    assert_eq!(main.base().rotation(), std::f64::consts::FRAC_PI_2);
    assert_eq!(main.base().scale().x(), 2.0);
    assert_eq!(main.base().scale().y(), 0.5);
    assert_eq!(main.base().alpha(), 0.75);
    assert_eq!(main.base().transform_origin().x(), 3.0);
    assert_eq!(main.base().texture_anchor().y(), 0.75);
    assert_eq!(main.base().floor_scale(), 120.0);
    assert_eq!(main.base().integration_origin(), -2.0);
    assert_eq!(main.base().initial_floor_position(), 4.5);
    assert!(main.base().allow_reverse_scroll());
    assert_eq!(main.base().z_order(), -3);

    let identity = line(&graph, "identity");
    assert_eq!(identity.base(), &fcs_model::CanonicalLineBase::identity());
    assert_eq!(
        identity.inherit(),
        &fcs_model::CanonicalLineInherit::default()
    );
    assert_eq!(
        identity.scroll_tempo(),
        &fcs_model::CanonicalScrollTempo::Global
    );
}

#[test]
fn parent_graph_has_stable_id_topology_and_component_composition() {
    let first = canonical(&format!(
        "{HEADER}lines {{\
            line root {{ position: vec2(10px, 20px); rotation: 90deg; scale: vec2(2.0, 3.0); alpha: 0.5; }}\
            line child {{ parent: @root; position: vec2(1px, 2px); scale: vec2(4.0, 5.0); }}\
        }}"
    ));
    let reordered = canonical(&format!(
        "{HEADER}lines {{\
            line child {{ parent: @root; position: vec2(1px, 2px); scale: vec2(4.0, 5.0); }}\
            line root {{ position: vec2(10px, 20px); rotation: 90deg; scale: vec2(2.0, 3.0); alpha: 0.5; }}\
        }}"
    ));

    let first_order = first
        .topological_order()
        .iter()
        .map(|id| id.textual().as_str())
        .collect::<Vec<_>>();
    let reordered_order = reordered
        .topological_order()
        .iter()
        .map(|id| id.textual().as_str())
        .collect::<Vec<_>>();
    assert_eq!(first_order, reordered_order);
    assert_eq!(first_order, vec!["root", "child"]);

    let child = first.world_state("child").expect("child world state");
    assert_eq!(child.origin().x(), 4.0);
    assert_eq!(child.origin().y(), 22.0);
    assert_eq!(child.rotation(), std::f64::consts::FRAC_PI_2);
    assert_eq!(child.scale().x(), 8.0);
    assert_eq!(child.scale().y(), 15.0);
    assert_eq!(child.alpha(), 0.5);
}

#[test]
fn parent_graph_rejects_unknown_self_parent_and_cycles_with_stable_codes() {
    for (parent, expected) in [
        ("@missing", DiagnosticCode::GRAPH_UNKNOWN_PARENT),
        ("@self", DiagnosticCode::GRAPH_CYCLE),
    ] {
        let source = format!("{HEADER}lines {{ line self {{ parent: {parent}; }} }}");
        let document = parse_document(&source).into_result().unwrap();
        let diagnostics = document.canonical_line_graph().unwrap_err();
        assert_eq!(diagnostics[0].code(), expected);
    }

    let document = parse_document(&format!(
        "{HEADER}lines {{\
            line a {{ parent: @b; }}\
            line b {{ parent: @a; }}\
        }}"
    ))
    .into_result()
    .unwrap();
    let diagnostics = document.canonical_line_graph().unwrap_err();
    assert_eq!(diagnostics[0].code(), DiagnosticCode::GRAPH_CYCLE);
}

#[test]
fn inherit_overrides_start_from_world_identity_for_each_component() {
    let graph = canonical(&format!(
        "{HEADER}lines {{\
            line root {{ position: vec2(10px, 20px); rotation: 90deg; scale: vec2(2.0, 3.0); alpha: 0.5; }}\
            line child {{ parent: @root; position: vec2(1px, 2px); rotation: 10deg; scale: vec2(4.0, 5.0); alpha: 0.25; inherit.position: false; inherit.rotation: false; inherit.scale: false; inherit.alpha: false; }}\
        }}"
    ));
    let child = graph.world_state("child").unwrap();
    assert_eq!(child.origin().x(), 1.0);
    assert_eq!(child.origin().y(), 2.0);
    assert_eq!(child.rotation(), 10f64.to_radians());
    assert_eq!(child.scale().x(), 4.0);
    assert_eq!(child.scale().y(), 5.0);
    assert_eq!(child.alpha(), 0.25);
}

#[test]
fn scroll_tempo_map_is_typed_and_keeps_one_key_domain() {
    let source = format!(
        "{HEADER}tempoMap {{ 0beat -> 180bpm; }}\
        lines {{ line main {{ scrollTempoMap {{ 0beat -> 180bpm; 4beat -> 240bpm; }} }} }}"
    );
    let document = parse_document(&source).into_result().unwrap();
    let line_ast = &document.lines[0];
    let LineBodyItem::ScrollTempoMap(map) = &line_ast.items[0] else {
        panic!("expected typed scrollTempoMap AST")
    };
    assert_eq!(map.points.len(), 2);
    let graph = document.canonical_line_graph().unwrap();
    let scroll = line(&graph, "main").scroll_tempo();
    let fcs_model::CanonicalScrollTempo::Override(map) = scroll else {
        panic!("expected override scroll tempo map")
    };
    assert_eq!(map.points().len(), 2);
    assert!(matches!(map.domain(), fcs_model::ScrollTempoDomain::Beat));
}

#[test]
fn scroll_tempo_map_rejects_mixed_domain_nonzero_origin_descending_and_bad_bpm() {
    let cases = [
        (
            "0beat -> 180bpm; 1s -> 200bpm;",
            DiagnosticCode::TEMPO_INVALID,
        ),
        ("1beat -> 180bpm;", DiagnosticCode::TEMPO_INVALID),
        (
            "0beat -> 180bpm; -1beat -> 200bpm;",
            DiagnosticCode::TEMPO_NON_MONOTONIC,
        ),
        ("0beat -> -1bpm;", DiagnosticCode::TEMPO_INVALID),
    ];
    for (points, expected) in cases {
        let source = format!("{HEADER}lines {{ line main {{ scrollTempoMap {{ {points} }} }} }}");
        let document = parse_document(&source).into_result().unwrap();
        let diagnostics = document.canonical_line_graph().unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code() == expected),
            "expected {expected}, got {diagnostics:?}"
        );
    }
}

#[test]
fn invalid_line_base_values_have_canonical_diagnostics() {
    let source = format!(
        "{HEADER}lines {{ line main {{\
            alpha: 2.0;\
            textureAnchor: vec2(-0.1, 1.1);\
            floorScale: 0px;\
            integrationOrigin: 1e308s + 1e308s;\
        }} }}"
    );
    let document = parse_document(&source).into_result().unwrap();
    let diagnostics = document.canonical_line_graph().unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code() == DiagnosticCode::NUMERIC_DOMAIN)
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code() == DiagnosticCode::NUMERIC_NON_FINITE)
    );
}

#[test]
fn expanded_line_entities_use_generated_and_explicit_canonical_ids() {
    let document = parse_document(
        r#"#fcs 5.0.0
format { profile: chart; }
collections { judgelines { Line {}; Line { id: "explicit"; }; } }
"#,
    )
    .into_result()
    .unwrap();
    let expanded = elaborate(&document, phase2_schema(), CompileTimeLimits::default()).unwrap();
    let ids = expanded.canonical_line_ids().unwrap();
    assert_eq!(
        ids.iter()
            .map(|id| id.textual().as_str())
            .collect::<Vec<_>>(),
        vec![
            "generated/line/collection/judgelines/item/0/order/0",
            "explicit"
        ]
    );
}
