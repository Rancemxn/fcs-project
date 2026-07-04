//! FCS (Functional Chart Specification) core library.
//!
//! Provides parsing, compilation, and bytecode handling for the FCS v4.0.0
//! music game chart format.

pub mod ast;
pub mod bytecode;
pub mod compiler;
pub mod error;
pub mod parser;
pub mod units;
pub mod vm;
