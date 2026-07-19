//! Compile-time expansion for Line-owned Track pieces.

use std::collections::BTreeMap;

use crate::ast::{
    Document, EntityExpression, ExpandedField, ExpandedTrack, ExpandedTrackInterpolation,
    ExpandedTrackPiece, ExpandedTrackPoint, ExpandedTrackSegment, Generator, GeneratorItem,
    Interpolation, LineBodyItem, SchemaValue, SourceEntityConstructor, SourceEntityConstructorKind,
    SourceExpression, SourceSpan, TrackDeclaration, TrackSegmentItem, Type, TypedValue,
};
use crate::schema::ConstructionSchema;

use super::eval::evaluate_with_context_expected;
use super::{CompileTimeContext, ElaboratorError as Diagnostic};

pub(super) fn expand_tracks(
    document: &Document,
    schema: &ConstructionSchema,
    context: CompileTimeContext,
) -> Result<Vec<ExpandedTrack>, Diagnostic> {
    let mut tracks = Vec::new();
    for line in &document.lines {
        for item in &line.items {
            let LineBodyItem::Tracks(block) = item else {
                continue;
            };
            for track in &block.tracks {
                tracks.push(expand_track(document, schema, &context, &line.name, track)?);
            }
        }
    }
    Ok(tracks)
}

fn expand_track(
    document: &Document,
    schema: &ConstructionSchema,
    context: &CompileTimeContext,
    owner: &str,
    track: &TrackDeclaration,
) -> Result<ExpandedTrack, Diagnostic> {
    let mut settings: BTreeMap<String, ExpandedField> = BTreeMap::new();
    for setting in &track.settings {
        let expected = match setting.name.as_str() {
            "blend" | "fill" | "extrapolateBefore" | "extrapolateAfter" => Type::String,
            "priority" => Type::Int,
            _ => {
                return Err(Diagnostic::InvalidOperation {
                    message: "unknown Track setting",
                    span: setting.name_span,
                });
            }
        };
        if let Some(previous) = settings.get(&setting.name) {
            return Err(Diagnostic::DuplicateEntityField {
                field: setting.name.clone(),
                span: setting.span,
                previous_span: previous.span(),
            });
        }
        let value = evaluate_with_context_expected(
            &setting.value,
            document.definitions.as_ref(),
            &BTreeMap::new(),
            schema,
            context,
            Some(&expected),
        )?;
        settings.insert(
            setting.name.clone(),
            ExpandedField::new(setting.name.clone(), value, setting.span),
        );
    }

    let mut pieces = Vec::new();
    expand_items(
        document,
        schema,
        context,
        track,
        &track.segments.items,
        &BTreeMap::new(),
        &mut pieces,
    )?;
    Ok(ExpandedTrack::new(
        owner.to_owned(),
        track.name.clone(),
        track.name_span,
        track.target.segments.join("."),
        track.target.span,
        track.value_type.clone(),
        settings,
        pieces,
        track.span,
    ))
}

fn expand_items(
    document: &Document,
    schema: &ConstructionSchema,
    context: &CompileTimeContext,
    track: &TrackDeclaration,
    items: &[TrackSegmentItem],
    bindings: &BTreeMap<String, TypedValue>,
    output: &mut Vec<ExpandedTrackPiece>,
) -> Result<(), Diagnostic> {
    for item in items {
        match item {
            TrackSegmentItem::DirectSegment(segment) => {
                let start = evaluate(
                    document,
                    schema,
                    context,
                    bindings,
                    &segment.interval.start,
                    None,
                )?;
                let end = evaluate(
                    document,
                    schema,
                    context,
                    bindings,
                    &segment.interval.end,
                    None,
                )?;
                let start_value = evaluate(
                    document,
                    schema,
                    context,
                    bindings,
                    &segment.start_value,
                    Some(&track.value_type),
                )?;
                let end_value = evaluate(
                    document,
                    schema,
                    context,
                    bindings,
                    &segment.end_value,
                    Some(&track.value_type),
                )?;
                let interpolation = expand_interpolation(
                    document,
                    schema,
                    context,
                    bindings,
                    &segment.interpolation,
                )?;
                push_piece(
                    context,
                    ExpandedTrackPiece::Segment(ExpandedTrackSegment::new(
                        start,
                        end,
                        start_value,
                        end_value,
                        interpolation,
                        segment.span,
                    )),
                    output,
                )?;
            }
            TrackSegmentItem::DirectPoint(point) => {
                let time = evaluate(document, schema, context, bindings, &point.time, None)?;
                let value = evaluate(
                    document,
                    schema,
                    context,
                    bindings,
                    &point.value,
                    Some(&track.value_type),
                )?;
                push_piece(
                    context,
                    ExpandedTrackPiece::Point(ExpandedTrackPoint::new(time, value, point.span)),
                    output,
                )?;
            }
            TrackSegmentItem::Conditional {
                condition,
                then_items,
                else_items,
                span,
            } => {
                let condition = evaluate(
                    document,
                    schema,
                    context,
                    bindings,
                    condition,
                    Some(&Type::Bool),
                )?;
                let TypedValue::Bool(selected) = condition else {
                    return Err(Diagnostic::NonConstantStructuralCondition { span: *span });
                };
                expand_items(
                    document,
                    schema,
                    context,
                    track,
                    if selected { then_items } else { else_items },
                    bindings,
                    output,
                )?;
            }
            TrackSegmentItem::Generator(generator) => {
                expand_generator(document, schema, context, track, generator, output)?;
            }
        }
    }
    Ok(())
}

fn expand_generator(
    document: &Document,
    schema: &ConstructionSchema,
    context: &CompileTimeContext,
    track: &TrackDeclaration,
    generator: &Generator,
    output: &mut Vec<ExpandedTrackPiece>,
) -> Result<(), Diagnostic> {
    let range =
        super::generator::evaluate_range_with_context(document, generator, schema, context)?;
    for index in 0..range.count() {
        context.consume("max_generator_iterations", generator.range.span)?;
        let mut bindings = BTreeMap::from([
            (
                generator.variable.clone(),
                range
                    .value_at(index)
                    .map_err(|_| Diagnostic::NumericOverflow {
                        span: generator.range.span,
                    })?,
            ),
            ("index".to_owned(), TypedValue::Int(index)),
            ("range".to_owned(), range.frame_value()),
        ]);
        expand_generator_items(
            document,
            schema,
            context,
            track,
            &generator.body,
            &mut bindings,
            output,
        )?;
    }
    Ok(())
}

fn expand_generator_items(
    document: &Document,
    schema: &ConstructionSchema,
    context: &CompileTimeContext,
    track: &TrackDeclaration,
    items: &[GeneratorItem],
    bindings: &mut BTreeMap<String, TypedValue>,
    output: &mut Vec<ExpandedTrackPiece>,
) -> Result<(), Diagnostic> {
    for item in items {
        match item {
            GeneratorItem::Let(statement) => {
                let value = evaluate(
                    document,
                    schema,
                    context,
                    bindings,
                    &statement.initializer,
                    Some(&statement.ty),
                )?;
                if bindings.insert(statement.name.clone(), value).is_some() {
                    return Err(Diagnostic::DuplicateBinding {
                        name: statement.name.clone(),
                        span: statement.name_span,
                        previous_span: statement.name_span,
                    });
                }
            }
            GeneratorItem::Conditional {
                condition,
                then_items,
                else_items,
                span,
            } => {
                let value = evaluate(
                    document,
                    schema,
                    context,
                    bindings,
                    condition,
                    Some(&Type::Bool),
                )?;
                let TypedValue::Bool(selected) = value else {
                    return Err(Diagnostic::NonConstantStructuralCondition { span: *span });
                };
                let mut branch_bindings = bindings.clone();
                expand_generator_items(
                    document,
                    schema,
                    context,
                    track,
                    if selected { then_items } else { else_items },
                    &mut branch_bindings,
                    output,
                )?;
            }
            GeneratorItem::Emit(expression) => {
                let EntityExpression::SourceConstructor(constructor) = expression else {
                    return Err(Diagnostic::CollectionTypeMismatch {
                        collection: format!("Track<{}>", track.value_type),
                        expected: Type::TrackSegment(Box::new(track.value_type.clone())),
                        actual: emitted_type(expression),
                        span: expression.span(),
                    });
                };
                let piece = expand_source_constructor(
                    document,
                    schema,
                    context,
                    track,
                    constructor,
                    bindings,
                )?;
                push_piece(context, piece, output)?;
            }
        }
    }
    Ok(())
}

fn expand_source_constructor(
    document: &Document,
    schema: &ConstructionSchema,
    context: &CompileTimeContext,
    track: &TrackDeclaration,
    constructor: &SourceEntityConstructor,
    bindings: &BTreeMap<String, TypedValue>,
) -> Result<ExpandedTrackPiece, Diagnostic> {
    let mut fields = BTreeMap::new();
    for field in &constructor.fields {
        let name = field.path.segments.join(".");
        if let Some(previous) = fields.insert(name.clone(), field) {
            return Err(Diagnostic::DuplicateEntityField {
                field: name,
                span: field.path.span,
                previous_span: previous.path.span,
            });
        }
    }
    match constructor.kind {
        SourceEntityConstructorKind::Segment => {
            let entity = Type::TrackSegment(Box::new(track.value_type.clone()));
            let start = expression_field(&fields, "start", constructor.span, &entity)?;
            let end = expression_field(&fields, "end", constructor.span, &entity)?;
            let start_value = expression_field(&fields, "startValue", constructor.span, &entity)?;
            let end_value = expression_field(&fields, "endValue", constructor.span, &entity)?;
            let interpolation =
                fields
                    .get("interpolation")
                    .ok_or(Diagnostic::MissingRequiredField {
                        entity: entity.clone(),
                        field: "interpolation".to_owned(),
                        span: constructor.span,
                    })?;
            reject_unknown_fields(
                &fields,
                &["start", "end", "startValue", "endValue", "interpolation"],
                entity,
            )?;
            Ok(ExpandedTrackPiece::Segment(ExpandedTrackSegment::new(
                evaluate(document, schema, context, bindings, start, None)?,
                evaluate(document, schema, context, bindings, end, None)?,
                evaluate(
                    document,
                    schema,
                    context,
                    bindings,
                    start_value,
                    Some(&track.value_type),
                )?,
                evaluate(
                    document,
                    schema,
                    context,
                    bindings,
                    end_value,
                    Some(&track.value_type),
                )?,
                expand_schema_interpolation(
                    document,
                    schema,
                    context,
                    bindings,
                    &interpolation.value,
                )?,
                constructor.span,
            )))
        }
        SourceEntityConstructorKind::Keyframe => {
            let entity = Type::Keyframe(Box::new(track.value_type.clone()));
            let time = expression_field(&fields, "time", constructor.span, &entity)?;
            let value = expression_field(&fields, "value", constructor.span, &entity)?;
            reject_unknown_fields(&fields, &["time", "value"], entity)?;
            Ok(ExpandedTrackPiece::Point(ExpandedTrackPoint::new(
                evaluate(document, schema, context, bindings, time, None)?,
                evaluate(
                    document,
                    schema,
                    context,
                    bindings,
                    value,
                    Some(&track.value_type),
                )?,
                constructor.span,
            )))
        }
        SourceEntityConstructorKind::RenderNode => Err(Diagnostic::CollectionTypeMismatch {
            collection: format!("Track<{}>", track.value_type),
            expected: Type::TrackSegment(Box::new(track.value_type.clone())),
            actual: Type::RenderNode,
            span: constructor.span,
        }),
    }
}

fn expression_field<'a>(
    fields: &'a BTreeMap<String, &crate::ast::SchemaField>,
    name: &str,
    span: SourceSpan,
    entity: &Type,
) -> Result<&'a SourceExpression, Diagnostic> {
    match fields.get(name) {
        Some(field) => match &field.value {
            SchemaValue::Expression(expression) => Ok(expression),
            _ => Err(Diagnostic::InvalidOperation {
                message: "Track constructor field requires an expression",
                span: field.value.span(),
            }),
        },
        None => Err(Diagnostic::MissingRequiredField {
            entity: entity.clone(),
            field: name.to_owned(),
            span,
        }),
    }
}

fn reject_unknown_fields(
    fields: &BTreeMap<String, &crate::ast::SchemaField>,
    known: &[&str],
    entity: Type,
) -> Result<(), Diagnostic> {
    if let Some((name, field)) = fields
        .iter()
        .find(|(name, _)| !known.contains(&name.as_str()))
    {
        Err(Diagnostic::UnknownEntityField {
            entity,
            field: name.clone(),
            span: field.path.span,
        })
    } else {
        Ok(())
    }
}

fn expand_interpolation(
    document: &Document,
    schema: &ConstructionSchema,
    context: &CompileTimeContext,
    bindings: &BTreeMap<String, TypedValue>,
    interpolation: &Interpolation,
) -> Result<ExpandedTrackInterpolation, Diagnostic> {
    match interpolation {
        Interpolation::Expression(expression) => Ok(ExpandedTrackInterpolation::Value(evaluate(
            document,
            schema,
            context,
            bindings,
            expression,
            Some(&Type::String),
        )?)),
        Interpolation::CubicBezier { values, .. } => Ok(ExpandedTrackInterpolation::CubicBezier(
            evaluate_bezier(document, schema, context, bindings, values)?,
        )),
    }
}

fn expand_schema_interpolation(
    document: &Document,
    schema: &ConstructionSchema,
    context: &CompileTimeContext,
    bindings: &BTreeMap<String, TypedValue>,
    value: &SchemaValue,
) -> Result<ExpandedTrackInterpolation, Diagnostic> {
    match value {
        SchemaValue::Expression(expression) => Ok(ExpandedTrackInterpolation::Value(evaluate(
            document,
            schema,
            context,
            bindings,
            expression,
            Some(&Type::String),
        )?)),
        SchemaValue::CubicBezier { values, .. } => Ok(ExpandedTrackInterpolation::CubicBezier(
            evaluate_bezier(document, schema, context, bindings, values)?,
        )),
        SchemaValue::Interval { span, .. } => Err(Diagnostic::InvalidOperation {
            message: "Track interpolation cannot be an interval",
            span: *span,
        }),
    }
}

fn evaluate_bezier(
    document: &Document,
    schema: &ConstructionSchema,
    context: &CompileTimeContext,
    bindings: &BTreeMap<String, TypedValue>,
    values: &[SourceExpression; 4],
) -> Result<[TypedValue; 4], Diagnostic> {
    let mut evaluated = Vec::with_capacity(4);
    for value in values {
        evaluated.push(evaluate(
            document,
            schema,
            context,
            bindings,
            value,
            Some(&Type::Float),
        )?);
    }
    Ok(evaluated
        .try_into()
        .expect("four Bezier source values produce four evaluated values"))
}

fn evaluate(
    document: &Document,
    schema: &ConstructionSchema,
    context: &CompileTimeContext,
    bindings: &BTreeMap<String, TypedValue>,
    expression: &SourceExpression,
    expected: Option<&Type>,
) -> Result<TypedValue, Diagnostic> {
    evaluate_with_context_expected(
        expression,
        document.definitions.as_ref(),
        bindings,
        schema,
        context,
        expected,
    )
}

fn push_piece(
    context: &CompileTimeContext,
    piece: ExpandedTrackPiece,
    output: &mut Vec<ExpandedTrackPiece>,
) -> Result<(), Diagnostic> {
    context.consume("max_generated_nodes", piece.span())?;
    output.push(piece);
    Ok(())
}

fn emitted_type(expression: &EntityExpression) -> Type {
    match expression {
        EntityExpression::Constructor(constructor) => constructor.entity_type.clone(),
        EntityExpression::SourceConstructor(constructor) => match constructor.kind {
            SourceEntityConstructorKind::RenderNode => Type::RenderNode,
            SourceEntityConstructorKind::Segment => Type::TrackSegment(Box::new(Type::Float)),
            SourceEntityConstructorKind::Keyframe => Type::Keyframe(Box::new(Type::Float)),
        },
        EntityExpression::Source(_) | EntityExpression::With(_) => Type::RenderNode,
    }
}
