use fcs_source::ast::Color;
use fcs_source::ast::{
    Beat, BinaryOperator, CollectionBlock, CollectionItem, Definition, EntityConstructor,
    EntityExpression, ExpandedEntity, ExpandedField, FunctionStatement, NoteVariant,
    SourceExpression, SourceLiteral, SourceSpan, Type, TypedExpression, TypedExpressionKind,
    TypedValue, UnaryOperator, WithExpression,
};
use fcs_source::diagnostic::{Diagnostic, DiagnosticCode};
use fcs_source::elaborator::{CompileTimeLimits, elaborate};
use fcs_source::parser::{
    ParseLimits, parse_document, parse_expression, parse_expression_with_limits, parse_type,
    parse_type_with_limits,
};
use fcs_source::schema::{FieldConstraint, phase2_schema};
use std::{fs, path::PathBuf};

fn example(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/fcs")
        .join(name);
    fs::read_to_string(path).unwrap()
}

fn elaborate_source(source: &str) -> Result<(), Vec<Diagnostic>> {
    let document = parse_document(source)
        .into_result()
        .expect("valid source syntax");
    elaborate(&document, phase2_schema(), CompileTimeLimits::default()).map(|_| ())
}

fn assert_code(result: Result<(), Vec<Diagnostic>>, expected: DiagnosticCode) {
    let errors = result.expect_err("source should produce a diagnostic");
    assert_eq!(errors[0].code(), expected);
}

#[test]
fn parses_definitions_with_global_byte_spans() {
    let source = "#fcs 5.0.0\nformat { profile: fragment; }\ndefinitions {\n  const SPACING: length = 120px;\n  fn choose(value: length) -> length {\n    if true { let local: length = value; return local; } else { return value; }\n  }\n}";
    let document = parse_document(source).into_result().unwrap();
    let definitions = document.definitions.as_ref().unwrap();
    assert_eq!(
        definitions.span,
        SourceSpan::new(source.find("definitions").unwrap(), source.len())
    );
    assert_eq!(definitions.declarations.len(), 2);

    let Definition::Const(declaration) = &definitions.declarations[0] else {
        panic!("expected const declaration");
    };
    assert_eq!(
        &source[declaration.span.start..declaration.span.end],
        "const SPACING: length = 120px;"
    );
    assert_eq!(
        declaration.initializer.span(),
        SourceSpan::new(
            source.find("120px").unwrap(),
            source.find("120px").unwrap() + 5
        )
    );

    let Definition::Function(function) = &definitions.declarations[1] else {
        panic!("expected function declaration");
    };
    assert_eq!(function.parameters[0].ty, Type::Length);
    assert_eq!(function.return_type, Type::Length);
    assert!(matches!(function.body[0], FunctionStatement::If(_)));
    let FunctionStatement::If(statement) = &function.body[0] else {
        unreachable!()
    };
    assert!(matches!(
        statement.then_branch[0],
        FunctionStatement::Let(_)
    ));
    assert!(matches!(
        statement.then_branch[1],
        FunctionStatement::Return(_)
    ));
    assert!(matches!(
        statement.else_branch[0],
        FunctionStatement::Return(_)
    ));
    assert_eq!(
        &source[function.span.start..function.span.end],
        "fn choose(value: length) -> length {\n    if true { let local: length = value; return local; } else { return value; }\n  }"
    );
}

#[test]
fn definitions_preserve_expression_comparisons_and_comment_delimiters() {
    let source = r#"#fcs 5.0.0
format { profile: fragment; }
definitions {
  const sum: int = 1 /* ; is comment text */ + 2;
  fn smaller(value: int) -> int {
    if value < sum { return value; } else { return sum; }
  }
}"#;
    let document = parse_document(source).into_result().unwrap();
    assert!(elaborate(&document, phase2_schema(), CompileTimeLimits::default()).is_ok());
}

#[test]
fn elaborates_const_and_pure_function() {
    let source = r#"#fcs 5.0.0
format { profile: fragment; }
definitions {
  const SPACING: length = 120px;
  fn twice(value: length) -> length { return value * 2; }
}"#;
    let document = parse_document(source).into_result().unwrap();
    let expanded = elaborate(&document, phase2_schema(), CompileTimeLimits::default()).unwrap();
    assert_eq!(expanded.source_version(), document.source_version);
    assert_eq!(expanded.profile(), document.profile);
    assert!(expanded.tempo_map().is_none());
    assert_eq!(expanded.collections().count(), 0);
}

#[test]
fn rejects_shadowing_in_nested_scope() {
    let source = r#"#fcs 5.0.0
format { profile: fragment; }
definitions { fn f(value: int) -> int { let value: int = 1; return value; } }"#;
    assert_code(elaborate_source(source), DiagnosticCode::NAME_DUPLICATE);
}

#[test]
fn rejects_duplicate_bindings_but_allows_sibling_branch_names() {
    let duplicate = r#"#fcs 5.0.0
format { profile: fragment; }
definitions { const value: int = 1; const value: int = 2; }"#;
    assert_code(elaborate_source(duplicate), DiagnosticCode::NAME_DUPLICATE);

    let siblings = r#"#fcs 5.0.0
format { profile: fragment; }
definitions {
  fn f(flag: bool) -> int {
    if flag { let value: int = 1; return value; }
    else { let value: int = 2; return value; }
  }
}"#;
    assert!(elaborate_source(siblings).is_ok());
}

#[test]
fn local_bindings_cannot_shadow_global_or_builtin_functions() {
    let global = r#"#fcs 5.0.0
format { profile: fragment; }
definitions {
  fn helper(value: int) -> int { return value; }
  fn f(helper: int) -> int { return helper; }
}"#;
    assert_code(elaborate_source(global), DiagnosticCode::NAME_SHADOWED);

    let builtin = r#"#fcs 5.0.0
format { profile: fragment; }
definitions { fn f(sin: float) -> float { return sin; } }"#;
    assert_code(elaborate_source(builtin), DiagnosticCode::NAME_SHADOWED);
}

#[test]
fn requires_exact_declared_and_return_types() {
    let initializer = r#"#fcs 5.0.0
format { profile: fragment; }
definitions { const value: float = 1; }"#;
    assert_code(elaborate_source(initializer), DiagnosticCode::TYPE_MISMATCH);

    let returned = r#"#fcs 5.0.0
format { profile: fragment; }
definitions { fn f() -> float { return 1; } }"#;
    assert_code(elaborate_source(returned), DiagnosticCode::TYPE_MISMATCH);
}

#[test]
fn rejects_unknown_and_runtime_only_names() {
    for name in ["missing", "s", "b", "q", "d", "p"] {
        let source = format!(
            "#fcs 5.0.0\nformat {{ profile: fragment; }}\ndefinitions {{ const value: float = {name}; }}"
        );
        let errors = elaborate_source(&source).expect_err("unknown name");
        assert_eq!(errors[0].code(), DiagnosticCode::NAME_UNKNOWN);
        assert!(errors[0].message().contains(name));
    }
}

#[test]
fn types_and_evaluates_phase2_pure_operators() {
    let source = r#"#fcs 5.0.0
format { profile: fragment; }
definitions {
  const i: int = (1 + 2) * 3 % 4;
  const f: float = 4.0 / 2.0;
  const len: length = 10px + 2px * 3;
  const scaled: length = 2 * 5px;
  const ratio: float = 10px / 2px;
  const logic: bool = true && !false;
  const equality: bool = "a" != "b";
  const vector: vec2<length> = vec2(1px, 2px);
  const component: length = vector.x;
  const vector_equal: bool = vector == vec2(1px, 2px);
}"#;
    assert!(elaborate_source(source).is_ok());

    let mixed = r#"#fcs 5.0.0
format { profile: fragment; }
definitions { const value: float = 1 + 2.0; }"#;
    let mixed_errors = elaborate_source(mixed).expect_err("mixed types");
    assert!(matches!(
        mixed_errors[0].code(),
        DiagnosticCode::TYPE_MISMATCH | DiagnosticCode::TYPE_INVALID_OPERATION
    ));
}

#[test]
fn evaluates_fixed_builtins_and_diagnoses_bad_calls() {
    let valid = r#"#fcs 5.0.0
format { profile: fragment; }
definitions {
  const circle: float = pi;
  const wave: float = sin(pi) + cos(0.0);
  const converted: float = toFloat(2);
  const close: bool = approxEq(wave, 1.0, 0.001);
}"#;
    assert!(elaborate_source(valid).is_ok());

    let arity = r#"#fcs 5.0.0
format { profile: fragment; }
definitions { const value: float = sin(); }"#;
    let arity_errors = elaborate_source(arity).expect_err("wrong arity");
    assert_eq!(arity_errors[0].code(), DiagnosticCode::TYPE_MISMATCH);
    assert!(arity_errors[0].message().contains("sin"));

    let argument_type = r#"#fcs 5.0.0
format { profile: fragment; }
definitions { const value: float = sin(1); }"#;
    assert_code(
        elaborate_source(argument_type),
        DiagnosticCode::TYPE_MISMATCH,
    );
}

#[test]
fn supports_forward_const_and_pure_function_references() {
    let source = r#"#fcs 5.0.0
format { profile: fragment; }
definitions {
  const result: int = select_flag(true);
  fn select_flag(flag: bool) -> int {
    let doubled: int = twice(BASE);
    if flag { return doubled; } else { return BASE; }
  }
  fn twice(value: int) -> int { return value * 2; }
  const BASE: int = 3;
}"#;
    assert!(elaborate_source(source).is_ok());
}

#[test]
fn requires_a_return_on_every_function_path() {
    let source = r#"#fcs 5.0.0
format { profile: fragment; }
definitions { fn f(flag: bool) -> int { if flag { return 1; } } }"#;
    let errors = elaborate_source(source).expect_err("missing return");
    assert_eq!(errors[0].code(), DiagnosticCode::TYPE_MISMATCH);
    assert!(errors[0].message().contains("f"));
}

#[test]
fn detects_const_and_function_cycles_before_evaluation() {
    let const_cycle = r#"#fcs 5.0.0
format { profile: fragment; }
definitions { const a: int = b; const b: int = a; }"#;
    let errors = elaborate_source(const_cycle).expect_err("constant cycle");
    assert_eq!(errors[0].code(), DiagnosticCode::NAME_CYCLE);
    assert_eq!(
        errors[0]
            .expansion_trace()
            .iter()
            .map(|frame| frame.subject().unwrap())
            .collect::<Vec<_>>(),
        ["a", "b", "a"]
    );

    let function_cycle = r#"#fcs 5.0.0
format { profile: fragment; }
definitions {
  fn a(value: int) -> int { return b(value); }
  fn b(value: int) -> int { return a(value); }
}"#;
    let errors = elaborate_source(function_cycle).expect_err("function cycle");
    assert_eq!(errors[0].code(), DiagnosticCode::NAME_CYCLE);
    assert_eq!(
        errors[0]
            .expansion_trace()
            .iter()
            .map(|frame| frame.subject().unwrap())
            .collect::<Vec<_>>(),
        ["a", "b", "a"]
    );
}

#[test]
fn collection_blocks_retain_forward_compatible_items_and_spans() {
    let constructor_span = SourceSpan::new(12, 28);
    let block_span = SourceSpan::new(0, 30);
    let block = CollectionBlock {
        collection_name: "main".into(),
        items: vec![CollectionItem::Constructor(EntityConstructor {
            entity_type: Type::Note,
            note_variant: Some(NoteVariant::Tap),
            fields: Vec::new(),
            span: constructor_span,
        })],
        span: block_span,
    };

    assert_eq!(block.span, block_span);
    assert_eq!(block.items[0].span(), constructor_span);
    match &block.items[0] {
        CollectionItem::Constructor(constructor) => {
            assert_eq!(constructor.span, constructor_span);
        }
        _ => panic!("expected constructor collection item"),
    }
}

#[test]
fn expanded_ir_exposes_only_read_accessors() {
    fn assert_accessor_api(field: &ExpandedField, entity: &ExpandedEntity) {
        let _: &str = field.path();
        let _: &TypedValue = field.value();
        let _: SourceSpan = field.span();
        let _: &Type = entity.entity_type();
        let _: Option<NoteVariant> = entity.variant();
        let _: SourceSpan = entity.span();
        let _: Option<&ExpandedField> = entity.field("gameplay.time");
        let _: usize = entity.fields().count();
        assert!(entity.is_lowered());
    }

    let _ = assert_accessor_api as fn(&ExpandedField, &ExpandedEntity);
}

#[test]
fn entity_expressions_compose_constructor_with_nested_with_blocks() {
    let constructor_span = SourceSpan::new(0, 8);
    let first_with_span = SourceSpan::new(0, 20);
    let second_with_span = SourceSpan::new(0, 32);
    let constructor = EntityExpression::Constructor(EntityConstructor {
        entity_type: Type::Note,
        note_variant: Some(NoteVariant::Tap),
        fields: Vec::new(),
        span: constructor_span,
    });
    let first = EntityExpression::With(WithExpression {
        base: Box::new(constructor),
        fields: Vec::new(),
        span: first_with_span,
    });
    let second = EntityExpression::With(WithExpression {
        base: Box::new(first),
        fields: Vec::new(),
        span: second_with_span,
    });

    assert_eq!(second.span(), second_with_span);
    let EntityExpression::With(second_with) = second else {
        panic!("expected outer with expression");
    };
    assert_eq!(second_with.base.span(), first_with_span);
    let EntityExpression::With(first_with) = *second_with.base else {
        panic!("expected inner with expression");
    };
    assert_eq!(first_with.base.span(), constructor_span);
}

#[test]
fn entity_expressions_compose_template_calls_with_with_blocks() {
    let call_span = SourceSpan::new(4, 18);
    let with_span = SourceSpan::new(4, 30);
    let call = SourceExpression::Call {
        callee: Box::new(SourceExpression::Name {
            name: "ghostTap".into(),
            span: SourceSpan::new(4, 12),
        }),
        arguments: Vec::new(),
        span: call_span,
    };
    let expression = EntityExpression::With(WithExpression {
        base: Box::new(EntityExpression::Source(call)),
        fields: Vec::new(),
        span: with_span,
    });

    assert_eq!(expression.span(), with_span);
    let EntityExpression::With(with_expression) = expression else {
        panic!("expected with expression");
    };
    assert_eq!(with_expression.base.span(), call_span);
    assert!(matches!(
        *with_expression.base,
        EntityExpression::Source(SourceExpression::Call { .. })
    ));
}

#[test]
fn parses_typed_templates_and_collections_with_source_spans() {
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 180bpm; }
templates {
  template ghost(at: beat) -> Note {
    return tap { gameplay.time: at; };
  }
}
collections {
  notes {
    ghost(1beat) with { presentation.positionX: 12px; };
  }
}"#;
    let document = parse_document(source)
        .into_result()
        .expect("template source should parse");
    let templates = document.templates.as_ref().expect("templates block");
    assert_eq!(templates.declarations.len(), 1);
    assert_eq!(templates.declarations[0].name, "ghost");
    assert_eq!(templates.declarations[0].parameters[0].ty, Type::Beat);
    assert_eq!(templates.declarations[0].return_type, Type::Note);
    assert!(matches!(
        templates.declarations[0].body,
        EntityExpression::Constructor(_)
    ));
    assert_eq!(document.collections.len(), 1);
    assert_eq!(document.collections[0].collection_name, "notes");
    assert!(matches!(
        document.collections[0].items[0],
        CollectionItem::Expression(EntityExpression::With(_))
    ));
}

fn generator_source(operator: &str, step: &str) -> String {
    format!(
        "#fcs 5.0.0\n\
         format {{ profile: fragment; }}\n\
         collections {{\n\
           notes {{\n\
             generate at: beat in 0beat{operator}4beat step {step} {{\n\
               emit tap {{ gameplay.time: at; }};\n\
             }}\n\
           }}\n\
         }}"
    )
}

#[test]
fn parses_only_frozen_generator_range_operators() {
    for (operator, inclusive_end) in [("..<", false), ("..=", true)] {
        let source = generator_source(operator, "1beat");
        let document = parse_document(&source)
            .into_result()
            .expect("Frozen generator range should parse");
        let CollectionItem::Generator(generator) = &document.collections[0].items[0] else {
            panic!("expected generator collection item");
        };
        assert_eq!(generator.variable, "at");
        assert_eq!(generator.variable_type, Type::Beat);
        assert_eq!(generator.range.inclusive_end, inclusive_end);
        assert_eq!(generator.body.len(), 1);
        assert_eq!(generator.span.start, source.find("generate").unwrap());
        assert!(generator.span.end > generator.span.start);
        assert!(generator.span.end <= source.len());
    }
}

#[test]
fn rejects_bare_generator_range_operator() {
    let source = generator_source("..", "1beat");
    assert_eq!(
        parse_document(&source)
            .into_result()
            .expect_err("bare generator range")[0]
            .code(),
        DiagnosticCode::SYNTAX_INVALID_TOKEN
    );
}

#[test]
fn retains_zero_generator_step_for_later_static_semantics() {
    let source = generator_source("..<", "0beat");
    let document = parse_document(&source)
        .into_result()
        .expect("zero step is syntactically valid");
    let CollectionItem::Generator(generator) = &document.collections[0].items[0] else {
        panic!("expected generator collection item");
    };
    assert!(matches!(
        generator.range.step,
        SourceExpression::Literal {
            literal: SourceLiteral::Beat(value),
            ..
        } if value == Beat::new(0, 1).unwrap()
    ));
}

#[test]
fn generator_elaboration_fails_before_partial_output() {
    let source = "#fcs 5.0.0\n\
         format { profile: fragment; }\n\
         collections {\n\
           notes {\n\
             tap { gameplay.time: 0beat; };\n\
             generate at: beat in 1beat..<3beat step 1beat {\n\
               emit tap { gameplay.time: at; };\n\
             }\n\
           }\n\
         }"
    .to_string();
    let document = parse_document(&source)
        .into_result()
        .expect("generator source should parse");
    let errors = elaborate(&document, phase2_schema(), CompileTimeLimits::default())
        .expect_err("I0 must not expand generators");
    assert_eq!(
        errors[0].code(),
        DiagnosticCode::IMPLEMENTATION_FEATURE_UNAVAILABLE
    );
    assert_eq!(
        errors[0].stage(),
        fcs_source::diagnostic::DiagnosticStage::Implementation
    );
    assert_eq!(
        errors[0].primary_span().start,
        source.find("generate").unwrap()
    );
}

#[test]
fn elaborates_templates_and_with_overrides_into_concrete_entities() {
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 180bpm; }
definitions { const X: length = 12px; }
templates {
  template ghost(at: beat, x: length) -> Note {
    return tap {
      gameplay.time: at;
      presentation.positionX: x;
    };
  }
}
collections {
  notes { ghost(1beat, X) with { presentation.alpha: 0.5; }; }
}"#;
    let document = parse_document(source).into_result().unwrap();
    let expanded = elaborate(&document, phase2_schema(), CompileTimeLimits::default()).unwrap();
    let collection = expanded.collections().next().expect("expanded collection");
    let entity = collection.entities().next().expect("expanded entity");
    assert_eq!(collection.name(), "notes");
    assert_eq!(entity.entity_type(), &Type::Note);
    assert_eq!(entity.variant(), Some(NoteVariant::Tap));
    assert_eq!(
        entity.field("gameplay.time").unwrap().value(),
        &TypedValue::Beat(Beat::new(1, 1).unwrap())
    );
    assert_eq!(
        entity.field("presentation.positionX").unwrap().value(),
        &TypedValue::Length(12.0)
    );
    assert_eq!(
        entity.field("presentation.alpha").unwrap().value(),
        &TypedValue::Float(0.5)
    );
}

#[test]
fn entity_elaboration_reports_schema_and_template_errors() {
    let unknown_field = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 180bpm; }
collections { notes { tap { gameplay.time: 1beat; presentation.unknown: 1; }; } }"#;
    let errors = elaborate_source(unknown_field).expect_err("unknown entity field");
    assert_eq!(errors[0].code(), DiagnosticCode::SCHEMA_UNKNOWN_FIELD);
    assert!(errors[0].message().contains("presentation.unknown"));

    let missing_required = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 180bpm; }
collections { notes { tap { presentation.alpha: 1.0; }; } }"#;
    let errors = elaborate_source(missing_required).expect_err("missing required field");
    assert_eq!(
        errors[0].code(),
        DiagnosticCode::SCHEMA_MISSING_REQUIRED_FIELD
    );
    assert!(errors[0].message().contains("gameplay.time"));

    let recursive = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 180bpm; }
templates {
  template a() -> Note { return b(); }
  template b() -> Note { return a(); }
}
collections { notes { a(); } }"#;
    let errors = elaborate_source(recursive).expect_err("recursive template");
    assert_eq!(errors[0].code(), DiagnosticCode::NAME_CYCLE);
    assert_eq!(
        errors[0]
            .expansion_trace()
            .iter()
            .map(|frame| frame.subject().unwrap())
            .collect::<Vec<_>>(),
        ["a", "b", "a"]
    );
}

#[test]
fn compile_time_collection_if_selects_one_branch_and_rejects_runtime_conditions() {
    let selected = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 180bpm; }
collections {
  notes {
    if true { tap { gameplay.time: 1beat; }; }
    else { tap { gameplay.time: 2beat; }; }
  }
}"#;
    let document = parse_document(selected).into_result().unwrap();
    let expanded = elaborate(&document, phase2_schema(), CompileTimeLimits::default()).unwrap();
    let entity = expanded
        .collections()
        .next()
        .unwrap()
        .entities()
        .next()
        .unwrap();
    assert_eq!(
        entity.field("gameplay.time").unwrap().value(),
        &TypedValue::Beat(Beat::new(1, 1).unwrap())
    );

    let runtime = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 180bpm; }
collections { notes { if missing { tap { gameplay.time: 1beat; }; } } }"#;
    assert_code(
        elaborate_source(runtime),
        DiagnosticCode::COMPILE_TIME_NON_CONSTANT_CONDITION,
    );
}

#[test]
fn parses_and_elaborates_the_public_template_fixture() {
    let document = parse_document(&example("templates.fcs"))
        .into_result()
        .unwrap();
    let expanded = elaborate(&document, phase2_schema(), CompileTimeLimits::default()).unwrap();
    let collection = expanded.collections().next().unwrap();
    assert_eq!(collection.name(), "notes");
    assert_eq!(collection.entities().count(), 1);
    assert_eq!(
        collection
            .entities()
            .next()
            .unwrap()
            .field("presentation.alpha")
            .unwrap()
            .value(),
        &TypedValue::Float(0.5)
    );
}

#[test]
fn phase2_schema_requires_note_time_and_types_position() {
    let schema = phase2_schema();
    let note = schema.entity(&Type::Note).unwrap();
    assert_eq!(note.field("gameplay.time").unwrap().ty, Type::Beat);
    assert_eq!(
        note.field("presentation.positionX").unwrap().ty,
        Type::Length
    );
}

#[test]
fn phase2_schema_exposes_only_gameplay_side_as_a_closed_string_enum() {
    let note = phase2_schema().entity(&Type::Note).unwrap();
    let side = note.field("gameplay.side").unwrap();

    assert_eq!(side.ty, Type::String);
    assert_eq!(
        side.constraint(),
        Some(&FieldConstraint::StringEnum(&["above", "below"]))
    );
    assert_eq!(note.field("gameplay.time").unwrap().constraint(), None);
    assert_eq!(
        note.field("presentation.texture").unwrap().constraint(),
        None
    );
}

#[test]
#[allow(clippy::redundant_pattern_matching)]
fn render_node_is_not_constructible_in_phase2() {
    assert!(matches!(phase2_schema().entity(&Type::RenderNode), None));
}

#[test]
fn phase2_note_schema_has_exact_fields_required_flags_and_variants() {
    let note = phase2_schema().entity(&Type::Note).unwrap();
    let note_variants = [
        NoteVariant::Tap,
        NoteVariant::Hold,
        NoteVariant::Flick,
        NoteVariant::Drag,
    ];
    let fields: Vec<_> = note
        .fields()
        .map(|field| (field.path.as_str(), field.ty.clone(), field.required))
        .collect();

    assert_eq!(
        fields,
        vec![
            ("gameplay.endTime", Type::Beat, false),
            ("gameplay.judgment.enabled", Type::Bool, false),
            ("gameplay.side", Type::String, false),
            ("gameplay.time", Type::Beat, true),
            ("presentation.alpha", Type::Float, false),
            ("presentation.color", Type::Color, false),
            ("presentation.positionX", Type::Length, false),
            ("presentation.scaleX", Type::Float, false),
            ("presentation.scaleY", Type::Float, false),
            ("presentation.scrollFactor", Type::Float, false),
            ("presentation.texture", Type::String, false),
            ("presentation.visibleFrom", Type::Beat, false),
            ("presentation.visibleUntil", Type::Beat, false),
            ("presentation.xOffset", Type::Length, false),
            ("presentation.yOffset", Type::Length, false),
            ("render.enabled", Type::Bool, false),
        ]
    );
    assert_eq!(note.note_variants(), Some(note_variants.as_slice()));
}

#[test]
fn phase2_line_schema_has_only_identity_fields() {
    let line = phase2_schema().entity(&Type::Line).unwrap();
    let fields: Vec<_> = line
        .fields()
        .map(|field| (field.path.as_str(), field.ty.clone(), field.required))
        .collect();

    assert_eq!(
        fields,
        vec![("id", Type::String, true), ("zOrder", Type::Int, false),]
    );
    assert_eq!(line.note_variants(), None);
}

#[test]
fn phase2_collections_emit_their_registered_entity_types_deterministically() {
    let schema = phase2_schema();
    let collections: Vec<_> = schema
        .collections()
        .map(|collection| {
            (
                collection.collection_name.as_str(),
                collection.emitted_entity_type.clone(),
            )
        })
        .collect();

    assert_eq!(
        collections,
        vec![("judgelines", Type::Line), ("notes", Type::Note)]
    );
    assert_eq!(
        schema.collection("notes").unwrap().emitted_entity_type,
        Type::Note
    );
    assert_eq!(
        schema.collection("judgelines").unwrap().emitted_entity_type,
        Type::Line
    );
    assert!(schema.collection("renderNodes").is_none());
}

#[test]
fn phase2_registers_exactly_note_and_line_as_constructible_entities() {
    let schema = phase2_schema();
    let entity_types: Vec<_> = schema
        .entities()
        .map(|entity| entity.entity_type.clone())
        .collect();

    assert_eq!(entity_types, vec![Type::Note, Type::Line]);
    assert!(schema.entity(&Type::RenderNode).is_none());
    assert!(
        schema
            .entity(&Type::TrackSegment(Box::new(Type::Beat)))
            .is_none()
    );
    assert!(
        schema
            .entity(&Type::Keyframe(Box::new(Type::Float)))
            .is_none()
    );
}

#[test]
fn phase2_types_keep_units_distinct() {
    assert_ne!(Type::Beat, Type::Time);
    assert_eq!(SourceSpan::new(3, 7).len(), 4);
    assert_eq!(TypedValue::Int(4).ty(), Type::Int);
}

#[test]
fn source_spans_are_half_open_and_allow_empty_ranges() {
    const SPAN: SourceSpan = SourceSpan::new(3, 7);
    let span = SPAN;
    assert_eq!(span.start, 3);
    assert_eq!(span.end, 7);
    assert_eq!(span.len(), 4);
    assert!(!span.is_empty());

    let empty = SourceSpan::new(5, 5);
    assert_eq!(empty.len(), 0);
    assert!(empty.is_empty());
}

#[test]
fn phase2_type_display_uses_canonical_spellings() {
    let cases = [
        (Type::Bool, "bool"),
        (Type::Int, "int"),
        (Type::Float, "float"),
        (Type::String, "string"),
        (Type::Time, "time"),
        (Type::Beat, "beat"),
        (Type::Length, "length"),
        (Type::Angle, "angle"),
        (Type::Color, "color"),
        (Type::Vec2(Box::new(Type::Length)), "vec2<length>"),
        (Type::Note, "Note"),
        (Type::Line, "Line"),
        (Type::RenderNode, "RenderNode"),
        (
            Type::TrackSegment(Box::new(Type::Beat)),
            "TrackSegment<beat>",
        ),
        (Type::Keyframe(Box::new(Type::Angle)), "Keyframe<angle>"),
    ];

    for (ty, expected) in cases {
        assert_eq!(ty.to_string(), expected);
    }
}

#[test]
fn scalar_typed_values_report_their_distinct_types() {
    let cases = [
        (TypedValue::Bool(true), Type::Bool),
        (TypedValue::Int(1), Type::Int),
        (TypedValue::Float(1.0), Type::Float),
        (TypedValue::String("value".into()), Type::String),
        (TypedValue::Time(1.0), Type::Time),
        (
            TypedValue::Beat(Beat::new(1, 2).expect("valid beat")),
            Type::Beat,
        ),
        (TypedValue::Length(1.0), Type::Length),
        (TypedValue::Angle(1.0), Type::Angle),
        (TypedValue::Color(Color::WHITE), Type::Color),
    ];

    for (value, expected) in cases {
        assert_eq!(value.ty(), expected);
    }
}

#[test]
fn source_spans_reject_reversed_bounds() {
    assert!(std::panic::catch_unwind(|| SourceSpan::new(7, 3)).is_err());

    let reversed = SourceSpan { start: 7, end: 3 };
    assert!(std::panic::catch_unwind(|| reversed.len()).is_err());
}

#[test]
fn typed_value_vec2_constructor_accepts_homogeneous_components() {
    let value = TypedValue::vec2(TypedValue::Length(10.0), TypedValue::Length(20.0))
        .expect("homogeneous length components should form a vec2");

    assert_eq!(value.ty(), Type::Vec2(Box::new(Type::Length)));
}

#[test]
fn typed_value_vec2_rejects_heterogeneous_components() {
    assert!(TypedValue::vec2(TypedValue::Length(10.0), TypedValue::Time(20.0)).is_none());

    let raw = TypedValue::Vec2(
        Box::new(TypedValue::Length(10.0)),
        Box::new(TypedValue::Time(20.0)),
    );
    assert!(std::panic::catch_unwind(|| raw.ty()).is_err());
}

#[test]
fn typed_literal_type_is_inferred_from_its_value() {
    let span = SourceSpan::new(8, 9);
    let typed = TypedExpression::literal(TypedValue::Int(1), span);

    assert_eq!(
        typed.expression(),
        &TypedExpressionKind::Literal(TypedValue::Int(1))
    );
    assert_eq!(typed.ty(), &Type::Int);
    assert_eq!(typed.span(), span);
}

#[test]
fn phase2_expression_nodes_keep_source_spans() {
    let span = SourceSpan::new(2, 5);
    let literal = SourceExpression::Literal {
        literal: SourceLiteral::Int(1),
        span,
    };
    let name = SourceExpression::Name {
        name: "value".into(),
        span,
    };
    let unary = SourceExpression::Unary {
        operator: UnaryOperator::Negate,
        operand: Box::new(literal.clone()),
        span,
    };
    let binary = SourceExpression::Binary {
        left: Box::new(literal.clone()),
        operator: BinaryOperator::Add,
        right: Box::new(literal.clone()),
        span,
    };
    let call = SourceExpression::Call {
        callee: Box::new(name.clone()),
        arguments: vec![literal.clone()],
        span,
    };
    let field_access = SourceExpression::FieldAccess {
        base: Box::new(name.clone()),
        field: "start".into(),
        span,
    };
    let vec2 = SourceExpression::Vec2 {
        x: Box::new(literal.clone()),
        y: Box::new(literal.clone()),
        span,
    };

    for expression in [literal, name, unary, binary, call, field_access, vec2] {
        assert_eq!(expression.span(), span);
    }

    let typed = TypedExpression::literal(TypedValue::Int(1), span);
    assert_eq!(typed.ty(), &Type::Int);
    assert_eq!(typed.span(), span);
}

#[test]
fn parses_typed_phase2_expression_shape() {
    let expression = parse_expression("1beat + 2beat * 3").into_result().unwrap();
    assert_eq!(expression.span().start, 0);
}

#[test]
fn parses_nested_type_syntax() {
    assert_eq!(
        parse_type("vec2<length>").into_result().unwrap(),
        Type::Vec2(Box::new(Type::Length))
    );
}

#[test]
fn identifiers_are_ascii_but_spans_remain_utf8_byte_offsets() {
    for source in ["\u{53d8}\u{91cf}", "ascii.\u{503c}", "\u{e9}clair"] {
        assert_eq!(
            parse_expression(source)
                .into_result()
                .expect_err("non-ascii identifier")[0]
                .code(),
            DiagnosticCode::SYNTAX_INVALID_TOKEN,
            "identifier {source:?}"
        );
    }

    let source = "\"\u{96ea}\" /* \u{96e8} */ + ascii";
    let expression = parse_expression(source).into_result().unwrap();

    assert_eq!(expression.span(), SourceSpan::new(0, source.len()));
    match expression {
        SourceExpression::Binary {
            left,
            operator: BinaryOperator::Add,
            right,
            span,
        } => {
            assert_eq!(left.span(), SourceSpan::new(0, "\"\u{96ea}\"".len()));
            let ascii_start = source.find("ascii").unwrap();
            assert_eq!(right.span(), SourceSpan::new(ascii_start, source.len()));
            assert_eq!(span, SourceSpan::new(0, source.len()));
        }
        other => panic!("expected addition, got {other:?}"),
    }
}

#[test]
fn comments_are_trivia_without_changing_literal_spans() {
    let source = "1 /* comment */ + // line\n 2";
    let expression = parse_expression(source).into_result().unwrap();

    match expression {
        SourceExpression::Binary {
            left,
            operator: BinaryOperator::Add,
            right,
            span,
        } => {
            assert_eq!(left.span(), SourceSpan::new(0, 1));
            let right_start = source.rfind('2').unwrap();
            assert_eq!(right.span(), SourceSpan::new(right_start, source.len()));
            assert_eq!(span, SourceSpan::new(0, source.len()));
        }
        other => panic!("expected addition, got {other:?}"),
    }
}

#[test]
fn parses_scalar_and_unit_literals() {
    let cases = [
        ("true", SourceLiteral::Bool(true)),
        ("false", SourceLiteral::Bool(false)),
        ("42", SourceLiteral::Int(42)),
        ("3.25", SourceLiteral::Float(3.25)),
        ("1e-3", SourceLiteral::Float(0.001)),
        (
            "\"snow \\u{96ea}\"",
            SourceLiteral::String("snow \u{96ea}".into()),
        ),
        (
            "#10203040",
            SourceLiteral::Color(Color::rgba(0x10, 0x20, 0x30, 0x40)),
        ),
        ("1500ms", SourceLiteral::Time(1.5)),
        ("2s", SourceLiteral::Time(2.0)),
        ("2min", SourceLiteral::Time(120.0)),
        ("2px", SourceLiteral::Length(2.0)),
        ("180deg", SourceLiteral::Angle(std::f64::consts::PI)),
        ("0.5rad", SourceLiteral::Angle(0.5)),
        ("1.25beat", SourceLiteral::Beat(Beat::new(5, 4).unwrap())),
    ];

    for (source, expected) in cases {
        assert_eq!(
            parse_expression(source).into_result().unwrap(),
            SourceExpression::Literal {
                literal: expected,
                span: SourceSpan::new(0, source.len()),
            },
            "literal {source}"
        );
    }
}

#[test]
fn unit_literals_must_remain_finite_after_conversion() {
    for source in ["1e309s", "1e309px"] {
        assert_eq!(
            parse_expression(source)
                .into_result()
                .expect_err("non-finite unit")[0]
                .code(),
            DiagnosticCode::NUMERIC_NON_FINITE,
            "literal {source}"
        );
    }

    let expression = parse_expression("2min == 120s").into_result().unwrap();
    let (left, right) = binary_operands(&expression, BinaryOperator::Equal);
    assert!(matches!(
        left,
        SourceExpression::Literal {
            literal: SourceLiteral::Time(120.0),
            ..
        }
    ));
    assert!(matches!(
        right,
        SourceExpression::Literal {
            literal: SourceLiteral::Time(120.0),
            ..
        }
    ));
}

#[test]
fn beat_literals_reduce_exactly_before_narrowing() {
    let cases = [
        ("0.1000000000000000000beat", Beat::new(1, 10).unwrap()),
        ("0.0000000000000000000beat", Beat::new(0, 1).unwrap()),
    ];
    for (source, expected) in cases {
        assert_eq!(
            parse_expression(source).into_result().unwrap(),
            SourceExpression::Literal {
                literal: SourceLiteral::Beat(expected),
                span: SourceSpan::new(0, source.len()),
            },
            "literal {source}"
        );
    }

    assert_eq!(
        parse_expression("0.0000000000000000001beat")
            .into_result()
            .expect_err("unrepresentable beat")[0]
            .code(),
        DiagnosticCode::SYNTAX_INVALID_TOKEN
    );
}

#[test]
fn string_escapes_match_the_documented_table() {
    let accepted = [
        (r#""\n""#, "\n"),
        (r#""\r""#, "\r"),
        (r#""\t""#, "\t"),
        (r#""\\""#, "\\"),
        (r#""\"""#, "\""),
        (r#""\u{0}""#, "\0"),
        (r#""\u{10FFFF}""#, "\u{10ffff}"),
    ];
    for (source, expected) in accepted {
        assert_eq!(
            parse_expression(source).into_result().unwrap(),
            SourceExpression::Literal {
                literal: SourceLiteral::String(expected.into()),
                span: SourceSpan::new(0, source.len()),
            },
            "escape {source:?}"
        );
    }

    for source in [
        r#""\u{D800}""#,
        r#""\u{110000}""#,
        r#""\u""#,
        r#""\u{}""#,
        r#""\u{1234567}""#,
        r#""\u{zz}""#,
        r#""\u{12""#,
    ] {
        assert_eq!(
            parse_expression(source)
                .into_result()
                .expect_err("invalid string escape")[0]
                .code(),
            DiagnosticCode::SYNTAX_INVALID_TOKEN,
            "escape {source:?}"
        );
    }
}

#[test]
fn parses_names_calls_fields_parentheses_and_vec2_construction() {
    let call = parse_expression("factory(1, nested.value)")
        .into_result()
        .unwrap();
    match call {
        SourceExpression::Call {
            callee,
            arguments,
            span,
        } => {
            assert!(matches!(
                *callee,
                SourceExpression::Name { ref name, .. } if name == "factory"
            ));
            assert_eq!(arguments.len(), 2);
            assert!(matches!(
                arguments[1],
                SourceExpression::FieldAccess { ref field, .. } if field == "value"
            ));
            assert_eq!(span, SourceSpan::new(0, 24));
        }
        other => panic!("expected call, got {other:?}"),
    }

    let grouped = parse_expression("(1 + 2) * 3").into_result().unwrap();
    match grouped {
        SourceExpression::Binary {
            left,
            operator: BinaryOperator::Multiply,
            ..
        } => {
            assert_eq!(left.span(), SourceSpan::new(0, 7));
            assert!(matches!(
                *left,
                SourceExpression::Binary {
                    operator: BinaryOperator::Add,
                    ..
                }
            ));
        }
        other => panic!("expected multiplication, got {other:?}"),
    }

    let vector = parse_expression("vec2(10px, 20px)").into_result().unwrap();
    assert!(matches!(vector, SourceExpression::Vec2 { .. }));
}

#[test]
fn parses_unary_operators_before_postfix_and_binary_operators() {
    let expression = parse_expression("!-value.field + 1").into_result().unwrap();

    match expression {
        SourceExpression::Binary {
            left,
            operator: BinaryOperator::Add,
            ..
        } => match *left {
            SourceExpression::Unary {
                operator: UnaryOperator::Not,
                operand,
                ..
            } => assert!(matches!(
                *operand,
                SourceExpression::Unary {
                    operator: UnaryOperator::Negate,
                    operand,
                    ..
                } if matches!(*operand, SourceExpression::FieldAccess { .. })
            )),
            other => panic!("expected logical not, got {other:?}"),
        },
        other => panic!("expected addition, got {other:?}"),
    }
}

#[test]
fn operator_precedence_follows_language_categories() {
    let expression = parse_expression("1 + 2 * 3 < 8 == true && false || true")
        .into_result()
        .unwrap();

    let (or_left, or_right) = binary_operands(&expression, BinaryOperator::Or);
    assert!(matches!(
        or_right,
        SourceExpression::Literal {
            literal: SourceLiteral::Bool(true),
            ..
        }
    ));
    let (and_left, _) = binary_operands(or_left, BinaryOperator::And);
    let (equality_left, _) = binary_operands(and_left, BinaryOperator::Equal);
    let (comparison_left, _) = binary_operands(equality_left, BinaryOperator::LessThan);
    let (_, additive_right) = binary_operands(comparison_left, BinaryOperator::Add);
    binary_operands(additive_right, BinaryOperator::Multiply);
}

#[test]
fn parses_every_binary_operator() {
    let cases = [
        ("1 + 2", BinaryOperator::Add),
        ("1 - 2", BinaryOperator::Subtract),
        ("1 * 2", BinaryOperator::Multiply),
        ("1 / 2", BinaryOperator::Divide),
        ("1 % 2", BinaryOperator::Remainder),
        ("1 == 2", BinaryOperator::Equal),
        ("1 != 2", BinaryOperator::NotEqual),
        ("1 < 2", BinaryOperator::LessThan),
        ("1 <= 2", BinaryOperator::LessThanOrEqual),
        ("1 > 2", BinaryOperator::GreaterThan),
        ("1 >= 2", BinaryOperator::GreaterThanOrEqual),
        ("true && false", BinaryOperator::And),
        ("true || false", BinaryOperator::Or),
    ];

    for (source, expected) in cases {
        let expression = parse_expression(source).into_result().unwrap();
        binary_operands(&expression, expected);
    }
}

#[test]
fn comparison_chains_lower_to_adjacent_comparisons() {
    for (source, comparison) in [
        ("a < b < c", BinaryOperator::LessThan),
        ("a <= b <= c", BinaryOperator::LessThanOrEqual),
    ] {
        let expression = parse_expression(source).into_result().unwrap();
        assert_eq!(expression.span(), SourceSpan::new(0, source.len()));
        let (first, second) = binary_operands(&expression, BinaryOperator::And);
        let (first_left, first_right) = binary_operands(first, comparison);
        let (second_left, second_right) = binary_operands(second, comparison);
        let middle_start = source.find('b').unwrap();

        assert!(matches!(
            first_left,
            SourceExpression::Name { name, span }
                if name == "a" && *span == SourceSpan::new(0, 1)
        ));
        assert!(matches!(
            first_right,
            SourceExpression::Name { name, span }
                if name == "b" && *span == SourceSpan::new(middle_start, middle_start + 1)
        ));
        assert!(matches!(
            second_left,
            SourceExpression::Name { name, span }
                if name == "b"
                    && *span == SourceSpan::new(middle_start, middle_start + 1)
        ));
        let c_start = source.rfind('c').unwrap();
        assert!(matches!(
            second_right,
            SourceExpression::Name { name, span }
                if name == "c" && *span == SourceSpan::new(c_start, c_start + 1)
        ));
        assert_eq!(first.span(), SourceSpan::new(0, middle_start + 1));
        assert_eq!(second.span(), SourceSpan::new(middle_start, source.len()));
    }
}

#[test]
fn parser_rejects_trailing_or_incomplete_input() {
    for source in ["1 2", "vec2(1, 2) trailing", "1 +", "\"unterminated"] {
        assert_eq!(
            parse_expression(source)
                .into_result()
                .expect_err("invalid expression")[0]
                .code(),
            if source == "\"unterminated" {
                DiagnosticCode::SYNTAX_UNCLOSED_STRING
            } else {
                DiagnosticCode::SYNTAX_INVALID_TOKEN
            },
            "expression {source:?}"
        );
    }

    for source in ["int extra", "vec2<length>>", "Unknown", "vec2<>"] {
        assert_eq!(
            parse_type(source).into_result().expect_err("invalid type")[0].code(),
            DiagnosticCode::SYNTAX_INVALID_TOKEN,
            "type {source:?}"
        );
    }
}

#[test]
fn parses_scalar_and_recursive_track_types() {
    let scalar_cases = [
        ("bool", Type::Bool),
        ("int", Type::Int),
        ("float", Type::Float),
        ("string", Type::String),
        ("time", Type::Time),
        ("beat", Type::Beat),
        ("length", Type::Length),
        ("angle", Type::Angle),
        ("color", Type::Color),
        ("Note", Type::Note),
        ("Line", Type::Line),
        ("RenderNode", Type::RenderNode),
    ];
    for (source, expected) in scalar_cases {
        assert_eq!(parse_type(source).into_result().unwrap(), expected);
    }

    assert_eq!(
        parse_type("TrackSegment<Keyframe<vec2<beat>>>")
            .into_result()
            .unwrap(),
        Type::TrackSegment(Box::new(Type::Keyframe(Box::new(Type::Vec2(Box::new(
            Type::Beat
        ))))))
    );
    assert_eq!(
        parse_type("Keyframe<TrackSegment<length>>")
            .into_result()
            .unwrap(),
        Type::Keyframe(Box::new(Type::TrackSegment(Box::new(Type::Length))))
    );
}

#[test]
fn parser_rejects_nesting_beyond_the_shared_limit() {
    const LIMIT: usize = 128;

    let nested_type = |depth: usize| format!("{}int{}", "vec2<".repeat(depth), ">".repeat(depth));
    let limits = ParseLimits {
        max_nesting_depth: LIMIT,
        ..ParseLimits::default()
    };
    assert!(
        parse_type_with_limits(&nested_type(LIMIT), limits)
            .into_result()
            .is_ok()
    );
    assert_eq!(
        parse_type_with_limits(&nested_type(LIMIT + 1), limits)
            .into_result()
            .expect_err("type nesting limit")[0]
            .code(),
        DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
    );

    let unary = |depth: usize| format!("{}value", "!".repeat(depth));
    assert!(
        parse_expression_with_limits(&unary(LIMIT), limits)
            .into_result()
            .is_ok()
    );
    assert_eq!(
        parse_expression_with_limits(&unary(LIMIT + 1), limits)
            .into_result()
            .expect_err("unary nesting limit")[0]
            .code(),
        DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
    );

    let grouped = |depth: usize| format!("{}value{}", "(".repeat(depth), ")".repeat(depth));
    assert!(
        parse_expression_with_limits(&grouped(LIMIT), limits)
            .into_result()
            .is_ok()
    );
    assert_eq!(
        parse_expression_with_limits(&grouped(LIMIT + 1), limits)
            .into_result()
            .expect_err("group nesting limit")[0]
            .code(),
        DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
    );

    let mixed = |unary_depth: usize, group_depth: usize| {
        format!(
            "{}{}value{}",
            "!".repeat(unary_depth),
            "(".repeat(group_depth),
            ")".repeat(group_depth)
        )
    };
    assert!(
        parse_expression_with_limits(&mixed(LIMIT / 2, LIMIT / 2), limits)
            .into_result()
            .is_ok()
    );
    let mixed_limit = parse_expression_with_limits(&mixed(LIMIT / 2 + 1, LIMIT / 2), limits);
    assert!(mixed_limit.output().is_none());
    assert_eq!(
        mixed_limit.diagnostics()[0].code(),
        DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
    );
    assert_eq!(
        mixed_limit.diagnostics()[0].primary_span(),
        SourceSpan::new(LIMIT, LIMIT + 1)
    );

    let power = |depth: usize| format!("value{}", " ** value".repeat(depth));
    assert!(
        parse_expression_with_limits(&power(LIMIT), limits)
            .into_result()
            .is_ok()
    );
    assert_eq!(
        parse_expression_with_limits(&power(LIMIT + 1), limits)
            .into_result()
            .expect_err("power nesting limit")[0]
            .code(),
        DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
    );
}

#[test]
fn generated_comparison_chains_evaluate_the_middle_expression_once() {
    let source = r#"#fcs 5.0.0
format { profile: fragment; }
definitions { const ordered: bool = 0 < (1 + 1) <= 3; }"#;
    let document = parse_document(source).into_result().unwrap();
    let limits = CompileTimeLimits {
        max_compile_time_operations: 4,
        ..CompileTimeLimits::default()
    };
    assert!(elaborate(&document, phase2_schema(), limits).is_ok());
}

#[test]
fn generated_comparison_chains_retain_all_middle_values_and_short_circuit() {
    let ordered_limits = CompileTimeLimits {
        max_compile_time_operations: 6,
        ..CompileTimeLimits::default()
    };
    for expression in [
        "0 < (1 + 1) <= 3 < 4",
        "(0 < (1 + 1) <= 3 < 4)",
        "((0 < (1 + 1) <= 3 < 4))",
    ] {
        let source = format!(
            "#fcs 5.0.0\nformat {{ profile: fragment; }}\ndefinitions {{ const ordered: bool = {expression}; }}"
        );
        let document = parse_document(&source).into_result().unwrap();
        assert!(elaborate(&document, phase2_schema(), ordered_limits).is_ok());
    }

    let stopped = r#"#fcs 5.0.0
format { profile: fragment; }
definitions { const stopped: bool = 3 < (1 + 1) <= 3 < 1 / 0; }"#;
    let stopped_document = parse_document(stopped).into_result().unwrap();
    let stopped_limits = CompileTimeLimits {
        max_compile_time_operations: 4,
        ..CompileTimeLimits::default()
    };
    assert!(elaborate(&stopped_document, phase2_schema(), stopped_limits).is_ok());
}

#[test]
fn explicit_logical_and_does_not_share_repeated_source_expressions() {
    let source = r#"#fcs 5.0.0
format { profile: fragment; }
definitions { const value: bool = 0 < (1 + 1) && (1 + 1) < 4; }"#;
    let document = parse_document(source).into_result().unwrap();
    let limits = CompileTimeLimits {
        max_compile_time_operations: 4,
        ..CompileTimeLimits::default()
    };
    let diagnostics = elaborate(&document, phase2_schema(), limits)
        .expect_err("explicit && evaluates both source expressions");
    assert_eq!(
        diagnostics[0].code(),
        DiagnosticCode::COMPILE_TIME_BUDGET_EXCEEDED
    );
}

#[test]
fn logical_operators_short_circuit_invalid_right_hand_values() {
    let source = r#"#fcs 5.0.0
format { profile: fragment; }
definitions {
  const and_value: bool = false && 1 / 0 == 0;
  const or_value: bool = true || 1 / 0 == 0;
}"#;
    assert!(elaborate_source(source).is_ok());
}

#[test]
fn power_is_rejected_by_the_i0_elaborator_boundary() {
    let source = r#"#fcs 5.0.0
format { profile: fragment; }
definitions { const value: int = 2 ** 3; }"#;
    let document = parse_document(source).into_result().unwrap();
    let diagnostics = elaborate(&document, phase2_schema(), CompileTimeLimits::default())
        .expect_err("I0 does not define power evaluation");
    assert_eq!(
        diagnostics[0].code(),
        DiagnosticCode::TYPE_INVALID_OPERATION
    );
}

fn binary_operands(
    expression: &SourceExpression,
    expected: BinaryOperator,
) -> (&SourceExpression, &SourceExpression) {
    match expression {
        SourceExpression::Binary {
            left,
            operator,
            right,
            ..
        } => {
            assert_eq!(*operator, expected);
            (left, right)
        }
        other => panic!("expected {expected:?} binary expression, got {other:?}"),
    }
}
