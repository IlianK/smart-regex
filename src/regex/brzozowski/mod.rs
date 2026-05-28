//! Brzozowski derivatives (deterministic)

pub mod standard;
pub mod annotated;

pub use standard::deriv;
pub use annotated::deriv_bc;