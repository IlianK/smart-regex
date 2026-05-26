//! Basic matching algorithms (boolean match only, no parse tree)

mod match_naive;
mod match_deriv;
mod match_pderiv;

pub use match_naive::match_naive;
pub use match_deriv::match_deriv;
pub use match_pderiv::match_pderiv;

// Re-export for convenience
pub use crate::derivatives::standard::*;
pub use crate::types::Regex;