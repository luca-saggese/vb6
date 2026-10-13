//! Compiler-facing lowering from typed VB6 HIR to Microsoft P-Code identities
//! in a versioned modern container. This crate does not execute programs.

#![warn(missing_docs)]

mod compiler;

pub use compiler::{CompileDiagnostic, PCodeBuilder, compile_hir, compile_source};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
