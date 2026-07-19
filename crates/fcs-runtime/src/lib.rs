//! Product-owned evaluation foundations for FCS runtime descriptors.
//!
//! I4.1 establishes the Core easing catalog. Track, transform, scroll,
//! Expression DAG, and independent reference evaluation remain later I4 units.

mod easing;

pub use easing::{EasingError, EasingId, evaluate_easing};
