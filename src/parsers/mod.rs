//! src/parsers/mod.rs

pub mod deriv_std;
pub mod deriv_bc;
pub mod pderiv_std;
pub mod pderiv_bc;
pub mod parser;
pub mod selection;

// Re-export parsers
pub use parser::parse;
pub use deriv_std::{parse_deriv_std_rec, parse_deriv_std_loop, parse_deriv_std_rec_timed, parse_deriv_std_loop_timed, parse_loop_traced, parse_recursive_traced};
pub use pderiv_std::{parse_pderiv_std, parse_pderiv_std_traced, parse_pderiv_std_timed};
pub use deriv_bc::{parse_deriv_bc, parse_bitcoded_traced, parse_bitcoded_recursive_timed};
pub use pderiv_bc::{parse_pderiv_bc, parse_pderiv_bc_traced, parse_pderiv_bc_timed};

/// Timing breakdown for all five parsers, split into three
/// positionally-aligned phases so all five report comparable numbers
/// in one table. What each phase is made of is NOT the same across
/// parsers -- only its position in the parse is:
///   - `setup_us`: work done before any character is consumed. For
///     deriv_bc this is a real encode (Regex -> ARegex via
///     `internalize`, building the bit-annotated representation);
///     pderiv_bc/pderiv_std have no such conversion, so their
///     `setup_us` is just the initial `r.clone()` into the starting
///     frontier (a plain tree clone, no bits or injection closures
///     involved), timed as its own phase purely so all three
///     non-recursive parsers' breakdowns stay comparable -- it is a
///     real, nonzero number, not folded into `derive_us`. Calling it
///     an "encode" the way deriv_bc's is would overclaim what it does.
///     deriv_std_rec/deriv_std_loop are the ones whose `setup_us` is
///     always 0: neither ever builds a frontier or wrapper before
///     deriving, so there is nothing to time here at all; see
///     `parse_deriv_std_loop_timed`'s doc comment.
///   - `derive_us`: the per-character stepping loop, for all five.
///   - `reconstruct_us`: producing the final `ParseTree` from whatever
///     witness survived. For deriv_bc/pderiv_bc this is a genuine
///     decode (interpreting an accumulated bit string via `decode`);
///     for pderiv_std there are no bits anywhere, and this is instead
///     `Inj::apply`, running the composed chain of injection closures
///     built up during the derive phase; for deriv_std_rec/
///     deriv_std_loop it is the backward injection pass itself
///     (`inject`, not `Inj::apply` -- these two build the parse tree
///     directly during that pass, with no separate closure-application
///     step). Calling pderiv_std's number a "decode" would overclaim
///     what it does -- it is reconstruction by closure application,
///     not by interpreting an encoded representation.
///
/// deriv_std_rec's own derive_us/reconstruct_us carry more measurement
/// overhead than the other four parsers': its two phases are
/// interleaved in a single recursive descent rather than two separate
/// passes, so isolating them costs roughly two extra clock reads per
/// input character, instead of the two or three total that the other
/// parsers' phase timing costs regardless of input length. See
/// `parse_deriv_std_rec_timed`'s doc comment for why this is the
/// accepted trade-off rather than something to design around.
#[derive(Debug, Clone, Copy, Default)]
pub struct PhaseTiming {
    pub setup_us: u64,
    pub derive_us: u64,
    pub reconstruct_us: u64,
}

// Re-export types
pub use crate::types::{ParseTree, flatten};
pub use selection::{ParserType, ALL};
