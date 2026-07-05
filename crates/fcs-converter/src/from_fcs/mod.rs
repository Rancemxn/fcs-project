//! FCS → Phigros format exporters (reverse direction).
//!
//! Unlike the `pgr`/`rpe`/`pec` import modules, this module works from
//! the FCS AST directly — evaluating expressions, sampling motion curves,
//! and producing the discrete keyframe/event model that Phigros formats expect.

pub mod autofill;
pub mod controls;
pub mod coord;
pub mod easing_map;
pub mod evaluator;
pub mod flattener;
pub mod note_util;
pub mod pec_writer;
pub mod warnings;
pub mod pgr_writer;
pub mod rpe_writer;
pub mod time;
