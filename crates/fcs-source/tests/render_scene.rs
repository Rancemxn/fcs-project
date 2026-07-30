use fcs_model::CanonicalRenderNodeKind;
use fcs_source::ast::{
    Document, EntityExpression, GeneratorOwner, RenderBlock, RenderBodyItem, RenderItem,
    RenderScene, SourceExpression, SourceSpan, TopLevelBlock, TopLevelBlockKind,
};
use fcs_source::diagnostic::{Diagnostic, DiagnosticCode};
use fcs_source::parser::{
    ParseLimits, parse_document, parse_render_scene, parse_render_scene_with_limits,
};

fn parsed_document(source: &str) -> Document {
    parse_document(source)
        .into_result()
        .expect("the Core envelope is syntactically valid")
}

fn render_block(document: &Document) -> &RenderBlock {
    let Some(TopLevelBlock::Render(block)) = document.top_level(TopLevelBlockKind::Render) else {
        panic!("expected a Render block");
    };
    block
}

fn scene(source: &str) -> RenderScene {
    let document = parsed_document(source);
    parse_render_scene(source, render_block(&document))
        .into_result()
        .expect("the render payload matches the section 2 grammar")
}

fn scene_errors(source: &str) -> Vec<Diagnostic> {
    let document = parsed_document(source);
    parse_render_scene(source, render_block(&document))
        .into_result()
        .expect_err("the render payload must be rejected")
}

fn collect_body_kinds(items: &[RenderBodyItem], kinds: &mut Vec<CanonicalRenderNodeKind>) {
    for item in items {
        if let RenderBodyItem::Children(children) = item {
            collect_item_kinds(&children.items, kinds);
        }
    }
}

fn collect_item_kinds(items: &[RenderItem], kinds: &mut Vec<CanonicalRenderNodeKind>) {
    for item in items {
        match item {
            RenderItem::Node(node) => {
                kinds.push(node.kind);
                collect_body_kinds(&node.items, kinds);
            }
            RenderItem::If(branch) => {
                collect_item_kinds(&branch.then_items, kinds);
                collect_item_kinds(&branch.else_items, kinds);
            }
            RenderItem::Entity(_) | RenderItem::Generator(_) => {}
        }
    }
}

const COMPLETE_SCENE: &str = r#"#fcs 5.0.0
format { profile: renderable; }
render profile 1.0.0 {
    viewport {
        width: 1920px;
        height: 1080px;
        colorSpace: "linear-srgb";
    }
    layer background {
        pass: "back";
        zOrder: -100;
        tracks {
            track fade -> opacity: float {
                segments {
                    point 0beat: 0.0;
                }
            }
        }
        children {
            group pulse {
                opacity: 0.8;
                children {
                    clipGroup masked {
                        children {
                            rect panel { size: vec2(4px, 4px); }
                            roundedRect card { radius: 2px; }
                            circle ring { radius: 100px; }
                            ellipse halo { fill: solid(#FF4444FF); }
                            line guide { }
                            polyline trace { }
                            polygon shard { }
                            path outline { }
                            image portrait { }
                            text caption { }
                        }
                    }
                }
            }
            if flag {
                sparkle();
            } else {
                rect fallback { }
            }
            if flag {
                generate i: int in 0..<8 step 1 {
                    emit tickMark(i);
                }
            }
            generate j: int in 0..<4 step 1 {
                emit tickMark(j);
            }
        }
    }
    layer overlay {
        pass: "front";
        children {
            group content {
                tracks {
                    track slide -> position: vec2<length> {
                        segments {
                            point 0beat: vec2(0px, 0px);
                        }
                    }
                }
                children {
                    generate k: int in 0..<2 step 1 {
                        emit tickMark(k);
                    }
                }
            }
        }
    }
}
"#;

#[test]
fn complete_scene_retains_names_kinds_and_item_order() {
    let scene = scene(COMPLETE_SCENE);

    assert_eq!(
        scene
            .viewport
            .fields
            .iter()
            .map(|field| field.path.segments.join("."))
            .collect::<Vec<_>>(),
        ["width", "height", "colorSpace"]
    );
    assert_eq!(
        scene
            .layers
            .iter()
            .map(|layer| layer.name.as_str())
            .collect::<Vec<_>>(),
        ["background", "overlay"]
    );

    let background = &scene.layers[0];
    let [
        RenderBodyItem::Field(pass),
        RenderBodyItem::Field(z_order),
        RenderBodyItem::Tracks(tracks),
        RenderBodyItem::Children(children),
    ] = background.items.as_slice()
    else {
        panic!("background must keep its ordered body items");
    };
    assert_eq!(pass.path.segments, ["pass"]);
    assert_eq!(z_order.path.segments, ["zOrder"]);
    assert_eq!(tracks.tracks[0].name, "fade");

    let [
        RenderItem::Node(pulse),
        RenderItem::If(with_else),
        RenderItem::If(without_else),
        RenderItem::Generator(direct),
    ] = children.items.as_slice()
    else {
        panic!("background children must keep their ordered render items");
    };
    assert_eq!(pulse.name, "pulse");
    assert_eq!(pulse.kind, CanonicalRenderNodeKind::Group);

    let RenderItem::Entity(EntityExpression::Source(SourceExpression::Call { .. })) =
        &with_else.then_items[0]
    else {
        panic!("the first if arm must hold an entity expression item");
    };
    let RenderItem::Node(fallback) = &with_else.else_items[0] else {
        panic!("the else arm must hold a node declaration");
    };
    assert_eq!(fallback.name, "fallback");
    assert_eq!(fallback.kind, CanonicalRenderNodeKind::Rect);
    assert!(without_else.else_items.is_empty());
    let RenderItem::Generator(conditional) = &without_else.then_items[0] else {
        panic!("the second if arm must hold a generator");
    };

    // Generators in a children block are owned by the enclosing layer or node.
    assert_eq!(direct.variable, "j");
    assert_eq!(conditional.variable, "i");
    for generator in [direct, conditional] {
        assert_eq!(
            *generator.owner,
            GeneratorOwner::RenderChildren {
                name: "background".to_owned()
            }
        );
    }
    let RenderBodyItem::Children(overlay_children) = &scene.layers[1].items[1] else {
        panic!("overlay must keep its children block");
    };
    let RenderItem::Node(content) = &overlay_children.items[0] else {
        panic!("overlay children must hold the content node");
    };
    let RenderBodyItem::Children(content_children) = &content.items[1] else {
        panic!("content must keep its children block after its tracks block");
    };
    let RenderItem::Generator(nested) = &content_children.items[0] else {
        panic!("content children must hold a generator");
    };
    assert_eq!(nested.variable, "k");
    assert_eq!(
        *nested.owner,
        GeneratorOwner::RenderChildren {
            name: "content".to_owned()
        }
    );

    let mut kinds = Vec::new();
    collect_body_kinds(&background.items, &mut kinds);
    collect_body_kinds(&scene.layers[1].items, &mut kinds);
    assert_eq!(
        kinds,
        [
            CanonicalRenderNodeKind::Group,
            CanonicalRenderNodeKind::ClipGroup,
            CanonicalRenderNodeKind::Rect,
            CanonicalRenderNodeKind::RoundedRect,
            CanonicalRenderNodeKind::Circle,
            CanonicalRenderNodeKind::Ellipse,
            CanonicalRenderNodeKind::Line,
            CanonicalRenderNodeKind::Polyline,
            CanonicalRenderNodeKind::Polygon,
            CanonicalRenderNodeKind::Path,
            CanonicalRenderNodeKind::Image,
            CanonicalRenderNodeKind::Text,
            CanonicalRenderNodeKind::Rect,
            CanonicalRenderNodeKind::Group,
        ]
    );
}

#[test]
fn complete_scene_spans_are_exact_source_byte_spans() {
    let source = COMPLETE_SCENE;
    let document = parsed_document(source);
    let block = render_block(&document);
    let scene = parse_render_scene(source, block)
        .into_result()
        .expect("the render payload matches the section 2 grammar");

    assert_eq!(scene.span, block.payload.span);

    let viewport_start = source.find("viewport").expect("viewport keyword");
    assert_eq!(
        scene.viewport.keyword_span,
        SourceSpan::new(viewport_start, viewport_start + "viewport".len())
    );
    assert_eq!(scene.viewport.span.start, viewport_start);

    let background = &scene.layers[0];
    let name_start = source.find("layer background").expect("layer") + "layer ".len();
    assert_eq!(
        background.name_span,
        SourceSpan::new(name_start, name_start + "background".len())
    );

    let RenderBodyItem::Children(children) = &background.items[3] else {
        panic!("background children");
    };
    let children_start = source.find("children").expect("children keyword");
    assert_eq!(
        children.keyword_span,
        SourceSpan::new(children_start, children_start + "children".len())
    );

    let RenderItem::Node(pulse) = &children.items[0] else {
        panic!("pulse node");
    };
    let RenderBodyItem::Children(pulse_children) = &pulse.items[1] else {
        panic!("pulse children");
    };
    let RenderItem::Node(masked) = &pulse_children.items[0] else {
        panic!("masked node");
    };
    let RenderBodyItem::Children(masked_children) = &masked.items[0] else {
        panic!("masked children");
    };
    let RenderItem::Node(panel) = &masked_children.items[0] else {
        panic!("panel node");
    };
    let rect_start = source.find("rect panel").expect("rect panel");
    assert_eq!(
        panel.kind_span,
        SourceSpan::new(rect_start, rect_start + "rect".len())
    );
    let panel_start = source.find("panel").expect("panel name");
    assert_eq!(
        panel.name_span,
        SourceSpan::new(panel_start, panel_start + "panel".len())
    );
    assert_eq!(panel.span.start, rect_start);
    assert_eq!(
        &source[panel.span.start..panel.span.end],
        "rect panel { size: vec2(4px, 4px); }"
    );

    let RenderItem::If(with_else) = &children.items[1] else {
        panic!("first if");
    };
    let if_start = source.find("if flag").expect("if keyword");
    assert_eq!(
        with_else.keyword_span,
        SourceSpan::new(if_start, if_start + "if".len())
    );
    let RenderItem::Entity(entity) = &with_else.then_items[0] else {
        panic!("entity item");
    };
    let entity_start = source.find("sparkle()").expect("entity expression");
    assert_eq!(
        entity.span(),
        SourceSpan::new(entity_start, entity_start + "sparkle()".len())
    );
}

#[test]
fn unknown_and_miscased_node_kinds_report_the_kind_token() {
    for (source, spelling) in [
        (
            r#"#fcs 5.0.0
format { profile: renderable; }
render profile 1.0.0 {
    viewport { width: 4px; height: 4px; }
    layer main {
        children {
            blob mystery { }
        }
    }
}
"#,
            "blob",
        ),
        (
            r#"#fcs 5.0.0
format { profile: renderable; }
render profile 1.0.0 {
    viewport { width: 4px; height: 4px; }
    layer main {
        children {
            Rect box { }
        }
    }
}
"#,
            "Rect",
        ),
    ] {
        let errors = scene_errors(source);
        assert_eq!(errors.len(), 1, "{spelling}");
        assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
        assert_eq!(errors[0].message(), "unknown render node kind");
        let start = source.find(spelling).expect("kind token");
        assert_eq!(
            errors[0].primary_span(),
            SourceSpan::new(start, start + spelling.len()),
            "{spelling}"
        );
    }
}

#[test]
fn missing_viewport_reports_the_first_item_or_the_close_brace() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
render profile 1.0.0 {
    layer main { pass: "front"; }
}
"#;
    let errors = scene_errors(source);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
    let layer = r#"layer main { pass: "front"; }"#;
    let start = source.find(layer).expect("layer block");
    assert_eq!(
        errors[0].primary_span(),
        SourceSpan::new(start, start + layer.len())
    );

    let empty = "#fcs 5.0.0\nformat { profile: renderable; }\nrender profile 1.0.0 { }";
    let errors = scene_errors(empty);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
    let close = empty.rfind('}').expect("payload close brace");
    assert_eq!(errors[0].primary_span(), SourceSpan::new(close, close + 1));
}

#[test]
fn duplicate_viewport_reports_the_second_keyword_with_the_first_as_label() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
render profile 1.0.0 {
    viewport { width: 4px; height: 4px; }
    viewport { width: 8px; height: 8px; }
    layer main { }
}
"#;
    let errors = scene_errors(source);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code(), DiagnosticCode::NAME_DUPLICATE);
    let first = source.find("viewport").expect("first viewport");
    let second = source.rfind("viewport").expect("second viewport");
    assert_eq!(
        errors[0].primary_span(),
        SourceSpan::new(second, second + "viewport".len())
    );
    assert_eq!(
        errors[0].labels()[0].span(),
        SourceSpan::new(first, first + "viewport".len())
    );
}

#[test]
fn viewport_after_a_layer_is_a_misplaced_block() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
render profile 1.0.0 {
    layer main { }
    viewport { width: 4px; height: 4px; }
}
"#;
    let errors = scene_errors(source);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_MISPLACED_BLOCK);
    let start = source.find("viewport").expect("viewport keyword");
    assert_eq!(
        errors[0].primary_span(),
        SourceSpan::new(start, start + "viewport".len())
    );
}

#[test]
fn generators_outside_children_blocks_are_misplaced() {
    for source in [
        r#"#fcs 5.0.0
format { profile: renderable; }
render profile 1.0.0 {
    viewport { }
    layer main {
        generate i: int in 0..<1 step 1 { emit tickMark(i); }
    }
}
"#,
        r#"#fcs 5.0.0
format { profile: renderable; }
render profile 1.0.0 {
    viewport { }
    generate i: int in 0..<1 step 1 { emit tickMark(i); }
}
"#,
    ] {
        let errors = scene_errors(source);
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].code(),
            DiagnosticCode::COMPILE_TIME_MISPLACED_GENERATOR
        );
        let start = source.find("generate").expect("generate keyword");
        assert_eq!(
            errors[0].primary_span(),
            SourceSpan::new(start, start + "generate".len())
        );
    }
}

#[test]
fn nested_generators_inside_children_are_rejected() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
render profile 1.0.0 {
    viewport { }
    layer main {
        children {
            generate i: int in 0..<1 step 1 {
                generate j: int in 0..<1 step 1 { emit tickMark(j); }
            }
        }
    }
}
"#;
    let errors = scene_errors(source);
    assert_eq!(
        errors[0].code(),
        DiagnosticCode::COMPILE_TIME_NESTED_GENERATOR
    );
    let start = source.rfind("generate").expect("inner generate keyword");
    assert_eq!(
        errors[0].primary_span(),
        SourceSpan::new(start, start + "generate".len())
    );
}

#[test]
fn malformed_and_misplaced_payload_structure_reports_the_offending_token() {
    // A layer without a name fails at its opening brace; a schema field at
    // profile level fails at its path token.
    let nameless = r#"#fcs 5.0.0
format { profile: renderable; }
render profile 1.0.0 {
    viewport { }
    layer { }
}
"#;
    let errors = scene_errors(nameless);
    assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
    let start = nameless.find("layer { }").expect("nameless layer") + "layer ".len();
    assert_eq!(errors[0].primary_span(), SourceSpan::new(start, start + 1));

    let misplaced_field = r#"#fcs 5.0.0
format { profile: renderable; }
render profile 1.0.0 {
    viewport { }
    pass: "front";
}
"#;
    let errors = scene_errors(misplaced_field);
    assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
    let start = misplaced_field.find("pass").expect("misplaced field");
    assert_eq!(
        errors[0].primary_span(),
        SourceSpan::new(start, start + "pass".len())
    );
}

#[test]
fn unbalanced_render_payloads_fail_at_the_core_boundary() {
    let source = "#fcs 5.0.0\nformat { profile: renderable; }\nrender profile 1.0.0 { layer x {";
    let errors = parse_document(source)
        .into_result()
        .expect_err("the Core parser owns payload balance");
    assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
}

#[test]
fn payload_nesting_reuses_the_core_depth_budget() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
render profile 1.0.0 {
    viewport { }
    layer main {
        children {
            group outer {
                children {
                    rect inner { }
                }
            }
        }
    }
}
"#;
    let document = parsed_document(source);
    let block = render_block(&document);
    assert!(
        parse_render_scene(source, block).into_result().is_ok(),
        "the payload parses under the default budget"
    );
    let errors = parse_render_scene_with_limits(
        source,
        block,
        ParseLimits {
            max_nesting_depth: 3,
            ..ParseLimits::default()
        },
    )
    .into_result()
    .expect_err("payload nesting is bounded");
    assert_eq!(errors[0].code(), DiagnosticCode::RESOURCE_LIMIT_EXCEEDED);
    let budget = errors[0].budget().expect("budget details");
    assert_eq!(budget.kind(), "max_nesting_depth");
    // The rebased diagnostic points at the fourth opening brace of the payload.
    let start = source.find("group outer {").expect("outer group") + "group outer ".len();
    assert_eq!(errors[0].primary_span(), SourceSpan::new(start, start + 1));
}

#[test]
fn render_source_conformance_fixtures_bind_the_render_aware_boundary() {
    let valid = include_str!("../../../docs/conformance/render/solid-rect-4x4.fcs");
    let document = parsed_document(valid);
    let scene = parse_render_scene(valid, render_block(&document))
        .into_result()
        .expect("the solid-rect fixture matches the section 2 grammar");
    assert_eq!(scene.layers[0].name, "main");
    let RenderBodyItem::Children(children) = scene.layers[0]
        .items
        .iter()
        .find(|item| matches!(item, RenderBodyItem::Children(_)))
        .expect("children block")
    else {
        unreachable!()
    };
    let RenderItem::Node(full) = &children.items[0] else {
        panic!("rect node");
    };
    assert_eq!(full.kind, CanonicalRenderNodeKind::Rect);
    assert_eq!(full.name, "full");

    let missing = include_str!("../../../docs/conformance/render/invalid-missing-viewport.fcs");
    let errors = scene_errors(missing);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);

    let unknown = include_str!("../../../docs/conformance/render/invalid-unknown-node-kind.fcs");
    let errors = scene_errors(unknown);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
    assert_eq!(errors[0].message(), "unknown render node kind");
    let start = unknown.find("unknown mystery").expect("unknown kind token");
    assert_eq!(
        errors[0].primary_span(),
        SourceSpan::new(start, start + "unknown".len())
    );
}
