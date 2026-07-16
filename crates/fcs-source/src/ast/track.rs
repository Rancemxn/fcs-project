//! Source-owned Line, Track, and segment grammar nodes.

use super::{
    EntityField, FieldPath, Generator, SourceBlock, SourceExpression, SourceSpan, SourceType, Type,
};

/// The source-ordered `lines` top-level block.
#[derive(Debug, Clone, PartialEq)]
pub struct LinesBlock {
    pub lines: Vec<LineDeclaration>,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// A named Line declaration and its ordered entity-body items.
#[derive(Debug, Clone, PartialEq)]
pub struct LineDeclaration {
    pub name: String,
    pub name_span: SourceSpan,
    pub items: Vec<LineBodyItem>,
    pub span: SourceSpan,
}

/// A source item contained by a Line entity block.
#[derive(Debug, Clone, PartialEq)]
pub enum LineBodyItem {
    Field(EntityField),
    Tracks(TracksBlock),
    ScrollTempoMap(SourceBlock),
}

impl LineBodyItem {
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Field(field) => field.span,
            Self::Tracks(tracks) => tracks.span,
            Self::ScrollTempoMap(block) => block.span,
        }
    }
}

/// A Line-owned ordered collection of Track declarations.
#[derive(Debug, Clone, PartialEq)]
pub struct TracksBlock {
    pub tracks: Vec<TrackDeclaration>,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// A typed Track source declaration before schema/static validation.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackDeclaration {
    pub name: String,
    pub name_span: SourceSpan,
    pub target: FieldPath,
    pub value_type: Type,
    pub value_type_source: SourceType,
    pub settings: Vec<TrackSetting>,
    pub segments: SegmentsBlock,
    pub span: SourceSpan,
}

/// An ordered Track setting assignment.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackSetting {
    pub name: String,
    pub name_span: SourceSpan,
    pub value: SourceExpression,
    pub span: SourceSpan,
}

/// A Track's required `segments` collection.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentsBlock {
    pub items: Vec<TrackSegmentItem>,
    pub span: SourceSpan,
    pub keyword_span: SourceSpan,
}

/// A source item in a Track segments collection.
#[derive(Debug, Clone, PartialEq)]
pub enum TrackSegmentItem {
    DirectSegment(DirectSegment),
    DirectPoint(DirectPoint),
    Generator(Generator),
    Conditional {
        condition: SourceExpression,
        then_items: Vec<TrackSegmentItem>,
        else_items: Vec<TrackSegmentItem>,
        span: SourceSpan,
    },
}

impl TrackSegmentItem {
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::DirectSegment(segment) => segment.span,
            Self::DirectPoint(point) => point.span,
            Self::Generator(generator) => generator.span,
            Self::Conditional { span, .. } => *span,
        }
    }
}

/// A schema-owned half-open interval used only by direct Track segments.
#[derive(Debug, Clone, PartialEq)]
pub struct HalfOpenInterval {
    pub start: SourceExpression,
    pub end: SourceExpression,
    pub span: SourceSpan,
}

/// A direct Track segment with source interpolation syntax retained.
#[derive(Debug, Clone, PartialEq)]
pub struct DirectSegment {
    pub interval: HalfOpenInterval,
    pub start_value: SourceExpression,
    pub end_value: SourceExpression,
    pub interpolation: Interpolation,
    pub span: SourceSpan,
}

/// A direct instantaneous Track point.
#[derive(Debug, Clone, PartialEq)]
pub struct DirectPoint {
    pub time: SourceExpression,
    pub value: SourceExpression,
    pub span: SourceSpan,
}

/// The schema-owned interpolation production.
#[derive(Debug, Clone, PartialEq)]
pub enum Interpolation {
    Expression(SourceExpression),
    CubicBezier {
        values: [SourceExpression; 4],
        span: SourceSpan,
    },
}

impl Interpolation {
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Expression(expression) => expression.span(),
            Self::CubicBezier { span, .. } => *span,
        }
    }
}
