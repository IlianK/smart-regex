//! Bit-coded partial-derivative parser, plain Regex + external bit-vector (Greedy).

pub mod pderiv;
pub mod parse;
pub mod traced;

pub use pderiv::{pderiv_bc, mk_eps_bits};
pub use parse::parse_pderiv_bc;
pub use traced::parse_pderiv_bc_traced;
