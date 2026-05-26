//! regex_engine

pub mod types;
pub mod derivatives;
pub mod basic;
pub mod posix;
pub mod demo;

// Re-export commonly used types and functions
pub use types::{Regex, ParseTree};
pub use basic::{match_naive, match_deriv, match_pderiv};
pub use derivatives::standard::{nullable, deriv, deriv_simp, pderiv};
pub use posix::{parse_posix, match_posix, flatten, mk_eps, inject};