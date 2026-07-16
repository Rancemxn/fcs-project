//! Source-owned extension, preserve, and envelope grammar nodes.

use crate::version::Version;

use super::{SchemaField, SourceObjectEntry, SourceSpan};

/// The source-ordered `extensions` top-level block.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionsBlock {
    pub declarations: Vec<ExtensionDeclaration>,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// An extension declaration before namespace capability/static validation.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionDeclaration {
    pub header: ExtensionHeader,
    pub requirement: ExtensionRequirement,
    pub requirement_span: SourceSpan,
    pub payload: OrderedObject,
    pub span: SourceSpan,
}

/// The namespace and schema version shared by extension and preserve payloads.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionHeader {
    pub namespace: String,
    pub namespace_span: SourceSpan,
    pub version: Version,
    pub version_span: SourceSpan,
    pub span: SourceSpan,
}

/// Whether an extension is required for canonical execution or merely optional metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionRequirement {
    Required,
    Optional,
}

/// An ordered Core object retained as a distinct envelope payload node.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderedObject {
    pub entries: Vec<SourceObjectEntry>,
    pub span: SourceSpan,
}

/// The source-ordered `preserve` top-level block.
#[derive(Debug, Clone, PartialEq)]
pub struct PreserveBlock {
    pub items: Vec<PreserveItem>,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// A source or payload entry retained in its original order.
#[derive(Debug, Clone, PartialEq)]
pub enum PreserveItem {
    Source(PreserveSource),
    Payload(PreservePayload),
}

impl PreserveItem {
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Source(source) => source.span,
            Self::Payload(payload) => payload.span,
        }
    }
}

/// The schema block describing the preserved source origin.
#[derive(Debug, Clone, PartialEq)]
pub struct PreserveSource {
    pub fields: Vec<SchemaField>,
    pub span: SourceSpan,
}

/// A preserve payload with an extension header but no execution requirement.
#[derive(Debug, Clone, PartialEq)]
pub struct PreservePayload {
    pub header: ExtensionHeader,
    pub payload: OrderedObject,
    pub span: SourceSpan,
}
