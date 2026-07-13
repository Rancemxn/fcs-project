//! Source entity construction nodes and lowered entity records.

use std::collections::BTreeMap;

use super::{DocumentProfile, TempoMap};
use super::{SourceExpression, SourceSpan, Type, TypedValue};
use crate::v5::version::Version;

/// A concrete named collection produced by compile-time expansion.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedCollection {
    name: String,
    entities: Vec<ExpandedEntity>,
}

impl ExpandedCollection {
    pub(crate) fn new(name: String, entities: Vec<ExpandedEntity>) -> Self {
        Self { name, entities }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn entities(&self) -> impl Iterator<Item = &ExpandedEntity> {
        self.entities.iter()
    }
}

/// Read-only result of elaborating an FCS 5 source document.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedSourceDocument {
    source_version: Version,
    profile: DocumentProfile,
    tempo_map: Option<TempoMap>,
    collections: Vec<ExpandedCollection>,
}

impl ExpandedSourceDocument {
    pub const fn source_version(&self) -> Version {
        self.source_version
    }

    pub const fn profile(&self) -> DocumentProfile {
        self.profile
    }

    pub const fn tempo_map(&self) -> Option<&TempoMap> {
        self.tempo_map.as_ref()
    }

    pub fn collections(&self) -> impl Iterator<Item = &ExpandedCollection> {
        self.collections.iter()
    }

    pub(crate) fn from_collections(
        source_version: Version,
        profile: DocumentProfile,
        tempo_map: Option<TempoMap>,
        collections: Vec<ExpandedCollection>,
    ) -> Self {
        Self {
            source_version,
            profile,
            tempo_map,
            collections,
        }
    }
}

/// A Note constructor variant recognized by the Phase 2 construction language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NoteVariant {
    Tap,
    Hold,
    Flick,
    Drag,
}

/// A dotted entity field path as written in source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldPath {
    pub segments: Vec<String>,
    pub span: SourceSpan,
}

/// A source field assignment within an entity constructor or `with` block.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityField {
    pub path: FieldPath,
    pub value: SourceExpression,
    pub span: SourceSpan,
}

/// A source entity constructor before schema validation and elaboration.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityConstructor {
    pub entity_type: Type,
    pub note_variant: Option<NoteVariant>,
    pub fields: Vec<EntityField>,
    pub span: SourceSpan,
}

/// A typed parameter accepted by a compile-time entity template.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateParameter {
    pub name: String,
    pub name_span: SourceSpan,
    pub ty: Type,
    pub span: SourceSpan,
}

/// A compile-time entity template declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateDeclaration {
    pub name: String,
    pub name_span: SourceSpan,
    pub parameters: Vec<TemplateParameter>,
    pub return_type: Type,
    pub body: EntityExpression,
    pub span: SourceSpan,
}

/// The source-level template registry.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplatesBlock {
    pub declarations: Vec<TemplateDeclaration>,
    pub span: SourceSpan,
}

/// A source item contained directly in a collection block.
///
/// Task 6 may add generator, `emit`, local binding, and compile-time conditional variants.
/// They are deliberately omitted until their semantics are defined.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum CollectionItem {
    Constructor(EntityConstructor),
    Expression(EntityExpression),
    Conditional {
        condition: SourceExpression,
        then_items: Vec<CollectionItem>,
        else_items: Vec<CollectionItem>,
        span: SourceSpan,
    },
}

impl CollectionItem {
    /// Returns this collection item's complete source span.
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Constructor(constructor) => constructor.span,
            Self::Expression(expression) => expression.span(),
            Self::Conditional { span, .. } => *span,
        }
    }
}

/// A source expression that evaluates to an entity value.
#[derive(Debug, Clone, PartialEq)]
pub enum EntityExpression {
    Constructor(EntityConstructor),
    Source(SourceExpression),
    With(WithExpression),
}

impl EntityExpression {
    /// Returns this entity expression node's complete source span.
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Constructor(constructor) => constructor.span,
            Self::Source(expression) => expression.span(),
            Self::With(with_expression) => with_expression.span,
        }
    }
}

/// A source expression that immutably replaces fields on an entity value.
#[derive(Debug, Clone, PartialEq)]
pub struct WithExpression {
    pub base: Box<EntityExpression>,
    pub fields: Vec<EntityField>,
    pub span: SourceSpan,
}

/// A source collection containing source items.
///
/// Task 6 may add generator, `emit`, local binding, and compile-time conditional item variants;
/// this bootstrap representation assigns none of them premature semantics.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionBlock {
    pub collection_name: String,
    pub items: Vec<CollectionItem>,
    pub span: SourceSpan,
}

/// The source-level named collection blocks in document order.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionsBlock {
    pub collections: Vec<CollectionBlock>,
    pub span: SourceSpan,
}

/// A concrete field value retained after successful elaboration.
///
/// Lowered fields own their value and source span and cannot retain a source expression.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedField {
    path: String,
    value: TypedValue,
    span: SourceSpan,
}

impl ExpandedField {
    pub(crate) fn new(path: String, value: TypedValue, span: SourceSpan) -> Self {
        Self { path, value, span }
    }

    /// Returns the canonical dotted field path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the concrete value produced by elaboration.
    pub fn value(&self) -> &TypedValue {
        &self.value
    }

    /// Returns the source provenance span of this field value.
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// A concrete entity retained after successful compile-time expansion.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedEntity {
    entity_type: Type,
    note_variant: Option<NoteVariant>,
    fields: BTreeMap<String, ExpandedField>,
    span: SourceSpan,
}

impl ExpandedEntity {
    pub(crate) fn new(
        entity_type: Type,
        note_variant: Option<NoteVariant>,
        fields: BTreeMap<String, ExpandedField>,
        span: SourceSpan,
    ) -> Self {
        Self {
            entity_type,
            note_variant,
            fields,
            span,
        }
    }

    pub(crate) fn replace_field(&mut self, field: ExpandedField) {
        self.fields.insert(field.path.clone(), field);
    }

    pub(crate) fn has_field(&self, path: &str) -> bool {
        self.fields.contains_key(path)
    }

    /// Returns the concrete entity type produced by elaboration.
    pub fn entity_type(&self) -> &Type {
        &self.entity_type
    }

    /// Returns the Note variant, when this is a Note entity.
    pub const fn variant(&self) -> Option<NoteVariant> {
        self.note_variant
    }

    /// Returns the source provenance span of this entity.
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    /// Looks up a lowered field by its canonical dotted path.
    pub fn field(&self, path: &str) -> Option<&ExpandedField> {
        self.fields.get(path)
    }

    /// Iterates through lowered fields in canonical path order.
    pub fn fields(&self) -> impl Iterator<Item = &ExpandedField> {
        self.fields.values()
    }

    /// Reports that this representation contains only concrete typed field values.
    pub const fn is_lowered(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_records_expose_values_in_path_order() {
        let first_span = SourceSpan::new(1, 2);
        let second_span = SourceSpan::new(3, 4);
        let entity_span = SourceSpan::new(0, 5);
        let first = ExpandedField {
            path: "gameplay.time".into(),
            value: TypedValue::Beat(super::super::Beat::new(1, 1).unwrap()),
            span: first_span,
        };
        let second = ExpandedField {
            path: "presentation.positionX".into(),
            value: TypedValue::Length(12.0),
            span: second_span,
        };
        let entity = ExpandedEntity {
            entity_type: Type::Note,
            note_variant: Some(NoteVariant::Tap),
            fields: [(second.path.clone(), second), (first.path.clone(), first)]
                .into_iter()
                .collect(),
            span: entity_span,
        };

        assert_eq!(entity.entity_type(), &Type::Note);
        assert_eq!(entity.variant(), Some(NoteVariant::Tap));
        assert_eq!(entity.span(), entity_span);
        assert!(entity.is_lowered());
        let fields: Vec<_> = entity.fields().map(ExpandedField::path).collect();
        assert_eq!(fields, ["gameplay.time", "presentation.positionX"]);
        let time = entity.field("gameplay.time").unwrap();
        assert_eq!(time.path(), "gameplay.time");
        assert_eq!(time.value().ty(), Type::Beat);
        assert_eq!(time.span(), first_span);
    }
}
