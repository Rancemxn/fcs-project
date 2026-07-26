//! Source-owned Render Profile scene grammar nodes.
//!
//! The Core parser preserves the `render` payload as a balanced group; a
//! Render-aware parser consumes that payload against the grammar in
//! `fcs-render.md` section 2 and produces the nodes here. These types are
//! grammar-shaped only: field meaning, enum membership and node-kind schema
//! belong to Render static validation, not to parsing.

use super::{EntityExpression, Generator, SchemaField, SourceExpression, SourceSpan, TracksBlock};

/// A parsed `render profile <semver> { ... }` payload.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderScene {
    pub viewport: RenderViewport,
    pub layers: Vec<RenderLayerDeclaration>,
    pub span: SourceSpan,
}

/// The `viewport` block. Exactly one is required, before any layer.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderViewport {
    pub fields: Vec<SchemaField>,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// A named `layer` declaration and its ordered body items.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderLayerDeclaration {
    pub name: String,
    pub name_span: SourceSpan,
    pub items: Vec<RenderBodyItem>,
    pub span: SourceSpan,
}

/// A source item contained by a layer or node body.
///
/// `SchemaField` is far larger than the other variants because a schema value
/// can hold four expressions, so it is boxed to keep the enum small.
#[derive(Debug, Clone, PartialEq)]
pub enum RenderBodyItem {
    Field(Box<SchemaField>),
    Tracks(TracksBlock),
    Children(RenderChildrenBlock),
}

impl RenderBodyItem {
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Field(field) => field.span,
            Self::Tracks(tracks) => tracks.span,
            Self::Children(children) => children.span,
        }
    }
}

/// A `children` block holding the ordered render items of one node.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderChildrenBlock {
    pub items: Vec<RenderItem>,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// One item inside a `children` block.
#[derive(Debug, Clone, PartialEq)]
pub enum RenderItem {
    Node(RenderNodeDeclaration),
    Entity(EntityExpression),
    If(RenderIf),
    Generator(Generator),
}

impl RenderItem {
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Node(node) => node.span,
            Self::Entity(entity) => entity.span(),
            Self::If(branch) => branch.span,
            Self::Generator(generator) => generator.span,
        }
    }
}

/// A compile-time `if`/`else` over render items.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderIf {
    pub condition: SourceExpression,
    pub then_items: Vec<RenderItem>,
    /// Empty when the source has no `else` arm; the parser records the arm's
    /// presence in `else_span` so validation can tell empty from absent.
    pub else_items: Vec<RenderItem>,
    pub else_span: Option<SourceSpan>,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// A named render node declaration and its ordered body items.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderNodeDeclaration {
    pub kind: RenderNodeKind,
    pub kind_span: SourceSpan,
    pub name: String,
    pub name_span: SourceSpan,
    pub items: Vec<RenderBodyItem>,
    pub span: SourceSpan,
}

/// The closed set of render node kinds.
///
/// `line`, `image` and `path` are already Core keywords; every other spelling
/// is a contextual keyword that the Core lexer emits as a plain identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderNodeKind {
    Group,
    ClipGroup,
    Rect,
    RoundedRect,
    Circle,
    Ellipse,
    Line,
    Polyline,
    Polygon,
    Path,
    Image,
    Text,
}

impl RenderNodeKind {
    /// The exact source spelling. Node kinds are structural terminals and are
    /// never quoted, so this is also the identifier the parser matches.
    pub const fn spelling(self) -> &'static str {
        match self {
            Self::Group => "group",
            Self::ClipGroup => "clipGroup",
            Self::Rect => "rect",
            Self::RoundedRect => "roundedRect",
            Self::Circle => "circle",
            Self::Ellipse => "ellipse",
            Self::Line => "line",
            Self::Polyline => "polyline",
            Self::Polygon => "polygon",
            Self::Path => "path",
            Self::Image => "image",
            Self::Text => "text",
        }
    }

    /// Resolves a contextual-keyword spelling. Returns `None` for any other
    /// identifier so the parser reports an unknown node kind rather than
    /// guessing a Core-adjacent meaning.
    pub fn from_spelling(spelling: &str) -> Option<Self> {
        Some(match spelling {
            "group" => Self::Group,
            "clipGroup" => Self::ClipGroup,
            "rect" => Self::Rect,
            "roundedRect" => Self::RoundedRect,
            "circle" => Self::Circle,
            "ellipse" => Self::Ellipse,
            "line" => Self::Line,
            "polyline" => Self::Polyline,
            "polygon" => Self::Polygon,
            "path" => Self::Path,
            "image" => Self::Image,
            "text" => Self::Text,
            _ => return None,
        })
    }

    /// Group and ClipGroup organize children; every other kind draws and so
    /// must carry a kind-compatible geometry.
    pub const fn is_drawable(self) -> bool {
        !matches!(self, Self::Group | Self::ClipGroup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [RenderNodeKind; 12] = [
        RenderNodeKind::Group,
        RenderNodeKind::ClipGroup,
        RenderNodeKind::Rect,
        RenderNodeKind::RoundedRect,
        RenderNodeKind::Circle,
        RenderNodeKind::Ellipse,
        RenderNodeKind::Line,
        RenderNodeKind::Polyline,
        RenderNodeKind::Polygon,
        RenderNodeKind::Path,
        RenderNodeKind::Image,
        RenderNodeKind::Text,
    ];

    #[test]
    fn every_kind_round_trips_through_its_source_spelling() {
        for kind in ALL {
            assert_eq!(RenderNodeKind::from_spelling(kind.spelling()), Some(kind));
        }
    }

    #[test]
    fn unknown_and_miscased_spellings_are_not_node_kinds() {
        for spelling in [
            "Rect",
            "RECT",
            "clipgroup",
            "roundedrect",
            "layer",
            "shape",
            "",
        ] {
            assert_eq!(
                RenderNodeKind::from_spelling(spelling),
                None,
                "{spelling} must not resolve"
            );
        }
    }

    #[test]
    fn only_group_and_clip_group_are_non_drawable() {
        let non_drawable: Vec<&str> = ALL
            .iter()
            .filter(|kind| !kind.is_drawable())
            .map(|kind| kind.spelling())
            .collect();
        assert_eq!(non_drawable, ["group", "clipGroup"]);
    }
}
