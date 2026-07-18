//! Immutable construction schemas for FCS 5 source entities.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::ast::{NoteVariant, Type};

/// A closed set of values accepted by a schema field beyond its base type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldConstraint {
    StringEnum(&'static [&'static str]),
    TimeOrBeat,
}

/// The schema of a field accepted by an entity constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSchema {
    pub path: String,
    pub required: bool,
    ty: Type,
    constraint: Option<FieldConstraint>,
}

impl FieldSchema {
    /// Returns the field's additional value constraint, if one is registered.
    pub fn constraint(&self) -> Option<&FieldConstraint> {
        self.constraint.as_ref()
    }

    /// Returns the type to use as an expression hint, when the field has one exact type.
    pub fn expected_type(&self) -> Option<&Type> {
        (!matches!(self.constraint, Some(FieldConstraint::TimeOrBeat))).then_some(&self.ty)
    }

    /// Returns whether a concrete value type is accepted by this field schema.
    pub fn accepts_type(&self, actual: &Type) -> bool {
        match self.constraint {
            Some(FieldConstraint::TimeOrBeat) => matches!(actual, Type::Time | Type::Beat),
            Some(FieldConstraint::StringEnum(_)) | None => actual == &self.ty,
        }
    }

    pub(crate) fn diagnostic_type(&self) -> &Type {
        &self.ty
    }
}

/// The construction schema of one entity type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitySchema {
    pub entity_type: Type,
    note_variants: Option<Vec<NoteVariant>>,
    fields: BTreeMap<String, FieldSchema>,
}

impl EntitySchema {
    /// Looks up a field by its canonical dotted path.
    pub fn field(&self, path: &str) -> Option<&FieldSchema> {
        self.fields.get(path)
    }

    /// Iterates through fields in canonical path order.
    pub fn fields(&self) -> impl Iterator<Item = &FieldSchema> {
        self.fields.values()
    }

    /// Returns the constructible Note variants when this is a Note schema.
    pub fn note_variants(&self) -> Option<&[NoteVariant]> {
        self.note_variants.as_deref()
    }
}

/// The entity type emitted by a named source collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionSchema {
    pub collection_name: String,
    pub emitted_entity_type: Type,
}

/// A deterministic, read-only registry of constructible entities and collections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructionSchema {
    entities: BTreeMap<Type, EntitySchema>,
    collections: BTreeMap<String, CollectionSchema>,
}

#[derive(Debug, PartialEq, Eq)]
enum SchemaConstructionError {
    FieldPathCollision(String),
    RepeatedEntityType(Type),
    CollectionNameConflict(String),
}

impl ConstructionSchema {
    /// Looks up the schema for a constructible entity type.
    pub fn entity(&self, entity_type: &Type) -> Option<&EntitySchema> {
        self.entities.get(entity_type)
    }

    /// Iterates through constructible entities in deterministic type order.
    pub fn entities(&self) -> impl Iterator<Item = &EntitySchema> {
        self.entities.values()
    }

    /// Looks up a collection schema by its source name.
    pub fn collection(&self, collection_name: &str) -> Option<&CollectionSchema> {
        self.collections.get(collection_name)
    }

    /// Iterates through collections in deterministic name order.
    pub fn collections(&self) -> impl Iterator<Item = &CollectionSchema> {
        self.collections.values()
    }
}

/// Returns the immutable bootstrap construction schema for Phase 2.
pub fn phase2_schema() -> &'static ConstructionSchema {
    static SCHEMA: OnceLock<ConstructionSchema> = OnceLock::new();
    SCHEMA.get_or_init(build_phase2_schema)
}

fn build_phase2_schema() -> ConstructionSchema {
    let note = note_entity(
        vec![
            NoteVariant::Tap,
            NoteVariant::Hold,
            NoteVariant::Flick,
            NoteVariant::Drag,
        ],
        vec![
            field("id", Type::String, false),
            field("line", Type::Line, false),
            constrained_field(
                "gameplay.time",
                Type::Beat,
                true,
                FieldConstraint::TimeOrBeat,
            ),
            constrained_field(
                "gameplay.endTime",
                Type::Beat,
                false,
                FieldConstraint::TimeOrBeat,
            ),
            constrained_field(
                "gameplay.side",
                Type::String,
                false,
                FieldConstraint::StringEnum(&["above", "below"]),
            ),
            field("gameplay.judgment.enabled", Type::Bool, false),
            constrained_field(
                "gameplay.judgeShape.kind",
                Type::String,
                false,
                FieldConstraint::StringEnum(&["lineDefault", "rectangle", "circle"]),
            ),
            field(
                "gameplay.judgeShape.center",
                Type::Vec2(Box::new(Type::Length)),
                false,
            ),
            field(
                "gameplay.judgeShape.halfExtents",
                Type::Vec2(Box::new(Type::Length)),
                false,
            ),
            field("gameplay.judgeShape.radius", Type::Length, false),
            constrained_field(
                "gameplay.soundPolicy",
                Type::String,
                false,
                FieldConstraint::StringEnum(&["default", "none", "resource"]),
            ),
            field("gameplay.soundResource", Type::String, false),
            constrained_field(
                "gameplay.scorePolicy",
                Type::String,
                false,
                FieldConstraint::StringEnum(&["default", "none", "custom"]),
            ),
            field("gameplay.scoreExtension", Type::String, false),
            field("render.enabled", Type::Bool, false),
            field("presentation.positionX", Type::Length, false),
            field("presentation.scrollFactor", Type::Float, false),
            field("presentation.xOffset", Type::Length, false),
            field("presentation.yOffset", Type::Length, false),
            field("presentation.alpha", Type::Float, false),
            field("presentation.scaleX", Type::Float, false),
            field("presentation.scaleY", Type::Float, false),
            field("presentation.rotation", Type::Angle, false),
            field("presentation.color", Type::Color, false),
            field("presentation.texture", Type::String, false),
            field("presentation.visibleFrom", Type::Beat, false),
            field("presentation.visibleUntil", Type::Beat, false),
        ],
    );
    let line = line_entity(vec![
        field("id", Type::String, false),
        field("zOrder", Type::Int, false),
    ]);

    checked_construction_schema(
        vec![note, line],
        vec![
            collection("notes", Type::Note),
            collection("judgelines", Type::Line),
        ],
    )
    .expect("the Phase 2 construction schema must not contain duplicate records")
}

fn note_entity(note_variants: Vec<NoteVariant>, fields: Vec<FieldSchema>) -> EntitySchema {
    checked_entity(Type::Note, Some(note_variants), fields)
        .expect("a Note schema must not contain duplicate fields")
}

fn line_entity(fields: Vec<FieldSchema>) -> EntitySchema {
    checked_entity(Type::Line, None, fields)
        .expect("a Line schema must not contain duplicate fields")
}

fn checked_entity(
    entity_type: Type,
    note_variants: Option<Vec<NoteVariant>>,
    fields: Vec<FieldSchema>,
) -> Result<EntitySchema, SchemaConstructionError> {
    let mut fields_by_path = BTreeMap::new();
    for field in fields {
        let path = field.path.clone();
        if fields_by_path.insert(path.clone(), field).is_some() {
            return Err(SchemaConstructionError::FieldPathCollision(path));
        }
    }

    Ok(EntitySchema {
        entity_type,
        note_variants,
        fields: fields_by_path,
    })
}

fn checked_construction_schema(
    entities: Vec<EntitySchema>,
    collections: Vec<CollectionSchema>,
) -> Result<ConstructionSchema, SchemaConstructionError> {
    let mut entities_by_type = BTreeMap::new();
    for entity in entities {
        let entity_type = entity.entity_type.clone();
        if entities_by_type
            .insert(entity_type.clone(), entity)
            .is_some()
        {
            return Err(SchemaConstructionError::RepeatedEntityType(entity_type));
        }
    }

    let mut collections_by_name = BTreeMap::new();
    for collection in collections {
        let collection_name = collection.collection_name.clone();
        if collections_by_name
            .insert(collection_name.clone(), collection)
            .is_some()
        {
            return Err(SchemaConstructionError::CollectionNameConflict(
                collection_name,
            ));
        }
    }

    Ok(ConstructionSchema {
        entities: entities_by_type,
        collections: collections_by_name,
    })
}

fn field(path: &str, ty: Type, required: bool) -> FieldSchema {
    FieldSchema {
        path: path.into(),
        ty,
        required,
        constraint: None,
    }
}

fn constrained_field(
    path: &str,
    ty: Type,
    required: bool,
    constraint: FieldConstraint,
) -> FieldSchema {
    FieldSchema {
        path: path.into(),
        ty,
        required,
        constraint: Some(constraint),
    }
}

fn collection(collection_name: &str, emitted_entity_type: Type) -> CollectionSchema {
    CollectionSchema {
        collection_name: collection_name.into(),
        emitted_entity_type,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_entity_rejects_duplicate_field_paths() {
        let result = checked_entity(
            Type::Line,
            None,
            vec![
                field("id", Type::String, true),
                field("id", Type::Int, false),
            ],
        );

        assert_eq!(
            result,
            Err(SchemaConstructionError::FieldPathCollision("id".into()))
        );
    }

    #[test]
    fn checked_construction_schema_rejects_duplicate_entity_types() {
        let first = checked_entity(Type::Line, None, Vec::new()).unwrap();
        let second = checked_entity(Type::Line, None, Vec::new()).unwrap();

        let result = checked_construction_schema(vec![first, second], Vec::new());

        assert_eq!(
            result,
            Err(SchemaConstructionError::RepeatedEntityType(Type::Line))
        );
    }

    #[test]
    fn checked_construction_schema_rejects_duplicate_collection_names() {
        let result = checked_construction_schema(
            Vec::new(),
            vec![
                collection("notes", Type::Note),
                collection("notes", Type::Line),
            ],
        );

        assert_eq!(
            result,
            Err(SchemaConstructionError::CollectionNameConflict(
                "notes".into()
            ))
        );
    }

    #[test]
    fn checked_construction_schema_derives_map_keys_from_records() {
        let note = checked_entity(Type::Note, Some(vec![NoteVariant::Tap]), Vec::new()).unwrap();
        let line = checked_entity(Type::Line, None, Vec::new()).unwrap();

        let schema = checked_construction_schema(
            vec![note, line],
            vec![
                collection("notes", Type::Note),
                collection("judgelines", Type::Line),
            ],
        )
        .unwrap();

        assert!(
            schema
                .entities
                .iter()
                .all(|(key, value)| key == &value.entity_type)
        );
        assert!(
            schema
                .collections
                .iter()
                .all(|(key, value)| key == &value.collection_name)
        );
    }
}
