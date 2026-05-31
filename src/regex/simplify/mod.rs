//! regex-engine/src/regex/simplify/mod.rs
//! 
//! Simplification rules for regular expressions

pub mod standard;
pub mod annotated;

pub use standard::{simplify, smart_seq};
pub use annotated::simp;