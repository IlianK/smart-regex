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
/// positionally-aligned phases so all five report comparable numbers
/// in one table, and so the four phases sum to the true wall-clock
/// cost of the call with nothing left unaccounted. What each phase is
/// made of is NOT the same across parsers -- only its position in the
/// parse is:
///   - `setup_us`: work done before any character is consumed. For
///     deriv_bc this is a real encode (Regex -> ARegex via
///     `internalize`, building the bit-annotated representation).
///     pderiv_std has no such conversion, so its `setup_us` is just
///     the initial `r.clone()` into the starting frontier (a plain
///     tree clone, no bits or injection closures involved), timed as
///     its own phase purely so all three non-recursive parsers'
///     breakdowns stay comparable -- it is a real, nonzero number,
///     not folded into `derive_us`. Calling it an "encode" the way
///     deriv_bc's is would overclaim what it does. pderiv_bc also has
///     no upfront conversion, but its `setup_us` is NOT just that
///     trivial clone: it additionally carries the accumulated time
///     spent in `flatten_alt` tagging `Alt` branches with bit prefixes
///     across the whole parse, which is the exact per-character
///     equivalent of what `internalize` does once, upfront, for
///     deriv_bc (see `internalize.rs`'s `Alt` case) -- pulled out of
///     what would otherwise be undifferentiated per-character work so
///     the two bit-coded parsers' "encoding" numbers are genuinely
///     comparable, not just positionally aligned. See
///     `pderiv_bc_timed`'s and `parse_pderiv_bc_timed`'s own doc
///     comments for exactly where that boundary falls -- in
///     particular, `Star`'s own bit-tagging stays in `derive_us` on
///     both bit-coded parsers, since `internalize` does not tag `Star`
///     either. deriv_std_rec/deriv_std_loop are the ones whose
///     `setup_us` is always 0: neither ever builds a frontier or
///     wrapper before deriving, so there is nothing to time here at
///     all; see `parse_deriv_std_loop_timed`'s doc comment.
///   - `derive_us`: the per-character stepping loop, for all five --
///     for pderiv_bc specifically, that loop's wall-clock time with
///     the `setup_us` portion above subtracted back out, so the four
///     fields still sum to the true total with nothing double-counted.
///     This phase's own timer now also covers dropping whatever this
///     step's own retained state needs: `deriv_std_rec`'s initial
///     `chars: Vec<char>` collection is timed into this phase's first
///     accumulated value (see `parse_deriv_std_rec_timed`'s doc
///     comment) instead of happening before any timer starts.
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
///     not by interpreting an encoded representation. This phase's own
///     timer now also covers dropping whatever retained state its own
///     function no longer needs once reconstruction is done: deriv_bc
///     drops its `chars`/final `ARegex`; pderiv_bc/pderiv_std drop
///     their final `frontier`; deriv_std_loop drops the whole
///     `expressions` history (every intermediate derivative, one per
///     input position -- previously the single biggest unaccounted
///     cost for this parser, since POSIX injection needs to revisit
///     every one of them, unlike the bit-coded/partial-derivative
///     parsers, which never retain more than their current state);
///     deriv_std_rec's own recursion drops each level's own `r_deriv`
///     the same way, inside this same timed window, once per input
///     character rather than once in bulk.
///   - `teardown_us`: the cost of dropping the `ParseTree` this call
///     returns, once its caller is done with it. Always 0 as directly
///     returned from any of these five functions -- none of them
///     controls what its own caller does with the tree it hands back,
///     so none can time that drop itself. `examples/bench_worker.rs`
///     is what actually measures and fills this field in: it keeps
///     only the `PhaseTiming` half of what every one of these
///     functions returns, so the `ParseTree` half gets dropped right
///     there, once, timed, and folded into the `PhaseTiming` it
///     reports onward. This is the one part of the four-way breakdown
///     that cannot live inside any single parser function.
///
/// deriv_std_rec's own derive_us/reconstruct_us carry more measurement
/// overhead than the other four parsers': its two phases are
/// interleaved in a single recursive descent rather than two separate
/// passes, so isolating them costs roughly two extra clock reads per
/// input character, instead of the two or three total that the other
/// parsers' phase timing costs regardless of input length. Dropping
/// each level's own `r_deriv` rides inside a timer that already
/// existed, so it adds no further per-character clock reads; timing
/// the initial `chars.collect()` adds one more one-time (not
/// per-character) timer pair in the outer wrapper function, negligible
/// against this parser's existing accepted per-character overhead. See
/// `parse_deriv_std_rec_timed`'s doc comment for why the two-reads-
/// per-character cost is the accepted trade-off rather than something
/// to design around.
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
