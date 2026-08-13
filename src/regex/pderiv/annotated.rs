//! regex-engine/src/regex/pderiv/annotated.rs
//!
//! Bit-coded Antimirov partial derivatives (pDerivBC), following the
//! "Bit-coded partial derivative parser" reference at
//! https://sulzmann.github.io/ProgrammingParadigms/pp-regular-expressions.html#(9)
//!
//! Unlike the bit-coded *Brzozowski* derivative (regex/deriv/annotated.rs),
//! this construction does NOT use the ARegex/internalize/fuse machinery.
//! The reference operates directly on the plain, un-annotated Regex type
//! and returns a *list* of (residual, bits) pairs per step -- one entry
//! per surviving strand of nondeterminism, each carrying its own
//! self-contained bit-string prefix, rather than a single tree with bits
//! attached to internal nodes the way deriv_bc's ARegex does.
//!
//! IMPORTANT -- disambiguation policy: this is a *faithful transcription*
//! of the reference exactly as given. Differential testing against the
//! standard (proven POSIX, Theorem 1) parser shows it does NOT compute
//! POSIX leftmost-longest results in general -- it computes GREEDY
//! leftmost results (Section 4.3.1) instead. The reference's own list
//! order is built purely by syntactic left-to-right traversal of `Alt`
//! nodes (`Choice`'s left branch always listed before its right branch),
//! with no notion of eventual match length, so `parsePDerivBC2`'s
//! "first nullable residual wins" selection always prefers whichever
//! alternative is reached first in traversal order -- exactly Greedy's
//! rule, not POSIX rule (A1)'s "longer match wins regardless of side".
//! Concretely: `(a+(b+ab))*` on `"ab"` -- the flops14 paper's own
//! motivating example for why Greedy and POSIX differ -- decodes here to
//! the two-iteration Greedy answer `[Left(a), Right(Left(b))]`, not the
//! one-iteration POSIX answer `[Right(Right(a,b))]` the standard parser
//! produces. Unlike the derivative-based construction (Fig. 3, Fig. 5),
//! whose forward/backward or single-forward-pass structure is
//! specifically built to resolve length comparisons correctly (Lemma 1,
//! Lemma 2, Theorem 1), the reference states no correctness claim at all
//! for `pDerivBC`/`parsePDerivBC`. See `tests/pderiv_bc_greedy_vs_posix.rs`
//! for the differential evidence.

use std::collections::HashSet;
use crate::types::Regex;
use crate::regex::nullable::standard::nullable;
use crate::regex::simplify::standard::smart_seq;

/// mkEpsBC for plain Regex -- the partial-derivative analogue of
/// `posix::bitcoded::mk_eps_bc`, which instead walks the bit-annotated
/// `ARegex` type used by the Brzozowski-derivative construction. This
/// version walks the plain `Regex` directly, exactly mirroring `mk_eps`
/// (src/posix/standard/deriv/mk_eps.rs) case for case but producing a
/// bit-string instead of a `ParseTree`.
///
/// Assumes nullable(r) holds.
pub fn mk_eps_bits(r: &Regex) -> Vec<bool> {
    match r {
        Regex::Phi => panic!("mk_eps_bits called on Phi"),
        Regex::Lit(c) => panic!("mk_eps_bits called on Lit('{}')", c),

        Regex::Eps => Vec::new(),

        Regex::Alt(r1, r2) => {
            if nullable(r1) {
                let mut bits = vec![false];
                bits.extend(mk_eps_bits(r1));
                bits
            } else {
                let mut bits = vec![true];
                bits.extend(mk_eps_bits(r2));
                bits
            }
        }

        Regex::Seq(r1, r2) => {
            let mut bits = mk_eps_bits(r1);
            bits.extend(mk_eps_bits(r2));
            bits
        }

        Regex::Star(_) => vec![true],
    }
}

/// `nubBy (\(r,_) (s,_) -> r == s)`: stable dedup keeping the *first*
/// occurrence of each residual regex and dropping later duplicates.
/// Since `pderiv_bc` always emits the higher-priority (leftmost) strand
/// before lower-priority ones at every choice point -- exactly mirroring
/// how `internalize` marks left branches `[0]` before right branches
/// `[1]` on the Brzozowski-derivative side -- keeping the first
/// occurrence keeps the higher-priority strand's bits and discards the
/// lower-priority duplicate's.
fn nub2(pairs: Vec<(Regex, Vec<bool>)>) -> Vec<(Regex, Vec<bool>)> {
    let mut seen: HashSet<Regex> = HashSet::with_capacity(pairs.len());
    let mut out = Vec::with_capacity(pairs.len());
    for (r, bs) in pairs {
        if seen.insert(r.clone()) {
            out.push((r, bs));
        }
    }
    out
}

/// Bit-coded partial derivative: `pDerivBC x r`.
///
/// Returns one `(residual, bits)` pair per surviving strand of
/// nondeterminism. `bits` records, for that strand alone, every choice
/// (which `Alt` branch, which `Star` iteration boundary) taken to reach
/// it -- self-contained, unlike `deriv_bc`'s annotations, which live on
/// the shared tree rather than per-strand.
pub fn pderiv_bc(r: &Regex, x: char) -> Vec<(Regex, Vec<bool>)> {
    match r {
        Regex::Eps => Vec::new(),
        Regex::Phi => Vec::new(),

        Regex::Lit(y) => {
            if *y == x {
                vec![(Regex::Eps, Vec::new())]
            } else {
                Vec::new()
            }
        }

        Regex::Alt(r1, r2) => {
            let mut out: Vec<(Regex, Vec<bool>)> = pderiv_bc(r1, x)
                .into_iter()
                .map(|(r1_, mut bs)| {
                    let mut b = vec![false];
                    b.append(&mut bs);
                    (r1_, b)
                })
                .collect();
            out.extend(
                pderiv_bc(r2, x)
                    .into_iter()
                    .map(|(r2_, mut bs)| {
                        let mut b = vec![true];
                        b.append(&mut bs);
                        (r2_, b)
                    }),
            );
            nub2(out)
        }

        Regex::Seq(r1, r2) => {
            if nullable(r1) {
                // Strand 1: continue r1, r2 untouched -- NOT smart-constructed
                // (matches the reference literally: the nullable-r1 branch of
                // pDerivBC builds `Cat r' s` directly, unlike the non-nullable
                // branch below and unlike deriv_bc's Seq case, both of which
                // smart-construct. This can leave a non-canonical `Eps · s`
                // residual on this strand -- harmless for correctness (mkEpsBC
                // and decode both handle it transparently) but means such a
                // residual won't dedup against an equivalent plain `s` arrived
                // at via another strand.)
                let mut out: Vec<(Regex, Vec<bool>)> = pderiv_bc(r1, x)
                    .into_iter()
                    .map(|(r1_, bs)| (Regex::seq(r1_, *r2.clone()), bs))
                    .collect();

                // Strand 2: r1 already matched empty (mk_eps_bits(r1) supplies
                // that un-derived prefix), jump straight into r2 -- mirrors
                // inject's Right case / deriv_bc's fuse(mkEpsBC(r1), ...).
                let eps_bits = mk_eps_bits(r1);
                out.extend(pderiv_bc(r2, x).into_iter().map(|(r2_, bs)| {
                    let mut b = eps_bits.clone();
                    b.extend(bs);
                    (r2_, b)
                }));

                nub2(out)
            } else {
                pderiv_bc(r1, x)
                    .into_iter()
                    .map(|(r1_, bs)| (smart_seq(r1_, r2), bs))
                    .collect()
            }
        }

        Regex::Star(r1) => {
            let out: Vec<(Regex, Vec<bool>)> = pderiv_bc(r1, x)
                .into_iter()
                .map(|(r1_, mut bs)| {
                    let mut b = vec![false];
                    b.append(&mut bs);
                    (smart_seq(r1_, &Regex::star(*r1.clone())), b)
                })
                .collect();
            nub2(out)
        }
    }
}


// -------------------------------
// Tests for pderiv_bc and mk_eps_bits
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Regex;

    fn set(pairs: &[(Regex, Vec<bool>)]) -> HashSet<(Regex, Vec<bool>)> {
        pairs.iter().cloned().collect()
    }

    fn as_set(pairs: Vec<(Regex, Vec<bool>)>) -> HashSet<(Regex, Vec<bool>)> {
        pairs.into_iter().collect()
    }

    // pderiv_bc(Phi/Eps, _) = []
    #[test]
    fn pderiv_bc_phi_empty() {
        assert_eq!(pderiv_bc(&Regex::Phi, 'a'), vec![]);
    }
    #[test]
    fn pderiv_bc_eps_empty() {
        assert_eq!(pderiv_bc(&Regex::Eps, 'a'), vec![]);
    }

    // pderiv_bc(Lit(c), c) = [(Eps, [])]
    #[test]
    fn pderiv_bc_lit_match() {
        assert_eq!(pderiv_bc(&Regex::lit('a'), 'a'), vec![(Regex::Eps, vec![])]);
    }
    #[test]
    fn pderiv_bc_lit_no_match() {
        assert_eq!(pderiv_bc(&Regex::lit('a'), 'b'), vec![]);
    }

    // pderiv_bc(a + ab, 'a') = [(Eps,[false]), (b,[true])]  (worked example, ch.3-style)
    #[test]
    fn pderiv_bc_alt_bits_tag_branch() {
        let r = Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b')));
        let d = pderiv_bc(&r, 'a');
        assert_eq!(
            as_set(d),
            set(&[(Regex::Eps, vec![false]), (Regex::lit('b'), vec![true])])
        );
    }

    // pderiv_bc(Seq(r1,r2), c), r1 not nullable: bits pass through unchanged
    #[test]
    fn pderiv_bc_seq_non_nullable_no_extra_bits() {
        let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
        let d = pderiv_bc(&r, 'a');
        assert_eq!(d, vec![(Regex::lit('b'), vec![])]);
    }

    // pderiv_bc(Seq(r1,r2), c), r1 nullable: both strands present
    #[test]
    fn pderiv_bc_seq_nullable_left_both_strands() {
        // (eps + a) . b  on 'a':
        //   strand 1 (continue r1=eps+a): pderiv_bc(eps+a,'a') = [(Eps,[true])]
        //     -> Cat(Eps, b), bits [true]
        //   strand 2 (r1 matched empty, jump to r2=b): pderiv_bc(b,'a') = [] (b != a)
        let r1 = Regex::alt(Regex::Eps, Regex::lit('a'));
        let r = Regex::seq(r1, Regex::lit('b'));
        let d = pderiv_bc(&r, 'a');
        assert_eq!(d, vec![(Regex::seq(Regex::Eps, Regex::lit('b')), vec![true])]);
    }

    // pderiv_bc(Star(r), c) tags with a leading `false` (one more iteration)
    #[test]
    fn pderiv_bc_star_tags_iteration() {
        let r = Regex::star(Regex::lit('a'));
        let d = pderiv_bc(&r, 'a');
        assert_eq!(d, vec![(Regex::star(Regex::lit('a')), vec![false])]);
    }
    #[test]
    fn pderiv_bc_star_wrong_char_empty() {
        let r = Regex::star(Regex::lit('a'));
        assert_eq!(pderiv_bc(&r, 'b'), vec![]);
    }

    // nub2 keeps the first occurrence, dropping a later duplicate's bits
    #[test]
    fn pderiv_bc_alt_dedups_identical_residuals_keeping_first() {
        // (a + a) on 'a': both branches produce Eps; left [false] must win
        let r = Regex::alt(Regex::lit('a'), Regex::lit('a'));
        let d = pderiv_bc(&r, 'a');
        assert_eq!(d, vec![(Regex::Eps, vec![false])]);
    }

    // mk_eps_bits mirrors mk_eps_bc's structure exactly, on plain Regex
    #[test]
    fn mk_eps_bits_eps_is_empty() {
        assert_eq!(mk_eps_bits(&Regex::Eps), Vec::<bool>::new());
    }
    #[test]
    fn mk_eps_bits_star_is_true() {
        assert_eq!(mk_eps_bits(&Regex::star(Regex::lit('a'))), vec![true]);
    }
    #[test]
    fn mk_eps_bits_alt_prefers_left_when_nullable() {
        let r = Regex::alt(Regex::Eps, Regex::lit('a'));
        assert_eq!(mk_eps_bits(&r), vec![false]);
    }
    #[test]
    fn mk_eps_bits_alt_uses_right_when_left_not_nullable() {
        let r = Regex::alt(Regex::lit('a'), Regex::Eps);
        assert_eq!(mk_eps_bits(&r), vec![true]);
    }
    #[test]
    fn mk_eps_bits_seq_concatenates() {
        let r = Regex::seq(
            Regex::alt(Regex::Eps, Regex::lit('a')),
            Regex::star(Regex::lit('b')),
        );
        // left alt nullable via Eps -> [false]; star -> [true]
        assert_eq!(mk_eps_bits(&r), vec![false, true]);
    }
    #[test]
    #[should_panic(expected = "mk_eps_bits called on Phi")]
    fn mk_eps_bits_phi_panics() {
        mk_eps_bits(&Regex::Phi);
    }
    #[test]
    #[should_panic(expected = "mk_eps_bits called on Lit")]
    fn mk_eps_bits_lit_panics() {
        mk_eps_bits(&Regex::lit('a'));
    }
}
