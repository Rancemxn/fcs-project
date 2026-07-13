mod time;
mod types;

pub use time::{Beat, BeatError, Bpm, InvalidBpm};
pub use types::{
    BinaryOperator, SourceExpression, SourceLiteral, SourceSpan, Type, TypedExpression,
    TypedExpressionKind, TypedValue, UnaryOperator,
};

use crate::v5::version::Version;

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
