use std::collections::{BTreeMap, BTreeSet};

use crate::v5::ast::{
    CollectionBlock, CollectionItem, Document, EntityConstructor, EntityExpression,
    ExpandedCollection, ExpandedEntity, ExpandedField, SourceExpression, SourceSpan,
    TemplateDeclaration, TemplatesBlock, Type, TypedValue, WithExpression,
};
use crate::v5::schema::{ConstructionSchema, EntitySchema, FieldConstraint};

use super::eval::evaluate_with_bindings;
use super::{CompileTimeLimits, Diagnostic};

pub(super) fn expand_collections(
    document: &Document,
    schema: &ConstructionSchema,
    limits: CompileTimeLimits,
) -> Result<Vec<ExpandedCollection>, Diagnostic> {
    let templates = template_map(document.templates.as_ref());
    let mut context = ExpansionContext {
        document,
        schema,
        templates,
        limits,
        template_instances: 0,
    };
    document
        .collections
        .iter()
        .map(|collection| context.expand_collection(collection))
        .collect()
}

fn template_map(block: Option<&TemplatesBlock>) -> BTreeMap<String, &TemplateDeclaration> {
    block
        .into_iter()
        .flat_map(|block| block.declarations.iter())
        .map(|template| (template.name.clone(), template))
        .collect()
}

struct ExpansionContext<'a> {
    document: &'a Document,
    schema: &'a ConstructionSchema,
    templates: BTreeMap<String, &'a TemplateDeclaration>,
    limits: CompileTimeLimits,
    template_instances: usize,
}

impl<'a> ExpansionContext<'a> {
    fn expand_collection(
        &mut self,
        collection: &CollectionBlock,
    ) -> Result<ExpandedCollection, Diagnostic> {
        let collection_schema = self
            .schema
            .collection(&collection.collection_name)
            .ok_or_else(|| Diagnostic::UnknownCollection {
                name: collection.collection_name.clone(),
                span: collection.span,
            })?;
        let mut entities = Vec::new();
        for item in &collection.items {
            self.expand_item(
                item,
                &collection_schema.emitted_entity_type,
                &collection.collection_name,
                &mut entities,
            )?;
        }
        Ok(ExpandedCollection::new(
            collection.collection_name.clone(),
            entities,
        ))
    }

    fn expand_item(
        &mut self,
        item: &CollectionItem,
        expected_type: &Type,
        collection_name: &str,
        output: &mut Vec<ExpandedEntity>,
    ) -> Result<(), Diagnostic> {
        match item {
            CollectionItem::Conditional {
                condition,
                then_items,
                else_items,
                span,
            } => {
                let value = match evaluate_with_bindings(
                    condition,
                    self.document.definitions.as_ref(),
                    &BTreeMap::new(),
                    self.limits,
                ) {
                    Ok(value) => value,
                    Err(Diagnostic::UnknownName { .. }) => {
                        return Err(Diagnostic::NonConstantStructuralCondition { span: *span });
                    }
                    Err(error) => return Err(error),
                };
                let TypedValue::Bool(selected) = value else {
                    return Err(Diagnostic::NonConstantStructuralCondition { span: *span });
                };
                let branch = if selected { then_items } else { else_items };
                for item in branch {
                    self.expand_item(item, expected_type, collection_name, output)?;
                }
            }
            CollectionItem::Constructor(constructor) => {
                let entity = self.expand_expression(
                    &EntityExpression::Constructor(constructor.clone()),
                    expected_type,
                    &BTreeMap::new(),
                    &mut Vec::new(),
                    0,
                )?;
                self.push_collection_entity(entity, expected_type, collection_name, output)?;
            }
            CollectionItem::Expression(expression) => {
                let entity = self.expand_expression(
                    expression,
                    expected_type,
                    &BTreeMap::new(),
                    &mut Vec::new(),
                    0,
                )?;
                self.push_collection_entity(entity, expected_type, collection_name, output)?;
            }
            CollectionItem::Generator(generator) => {
                return Err(Diagnostic::FeatureUnavailable {
                    feature: "compile-time-generator",
                    span: generator.span,
                });
            }
        }
        Ok(())
    }

    fn push_collection_entity(
        &self,
        entity: ExpandedEntity,
        expected_type: &Type,
        collection_name: &str,
        output: &mut Vec<ExpandedEntity>,
    ) -> Result<(), Diagnostic> {
        if entity.entity_type() != expected_type {
            return Err(Diagnostic::CollectionTypeMismatch {
                collection: collection_name.to_owned(),
                expected: expected_type.clone(),
                actual: entity.entity_type().clone(),
                span: entity.span(),
            });
        }
        output.push(entity);
        Ok(())
    }

    fn expand_expression(
        &mut self,
        expression: &EntityExpression,
        expected_type: &Type,
        bindings: &BTreeMap<String, TypedValue>,
        stack: &mut Vec<String>,
        depth: usize,
    ) -> Result<ExpandedEntity, Diagnostic> {
        if depth > self.limits.max_expansion_depth {
            return Err(Diagnostic::LimitExceeded {
                limit: "max_expansion_depth",
                span: expression.span(),
            });
        }
        match expression {
            EntityExpression::Constructor(constructor) => {
                self.expand_constructor(constructor, expected_type, bindings)
            }
            EntityExpression::Source(SourceExpression::Call {
                callee,
                arguments,
                span,
            }) => {
                let SourceExpression::Name { name, .. } = callee.as_ref() else {
                    return Err(Diagnostic::InvalidOperation {
                        message: "entity template call must use a name",
                        span: *span,
                    });
                };
                self.expand_template_call(
                    name,
                    arguments,
                    *span,
                    expected_type,
                    bindings,
                    stack,
                    depth,
                )
            }
            EntityExpression::Source(expression) => Err(Diagnostic::InvalidOperation {
                message: "entity expression must be a constructor or template call",
                span: expression.span(),
            }),
            EntityExpression::With(with_expression) => {
                self.expand_with(with_expression, expected_type, bindings, stack, depth)
            }
        }
    }

    fn expand_constructor(
        &mut self,
        constructor: &EntityConstructor,
        expected_type: &Type,
        bindings: &BTreeMap<String, TypedValue>,
    ) -> Result<ExpandedEntity, Diagnostic> {
        if &constructor.entity_type != expected_type {
            return Err(Diagnostic::CollectionTypeMismatch {
                collection: expected_type.to_string(),
                expected: expected_type.clone(),
                actual: constructor.entity_type.clone(),
                span: constructor.span,
            });
        }
        let schema = self
            .schema
            .entity(&constructor.entity_type)
            .ok_or_else(|| Diagnostic::NonConstructibleEntity {
                entity: constructor.entity_type.clone(),
                span: constructor.span,
            })?;
        if let Some(variant) = constructor.note_variant
            && !schema
                .note_variants()
                .is_some_and(|variants| variants.contains(&variant))
        {
            return Err(Diagnostic::NonConstructibleEntity {
                entity: constructor.entity_type.clone(),
                span: constructor.span,
            });
        }
        let fields = self.evaluate_fields(&constructor.fields, schema, bindings)?;
        self.require_fields(schema, &fields, constructor.span)?;
        Ok(ExpandedEntity::new(
            constructor.entity_type.clone(),
            constructor.note_variant,
            fields,
            constructor.span,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn expand_template_call(
        &mut self,
        name: &str,
        arguments: &[SourceExpression],
        span: SourceSpan,
        expected_type: &Type,
        bindings: &BTreeMap<String, TypedValue>,
        stack: &mut Vec<String>,
        depth: usize,
    ) -> Result<ExpandedEntity, Diagnostic> {
        let template = *self
            .templates
            .get(name)
            .ok_or_else(|| Diagnostic::UnknownTemplate {
                name: name.to_owned(),
                span,
            })?;
        if stack.iter().any(|entry| entry == name) {
            let start = stack.iter().position(|entry| entry == name).unwrap_or(0);
            let mut chain = stack[start..].to_vec();
            chain.push(name.to_owned());
            return Err(Diagnostic::RecursiveTemplate { chain, span });
        }
        if arguments.len() != template.parameters.len() {
            return Err(Diagnostic::WrongArity {
                callee: name.to_owned(),
                expected: template.parameters.len(),
                actual: arguments.len(),
                span,
            });
        }
        if &template.return_type != expected_type {
            return Err(Diagnostic::TypeMismatch {
                expected: expected_type.clone(),
                actual: template.return_type.clone(),
                span,
            });
        }
        self.template_instances = self.template_instances.saturating_add(1);
        if self.template_instances > self.limits.max_template_instances {
            return Err(Diagnostic::LimitExceeded {
                limit: "max_template_instances",
                span,
            });
        }
        let mut local_bindings = bindings.clone();
        for (argument, parameter) in arguments.iter().zip(&template.parameters) {
            let value = evaluate_with_bindings(
                argument,
                self.document.definitions.as_ref(),
                bindings,
                self.limits,
            )?;
            if value.ty() != parameter.ty {
                return Err(Diagnostic::TypeMismatch {
                    expected: parameter.ty.clone(),
                    actual: value.ty(),
                    span: argument.span(),
                });
            }
            local_bindings.insert(parameter.name.clone(), value);
        }
        stack.push(name.to_owned());
        let result = self.expand_expression(
            &template.body,
            &template.return_type,
            &local_bindings,
            stack,
            depth + 1,
        );
        stack.pop();
        result
    }

    fn expand_with(
        &mut self,
        expression: &WithExpression,
        expected_type: &Type,
        bindings: &BTreeMap<String, TypedValue>,
        stack: &mut Vec<String>,
        depth: usize,
    ) -> Result<ExpandedEntity, Diagnostic> {
        let mut entity =
            self.expand_expression(&expression.base, expected_type, bindings, stack, depth)?;
        let schema = self.schema.entity(expected_type).ok_or_else(|| {
            Diagnostic::NonConstructibleEntity {
                entity: expected_type.clone(),
                span: expression.span,
            }
        })?;
        let mut seen = BTreeSet::new();
        for field in &expression.fields {
            let path = field.path.segments.join(".");
            if !seen.insert(path.clone()) {
                return Err(Diagnostic::DuplicateEntityField {
                    field: path,
                    span: field.span,
                    previous_span: field.span,
                });
            }
            let field_schema =
                schema
                    .field(&path)
                    .ok_or_else(|| Diagnostic::UnknownEntityField {
                        entity: expected_type.clone(),
                        field: path.clone(),
                        span: field.path.span,
                    })?;
            let value = evaluate_with_bindings(
                &field.value,
                self.document.definitions.as_ref(),
                bindings,
                self.limits,
            )?;
            validate_field_type(field_schema, &value, field.value.span())?;
            entity.replace_field(ExpandedField::new(path, value, field.span));
        }
        self.require_entity_fields(schema, &entity, expression.span)?;
        Ok(entity)
    }

    fn evaluate_fields(
        &self,
        fields: &[crate::v5::ast::EntityField],
        schema: &EntitySchema,
        bindings: &BTreeMap<String, TypedValue>,
    ) -> Result<BTreeMap<String, ExpandedField>, Diagnostic> {
        let mut result: BTreeMap<String, ExpandedField> = BTreeMap::new();
        for field in fields {
            let path = field.path.segments.join(".");
            if let Some(previous) = result.get(&path) {
                return Err(Diagnostic::DuplicateEntityField {
                    field: path,
                    span: field.span,
                    previous_span: previous.span(),
                });
            }
            let field_schema =
                schema
                    .field(&path)
                    .ok_or_else(|| Diagnostic::UnknownEntityField {
                        entity: schema.entity_type.clone(),
                        field: path.clone(),
                        span: field.path.span,
                    })?;
            let value = evaluate_with_bindings(
                &field.value,
                self.document.definitions.as_ref(),
                bindings,
                self.limits,
            )?;
            validate_field_type(field_schema, &value, field.value.span())?;
            result.insert(path.clone(), ExpandedField::new(path, value, field.span));
        }
        Ok(result)
    }

    fn require_fields(
        &self,
        schema: &EntitySchema,
        fields: &BTreeMap<String, ExpandedField>,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        if let Some(field) = schema
            .fields()
            .find(|field| field.required && !fields.contains_key(&field.path))
        {
            return Err(Diagnostic::MissingRequiredField {
                entity: schema.entity_type.clone(),
                field: field.path.clone(),
                span,
            });
        }
        Ok(())
    }

    fn require_entity_fields(
        &self,
        schema: &EntitySchema,
        entity: &ExpandedEntity,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        if let Some(field) = schema
            .fields()
            .find(|field| field.required && !entity.has_field(&field.path))
        {
            return Err(Diagnostic::MissingRequiredField {
                entity: schema.entity_type.clone(),
                field: field.path.clone(),
                span,
            });
        }
        Ok(())
    }
}

fn validate_field_type(
    field: &crate::v5::schema::FieldSchema,
    value: &TypedValue,
    span: SourceSpan,
) -> Result<(), Diagnostic> {
    if value.ty() != field.ty {
        return Err(Diagnostic::TypeMismatch {
            expected: field.ty.clone(),
            actual: value.ty(),
            span,
        });
    }
    if let Some(FieldConstraint::StringEnum(values)) = field.constraint()
        && let TypedValue::String(value) = value
        && !values.contains(&value.as_str())
    {
        return Err(Diagnostic::InvalidOperation {
            message: "string value is outside the schema enum",
            span,
        });
    }
    Ok(())
}
