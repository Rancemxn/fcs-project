//! FCS (Functional Chart Specification) core library.
//!
//! The existing public modules implement the current FCS v4 toolchain. The
//! versioned `v5` module contains the staged FCS 5 front end until final cutover.

pub mod ast;
pub mod bytecode;
pub mod compiler;
pub mod error;
pub mod parser;
pub mod units;
pub mod v5;
pub mod vm;
