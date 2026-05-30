//! Re-exports trace types from the crate root (src/trace.rs).
//!
//! Diagnostics code can import from either crate::trace or crate::diagnostics::trace.
//! Posix code imports directly from crate::trace to avoid a circular dependency.

pub use crate::trace::{
    DerivStep, InjectStep, MkEpsResult, ParseTrace,
    BitStep, BitTrace,
};