//! Compiler-facing lowering from the existing typed VB6 HIR to canonical,
//! versioned VB6R P-Code. This crate does not execute generated programs.

#![warn(missing_docs)]

mod compiler;

pub use compiler::{CompileDiagnostic, PCodeBuilder, compile_hir, compile_source};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
