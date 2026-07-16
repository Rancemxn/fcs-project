//! Source document envelope and source-ordered top-level declarations.

use super::{
    CollectionsBlock, DefinitionsBlock, LineDeclaration, LinesBlock, SourceSpan, TempoMap,
};
use crate::version::{FCS_SOURCE_VERSION, Version};

/// The profile declared by a document's mandatory `format` block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentProfile {
    Fragment,
    Chart,
    Playable,
    Renderable,
    Publishable,
}

/// A capability feature listed by a `format.features` array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileFeature {
    Playable,
    Renderable,
}

/// The source span and value of the `profile` format field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatProfile {
    pub value: DocumentProfile,
    pub span: SourceSpan,
    pub name_span: SourceSpan,
}

/// One ordered entry in a format feature array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatFeature {
    pub value: ProfileFeature,
    pub span: SourceSpan,
}

/// The source-ordered `features` list in a format block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureList {
    pub features: Vec<FormatFeature>,
    pub span: SourceSpan,
    pub name_span: SourceSpan,
}

/// The complete syntax of the mandatory document format block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatBlock {
    pub profile: FormatProfile,
    pub features: Option<FeatureList>,
    /// Format fields remain source ordered for duplicate/error reporting.
    pub fields: Vec<FormatField>,
    pub span: SourceSpan,
    pub close_span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// One source-ordered format field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatField {
    Profile(FormatProfile),
    Features(FeatureList),
}

/// A balanced Core token group retained for an envelope whose owning semantic
/// grammar belongs to a later phase (for example Render or an extension body).
///
/// The public AST deliberately exposes only source spans and balanced structure;
/// lexer/Chumsky token types remain crate-private.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceGroup {
    pub open_span: SourceSpan,
    pub close_span: SourceSpan,
    pub elements: Vec<SourceElement>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceElement {
    Atom(SourceSpan),
    Group(SourceGroup),
}

/// A generic balanced top-level block. Its block kind is carried by the
/// enclosing [`TopLevelBlock`] variant; the group preserves nested source
/// boundaries without inventing later-phase schema semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBlock {
    pub body: SourceGroup,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// The `tempoMap` top-level envelope with its complete block span.
#[derive(Debug, Clone, PartialEq)]
pub struct TempoMapBlock {
    pub map: TempoMap,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// The Render Core envelope. Render payload semantics are owned by I9.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderBlock {
    pub version: Version,
    pub payload: SourceGroup,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// A top-level declaration in source order.
#[derive(Debug, Clone, PartialEq)]
pub enum TopLevelBlock {
    Meta(SourceBlock),
    Contributors(SourceBlock),
    Credits(SourceBlock),
    Resources(SourceBlock),
    Artwork(SourceBlock),
    Sync(SourceBlock),
    Definitions(DefinitionsBlock),
    TempoMap(TempoMapBlock),
    Lines(LinesBlock),
    Collections(CollectionsBlock),
    Render(RenderBlock),
    Extensions(SourceBlock),
    Preserve(SourceBlock),
}

/// Stable kind labels for source-order inspection and duplicate checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TopLevelBlockKind {
    Meta,
    Contributors,
    Credits,
    Resources,
    Artwork,
    Sync,
    Definitions,
    TempoMap,
    Lines,
    Collections,
    Render,
    Extensions,
    Preserve,
}

impl TopLevelBlock {
    pub const fn kind(&self) -> TopLevelBlockKind {
        match self {
            Self::Meta(_) => TopLevelBlockKind::Meta,
            Self::Contributors(_) => TopLevelBlockKind::Contributors,
            Self::Credits(_) => TopLevelBlockKind::Credits,
            Self::Resources(_) => TopLevelBlockKind::Resources,
            Self::Artwork(_) => TopLevelBlockKind::Artwork,
            Self::Sync(_) => TopLevelBlockKind::Sync,
            Self::Definitions(_) => TopLevelBlockKind::Definitions,
            Self::TempoMap(_) => TopLevelBlockKind::TempoMap,
            Self::Lines(_) => TopLevelBlockKind::Lines,
            Self::Collections(_) => TopLevelBlockKind::Collections,
            Self::Render(_) => TopLevelBlockKind::Render,
            Self::Extensions(_) => TopLevelBlockKind::Extensions,
            Self::Preserve(_) => TopLevelBlockKind::Preserve,
        }
    }

    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Meta(block)
            | Self::Contributors(block)
            | Self::Credits(block)
            | Self::Resources(block)
            | Self::Artwork(block)
            | Self::Sync(block)
            | Self::Extensions(block)
            | Self::Preserve(block) => block.span,
            Self::Lines(block) => block.span,
            Self::Definitions(block) => block.span,
            Self::TempoMap(block) => block.span,
            Self::Collections(block) => block.span,
            Self::Render(block) => block.span,
        }
    }
}

/// A parsed FCS source document.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub source_version: Version,
    pub format: FormatBlock,
    /// Convenience projection of `format.profile` retained for existing callers.
    pub profile: DocumentProfile,
    pub tempo_map: Option<TempoMap>,
    pub definitions: Option<DefinitionsBlock>,
    pub collections: Vec<super::CollectionBlock>,
    pub lines: Vec<LineDeclaration>,
    top_level_blocks: Vec<TopLevelBlock>,
}

impl Document {
    pub(crate) fn new(
        source_version: Version,
        format: FormatBlock,
        top_level_blocks: Vec<TopLevelBlock>,
    ) -> Self {
        let profile = format.profile.value;
        let tempo_map = top_level_blocks.iter().find_map(|block| match block {
            TopLevelBlock::TempoMap(block) => Some(block.map.clone()),
            _ => None,
        });
        let definitions = top_level_blocks.iter().find_map(|block| match block {
            TopLevelBlock::Definitions(block) => Some(block.clone()),
            _ => None,
        });
        let collections = top_level_blocks
            .iter()
            .find_map(|block| match block {
                TopLevelBlock::Collections(block) => Some(block.collections.clone()),
                _ => None,
            })
            .unwrap_or_default();
        let lines = top_level_blocks
            .iter()
            .find_map(|block| match block {
                TopLevelBlock::Lines(block) => Some(block.lines.clone()),
                _ => None,
            })
            .unwrap_or_default();
        Self {
            source_version,
            format,
            profile,
            tempo_map,
            definitions,
            collections,
            lines,
            top_level_blocks,
        }
    }

    /// Returns every top-level block in source order, including blocks that
    /// are not consumed by the current semantic phase.
    pub fn top_level_blocks(&self) -> &[TopLevelBlock] {
        &self.top_level_blocks
    }

    /// Returns the first block of a kind, if present.
    pub fn top_level(&self, kind: TopLevelBlockKind) -> Option<&TopLevelBlock> {
        self.top_level_blocks
            .iter()
            .find(|block| block.kind() == kind)
    }
}

impl Default for FormatBlock {
    fn default() -> Self {
        let span = SourceSpan::new(0, 0);
        Self {
            profile: FormatProfile {
                value: DocumentProfile::Fragment,
                span,
                name_span: span,
            },
            features: None,
            fields: Vec::new(),
            span,
            close_span: span,
            keyword_span: span,
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new(FCS_SOURCE_VERSION, FormatBlock::default(), Vec::new())
    }
}
