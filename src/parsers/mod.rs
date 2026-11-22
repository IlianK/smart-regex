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

/// Timing breakdown for all five parsers, split into four
/// positionally-aligned phases that sum to the call's true wall-clock
/// cost. What each phase measures differs by parser; only its position
/// in the parse is common:
///   - `setup_us`: work before the first character is consumed.
///     deriv_bc: the Regex -> ARegex encode (`internalize`). pderiv_std:
///     just the initial `r.clone()` into the starting frontier (not a
///     real encode, timed anyway for comparability). pderiv_bc: that
///     same initial clone plus the accumulated time `flatten_alt`
///     spends tagging `Alt` branches with bit prefixes -- the
///     per-character equivalent of what `internalize` does once
///     upfront for deriv_bc, pulled out so the two bit-coded parsers'
///     numbers stay comparable (`Star`'s own tagging stays in
///     `derive_us` on both, since `internalize` doesn't tag `Star`
///     either). deriv_std_rec/loop: always 0 -- neither builds a
///     frontier or wrapper upfront.
///   - `derive_us`: the per-character stepping loop, for all five
///     (pderiv_bc's `setup_us` portion subtracted back out so the four
///     fields sum correctly). Also covers dropping each step's own
///     retained state that isn't needed past this phase, e.g.
///     deriv_std_rec's initial `chars: Vec<char>` collection.
///   - `reconstruct_us`: producing the final `ParseTree`. deriv_bc/
///     pderiv_bc: a genuine decode of the accumulated bits. pderiv_std:
///     `Inj::apply`, running the composed injection-closure chain built
///     during derive (not a decode -- there are no bits). deriv_std_rec/
///     loop: the backward injection pass itself, which builds the tree
///     directly with no separate closure-application step. Also covers
///     dropping whichever retained state its own function no longer
///     needs: deriv_bc's `chars`/final `ARegex`; pderiv_bc/pderiv_std's
///     final `frontier`; deriv_std_loop's entire `expressions` history
///     (every intermediate derivative, since POSIX injection must
///     revisit each one); deriv_std_rec's per-level `r_deriv`, dropped
///     one level at a time as the recursion unwinds.
///   - `teardown_us`: dropping the returned `ParseTree` itself, timed
///     not inside any of these five functions (none controls what its
///     caller does with the tree) but in `examples/bench_worker.rs`,
///     the only place positioned to measure it. Always 0 as directly
///     returned from any of the five parse functions.
///
/// deriv_std_rec's derive_us/reconstruct_us carry a little more
/// measurement overhead than the other four: its two phases interleave
/// within one recursive descent rather than running as separate
/// passes, costing roughly two extra clock reads per character --
/// negligible against the parser's existing per-character cost, and
/// the accepted trade-off for keeping its phase split at all.
#[derive(Debug, Clone, Copy, Default)]
pub struct PhaseTiming {
    pub setup_us: u64,
    pub derive_us: u64,
    pub reconstruct_us: u64,
    pub teardown_us: u64,
}

// Re-export types
pub use crate::types::{ParseTree, flatten};
pub use selection::{ParserType, ALL};
