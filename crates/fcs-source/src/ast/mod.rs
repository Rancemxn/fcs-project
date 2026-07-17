mod color;
mod definitions;
mod document;
mod entity;
mod extension;
mod metadata;
mod time;
mod track;
mod types;

pub use color::Color;
pub use entity::{
    CollectionBlock, CollectionItem, CollectionsBlock, EntityConstructor, EntityExpression,
    EntityField, ExpandedCollection, ExpandedEntity, ExpandedField, ExpandedInvariantViolation,
    ExpandedSourceDocument, FieldPath, Generator, GeneratorItem, GeneratorOwner, NoteVariant,
    SchemaField, SchemaValue, SourceEntityConstructor, SourceEntityConstructorKind, SourceRange,
    WithExpression,
};
pub use extension::{
    ExtensionDeclaration, ExtensionHeader, ExtensionRequirement, ExtensionsBlock, OrderedObject,
    PreserveBlock, PreserveItem, PreservePayload, PreserveSource,
};
pub use metadata::{
    ArtworkBlock, ContributorDeclaration, ContributorsBlock, CreditEntry, CreditsBlock, MetaBlock,
    ResourceDeclaration, ResourceKind, ResourcesBlock, SyncBlock,
};
pub use time::{Beat, BeatError, Bpm, InvalidBpm, SourceBpm};
pub use track::{
    DirectPoint, DirectSegment, HalfOpenInterval, Interpolation, LineBodyItem, LineDeclaration,
    LinesBlock, SegmentsBlock, TrackDeclaration, TrackSegmentItem, TrackSetting, TracksBlock,
};
pub use types::{
    BinaryOperator, GeneratorRangeValue, SourceChooseArm, SourceExpression, SourceLiteral,
    SourceObjectEntry, SourceSpan, SourceType, SourceTypeKind, Type, TypedExpression,
    TypedExpressionKind, TypedValue, UnaryOperator,
};

#[derive(Debug, Clone, PartialEq)]
pub struct TempoMap {
    pub points: Vec<TempoPoint>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempoPoint {
    pub beat: Beat,
    pub bpm: SourceBpm,
}
pub use definitions::{
    ConstDeclaration, Definition, DefinitionsBlock, FunctionDeclaration, FunctionParameter,
    FunctionStatement, IfStatement, LetStatement, ReturnEntityStatement, ReturnStatement,
    TemplateDeclaration, TemplateIfStatement, TemplateParameter, TemplateStatement,
};
pub use document::{
    Document, DocumentProfile, FeatureList, FormatBlock, FormatFeature, FormatField, FormatProfile,
    ProfileFeature, RenderBlock, SourceBlock, SourceElement, SourceGroup, TempoMapBlock,
    TopLevelBlock, TopLevelBlockKind,
};
