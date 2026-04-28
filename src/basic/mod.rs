//! Basic matching algorithms
//!
//! This module contains the three fundamental approaches:
//! 1. Naive recursive matcher
//! 2. Brzozowski derivatives (DFA)
//! 3. Antimirov partial derivatives (NFA)

mod regex;
mod naive;
mod brzozowski;
mod antimirov;
mod common;

// Re-exports
pub use regex::Regex;
pub use naive::match_naive;
pub use brzozowski::{match_deriv, deriv, deriv_simp};
pub use antimirov::match_pderiv;
pub use common::{simplify, smart_seq, nullable};