//! Product-owned evaluation foundations for FCS runtime descriptors.
//!
//! I4.1 establishes the Core easing catalog, I4.2 adds canonical Track
//! evaluation, I4.3 adds deterministic Line transform evaluation, and I4.4
//! adds direct-seek Line scroll evaluation. Expression DAG and independent
//! reference evaluation remain later I4 units.

mod easing;
mod expression;
mod scroll;
mod track;
mod transform;

pub use easing::{EasingError, EasingId, evaluate_easing};
pub use expression::{ExpressionEnvironment, ExpressionEvaluationError, evaluate_expression};
pub use scroll::{
    EvaluatedLineScroll, LineScrollDistance, ScrollEvaluationError, evaluate_line_scroll,
    evaluate_note_distance,
};
pub use track::{TrackEvaluationError, evaluate_track_set};
pub use transform::{
    EvaluatedLineComponents, EvaluatedLineTransform, LineTransformError, LineTransformMatrix,
    evaluate_line_transform,
};
