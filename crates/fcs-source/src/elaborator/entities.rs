use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{
    CollectionBlock, CollectionItem, Definition, DefinitionsBlock, Document, EntityConstructor,
    EntityExpression, ExpandedCollection, ExpandedEntity, ExpandedField, SourceExpression,
    SourceLiteral, SourceSpan, TemplateDeclaration, TemplateStatement, Type, TypedValue,
    WithExpression,
};
use crate::diagnostic::ExpansionTraceKind;
use crate::schema::{ConstructionSchema, EntitySchema, FieldConstraint};

use super::eval::{evaluate_with_bindings, infer_expression};
use super::scope::{Binding, Scope};
use super::{CompileTimeLimits, DependencyTraceNode, ElaboratorError as Diagnostic};

pub(super) fn validate_static_entities(
    document: &Document,
    schema: &ConstructionSchema,
) -> Result<(), Diagnostic> {
    let templates = template_map(document.definitions.as_ref());
    let functions = function_map(document.definitions.as_ref());
    let root = definition_scope(document.definitions.as_ref())?;
    let line_names = document
        .lines
        .iter()
        .map(|line| line.name.clone())
        .collect::<BTreeSet<_>>();
    let validator = StaticEntityValidator {
        document,
        schema,
        templates,
        functions,
        root,
        line_names,
    };
    validator.validate_templates(document.definitions.as_ref())?;
    validator.validate_collections()
}

fn function_map(
    block: Option<&DefinitionsBlock>,
) -> BTreeMap<String, &crate::ast::FunctionDeclaration> {
    block
        .into_iter()
        .flat_map(|block| block.declarations.iter())
        .filter_map(|definition| match definition {
            Definition::Function(function) => Some((function.name.clone(), function)),
            Definition::Const(_) | Definition::Template(_) => None,
        })
        .collect()
}

fn definition_scope(block: Option<&DefinitionsBlock>) -> Result<Scope, Diagnostic> {
    let mut root = Scope::root_with_builtins()?;
    let Some(block) = block else {
        return Ok(root);
    };
    for definition in &block.declarations {
        match definition {
            Definition::Const(constant) => root.declare(
                constant.name.clone(),
                Binding {
                    ty: constant.ty.clone(),
                    value: None,
                    span: constant.name_span,
                },
            )?,
            Definition::Function(function) => {
                root.reserve(function.name.clone(), function.name_span);
            }
            Definition::Template(template) => {
                root.reserve(template.name.clone(), template.name_span);
            }
        }
    }
    Ok(root)
}

struct StaticEntityValidator<'a> {
    document: &'a Document,
    schema: &'a ConstructionSchema,
    templates: BTreeMap<String, &'a TemplateDeclaration>,
    functions: BTreeMap<String, &'a crate::ast::FunctionDeclaration>,
    root: Scope,
    line_names: BTreeSet<String>,
}

impl<'a> StaticEntityValidator<'a> {
    fn validate_templates(&self, definitions: Option<&DefinitionsBlock>) -> Result<(), Diagnostic> {
        let Some(definitions) = definitions else {
            return Ok(());
        };
        for definition in &definitions.declarations {
            let Definition::Template(template) = definition else {
                continue;
            };
            let Some(template_schema) = self.schema.entity(&template.return_type) else {
                return Err(Diagnostic::NonConstructibleEntity {
                    entity: template.return_type.clone(),
                    span: template.span,
                });
            };
            let mut scope = self.root.child();
            for parameter in &template.parameters {
                scope.declare(
                    parameter.name.clone(),
                    Binding {
                        ty: parameter.ty.clone(),
                        value: None,
                        span: parameter.name_span,
                    },
                )?;
            }
            if !self.validate_template_block(
                &template.body,
                &scope,
                &template.return_type,
                template_schema,
            )? {
                return Err(Diagnostic::MissingReturn {
                    function: template.name.clone(),
                    span: template.span,
                });
            }
        }
        Ok(())
    }

    fn validate_template_block(
        &self,
        statements: &[TemplateStatement],
        initial_scope: &Scope,
        return_type: &Type,
        schema: &EntitySchema,
    ) -> Result<bool, Diagnostic> {
        let mut scope = initial_scope.clone();
        for statement in statements {
            match statement {
                TemplateStatement::Let(statement) => {
                    let actual = self.validate_expression(&statement.initializer, &scope)?;
                    require_static_type(&statement.ty, &actual, statement.initializer.span())?;
                    scope.declare(
                        statement.name.clone(),
                        Binding {
                            ty: statement.ty.clone(),
                            value: None,
                            span: statement.name_span,
                        },
                    )?;
                }
                TemplateStatement::If(statement) => {
                    let condition = self.validate_expression(&statement.condition, &scope)?;
                    require_static_type(&Type::Bool, &condition, statement.condition.span())?;
                    let then_returns = self.validate_template_block(
                        &statement.then_branch,
                        &scope.child(),
                        return_type,
                        schema,
                    )?;
                    let else_returns = self.validate_template_block(
                        &statement.else_branch,
                        &scope.child(),
                        return_type,
                        schema,
                    )?;
                    if then_returns && else_returns {
                        return Ok(true);
                    }
                }
                TemplateStatement::Return(statement) => {
                    self.validate_entity_expression(
                        &statement.value,
                        return_type,
                        &scope,
                        schema,
                        true,
                    )?;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn validate_collections(&self) -> Result<(), Diagnostic> {
        for collection in &self.document.collections {
            let Some(collection_schema) = self.schema.collection(&collection.collection_name)
            else {
                return Err(Diagnostic::UnknownCollection {
                    name: collection.collection_name.clone(),
                    span: collection.span,
                });
            };
            self.validate_collection_items(
                &collection.items,
                &collection_schema.emitted_entity_type,
            )?;
        }
        Ok(())
    }

    fn validate_collection_items(
        &self,
        items: &[CollectionItem],
        expected_type: &Type,
    ) -> Result<(), Diagnostic> {
        for item in items {
            match item {
                CollectionItem::Constructor(constructor) => {
                    self.validate_entity_expression(
                        &EntityExpression::Constructor(constructor.clone()),
                        expected_type,
                        &self.root,
                        self.schema.entity(expected_type).ok_or_else(|| {
                            Diagnostic::NonConstructibleEntity {
                                entity: expected_type.clone(),
                                span: constructor.span,
                            }
                        })?,
                        false,
                    )?;
                }
                CollectionItem::Expression(expression) => {
                    self.validate_entity_expression(
                        expression,
                        expected_type,
                        &self.root,
                        self.schema.entity(expected_type).ok_or_else(|| {
                            Diagnostic::NonConstructibleEntity {
                                entity: expected_type.clone(),
                                span: expression.span(),
                            }
                        })?,
                        false,
                    )?;
                }
                CollectionItem::Conditional {
                    then_items,
                    else_items,
                    ..
                } => {
                    self.validate_collection_items(then_items, expected_type)?;
                    self.validate_collection_items(else_items, expected_type)?;
                }
                CollectionItem::Generator(_) => {}
            }
        }
        Ok(())
    }

    fn validate_entity_expression(
        &self,
        expression: &EntityExpression,
        expected_type: &Type,
        scope: &Scope,
        schema: &EntitySchema,
        require_template_fields: bool,
    ) -> Result<(), Diagnostic> {
        match expression {
            EntityExpression::Constructor(constructor) => {
                if &constructor.entity_type != expected_type {
                    return Err(Diagnostic::CollectionTypeMismatch {
                        collection: expected_type.to_string(),
                        expected: expected_type.clone(),
                        actual: constructor.entity_type.clone(),
                        span: constructor.span,
                    });
                }
                self.validate_constructor(constructor, scope, schema, require_template_fields)
            }
            EntityExpression::SourceConstructor(constructor) => {
                let _ = (constructor, expected_type);
                Ok(())
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
                let template =
                    self.templates
                        .get(name)
                        .ok_or_else(|| Diagnostic::UnknownTemplate {
                            name: name.clone(),
                            span: *span,
                        })?;
                if arguments.len() != template.parameters.len() {
                    return Err(Diagnostic::WrongArity {
                        callee: name.clone(),
                        expected: template.parameters.len(),
                        actual: arguments.len(),
                        span: *span,
                    });
                }
                for (argument, parameter) in arguments.iter().zip(&template.parameters) {
                    let actual = self.validate_expression(argument, scope)?;
                    require_static_type(&parameter.ty, &actual, argument.span())?;
                }
                require_static_type(expected_type, &template.return_type, *span)
            }
            EntityExpression::Source(expression) => Err(Diagnostic::InvalidOperation {
                message: "entity expression must be a constructor or template call",
                span: expression.span(),
            }),
            EntityExpression::With(with_expression) => {
                self.validate_entity_expression(
                    &with_expression.base,
                    expected_type,
                    scope,
                    schema,
                    require_template_fields,
                )?;
                self.validate_fields(
                    &with_expression.fields,
                    scope,
                    schema,
                    with_expression.span,
                    false,
                    false,
                )
            }
        }
    }

    fn validate_constructor(
        &self,
        constructor: &EntityConstructor,
        scope: &Scope,
        schema: &EntitySchema,
        require_template_fields: bool,
    ) -> Result<(), Diagnostic> {
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
        self.validate_fields(
            &constructor.fields,
            scope,
            schema,
            constructor.span,
            true,
            require_template_fields,
        )
    }

    fn validate_fields(
        &self,
        fields: &[crate::ast::EntityField],
        scope: &Scope,
        schema: &EntitySchema,
        span: SourceSpan,
        require_required: bool,
        require_template_line: bool,
    ) -> Result<(), Diagnostic> {
        let mut seen = BTreeMap::<String, SourceSpan>::new();
        for field in fields {
            let path = field.path.segments.join(".");
            if let Some(previous_span) = seen.insert(path.clone(), field.span) {
                return Err(Diagnostic::DuplicateEntityField {
                    field: path,
                    span: field.span,
                    previous_span,
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
            if is_structural_field(&path) && contains_runtime_choose(&field.value) {
                return Err(Diagnostic::DynamicFieldForbidden {
                    field: path,
                    span: field.value.span(),
                });
            }
            let actual = self.validate_expression(&field.value, scope)?;
            require_static_type(&field_schema.ty, &actual, field.value.span())?;
            if let Ok(value) = evaluate_with_bindings(
                &field.value,
                self.document.definitions.as_ref(),
                &BTreeMap::new(),
                CompileTimeLimits::default(),
            ) {
                validate_field_type(field_schema, &value, field.value.span())?;
            }
            if let Some(FieldConstraint::StringEnum(values)) = field_schema.constraint()
                && let SourceExpression::Literal {
                    literal: SourceLiteral::String(value),
                    span,
                } = &field.value
                && !values.contains(&value.as_str())
            {
                return Err(Diagnostic::InvalidOperation {
                    message: "string value is outside the schema enum",
                    span: *span,
                });
            }
        }
        if require_required
            && let Some(required) = schema
                .fields()
                .find(|field| field.required && !seen.contains_key(&field.path))
        {
            return Err(Diagnostic::MissingRequiredField {
                entity: schema.entity_type.clone(),
                field: required.path.clone(),
                span,
            });
        }
        if require_template_line && schema.entity_type == Type::Note && !seen.contains_key("line") {
            return Err(Diagnostic::MissingRequiredField {
                entity: schema.entity_type.clone(),
                field: "line".to_owned(),
                span,
            });
        }
        Ok(())
    }

    fn validate_expression(
        &self,
        expression: &SourceExpression,
        scope: &Scope,
    ) -> Result<Type, Diagnostic> {
        validate_line_references(expression, &self.line_names)?;
        infer_expression(expression, scope, &self.functions)
    }
}

fn require_static_type(expected: &Type, actual: &Type, span: SourceSpan) -> Result<(), Diagnostic> {
    if expected == actual {
        Ok(())
    } else {
        Err(Diagnostic::TypeMismatch {
            expected: expected.clone(),
            actual: actual.clone(),
            span,
        })
    }
}

fn validate_line_references(
    expression: &SourceExpression,
    line_names: &BTreeSet<String>,
) -> Result<(), Diagnostic> {
    match expression {
        SourceExpression::Reference { name, span } => {
            if line_names.contains(name) {
                Ok(())
            } else {
                Err(Diagnostic::UnknownName {
                    name: name.clone(),
                    span: *span,
                })
            }
        }
        SourceExpression::Unary { operand, .. }
        | SourceExpression::FieldAccess { base: operand, .. } => {
            validate_line_references(operand, line_names)
        }
        SourceExpression::Binary { left, right, .. }
        | SourceExpression::Vec2 {
            x: left, y: right, ..
        } => {
            validate_line_references(left, line_names)?;
            validate_line_references(right, line_names)
        }
        SourceExpression::Call {
            callee, arguments, ..
        } => {
            validate_line_references(callee, line_names)?;
            for argument in arguments {
                validate_line_references(argument, line_names)?;
            }
            Ok(())
        }
        SourceExpression::Array { elements, .. } => {
            for element in elements {
                validate_line_references(element, line_names)?;
            }
            Ok(())
        }
        SourceExpression::Object { entries, .. } => {
            for entry in entries {
                validate_line_references(&entry.value, line_names)?;
            }
            Ok(())
        }
        SourceExpression::Index { base, index, .. } => {
            validate_line_references(base, line_names)?;
            validate_line_references(index, line_names)
        }
        SourceExpression::Choose {
            arms, else_value, ..
        } => {
            for arm in arms {
                validate_line_references(&arm.condition, line_names)?;
                validate_line_references(&arm.value, line_names)?;
            }
            validate_line_references(else_value, line_names)
        }
        SourceExpression::Literal { .. } | SourceExpression::Name { .. } => Ok(()),
    }
}

fn contains_runtime_choose(expression: &SourceExpression) -> bool {
    match expression {
        SourceExpression::Choose { .. } => true,
        SourceExpression::Unary { operand, .. }
        | SourceExpression::FieldAccess { base: operand, .. } => contains_runtime_choose(operand),
        SourceExpression::Binary { left, right, .. }
        | SourceExpression::Vec2 {
            x: left, y: right, ..
        } => contains_runtime_choose(left) || contains_runtime_choose(right),
        SourceExpression::Call {
            callee, arguments, ..
        } => contains_runtime_choose(callee) || arguments.iter().any(contains_runtime_choose),
        SourceExpression::Array { elements, .. } => elements.iter().any(contains_runtime_choose),
        SourceExpression::Object { entries, .. } => entries
            .iter()
            .any(|entry| contains_runtime_choose(&entry.value)),
        SourceExpression::Index { base, index, .. } => {
            contains_runtime_choose(base) || contains_runtime_choose(index)
        }
        SourceExpression::Literal { .. }
        | SourceExpression::Reference { .. }
        | SourceExpression::Name { .. } => false,
    }
}

fn is_structural_field(path: &str) -> bool {
    matches!(
        path,
        "line"
            | "gameplay.time"
            | "gameplay.endTime"
            | "gameplay.side"
            | "gameplay.judgment.enabled"
            | "gameplay.judgeShape.kind"
            | "gameplay.soundPolicy"
            | "gameplay.soundResource"
            | "gameplay.scorePolicy"
            | "gameplay.scoreExtension"
    )
}

pub(super) fn expand_collections(
    document: &Document,
    schema: &ConstructionSchema,
    limits: CompileTimeLimits,
) -> Result<Vec<ExpandedCollection>, Diagnostic> {
    validate_static_entities(document, schema)?;
    let templates = template_map(document.definitions.as_ref());
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

fn template_map(
    block: Option<&crate::ast::DefinitionsBlock>,
) -> BTreeMap<String, &TemplateDeclaration> {
    block
        .into_iter()
        .flat_map(|block| block.declarations.iter())
        .filter_map(|definition| match definition {
            crate::ast::Definition::Template(template) => Some((template.name.clone(), template)),
            crate::ast::Definition::Const(_) | crate::ast::Definition::Function(_) => None,
        })
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
                bound: self.limits.max_expansion_depth,
                observed: depth,
                span: expression.span(),
            });
        }
        match expression {
            EntityExpression::Constructor(constructor) => {
                self.expand_constructor(constructor, expected_type, bindings)
            }
            EntityExpression::SourceConstructor(constructor) => {
                Err(Diagnostic::FeatureUnavailable {
                    feature: "source entity constructor",
                    span: constructor.span,
                })
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
        let mut fields = fields;
        self.apply_defaults(schema, &mut fields, constructor.span);
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
            let mut chain = stack[start..]
                .iter()
                .map(|subject| DependencyTraceNode {
                    kind: ExpansionTraceKind::Template,
                    name: subject.clone(),
                    span: self
                        .templates
                        .get(subject)
                        .map(|template| template.name_span)
                        .unwrap_or(span),
                })
                .collect::<Vec<_>>();
            chain.push(DependencyTraceNode {
                kind: ExpansionTraceKind::Template,
                name: name.to_owned(),
                span: template.name_span,
            });
            return Err(Diagnostic::RecursiveDependency { chain });
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
                bound: self.limits.max_template_instances,
                observed: self.template_instances,
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
        let result = self.expand_template_statements(
            &template.body,
            &template.return_type,
            &local_bindings,
            stack,
            depth + 1,
        );
        stack.pop();
        result?.ok_or_else(|| Diagnostic::MissingReturn {
            function: template.name.clone(),
            span: template.span,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn expand_template_statements(
        &mut self,
        statements: &[TemplateStatement],
        return_type: &Type,
        bindings: &BTreeMap<String, TypedValue>,
        stack: &mut Vec<String>,
        depth: usize,
    ) -> Result<Option<ExpandedEntity>, Diagnostic> {
        let mut local_bindings = bindings.clone();
        for statement in statements {
            match statement {
                TemplateStatement::Let(statement) => {
                    let value = evaluate_with_bindings(
                        &statement.initializer,
                        self.document.definitions.as_ref(),
                        &local_bindings,
                        self.limits,
                    )?;
                    if value.ty() != statement.ty {
                        return Err(Diagnostic::TypeMismatch {
                            expected: statement.ty.clone(),
                            actual: value.ty(),
                            span: statement.initializer.span(),
                        });
                    }
                    if local_bindings.contains_key(&statement.name) {
                        return Err(Diagnostic::DuplicateBinding {
                            name: statement.name.clone(),
                            span: statement.name_span,
                            previous_span: statement.name_span,
                        });
                    }
                    local_bindings.insert(statement.name.clone(), value);
                }
                TemplateStatement::If(statement) => {
                    let condition = evaluate_with_bindings(
                        &statement.condition,
                        self.document.definitions.as_ref(),
                        &local_bindings,
                        self.limits,
                    )?;
                    let TypedValue::Bool(selected) = condition else {
                        return Err(Diagnostic::NonConstantStructuralCondition {
                            span: statement.span,
                        });
                    };
                    let branch = if selected {
                        &statement.then_branch
                    } else {
                        &statement.else_branch
                    };
                    if let Some(entity) = self.expand_template_statements(
                        branch,
                        return_type,
                        &local_bindings,
                        stack,
                        depth,
                    )? {
                        return Ok(Some(entity));
                    }
                }
                TemplateStatement::Return(statement) => {
                    return self
                        .expand_expression(
                            &statement.value,
                            return_type,
                            &local_bindings,
                            stack,
                            depth,
                        )
                        .map(Some);
                }
            }
        }
        Ok(None)
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
        fields: &[crate::ast::EntityField],
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

    fn apply_defaults(
        &self,
        schema: &EntitySchema,
        fields: &mut BTreeMap<String, ExpandedField>,
        span: SourceSpan,
    ) {
        for path in [
            "gameplay.side",
            "gameplay.judgment.enabled",
            "render.enabled",
            "presentation.positionX",
            "presentation.scrollFactor",
            "presentation.xOffset",
            "presentation.yOffset",
            "presentation.alpha",
            "presentation.scaleX",
            "presentation.scaleY",
            "presentation.color",
            "zOrder",
        ] {
            if fields.contains_key(path) || schema.field(path).is_none() {
                continue;
            }
            let value = match path {
                "gameplay.side" => TypedValue::String("above".to_owned()),
                "gameplay.judgment.enabled" | "render.enabled" => TypedValue::Bool(true),
                "presentation.positionX" | "presentation.xOffset" | "presentation.yOffset" => {
                    TypedValue::Length(0.0)
                }
                "presentation.scrollFactor"
                | "presentation.alpha"
                | "presentation.scaleX"
                | "presentation.scaleY" => TypedValue::Float(1.0),
                "presentation.color" => TypedValue::Color(crate::ast::Color::WHITE),
                "zOrder" => TypedValue::Int(0),
                _ => continue,
            };
            fields.insert(
                path.to_owned(),
                ExpandedField::new(path.to_owned(), value, span),
            );
        }
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
    field: &crate::schema::FieldSchema,
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
