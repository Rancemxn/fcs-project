//! Product-owned evaluation foundations for FCS runtime descriptors.
//!
//! I4.1 establishes the Core easing catalog, I4.2 adds canonical Track
//! evaluation, and I4.3 adds deterministic Line transform evaluation. Scroll,
//! Expression DAG, and independent reference evaluation remain later I4 units.

mod easing;
mod track;
mod transform;

pub use easing::{EasingError, EasingId, evaluate_easing};
pub use track::{TrackEvaluationError, evaluate_track_set};
pub use transform::{
    EvaluatedLineComponents, EvaluatedLineTransform, LineTransformError, LineTransformMatrix,
    evaluate_line_transform,
};
