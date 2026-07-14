mod color;
mod definitions;
mod entity;
mod time;
mod types;

pub use color::Color;
pub use entity::{
    CollectionBlock, CollectionItem, CollectionsBlock, EntityConstructor, EntityExpression,
    EntityField, ExpandedCollection, ExpandedEntity, ExpandedField, ExpandedSourceDocument,
    FieldPath, Generator, GeneratorItem, NoteVariant, SourceRange, TemplateDeclaration,
    TemplateParameter, TemplatesBlock, WithExpression,
};
pub use time::{Beat, BeatError, Bpm, InvalidBpm};
pub use types::{
    BinaryOperator, SourceExpression, SourceLiteral, SourceSpan, Type, TypedExpression,
    TypedExpressionKind, TypedValue, UnaryOperator,
};

use crate::version::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentProfile {
    Fragment,
    Chart,
    Playable,
    Renderable,
    Publishable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub source_version: Version,
    pub profile: DocumentProfile,
    pub tempo_map: Option<TempoMap>,
    pub definitions: Option<DefinitionsBlock>,
    pub templates: Option<TemplatesBlock>,
    pub collections: Vec<CollectionBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TempoMap {
    pub points: Vec<TempoPoint>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempoPoint {
    pub beat: Beat,
    pub bpm: Bpm,
}
pub use definitions::{
    ConstDeclaration, Definition, DefinitionsBlock, FunctionDeclaration, FunctionParameter,
    FunctionStatement, IfStatement, LetStatement, ReturnStatement,
};
