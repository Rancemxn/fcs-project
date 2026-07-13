//! Immutable construction schemas for FCS 5 source entities.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::ast::{NoteVariant, Type};

/// The schema of a field accepted by an entity constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSchema {
    pub path: String,
    pub ty: Type,
    pub required: bool,
}

/// The construction schema of one entity type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitySchema {
    pub entity_type: Type,
    variants: Vec<NoteVariant>,
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

    /// Returns the constructible Note variants, or an empty slice for non-Note entities.
    pub fn variants(&self) -> &[NoteVariant] {
        &self.variants
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
    let note = entity(
        Type::Note,
        vec![
            NoteVariant::Tap,
            NoteVariant::Hold,
            NoteVariant::Flick,
            NoteVariant::Drag,
        ],
        vec![
            field("gameplay.time", Type::Beat, true),
            field("gameplay.endTime", Type::Beat, false),
            // The kernel has no `above | below` enum type yet. Phase 2 therefore records
            // `gameplay.side` as string and defers allowed-value validation.
            field("gameplay.side", Type::String, false),
            field("gameplay.judgment.enabled", Type::Bool, false),
            field("render.enabled", Type::Bool, false),
            field("presentation.positionX", Type::Length, false),
            field("presentation.scrollFactor", Type::Float, false),
            field("presentation.xOffset", Type::Length, false),
            field("presentation.yOffset", Type::Length, false),
            field("presentation.alpha", Type::Float, false),
            field("presentation.scaleX", Type::Float, false),
            field("presentation.scaleY", Type::Float, false),
            field("presentation.color", Type::Color, false),
            field("presentation.texture", Type::String, false),
            field("presentation.visibleFrom", Type::Beat, false),
            field("presentation.visibleUntil", Type::Beat, false),
        ],
    );
    let line = entity(
        Type::Line,
        Vec::new(),
        vec![
            field("id", Type::String, true),
            field("zOrder", Type::Int, false),
        ],
    );

    ConstructionSchema {
        entities: [(Type::Note, note), (Type::Line, line)]
            .into_iter()
            .collect(),
        collections: [
            collection("notes", Type::Note),
            collection("judgelines", Type::Line),
        ]
        .into_iter()
        .map(|schema| (schema.collection_name.clone(), schema))
        .collect(),
    }
}

fn entity(entity_type: Type, variants: Vec<NoteVariant>, fields: Vec<FieldSchema>) -> EntitySchema {
    EntitySchema {
        entity_type,
        variants,
        fields: fields
            .into_iter()
            .map(|schema| (schema.path.clone(), schema))
            .collect(),
    }
}

fn field(path: &str, ty: Type, required: bool) -> FieldSchema {
    FieldSchema {
        path: path.into(),
        ty,
        required,
    }
}

fn collection(collection_name: &str, emitted_entity_type: Type) -> CollectionSchema {
    CollectionSchema {
        collection_name: collection_name.into(),
        emitted_entity_type,
    }
}
