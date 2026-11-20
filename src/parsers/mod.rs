//! src/parsers/mod.rs

pub mod deriv_std;
pub mod deriv_bc;
pub mod pderiv_std;
pub mod pderiv_bc;
pub mod parser;
pub mod selection;

// Re-export parsers
pub use parser::parse;
pub use deriv_std::{parse_deriv_std_rec, parse_deriv_std_loop, parse_loop_traced, parse_recursive_traced};
pub use pderiv_std::{parse_pderiv_std, parse_pderiv_std_traced, parse_pderiv_std_timed};
pub use deriv_bc::{parse_deriv_bc, parse_bitcoded_traced, parse_bitcoded_recursive_timed};
pub use pderiv_bc::{parse_pderiv_bc, parse_pderiv_bc_traced, parse_pderiv_bc_timed};

/// Timing breakdown for the non-recursive parsers (deriv_bc, pderiv_bc,
/// pderiv_std): how much of a parse was encode (building the initial
/// state -- Regex -> ARegex via `internalize` for deriv_bc; a cheaper
/// clone into the initial frontier for pderiv_bc and pderiv_std), derive
/// (the per-character stepping), and decode (bits or injection closure
/// -> ParseTree).
#[derive(Debug, Clone, Copy, Default)]
pub struct BcTiming {
    pub encode_us: u64,
    pub derive_us: u64,
    pub decode_us: u64,
}

// Re-export types
pub use crate::types::{ParseTree, flatten};
pub use selection::{ParserType, ALL};
