//! Source-owned Render Profile scene grammar nodes.
//!
//! The Core parser preserves the `render` payload as a balanced group; a
//! Render-aware parser consumes that payload against the grammar in
//! `fcs-render.md` section 2 and produces the nodes here. These types are
//! grammar-shaped only: field meaning, enum membership and node-kind schema
//! belong to Render static validation, not to parsing. Node kinds therefore
//! reuse `fcs_model::CanonicalRenderNodeKind` and only their source spellings
//! are defined here.

use fcs_model::CanonicalRenderNodeKind;

use super::{EntityExpression, Generator, SchemaField, SourceExpression, SourceSpan, TracksBlock};

/// The body of a `render profile <semver> { ... }` payload.
///
/// The declared semver is not repeated here: it stays on `RenderBlock::version`,
/// which is what the section 2 profile-version gate reads.
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
///
/// A generator here is an ordinary Core `Generator` whose owner is
/// `GeneratorOwner::RenderChildren`, so expansion diagnostics are framed by the
/// enclosing layer or node rather than by a fabricated collection.
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
    /// Empty when the source has no `else` arm. Section 3.2 expands the
    /// construct away before the canonical scene graph, so an empty arm and an
    /// absent arm contribute the same nothing and are not distinguished.
    pub else_items: Vec<RenderItem>,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// A named render node declaration and its ordered body items.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderNodeDeclaration {
    pub kind: CanonicalRenderNodeKind,
    pub kind_span: SourceSpan,
    pub name: String,
    pub name_span: SourceSpan,
    pub items: Vec<RenderBodyItem>,
    pub span: SourceSpan,
}

/// The exact source spelling of a node kind.
///
/// The kind set is closed and identical on both sides of lowering, so the
/// canonical enum is the single definition of it; only the spellings are
/// grammar-owned. The match is exhaustive, so a future kind added to the
/// canonical enum fails to compile here instead of diverging silently.
///
/// `line`, `image` and `path` are already Core keywords; every other spelling
/// is a contextual keyword that the Core lexer emits as a plain identifier.
/// Node kinds are structural terminals and are never quoted, so this is also
/// the identifier the parser matches.
pub const fn render_node_kind_spelling(kind: CanonicalRenderNodeKind) -> &'static str {
    match kind {
        CanonicalRenderNodeKind::Group => "group",
        CanonicalRenderNodeKind::ClipGroup => "clipGroup",
        CanonicalRenderNodeKind::Rect => "rect",
        CanonicalRenderNodeKind::RoundedRect => "roundedRect",
        CanonicalRenderNodeKind::Circle => "circle",
        CanonicalRenderNodeKind::Ellipse => "ellipse",
        CanonicalRenderNodeKind::Line => "line",
        CanonicalRenderNodeKind::Polyline => "polyline",
        CanonicalRenderNodeKind::Polygon => "polygon",
        CanonicalRenderNodeKind::Path => "path",
        CanonicalRenderNodeKind::Image => "image",
        CanonicalRenderNodeKind::Text => "text",
    }
}

/// Resolves a contextual-keyword spelling. Returns `None` for any other
/// identifier so the parser reports an unknown node kind rather than guessing a
/// Core-adjacent meaning.
pub fn render_node_kind_from_spelling(spelling: &str) -> Option<CanonicalRenderNodeKind> {
    Some(match spelling {
        "group" => CanonicalRenderNodeKind::Group,
        "clipGroup" => CanonicalRenderNodeKind::ClipGroup,
        "rect" => CanonicalRenderNodeKind::Rect,
        "roundedRect" => CanonicalRenderNodeKind::RoundedRect,
        "circle" => CanonicalRenderNodeKind::Circle,
        "ellipse" => CanonicalRenderNodeKind::Ellipse,
        "line" => CanonicalRenderNodeKind::Line,
        "polyline" => CanonicalRenderNodeKind::Polyline,
        "polygon" => CanonicalRenderNodeKind::Polygon,
        "path" => CanonicalRenderNodeKind::Path,
        "image" => CanonicalRenderNodeKind::Image,
        "text" => CanonicalRenderNodeKind::Text,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{
        FieldPath, GeneratorOwner, SchemaValue, SourceExpression, SourceLiteral, SourceRange, Type,
    };

    /// Every spelling section 2 lists for `renderNodeKind`, in grammar order.
    const SPELLINGS: [&str; 12] = [
        "group",
        "clipGroup",
        "rect",
        "roundedRect",
        "circle",
        "ellipse",
        "line",
        "polyline",
        "polygon",
        "path",
        "image",
        "text",
    ];

    fn span(start: usize, end: usize) -> SourceSpan {
        SourceSpan::new(start, end)
    }

    fn field(start: usize, end: usize) -> SchemaField {
        SchemaField {
            path: FieldPath {
                segments: vec!["opacity".to_owned()],
                span: span(start, start),
            },
            value: SchemaValue::Expression(SourceExpression::Literal {
                literal: SourceLiteral::Bool(true),
                span: span(start, end),
            }),
            span: span(start, end),
        }
    }

    #[test]
    fn every_grammar_spelling_round_trips_through_the_canonical_kind() {
        for spelling in SPELLINGS {
            let kind =
                render_node_kind_from_spelling(spelling).expect("grammar spelling must resolve");
            assert_eq!(render_node_kind_spelling(kind), spelling);
        }
    }

    #[test]
    fn the_grammar_spellings_cover_every_canonical_kind_ordinal() {
        let mut ordinals: Vec<u16> = SPELLINGS
            .iter()
            .map(|spelling| {
                render_node_kind_from_spelling(spelling)
                    .expect("grammar spelling must resolve")
                    .ordinal()
            })
            .collect();
        ordinals.sort_unstable();
        ordinals.dedup();
        assert_eq!(ordinals, (1..=12).collect::<Vec<u16>>());
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
                render_node_kind_from_spelling(spelling),
                None,
                "{spelling} must not resolve"
            );
        }
    }

    #[test]
    fn body_item_span_reports_the_wrapped_item_span() {
        let items = [
            RenderBodyItem::Field(Box::new(field(1, 4))),
            RenderBodyItem::Tracks(TracksBlock {
                tracks: Vec::new(),
                span: span(5, 9),
                keyword_span: span(5, 6),
            }),
            RenderBodyItem::Children(RenderChildrenBlock {
                items: Vec::new(),
                span: span(10, 16),
                keyword_span: span(10, 11),
            }),
        ];
        let spans: Vec<SourceSpan> = items.iter().map(RenderBodyItem::span).collect();
        assert_eq!(spans, [span(1, 4), span(5, 9), span(10, 16)]);
    }

    #[test]
    fn render_item_span_reports_the_wrapped_item_span() {
        let items = [
            RenderItem::Node(RenderNodeDeclaration {
                kind: CanonicalRenderNodeKind::Rect,
                kind_span: span(1, 5),
                name: "bar".to_owned(),
                name_span: span(6, 9),
                items: Vec::new(),
                span: span(1, 12),
            }),
            RenderItem::Entity(EntityExpression::Source(SourceExpression::Name {
                name: "tick".to_owned(),
                span: span(13, 17),
            })),
            RenderItem::If(RenderIf {
                condition: SourceExpression::Literal {
                    literal: SourceLiteral::Bool(true),
                    span: span(21, 25),
                },
                then_items: Vec::new(),
                else_items: Vec::new(),
                span: span(18, 30),
                keyword_span: span(18, 20),
            }),
            RenderItem::Generator(Generator {
                owner: Box::new(GeneratorOwner::RenderChildren {
                    name: "background".to_owned(),
                }),
                variable: "i".to_owned(),
                variable_span: span(40, 41),
                variable_type: Type::Int,
                range: SourceRange {
                    start: SourceExpression::Literal {
                        literal: SourceLiteral::Int(0),
                        span: span(42, 43),
                    },
                    end: SourceExpression::Literal {
                        literal: SourceLiteral::Int(8),
                        span: span(46, 47),
                    },
                    step: SourceExpression::Literal {
                        literal: SourceLiteral::Int(1),
                        span: span(53, 54),
                    },
                    inclusive_end: false,
                    span: span(42, 54),
                },
                body: Vec::new(),
                span: span(31, 60),
            }),
        ];
        let spans: Vec<SourceSpan> = items.iter().map(RenderItem::span).collect();
        assert_eq!(
            spans,
            [span(1, 12), span(13, 17), span(18, 30), span(31, 60)]
        );
    }
}
