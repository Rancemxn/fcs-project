//! Product-owned evaluation foundations for FCS runtime descriptors.
//!
//! I4.1 establishes the Core easing catalog and I4.2 adds canonical Track
//! evaluation. Transform, scroll, Expression DAG, and independent reference
//! evaluation remain later I4 units.

mod easing;
mod track;

pub use easing::{EasingError, EasingId, evaluate_easing};
pub use track::{TrackEvaluationError, evaluate_track_set};
