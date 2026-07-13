//! Source entity construction nodes and lowered entity records.

use std::collections::BTreeMap;

use super::{SourceExpression, SourceSpan, Type, TypedValue};

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

/// A source expression that immutably replaces fields on an entity value.
#[derive(Debug, Clone, PartialEq)]
pub struct WithExpression {
    pub base: SourceExpression,
    pub fields: Vec<EntityField>,
    pub span: SourceSpan,
}

/// A source collection containing direct entity constructors.
///
/// Generator, `emit`, local binding, and compile-time conditional items are deliberately
/// absent. The collection item representation will be widened when those syntaxes are
/// introduced; this bootstrap type does not assign them premature semantics.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionBlock {
    pub collection_name: String,
    pub constructors: Vec<EntityConstructor>,
    pub span: SourceSpan,
}

/// A concrete field value retained after successful elaboration.
///
/// Lowered fields own their value and source span and cannot retain a source expression.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedField {
    pub path: String,
    pub value: TypedValue,
    pub span: SourceSpan,
}

/// A concrete entity retained after successful compile-time expansion.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedEntity {
    pub entity_type: Type,
    pub note_variant: Option<NoteVariant>,
    pub fields: BTreeMap<String, ExpandedField>,
    pub span: SourceSpan,
}
