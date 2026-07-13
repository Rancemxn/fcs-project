use fcs_core::units::Color;
use fcs_core::v5::ast::{
    Beat, BinaryOperator, CollectionBlock, CollectionItem, EntityConstructor, EntityExpression,
    ExpandedEntity, ExpandedField, NoteVariant, SourceExpression, SourceLiteral, SourceSpan, Type,
    TypedExpression, TypedExpressionKind, TypedValue, UnaryOperator, WithExpression,
};
use fcs_core::v5::parser::{ParseError, parse_expression, parse_type};
use fcs_core::v5::schema::{FieldConstraint, phase2_schema};

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
    let expression = parse_expression("1beat + 2beat * 3").unwrap();
    assert_eq!(expression.span().start, 0);
}

#[test]
fn parses_nested_type_syntax() {
    assert_eq!(
        parse_type("vec2<length>").unwrap(),
        Type::Vec2(Box::new(Type::Length))
    );
}

#[test]
fn identifiers_are_ascii_but_spans_remain_utf8_byte_offsets() {
    for source in ["变量", "ascii.值", "éclair"] {
        assert_eq!(
            parse_expression(source),
            Err(ParseError::InvalidSyntax("expression")),
            "identifier {source:?}"
        );
    }

    let source = "\"雪\" /* 雨 */ + ascii";
    let expression = parse_expression(source).unwrap();

    assert_eq!(expression.span(), SourceSpan::new(0, source.len()));
    match expression {
        SourceExpression::Binary {
            left,
            operator: BinaryOperator::Add,
            right,
            span,
        } => {
            assert_eq!(left.span(), SourceSpan::new(0, "\"雪\"".len()));
            let ascii_start = source.find("ascii").unwrap();
            assert_eq!(right.span(), SourceSpan::new(ascii_start, source.len()));
            assert_eq!(span, SourceSpan::new(0, source.len()));
        }
        other => panic!("expected addition, got {other:?}"),
    }
}

#[test]
fn comments_are_trivia_without_changing_literal_spans() {
    let source = "1 /* 雪 */ + // line\n 2";
    let expression = parse_expression(source).unwrap();

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
            SourceLiteral::String("snow 雪".into()),
        ),
        (
            "#10203040",
            SourceLiteral::Color(Color::rgba(0x10, 0x20, 0x30, 0x40)),
        ),
        ("1500ms", SourceLiteral::Time(1.5)),
        ("2s", SourceLiteral::Time(2.0)),
        ("2min", SourceLiteral::Time(120.0)),
        ("2px", SourceLiteral::Length(2.0)),
        ("2vw", SourceLiteral::Length(38.4)),
        ("2vh", SourceLiteral::Length(21.6)),
        ("180deg", SourceLiteral::Angle(std::f64::consts::PI)),
        ("0.5rad", SourceLiteral::Angle(0.5)),
        ("1.25beat", SourceLiteral::Beat(Beat::new(5, 4).unwrap())),
    ];

    for (source, expected) in cases {
        assert_eq!(
            parse_expression(source).unwrap(),
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
    for source in ["1e308vw", "1e308vh"] {
        assert_eq!(
            parse_expression(source),
            Err(ParseError::InvalidSyntax("expression")),
            "literal {source}"
        );
    }

    let expression = parse_expression("2min == 120s").unwrap();
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
            parse_expression(source).unwrap(),
            SourceExpression::Literal {
                literal: SourceLiteral::Beat(expected),
                span: SourceSpan::new(0, source.len()),
            },
            "literal {source}"
        );
    }

    assert_eq!(
        parse_expression("0.0000000000000000001beat"),
        Err(ParseError::InvalidSyntax("expression"))
    );
}

#[test]
fn string_escapes_match_the_documented_table() {
    let accepted = [
        (r#""\n""#, "\n"),
        (r#""\t""#, "\t"),
        (r#""\\""#, "\\"),
        (r#""\"""#, "\""),
        (r#""\u{0}""#, "\0"),
        (r#""\u{10FFFF}""#, "\u{10ffff}"),
    ];
    for (source, expected) in accepted {
        assert_eq!(
            parse_expression(source).unwrap(),
            SourceExpression::Literal {
                literal: SourceLiteral::String(expected.into()),
                span: SourceSpan::new(0, source.len()),
            },
            "escape {source:?}"
        );
    }

    for source in [
        r#""\r""#,
        r#""\u{D800}""#,
        r#""\u{110000}""#,
        r#""\u""#,
        r#""\u{}""#,
        r#""\u{1234567}""#,
        r#""\u{zz}""#,
        r#""\u{12""#,
    ] {
        assert_eq!(
            parse_expression(source),
            Err(ParseError::InvalidSyntax("expression")),
            "escape {source:?}"
        );
    }
}

#[test]
fn parses_names_calls_fields_parentheses_and_vec2_construction() {
    let call = parse_expression("factory(1, nested.value)").unwrap();
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

    let grouped = parse_expression("(1 + 2) * 3").unwrap();
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

    let vector = parse_expression("vec2(10px, 20px)").unwrap();
    assert!(matches!(vector, SourceExpression::Vec2 { .. }));
}

#[test]
fn parses_unary_operators_before_postfix_and_binary_operators() {
    let expression = parse_expression("!-value.field + 1").unwrap();

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
    let expression = parse_expression("1 + 2 * 3 < 8 == true && false || true").unwrap();

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
        let expression = parse_expression(source).unwrap();
        binary_operands(&expression, expected);
    }
}

#[test]
fn comparison_chains_lower_to_adjacent_comparisons() {
    for (source, comparison) in [
        ("a < b < c", BinaryOperator::LessThan),
        ("a == b == c", BinaryOperator::Equal),
    ] {
        let expression = parse_expression(source).unwrap();
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
            parse_expression(source),
            Err(ParseError::InvalidSyntax("expression")),
            "expression {source:?}"
        );
    }

    for source in ["int extra", "vec2<length>>", "Unknown", "vec2<>"] {
        assert_eq!(
            parse_type(source),
            Err(ParseError::InvalidSyntax("type")),
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
        assert_eq!(parse_type(source).unwrap(), expected);
    }

    assert_eq!(
        parse_type("TrackSegment<Keyframe<vec2<beat>>>").unwrap(),
        Type::TrackSegment(Box::new(Type::Keyframe(Box::new(Type::Vec2(Box::new(
            Type::Beat
        ))))))
    );
    assert_eq!(
        parse_type("Keyframe<TrackSegment<length>>").unwrap(),
        Type::Keyframe(Box::new(Type::TrackSegment(Box::new(Type::Length))))
    );
}

#[test]
fn parser_rejects_nesting_beyond_the_shared_limit() {
    const LIMIT: usize = 128;

    let nested_type = |depth: usize| format!("{}int{}", "vec2<".repeat(depth), ">".repeat(depth));
    assert!(parse_type(&nested_type(LIMIT)).is_ok());
    assert_eq!(
        parse_type(&nested_type(LIMIT + 1)),
        Err(ParseError::InvalidSyntax("type"))
    );

    let unary = |depth: usize| format!("{}value", "!".repeat(depth));
    assert!(parse_expression(&unary(LIMIT)).is_ok());
    assert_eq!(
        parse_expression(&unary(LIMIT + 1)),
        Err(ParseError::InvalidSyntax("expression"))
    );

    let grouped = |depth: usize| format!("{}value{}", "(".repeat(depth), ")".repeat(depth));
    assert!(parse_expression(&grouped(LIMIT)).is_ok());
    assert_eq!(
        parse_expression(&grouped(LIMIT + 1)),
        Err(ParseError::InvalidSyntax("expression"))
    );

    let mixed = |unary_depth: usize, group_depth: usize| {
        format!(
            "{}{}value{}",
            "!".repeat(unary_depth),
            "(".repeat(group_depth),
            ")".repeat(group_depth)
        )
    };
    assert!(parse_expression(&mixed(LIMIT / 2, LIMIT / 2)).is_ok());
    assert_eq!(
        parse_expression(&mixed(LIMIT / 2 + 1, LIMIT / 2)),
        Err(ParseError::InvalidSyntax("expression"))
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
