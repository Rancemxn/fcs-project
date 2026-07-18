//! Source entity construction nodes and lowered entity records.

use std::collections::BTreeMap;

use super::{DocumentProfile, LetStatement, TempoMap};
use super::{SourceExpression, SourceSpan, Type, TypedValue};
use crate::version::Version;
use fcs_model::{
    AudioOffset, Beat as CanonicalBeat, CanonicalTextualId, CanonicalTime, ChartTimeMap,
    EntityKind, ExpansionPath, IdError, StableId, StableIdRegistry, TempoError, TempoPoint,
};

/// A violation detected while constructing or auditing expanded output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpandedInvariantViolation {
    EmptyCollectionName,
    DuplicateCollectionName,
    NonConcreteEntity,
    EmptyFieldPath,
    FieldPathKeyMismatch,
    NonConcreteFieldValue,
}

impl ExpandedInvariantViolation {
    pub const fn message(self) -> &'static str {
        match self {
            Self::EmptyCollectionName => "expanded collection name must not be empty",
            Self::DuplicateCollectionName => "expanded collection names must be unique",
            Self::NonConcreteEntity => "expanded output contains a non-concrete entity",
            Self::EmptyFieldPath => "expanded field path must not be empty",
            Self::FieldPathKeyMismatch => "expanded field map key must match its field path",
            Self::NonConcreteFieldValue => "expanded output contains a non-concrete field value",
        }
    }
}

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

/// A Note identity paired with its canonical chart-time value and exact beat provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalNoteTime {
    stable_id: StableId,
    canonical_time: CanonicalTime,
}

impl CanonicalNoteTime {
    pub fn stable_id(&self) -> &StableId {
        &self.stable_id
    }

    pub const fn source_beat(&self) -> CanonicalBeat {
        self.canonical_time.source_beat()
    }

    pub const fn chart_time_seconds(&self) -> f64 {
        self.canonical_time.chart_time_seconds()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalNoteTimeError {
    Identity(IdError),
    Tempo(TempoError),
    MissingGameplayTime,
    InvalidGameplayTime,
}

impl ExpandedSourceDocument {
    pub fn source_version(&self) -> Version {
        self.source_version.clone()
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

    /// Lowers the currently available expanded Note identities without performing
    /// later time, graph, Track, or gameplay normalization.
    pub fn canonical_note_ids(&self) -> Result<Vec<StableId>, IdError> {
        let mut registry = StableIdRegistry::new();
        let mut ids = Vec::new();
        for collection in &self.collections {
            for (expanded_order, entity) in collection.entities.iter().enumerate() {
                if entity.entity_type != Type::Note {
                    continue;
                }
                let textual = entity
                    .field("id")
                    .and_then(|field| match field.value() {
                        TypedValue::String(value) => {
                            Some(CanonicalTextualId::explicit(value.clone()))
                        }
                        _ => None,
                    })
                    .transpose()?;
                let textual = match textual {
                    Some(textual) => textual,
                    None => {
                        let path = entity
                            .expansion_path()
                            .ok_or(IdError::MissingExpansionPath)?;
                        CanonicalTextualId::generated(EntityKind::Note, path, expanded_order as u64)
                    }
                };
                ids.push(registry.insert(EntityKind::Note, textual)?);
            }
        }
        Ok(ids)
    }

    /// Converts the source tempo map to the canonical global chart-time model.
    pub fn canonical_time_map(&self) -> Result<ChartTimeMap, TempoError> {
        let tempo_map = self.tempo_map.as_ref().ok_or(TempoError::EmptyTempoMap)?;
        let points = tempo_map
            .points
            .iter()
            .map(|point| {
                CanonicalBeat::new(point.beat.numerator(), point.beat.denominator())
                    .map(|beat| TempoPoint {
                        beat,
                        bpm: point.bpm.get(),
                    })
                    .map_err(|_| TempoError::InvalidBeat)
            })
            .collect::<Result<Vec<_>, _>>()?;
        ChartTimeMap::new(points)
    }

    /// Normalizes Note gameplay beats while retaining exact source-beat provenance.
    pub fn canonical_note_times(
        &self,
        time_map: &ChartTimeMap,
    ) -> Result<Vec<CanonicalNoteTime>, CanonicalNoteTimeError> {
        let ids = self
            .canonical_note_ids()
            .map_err(CanonicalNoteTimeError::Identity)?;
        let mut source_beats = Vec::new();
        for collection in &self.collections {
            for entity in &collection.entities {
                if entity.entity_type != Type::Note {
                    continue;
                }
                let value = entity
                    .field("gameplay.time")
                    .ok_or(CanonicalNoteTimeError::MissingGameplayTime)?
                    .value();
                let TypedValue::Beat(beat) = value else {
                    return Err(CanonicalNoteTimeError::InvalidGameplayTime);
                };
                source_beats.push(
                    CanonicalBeat::new(beat.numerator(), beat.denominator())
                        .map_err(|_| CanonicalNoteTimeError::InvalidGameplayTime)?,
                );
            }
        }
        ids.into_iter()
            .zip(source_beats)
            .map(|(stable_id, source_beat)| {
                let canonical_time = time_map
                    .chart_time(source_beat)
                    .map_err(CanonicalNoteTimeError::Tempo)?;
                Ok(CanonicalNoteTime {
                    stable_id,
                    canonical_time,
                })
            })
            .collect()
    }

    /// Applies the FCS sync affine boundary without changing canonical chart time.
    pub fn audio_time(
        &self,
        offset: AudioOffset,
        chart_time_seconds: f64,
    ) -> Result<f64, TempoError> {
        offset.audio_time(chart_time_seconds)
    }

    pub(crate) fn try_from_collections(
        source_version: Version,
        profile: DocumentProfile,
        tempo_map: Option<TempoMap>,
        collections: Vec<ExpandedCollection>,
    ) -> Result<Self, ExpandedInvariantViolation> {
        let document = Self {
            source_version,
            profile,
            tempo_map,
            collections,
        };
        document.validate_invariants()?;
        Ok(document)
    }

    /// Audits the expanded-output boundary for concrete, typed, source-free data.
    pub fn validate_invariants(&self) -> Result<(), ExpandedInvariantViolation> {
        let mut names = std::collections::BTreeSet::new();
        for collection in &self.collections {
            if collection.name.is_empty() {
                return Err(ExpandedInvariantViolation::EmptyCollectionName);
            }
            if !names.insert(&collection.name) {
                return Err(ExpandedInvariantViolation::DuplicateCollectionName);
            }
            for entity in &collection.entities {
                entity.validate_invariants()?;
            }
        }
        Ok(())
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

/// A source entity constructor whose semantic type is owned by a later phase.
///
/// `RenderNode`, `segment`, and `keyframe` are valid source productions, but their
/// static schema/element type is not available to the I1 source parser. Keeping the
/// constructor kind separate prevents the parser from inventing a placeholder `Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceEntityConstructorKind {
    RenderNode,
    Segment,
    Keyframe,
}

/// A fully spanned source constructor retained before static schema validation.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceEntityConstructor {
    pub kind: SourceEntityConstructorKind,
    pub fields: Vec<SchemaField>,
    pub span: SourceSpan,
}

/// A source field whose value is owned by a schema production rather than the
/// ordinary expression grammar.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaField {
    pub path: FieldPath,
    pub value: SchemaValue,
    pub span: SourceSpan,
}

/// Schema-owned values retained before static validation.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaValue {
    Expression(SourceExpression),
    CubicBezier {
        values: [SourceExpression; 4],
        span: SourceSpan,
    },
    Interval {
        start: SourceExpression,
        end: SourceExpression,
        span: SourceSpan,
    },
}

impl SchemaValue {
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Expression(expression) => expression.span(),
            Self::CubicBezier { span, .. } | Self::Interval { span, .. } => *span,
        }
    }
}

/// A source range used by a compile-time generator.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceRange {
    pub start: SourceExpression,
    pub end: SourceExpression,
    pub step: SourceExpression,
    pub inclusive_end: bool,
    pub span: SourceSpan,
}

/// A compile-time generator contained in a collection block.
#[derive(Debug, Clone, PartialEq)]
pub struct Generator {
    /// The syntactic owner of this generator. The owner is retained without
    /// resolving its registered entity type; later phases own that binding.
    pub owner: Box<GeneratorOwner>,
    pub variable: String,
    pub variable_span: SourceSpan,
    pub variable_type: Type,
    pub range: SourceRange,
    pub body: Vec<GeneratorItem>,
    pub span: SourceSpan,
}

/// A source-level owner context for a generator.
#[derive(Debug, Clone, PartialEq)]
pub enum GeneratorOwner {
    /// A generator directly or conditionally contained by a named collection.
    Collection { name: String },
    /// A generator contained by a Track's `segments` collection.
    TrackSegments {
        track: String,
        target: FieldPath,
        span: SourceSpan,
    },
}

/// A source item emitted by a compile-time generator.
#[derive(Debug, Clone, PartialEq)]
pub enum GeneratorItem {
    Let(LetStatement),
    Emit(EntityExpression),
    Conditional {
        condition: SourceExpression,
        then_items: Vec<GeneratorItem>,
        else_items: Vec<GeneratorItem>,
        span: SourceSpan,
    },
}

impl GeneratorItem {
    /// Returns this generator statement's complete source span.
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Let(statement) => statement.span,
            Self::Conditional { span, .. } => *span,
            Self::Emit(expression) => expression.span(),
        }
    }
}

/// A source item contained directly in a collection block.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum CollectionItem {
    Constructor(EntityConstructor),
    Expression(EntityExpression),
    Generator(Generator),
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
            Self::Generator(generator) => generator.span,
            Self::Conditional { span, .. } => *span,
        }
    }
}

/// A source expression that evaluates to an entity value.
#[derive(Debug, Clone, PartialEq)]
pub enum EntityExpression {
    Constructor(EntityConstructor),
    SourceConstructor(SourceEntityConstructor),
    Source(SourceExpression),
    With(WithExpression),
}

impl EntityExpression {
    /// Returns this entity expression node's complete source span.
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Constructor(constructor) => constructor.span,
            Self::SourceConstructor(constructor) => constructor.span,
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
    expansion_path: Option<ExpansionPath>,
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
            expansion_path: None,
        }
    }

    pub(crate) fn set_expansion_path(&mut self, path: ExpansionPath) {
        self.expansion_path = Some(path);
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

    /// Returns the deterministic source/expansion provenance used by I3 canonical IDs.
    pub fn expansion_path(&self) -> Option<&ExpansionPath> {
        self.expansion_path.as_ref()
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

    fn validate_invariants(&self) -> Result<(), ExpandedInvariantViolation> {
        if !self.entity_type.is_entity_type() {
            return Err(ExpandedInvariantViolation::NonConcreteEntity);
        }
        for (key, field) in &self.fields {
            if field.path.is_empty() {
                return Err(ExpandedInvariantViolation::EmptyFieldPath);
            }
            if key != &field.path {
                return Err(ExpandedInvariantViolation::FieldPathKeyMismatch);
            }
            if !field.value.is_concrete() {
                return Err(ExpandedInvariantViolation::NonConcreteFieldValue);
            }
        }
        Ok(())
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
            expansion_path: None,
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

    #[test]
    fn expanded_document_validation_rejects_structural_leaks() {
        let span = SourceSpan::new(0, 1);
        let invalid_value = ExpandedField {
            path: "gameplay.time".into(),
            value: TypedValue::Vec2(
                Box::new(TypedValue::Int(0)),
                Box::new(TypedValue::String("runtime".into())),
            ),
            span,
        };
        let invalid_field_entity = ExpandedEntity {
            entity_type: Type::Note,
            note_variant: Some(NoteVariant::Tap),
            fields: [(invalid_value.path.clone(), invalid_value)]
                .into_iter()
                .collect(),
            span,
            expansion_path: None,
        };
        let invalid_field = ExpandedSourceDocument::try_from_collections(
            Version::new(5, 0, 0),
            DocumentProfile::Fragment,
            None,
            vec![ExpandedCollection::new(
                "notes".into(),
                vec![invalid_field_entity],
            )],
        );
        assert_eq!(
            invalid_field,
            Err(ExpandedInvariantViolation::NonConcreteFieldValue)
        );

        let empty_path_entity = ExpandedEntity {
            entity_type: Type::Note,
            note_variant: Some(NoteVariant::Tap),
            fields: [(
                String::new(),
                ExpandedField {
                    path: String::new(),
                    value: TypedValue::Int(0),
                    span,
                },
            )]
            .into_iter()
            .collect(),
            span,
            expansion_path: None,
        };
        let empty_path = ExpandedSourceDocument::try_from_collections(
            Version::new(5, 0, 0),
            DocumentProfile::Fragment,
            None,
            vec![ExpandedCollection::new(
                "notes".into(),
                vec![empty_path_entity],
            )],
        );
        assert_eq!(empty_path, Err(ExpandedInvariantViolation::EmptyFieldPath));

        let mismatched_key_entity = ExpandedEntity {
            entity_type: Type::Note,
            note_variant: Some(NoteVariant::Tap),
            fields: [(
                "wrong.key".into(),
                ExpandedField {
                    path: "gameplay.time".into(),
                    value: TypedValue::Int(0),
                    span,
                },
            )]
            .into_iter()
            .collect(),
            span,
            expansion_path: None,
        };
        let mismatched_key = ExpandedSourceDocument::try_from_collections(
            Version::new(5, 0, 0),
            DocumentProfile::Fragment,
            None,
            vec![ExpandedCollection::new(
                "notes".into(),
                vec![mismatched_key_entity],
            )],
        );
        assert_eq!(
            mismatched_key,
            Err(ExpandedInvariantViolation::FieldPathKeyMismatch)
        );

        let invalid_entity = ExpandedEntity {
            entity_type: Type::Bool,
            note_variant: None,
            fields: BTreeMap::new(),
            span,
            expansion_path: None,
        };
        let invalid_type = ExpandedSourceDocument::try_from_collections(
            Version::new(5, 0, 0),
            DocumentProfile::Fragment,
            None,
            vec![ExpandedCollection::new(
                "notes".into(),
                vec![invalid_entity],
            )],
        );
        assert_eq!(
            invalid_type,
            Err(ExpandedInvariantViolation::NonConcreteEntity)
        );
    }

    #[test]
    fn expanded_document_validation_rejects_empty_and_duplicate_collections() {
        let empty = ExpandedSourceDocument::try_from_collections(
            Version::new(5, 0, 0),
            DocumentProfile::Fragment,
            None,
            vec![ExpandedCollection::new(String::new(), Vec::new())],
        );
        assert_eq!(empty, Err(ExpandedInvariantViolation::EmptyCollectionName));

        let duplicate = ExpandedSourceDocument::try_from_collections(
            Version::new(5, 0, 0),
            DocumentProfile::Fragment,
            None,
            vec![
                ExpandedCollection::new("notes".into(), Vec::new()),
                ExpandedCollection::new("notes".into(), Vec::new()),
            ],
        );
        assert_eq!(
            duplicate,
            Err(ExpandedInvariantViolation::DuplicateCollectionName)
        );
    }
}
