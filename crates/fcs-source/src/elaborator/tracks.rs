//! Compile-time expansion for Line-owned Track pieces.

use std::collections::BTreeMap;

use crate::ast::{
    Document, EntityExpression, ExpandedField, ExpandedTrack, ExpandedTrackInterpolation,
    ExpandedTrackPiece, ExpandedTrackPoint, ExpandedTrackSegment, FunctionDeclaration, Generator,
    GeneratorItem, Interpolation, LineBodyItem, SchemaValue, SourceEntityConstructor,
    SourceEntityConstructorKind, SourceExpression, SourceSpan, TrackDeclaration, TrackSegmentItem,
    Type, TypedValue,
};
use crate::schema::ConstructionSchema;

use super::entities::{definition_scope, function_map, require_static_type};
use super::eval::{evaluate_with_context_expected, infer_expression_with_expected};
use super::scope::{Binding, Scope};
use super::{CompileTimeContext, ElaboratorError as Diagnostic};

pub(super) fn expand_tracks(
    document: &Document,
    schema: &ConstructionSchema,
    context: CompileTimeContext,
) -> Result<Vec<ExpandedTrack>, Diagnostic> {
    validate_tracks(document, schema)?;
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

fn validate_tracks(document: &Document, schema: &ConstructionSchema) -> Result<(), Diagnostic> {
    let root = definition_scope(document.definitions.as_ref())?;
    let functions = function_map(document.definitions.as_ref());
    for line in &document.lines {
        for item in &line.items {
            let LineBodyItem::Tracks(block) = item else {
                continue;
            };
            for track in &block.tracks {
                validate_track_items(&track.segments.items, &root, &functions, schema, track)?;
            }
        }
    }
    Ok(())
}

fn validate_track_items(
    items: &[TrackSegmentItem],
    scope: &Scope,
    functions: &BTreeMap<String, &FunctionDeclaration>,
    schema: &ConstructionSchema,
    track: &TrackDeclaration,
) -> Result<(), Diagnostic> {
    for item in items {
        match item {
            TrackSegmentItem::DirectSegment(segment) => {
                validate_track_time(&segment.interval.start, scope, functions, schema)?;
                validate_track_time(&segment.interval.end, scope, functions, schema)?;
                validate_track_value(
                    &segment.start_value,
                    &track.value_type,
                    scope,
                    functions,
                    schema,
                )?;
                validate_track_value(
                    &segment.end_value,
                    &track.value_type,
                    scope,
                    functions,
                    schema,
                )?;
                validate_interpolation(&segment.interpolation, scope, functions, schema)?;
            }
            TrackSegmentItem::DirectPoint(point) => {
                validate_track_time(&point.time, scope, functions, schema)?;
                validate_track_value(&point.value, &track.value_type, scope, functions, schema)?;
            }
            TrackSegmentItem::Conditional {
                condition,
                then_items,
                else_items,
                ..
            } => {
                validate_track_value(condition, &Type::Bool, scope, functions, schema)?;
                validate_track_items(then_items, &scope.child(), functions, schema, track)?;
                validate_track_items(else_items, &scope.child(), functions, schema, track)?;
            }
            TrackSegmentItem::Generator(generator) => {
                validate_track_generator(generator, scope, functions, schema, track)?;
            }
        }
    }
    Ok(())
}

fn validate_track_time(
    expression: &SourceExpression,
    scope: &Scope,
    functions: &BTreeMap<String, &FunctionDeclaration>,
    schema: &ConstructionSchema,
) -> Result<(), Diagnostic> {
    let actual = infer_expression_with_expected(expression, scope, functions, schema, None)?;
    if matches!(actual, Type::Time | Type::Beat) {
        Ok(())
    } else {
        Err(Diagnostic::InvalidOperation {
            message: "Track time must be beat or time",
            span: expression.span(),
        })
    }
}

fn validate_track_value(
    expression: &SourceExpression,
    expected: &Type,
    scope: &Scope,
    functions: &BTreeMap<String, &FunctionDeclaration>,
    schema: &ConstructionSchema,
) -> Result<(), Diagnostic> {
    let actual =
        infer_expression_with_expected(expression, scope, functions, schema, Some(expected))?;
    require_static_type(expected, &actual, expression.span())
}

fn validate_interpolation(
    interpolation: &Interpolation,
    scope: &Scope,
    functions: &BTreeMap<String, &FunctionDeclaration>,
    schema: &ConstructionSchema,
) -> Result<(), Diagnostic> {
    match interpolation {
        Interpolation::Expression(expression) => {
            validate_track_value(expression, &Type::String, scope, functions, schema)
        }
        Interpolation::CubicBezier { values, .. } => {
            for value in values {
                validate_track_value(value, &Type::Float, scope, functions, schema)?;
            }
            Ok(())
        }
    }
}

fn validate_track_generator(
    generator: &Generator,
    initial_scope: &Scope,
    functions: &BTreeMap<String, &FunctionDeclaration>,
    schema: &ConstructionSchema,
    track: &TrackDeclaration,
) -> Result<(), Diagnostic> {
    if !matches!(generator.variable_type, Type::Int | Type::Beat) {
        return Err(Diagnostic::InvalidGeneratorRange {
            span: generator.range.span,
            message: "generator range values must match an int or beat variable",
        });
    }
    for expression in [
        &generator.range.start,
        &generator.range.end,
        &generator.range.step,
    ] {
        let actual =
            infer_expression_with_expected(expression, initial_scope, functions, schema, None)?;
        if actual != generator.variable_type {
            return Err(Diagnostic::InvalidGeneratorRange {
                span: generator.range.span,
                message: "generator range values must match an int or beat variable",
            });
        }
    }
    let mut scope = initial_scope.child();
    for (name, ty, span) in [
        ("index".to_owned(), Type::Int, generator.variable_span),
        (
            "range".to_owned(),
            Type::GeneratorRange(Box::new(generator.variable_type.clone())),
            generator.range.span,
        ),
        (
            generator.variable.clone(),
            generator.variable_type.clone(),
            generator.variable_span,
        ),
    ] {
        scope.declare(
            name,
            Binding {
                ty,
                value: None,
                span,
            },
        )?;
    }
    validate_track_generator_items(&generator.body, &scope, functions, schema, track)
}

fn validate_track_generator_items(
    items: &[GeneratorItem],
    initial_scope: &Scope,
    functions: &BTreeMap<String, &FunctionDeclaration>,
    schema: &ConstructionSchema,
    track: &TrackDeclaration,
) -> Result<(), Diagnostic> {
    let mut scope = initial_scope.clone();
    for item in items {
        match item {
            GeneratorItem::Let(statement) => {
                validate_track_value(
                    &statement.initializer,
                    &statement.ty,
                    &scope,
                    functions,
                    schema,
                )?;
                scope.declare(
                    statement.name.clone(),
                    Binding {
                        ty: statement.ty.clone(),
                        value: None,
                        span: statement.name_span,
                    },
                )?;
            }
            GeneratorItem::Conditional {
                condition,
                then_items,
                else_items,
                ..
            } => {
                validate_track_value(condition, &Type::Bool, &scope, functions, schema)?;
                validate_track_generator_items(
                    then_items,
                    &scope.child(),
                    functions,
                    schema,
                    track,
                )?;
                validate_track_generator_items(
                    else_items,
                    &scope.child(),
                    functions,
                    schema,
                    track,
                )?;
            }
            GeneratorItem::Emit(expression) => {
                validate_track_emit(expression, &scope, functions, schema, track)?;
            }
        }
    }
    Ok(())
}

fn validate_track_emit(
    expression: &EntityExpression,
    scope: &Scope,
    functions: &BTreeMap<String, &FunctionDeclaration>,
    schema: &ConstructionSchema,
    track: &TrackDeclaration,
) -> Result<(), Diagnostic> {
    let EntityExpression::SourceConstructor(constructor) = expression else {
        return Err(Diagnostic::CollectionTypeMismatch {
            collection: format!("Track<{}>", track.value_type),
            expected: Type::TrackSegment(Box::new(track.value_type.clone())),
            actual: emitted_type(expression),
            span: expression.span(),
        });
    };
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
                    .ok_or_else(|| Diagnostic::MissingRequiredField {
                        entity: entity.clone(),
                        field: "interpolation".to_owned(),
                        span: constructor.span,
                    })?;
            reject_unknown_fields(
                &fields,
                &["start", "end", "startValue", "endValue", "interpolation"],
                entity,
            )?;
            validate_track_time(start, scope, functions, schema)?;
            validate_track_time(end, scope, functions, schema)?;
            validate_track_value(start_value, &track.value_type, scope, functions, schema)?;
            validate_track_value(end_value, &track.value_type, scope, functions, schema)?;
            validate_schema_interpolation(&interpolation.value, scope, functions, schema)
        }
        SourceEntityConstructorKind::Keyframe => {
            let entity = Type::Keyframe(Box::new(track.value_type.clone()));
            let time = expression_field(&fields, "time", constructor.span, &entity)?;
            let value = expression_field(&fields, "value", constructor.span, &entity)?;
            reject_unknown_fields(&fields, &["time", "value"], entity)?;
            validate_track_time(time, scope, functions, schema)?;
            validate_track_value(value, &track.value_type, scope, functions, schema)
        }
        SourceEntityConstructorKind::RenderNode => Err(Diagnostic::CollectionTypeMismatch {
            collection: format!("Track<{}>", track.value_type),
            expected: Type::TrackSegment(Box::new(track.value_type.clone())),
            actual: Type::RenderNode,
            span: constructor.span,
        }),
    }
}

fn validate_schema_interpolation(
    value: &SchemaValue,
    scope: &Scope,
    functions: &BTreeMap<String, &FunctionDeclaration>,
    schema: &ConstructionSchema,
) -> Result<(), Diagnostic> {
    match value {
        SchemaValue::Expression(expression) => {
            validate_track_value(expression, &Type::String, scope, functions, schema)
        }
        SchemaValue::CubicBezier { values, .. } => {
            for value in values {
                validate_track_value(value, &Type::Float, scope, functions, schema)?;
            }
            Ok(())
        }
        SchemaValue::Interval { span, .. } => Err(Diagnostic::InvalidOperation {
            message: "Track interpolation cannot be an interval",
            span: *span,
        }),
    }
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
