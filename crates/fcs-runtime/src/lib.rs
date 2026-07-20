//! Product-owned evaluation foundations for FCS runtime descriptors.
//!
//! I4.1 establishes the Core easing catalog, I4.2 adds canonical Track
//! evaluation, I4.3 adds deterministic Line transforms, and I4.4-I4.7 add
//! direct-seek Line scroll evaluation through bounded exact integration.
//! Independent reference evaluation remains a later I4 unit.

mod descriptor;
mod easing;
mod expression;
mod scroll;
mod track;
mod transform;

pub use descriptor::{DescriptorEvaluationError, evaluate_descriptor};
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
