//! regex-engine/src/regex/antimirov/mod.rs
//! 
//! Antimirov partial derivatives (non-deterministic, set-based)

pub mod standard;
pub mod annotated;

pub use standard::pderiv;
// pub use annotated::pderiv_bc;  