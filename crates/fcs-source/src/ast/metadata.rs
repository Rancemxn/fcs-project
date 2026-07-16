//! Source-owned metadata, resource, artwork, and sync grammar nodes.

use super::{SchemaField, SourceSpan};

/// The source-ordered `meta` top-level block.
#[derive(Debug, Clone, PartialEq)]
pub struct MetaBlock {
    pub fields: Vec<SchemaField>,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// The source-ordered `contributors` top-level block.
#[derive(Debug, Clone, PartialEq)]
pub struct ContributorsBlock {
    pub people: Vec<ContributorDeclaration>,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// A named contributor declaration before identity/static validation.
#[derive(Debug, Clone, PartialEq)]
pub struct ContributorDeclaration {
    pub name: String,
    pub name_span: SourceSpan,
    pub fields: Vec<SchemaField>,
    pub span: SourceSpan,
}

/// The source-ordered `credits` top-level block.
#[derive(Debug, Clone, PartialEq)]
pub struct CreditsBlock {
    pub entries: Vec<CreditEntry>,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// A credit record whose display and reference semantics belong to later phases.
#[derive(Debug, Clone, PartialEq)]
pub struct CreditEntry {
    pub fields: Vec<SchemaField>,
    pub span: SourceSpan,
}

/// The source-ordered `resources` top-level block.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourcesBlock {
    pub resources: Vec<ResourceDeclaration>,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// A Core resource kind spelling. Resource legality and payload resolution are later-phase work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Audio,
    Image,
    Font,
    Texture,
    Path,
    Shader,
    Binary,
}

/// A named resource declaration before path/hash/reference validation.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceDeclaration {
    pub kind: ResourceKind,
    pub kind_span: SourceSpan,
    pub name: String,
    pub name_span: SourceSpan,
    pub fields: Vec<SchemaField>,
    pub span: SourceSpan,
}

/// The source-ordered `artwork` top-level block.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtworkBlock {
    pub fields: Vec<SchemaField>,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// The source-ordered `sync` top-level block.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncBlock {
    pub fields: Vec<SchemaField>,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}
