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
/// pderiv_std), split into three positionally-aligned phases so all
/// three report comparable numbers in one table. What each phase is
/// made of is NOT the same across parsers -- only its position in the
/// parse is:
///   - `setup_us`: work done before any character is consumed. For
///     deriv_bc this is a real encode (Regex -> ARegex via
///     `internalize`, building the bit-annotated representation); for
///     pderiv_bc and pderiv_std there is no annotated representation
///     at all, so this is just the clone into the initial frontier.
///   - `derive_us`: the per-character stepping loop, for all three.
///   - `reconstruct_us`: producing the final `ParseTree` from whatever
///     per-strand witness survived. For deriv_bc/pderiv_bc this is a
///     genuine decode (interpreting an accumulated bit string via
///     `decode`); for pderiv_std there are no bits anywhere, and this
///     is instead `Inj::apply`, running the composed chain of
///     injection closures built up during the derive phase. Calling
///     pderiv_std's number a "decode" would overclaim what it does --
///     it is reconstruction by closure application, not by
///     interpreting an encoded representation.
#[derive(Debug, Clone, Copy, Default)]
pub struct PhaseTiming {
    pub setup_us: u64,
    pub derive_us: u64,
    pub reconstruct_us: u64,
}

// Re-export types
pub use crate::types::{ParseTree, flatten};
pub use selection::{ParserType, ALL};
