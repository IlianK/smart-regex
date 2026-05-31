//! regex-engine/src/matchers/mod.rs
//! 
//! Boolean matchers (match only, no parse tree)

mod match_naive;
mod match_deriv;
mod match_pderiv;
pub mod selection;

pub use match_naive::match_naive;
pub use match_deriv::match_deriv;
pub use match_pderiv::match_pderiv;
pub use selection::MatcherType;