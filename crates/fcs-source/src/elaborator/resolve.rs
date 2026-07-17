//! Deterministic lexical name resolution for compile-time declarations.

use crate::ast::{
    CollectionItem, Definition, DefinitionsBlock, Document, EntityExpression, FunctionDeclaration,
    FunctionStatement, Generator, GeneratorItem, LineBodyItem, SourceExpression, SourceSpan,
    TemplateDeclaration, TemplateStatement, Type,
};

use super::ElaboratorError as Diagnostic;
use super::scope::{Binding, Scope};

pub(super) fn check_document(document: &Document) -> Result<(), Diagnostic> {
    let root = definitions_scope(document.definitions.as_ref())?;
    if let Some(definitions) = &document.definitions {
        for definition in &definitions.declarations {
            check_definition(definition, &root)?;
        }
    }
    check_collections(document, &root)?;
    check_line_generators(document, &root)
}

fn definitions_scope(definitions: Option<&DefinitionsBlock>) -> Result<Scope, Diagnostic> {
    let mut root = Scope::root_with_builtins()?;
    let Some(definitions) = definitions else {
        return Ok(root);
    };
    for definition in &definitions.declarations {
        match definition {
            Definition::Const(declaration) => {
                root.declare(
                    declaration.name.clone(),
                    Binding {
                        ty: declaration.ty.clone(),
                        value: None,
                        span: declaration.name_span,
                    },
                )?;
            }
            Definition::Function(declaration) => {
                root.reserve(declaration.name.clone(), declaration.name_span);
            }
            Definition::Template(declaration) => {
                root.reserve(declaration.name.clone(), declaration.name_span);
            }
        }
    }
    Ok(root)
}

fn check_definition(definition: &Definition, root: &Scope) -> Result<(), Diagnostic> {
    match definition {
        Definition::Const(declaration) => resolve_expression(&declaration.initializer, root),
        Definition::Function(declaration) => check_function(declaration, root),
        Definition::Template(declaration) => check_template(declaration, root),
    }
}

fn check_function(declaration: &FunctionDeclaration, root: &Scope) -> Result<(), Diagnostic> {
    let mut scope = root.child();
    for parameter in &declaration.parameters {
        declare_local(
            &mut scope,
            parameter.name.clone(),
            parameter.ty.clone(),
            parameter.name_span,
        )?;
    }
    check_function_block(&declaration.body, &scope)
}

fn check_function_block(
    statements: &[FunctionStatement],
    initial_scope: &Scope,
) -> Result<(), Diagnostic> {
    let mut scope = initial_scope.clone();
    for statement in statements {
        match statement {
            FunctionStatement::Let(statement) => {
                resolve_expression(&statement.initializer, &scope)?;
                declare_local(
                    &mut scope,
                    statement.name.clone(),
                    statement.ty.clone(),
                    statement.name_span,
                )?;
            }
            FunctionStatement::Return(statement) => {
                resolve_expression(&statement.value, &scope)?;
            }
            FunctionStatement::If(statement) => {
                resolve_expression(&statement.condition, &scope)?;
                check_function_block(&statement.then_branch, &scope.child())?;
                check_function_block(&statement.else_branch, &scope.child())?;
            }
        }
    }
    Ok(())
}

fn check_template(declaration: &TemplateDeclaration, root: &Scope) -> Result<(), Diagnostic> {
    let mut scope = root.child();
    for parameter in &declaration.parameters {
        declare_local(
            &mut scope,
            parameter.name.clone(),
            parameter.ty.clone(),
            parameter.name_span,
        )?;
    }
    check_template_block(&declaration.body, &scope)
}

fn check_template_block(
    statements: &[TemplateStatement],
    initial_scope: &Scope,
) -> Result<(), Diagnostic> {
    let mut scope = initial_scope.clone();
    for statement in statements {
        match statement {
            TemplateStatement::Let(statement) => {
                resolve_expression(&statement.initializer, &scope)?;
                declare_local(
                    &mut scope,
                    statement.name.clone(),
                    statement.ty.clone(),
                    statement.name_span,
                )?;
            }
            TemplateStatement::If(statement) => {
                resolve_expression(&statement.condition, &scope)?;
                check_template_block(&statement.then_branch, &scope.child())?;
                check_template_block(&statement.else_branch, &scope.child())?;
            }
            TemplateStatement::Return(statement) => {
                resolve_entity_expression(&statement.value, &scope)?;
            }
        }
    }
    Ok(())
}

fn check_collections(document: &Document, root: &Scope) -> Result<(), Diagnostic> {
    for collection in &document.collections {
        check_collection_items(&collection.items, root)?;
    }
    Ok(())
}

fn check_collection_items(items: &[CollectionItem], scope: &Scope) -> Result<(), Diagnostic> {
    for item in items {
        match item {
            CollectionItem::Constructor(constructor) => {
                resolve_entity_fields(&constructor.fields, scope)?;
            }
            CollectionItem::Expression(expression) => {
                resolve_entity_expression(expression, scope)?;
            }
            CollectionItem::Conditional {
                condition,
                then_items,
                else_items,
                ..
            } => {
                resolve_optional_expression(condition, scope)?;
                check_collection_items(then_items, scope)?;
                check_collection_items(else_items, scope)?;
            }
            CollectionItem::Generator(generator) => check_generator(generator, scope)?,
        }
    }
    Ok(())
}

fn check_generator(generator: &Generator, scope: &Scope) -> Result<(), Diagnostic> {
    resolve_expression(&generator.range.start, scope)?;
    resolve_expression(&generator.range.end, scope)?;
    resolve_expression(&generator.range.step, scope)?;
    let mut generator_scope = scope.child();
    declare_local(
        &mut generator_scope,
        "index".to_owned(),
        Type::Int,
        generator.variable_span,
    )?;
    declare_local(
        &mut generator_scope,
        "range".to_owned(),
        Type::GeneratorRange(Box::new(generator.variable_type.clone())),
        generator.range.span,
    )?;
    declare_local(
        &mut generator_scope,
        generator.variable.clone(),
        generator.variable_type.clone(),
        generator.variable_span,
    )?;
    check_generator_items(&generator.body, &generator_scope)
}

fn check_generator_items(items: &[GeneratorItem], initial_scope: &Scope) -> Result<(), Diagnostic> {
    let mut scope = initial_scope.clone();
    for item in items {
        match item {
            GeneratorItem::Let(statement) => {
                resolve_expression(&statement.initializer, &scope)?;
                declare_local(
                    &mut scope,
                    statement.name.clone(),
                    statement.ty.clone(),
                    statement.name_span,
                )?;
            }
            GeneratorItem::Conditional {
                condition,
                then_items,
                else_items,
                ..
            } => {
                resolve_expression(condition, &scope)?;
                check_generator_items(then_items, &scope.child())?;
                check_generator_items(else_items, &scope.child())?;
            }
            GeneratorItem::Emit(expression) => {
                resolve_entity_expression(expression, &scope)?;
            }
        }
    }
    Ok(())
}

fn check_line_generators(document: &Document, root: &Scope) -> Result<(), Diagnostic> {
    // Track field expressions remain runtime/canonical inputs. Only generator-owned
    // ranges, bindings, and structural conditions belong to this name pass.
    for line in &document.lines {
        for item in &line.items {
            let LineBodyItem::Tracks(tracks) = item else {
                continue;
            };
            for track in &tracks.tracks {
                check_track_items(&track.segments.items, root)?;
            }
        }
    }
    Ok(())
}

fn check_track_items(
    items: &[crate::ast::TrackSegmentItem],
    scope: &Scope,
) -> Result<(), Diagnostic> {
    for item in items {
        match item {
            crate::ast::TrackSegmentItem::Generator(generator) => {
                check_generator(generator, scope)?;
            }
            crate::ast::TrackSegmentItem::Conditional {
                condition,
                then_items,
                else_items,
                ..
            } => {
                resolve_expression(condition, scope)?;
                check_track_items(then_items, scope)?;
                check_track_items(else_items, scope)?;
            }
            crate::ast::TrackSegmentItem::DirectSegment(_)
            | crate::ast::TrackSegmentItem::DirectPoint(_) => {}
        }
    }
    Ok(())
}

fn declare_local(
    scope: &mut Scope,
    name: String,
    ty: Type,
    span: SourceSpan,
) -> Result<(), Diagnostic> {
    scope.declare(
        name,
        Binding {
            ty,
            value: None,
            span,
        },
    )
}

fn resolve_expression(expression: &SourceExpression, scope: &Scope) -> Result<(), Diagnostic> {
    match expression {
        SourceExpression::Literal { .. }
        | SourceExpression::Object { .. }
        | SourceExpression::Reference { .. }
        | SourceExpression::Choose { .. } => Ok(()),
        SourceExpression::Name { name, span } => {
            scope
                .contains(name)
                .then_some(())
                .ok_or_else(|| Diagnostic::UnknownName {
                    name: name.clone(),
                    span: *span,
                })
        }
        SourceExpression::Unary { operand, .. } => resolve_expression(operand, scope),
        SourceExpression::Binary { left, right, .. } => {
            resolve_expression(left, scope)?;
            resolve_expression(right, scope)
        }
        SourceExpression::Call {
            callee, arguments, ..
        } => {
            resolve_expression(callee, scope)?;
            for argument in arguments {
                resolve_expression(argument, scope)?;
            }
            Ok(())
        }
        SourceExpression::FieldAccess { base, .. } => resolve_expression(base, scope),
        SourceExpression::Index { base, index, .. } => {
            resolve_expression(base, scope)?;
            resolve_expression(index, scope)
        }
        SourceExpression::Vec2 { x, y, .. } => {
            resolve_expression(x, scope)?;
            resolve_expression(y, scope)
        }
        SourceExpression::Array { elements, .. } => {
            for element in elements {
                resolve_expression(element, scope)?;
            }
            Ok(())
        }
    }
}

fn resolve_entity_expression(
    expression: &EntityExpression,
    scope: &Scope,
) -> Result<(), Diagnostic> {
    match expression {
        EntityExpression::Constructor(constructor) => {
            resolve_entity_fields(&constructor.fields, scope)
        }
        EntityExpression::SourceConstructor(constructor) => {
            for field in &constructor.fields {
                if let crate::ast::SchemaValue::Expression(expression) = &field.value {
                    resolve_optional_expression(expression, scope)?;
                }
            }
            Ok(())
        }
        EntityExpression::Source(expression) => match expression {
            SourceExpression::Call { .. } => resolve_expression(expression, scope),
            _ => resolve_optional_expression(expression, scope),
        },
        EntityExpression::With(expression) => {
            resolve_entity_expression(&expression.base, scope)?;
            resolve_entity_fields(&expression.fields, scope)
        }
    }
}

fn resolve_entity_fields(
    fields: &[crate::ast::EntityField],
    scope: &Scope,
) -> Result<(), Diagnostic> {
    for field in fields {
        resolve_optional_expression(&field.value, scope)?;
    }
    Ok(())
}

fn resolve_optional_expression(
    expression: &SourceExpression,
    scope: &Scope,
) -> Result<(), Diagnostic> {
    // Entity field values may be runtime expressions or schema-owned values. Their
    // dynamic-field and environment rules belong to the later schema/runtime seam;
    // known compile-time bindings are still traversed, but an unresolved field name
    // must not be reclassified here.
    match resolve_expression(expression, scope) {
        Err(Diagnostic::UnknownName { .. }) => Ok(()),
        result => result,
    }
}
