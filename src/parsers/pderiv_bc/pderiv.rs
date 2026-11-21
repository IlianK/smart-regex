//! src/parsers/pderiv_bc/pderiv.rs
//! 
//! Bit-Coded Partial-Derivative-Based Parser
//! Antimirov Partial Derivative 
//! plain Regex + external per-strand bits, GREEDY.

use std::collections::HashSet;
use std::time::Instant;
use crate::types::{AppendBits, Regex};
use crate::regex::nullable::nullable;
use crate::regex::simplify::smart_seq;

/// mkEpsBC for plain Regex: bit-string analogue of `mk_eps`. Assumes nullable(r) holds.
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

/// Stable dedup on the residual, keeping the first (highest-priority).
/// Matches the reference `nub2 = nubBy (\(r,_) (s,_) -> r == s)`: the
/// bit list is intentionally excluded from the comparison. Generic over
/// the bits type: `pderiv_bc`'s own call carries small per-step deltas
/// (`Vec<bool>`), `step_frontier`'s call carries the accumulated
/// per-strand history (`AppendBits`) -- dedup-by-residual doesn't care
/// which.
pub(super) fn nub2<B>(pairs: Vec<(Regex, B)>) -> Vec<(Regex, B)> {
    let mut seen: HashSet<Regex> = HashSet::with_capacity(pairs.len());
    let mut out = Vec::with_capacity(pairs.len());
    for (r, bs) in pairs {
        if seen.insert(r.clone()) {
            out.push((r, bs));
        }
    }
    out
}

/// Flattens a chain of nested `Alt`s into its leaves, pairing each leaf
/// with the bit prefix its position in the chain assigns (`false` for
/// a left branch, `true` for a right, accumulated down the spine in
/// encounter order, which is also append order).
///
/// Recursing through `pderiv_bc`'s own `Alt` arm one nesting level at a
/// time and calling `nub2` at every level was semantically fine
/// (`nub2`'s "keep first occurrence" composes correctly regardless of
/// whether it's applied once on the flattened list or once per level
/// on a list that grows by one element per level) but cost O(p)
/// redundant dedup passes for a p-wide alternation -- O(p^2) clones at
/// minimum, confirmed empirically closer to O(p^2.5-3) once allocation
/// overhead is included, almost entirely inside the single call that
/// processes the first character of a p-wide pattern, before the
/// frontier has even split into separate residuals. Flattening first
/// and deduping exactly once after gives the identical result (same
/// residuals, same bits, same leftmost-priority winner on a tie) for a
/// fraction of the cost.
///
/// `prefix` is an `AppendBits`, not a `Vec<bool>` with push-before/
/// pop-after backtracking: on a right-nested chain, the k-th leaf sits
/// at depth k, so a `Vec<bool>`'s O(depth) clone at every leaf still
/// costs O(p) + O(p-1) + ... = O(p^2) total across a p-leaf chain, even
/// after the fix above removed the redundant per-level dedup passes.
/// `AppendBits::append`/`clone` are both O(1) amortized regardless of
/// the existing chain's length (the same sharing `Inj`/`InjNode` use
/// for injection closures, see pderiv_std), so extending the prefix by
/// one bit per level and cloning the handle at each leaf is O(1) each,
/// O(p) total -- and needs no explicit pop, since each recursive call
/// gets its own extended value and the caller's own `prefix` binding is
/// untouched by it.
fn flatten_alt<'a>(r: &'a Regex, prefix: &AppendBits, out: &mut Vec<(&'a Regex, AppendBits)>) {
    match r {
        Regex::Alt(r1, r2) => {
            flatten_alt(r1, &prefix.append(&[false]), out);
            flatten_alt(r2, &prefix.append(&[true]), out);
        }
        other => out.push((other, prefix.clone())),
    }
}

/// Bit-coded partial derivative (pDerivBC): one `(residual, bits)` pair per surviving strand.
pub fn pderiv_bc(r: &Regex, x: char) -> Vec<(Regex, AppendBits)> {
    match r {
        Regex::Eps => Vec::new(),
        Regex::Phi => Vec::new(),

        Regex::Lit(y) => {
            if *y == x {
                vec![(Regex::Eps, AppendBits::empty())]
            } else {
                Vec::new()
            }
        }

        Regex::Alt(_, _) => {
            let mut leaves: Vec<(&Regex, AppendBits)> = Vec::new();
            flatten_alt(r, &AppendBits::empty(), &mut leaves);

            let mut out: Vec<(Regex, AppendBits)> = Vec::new();
            for (leaf, leaf_prefix) in leaves {
                for (r_, bs) in pderiv_bc(leaf, x) {
                    out.push((r_, leaf_prefix.append_bits(&bs)));
                }
            }
            nub2(out)
        }

        Regex::Seq(r1, r2) => {
            if nullable(r1) {
                // Strand 1: continue r1, then r2. smart_seq, not raw Seq:
                // a nullable r1's own derivative can itself be Eps, and
                // without collapsing that wrapper this strand and strand 2
                // below ("r1 matched empty, jump to r2") can reach the same
                // residual in two syntactically different shapes, defeating
                // nub2's structural-equality dedup. PPDP12 licenses exactly
                // this collapse ("we omit some obvious simplifications,
                // e.g. (eps,A*) -> A*", Fig. 10). Bits are external to the
                // Regex shape here (unlike ARegex), so the collapse needs
                // no companion bit adjustment -- bs rides along unchanged,
                // same as the non-nullable branch below.
                let mut out: Vec<(Regex, AppendBits)> = pderiv_bc(r1, x)
                    .into_iter()
                    .map(|(r1_, bs)| (smart_seq(r1_, r2), bs))
                    .collect();

                // Strand 2: r1 matched empty, jump straight into r2.
                let eps_bits = AppendBits::from(mk_eps_bits(r1));
                out.extend(pderiv_bc(r2, x).into_iter().map(|(r2_, bs)| {
                    (r2_, eps_bits.append_bits(&bs))
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
            let out: Vec<(Regex, AppendBits)> = pderiv_bc(r1, x)
                .into_iter()
                .map(|(r1_, bs)| {
                    let b = AppendBits::from(vec![false]).append_bits(&bs);
                    (smart_seq(r1_, &Regex::star(*r1.clone())), b)
                })
                .collect();
            nub2(out)
        }
    }
}

/// Same partial derivative, plus the accumulated time spent tagging
/// `Alt` branches via `flatten_alt` -- the same bit-prefix assignment
/// (`false` for a left branch, `true` for a right one) that
/// `internalize` does once, upfront, for the whole tree, for
/// `deriv_bc`. Here it happens fresh on every character instead, for
/// whatever `Alt` nodes survive in the current residual, which is
/// exactly why it was invisible before: buried inside the same
/// `derive_us` total as everything else `pderiv_bc` does. Threaded
/// through recursion the same way `deriv_std_rec_timed`'s
/// accumulators are (see that function's own doc comment), so the
/// plain `pderiv_bc` above pays no cost for this.
///
/// `Star`'s own tagging just below (`AppendBits::from(vec![false])`)
/// is deliberately NOT counted into `tag_us`: `internalize` does not
/// tag `Star` either (`ARegex::Star(Bits::empty(), ...)`; see
/// `internalize.rs`) -- that cost is carried by `deriv_bc`'s own
/// derive loop on the bit-coded-derivative side, so it stays part of
/// this function's "derive" time here too, consistently.
pub fn pderiv_bc_timed(r: &Regex, x: char, tag_us: &mut u64) -> Vec<(Regex, AppendBits)> {
    match r {
        Regex::Eps => Vec::new(),
        Regex::Phi => Vec::new(),

        Regex::Lit(y) => {
            if *y == x {
                vec![(Regex::Eps, AppendBits::empty())]
            } else {
                Vec::new()
            }
        }

        Regex::Alt(_, _) => {
            let t = Instant::now();
            let mut leaves: Vec<(&Regex, AppendBits)> = Vec::new();
            flatten_alt(r, &AppendBits::empty(), &mut leaves);
            *tag_us += t.elapsed().as_micros() as u64;

            let mut out: Vec<(Regex, AppendBits)> = Vec::new();
            for (leaf, leaf_prefix) in leaves {
                for (r_, bs) in pderiv_bc_timed(leaf, x, tag_us) {
                    out.push((r_, leaf_prefix.append_bits(&bs)));
                }
            }
            nub2(out)
        }

        Regex::Seq(r1, r2) => {
            if nullable(r1) {
                let mut out: Vec<(Regex, AppendBits)> = pderiv_bc_timed(r1, x, tag_us)
                    .into_iter()
                    .map(|(r1_, bs)| (smart_seq(r1_, r2), bs))
                    .collect();

                let eps_bits = AppendBits::from(mk_eps_bits(r1));
                out.extend(pderiv_bc_timed(r2, x, tag_us).into_iter().map(|(r2_, bs)| {
                    (r2_, eps_bits.append_bits(&bs))
                }));

                nub2(out)
            } else {
                pderiv_bc_timed(r1, x, tag_us)
                    .into_iter()
                    .map(|(r1_, bs)| (smart_seq(r1_, r2), bs))
                    .collect()
            }
        }

        Regex::Star(r1) => {
            let out: Vec<(Regex, AppendBits)> = pderiv_bc_timed(r1, x, tag_us)
                .into_iter()
                .map(|(r1_, bs)| {
                    let b = AppendBits::from(vec![false]).append_bits(&bs);
                    (smart_seq(r1_, &Regex::star(*r1.clone())), b)
                })
                .collect();
            nub2(out)
        }
    }
}


// Tests for pderiv_bc and mk_eps_bits

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Regex;

    /// Materializes `pderiv_bc`'s `AppendBits` results back to plain
    /// `Vec<bool>` so every expected-value literal below can stay
    /// exactly as written against the reference rules, unchanged by the
    /// O(1)-amortized representation `pderiv_bc` now returns internally.
    fn materialize(pairs: Vec<(Regex, AppendBits)>) -> Vec<(Regex, Vec<bool>)> {
        pairs.into_iter().map(|(r, b)| (r, b.to_vec())).collect()
    }

    fn set(pairs: &[(Regex, Vec<bool>)]) -> HashSet<(Regex, Vec<bool>)> {
        pairs.iter().cloned().collect()
    }

    fn as_set(pairs: Vec<(Regex, Vec<bool>)>) -> HashSet<(Regex, Vec<bool>)> {
        pairs.into_iter().collect()
    }

    // pderiv_bc(Phi/Eps, _) = []
    #[test]
    fn pderiv_bc_phi_empty() {
        assert_eq!(materialize(pderiv_bc(&Regex::Phi, 'a')), vec![]);
    }
    #[test]
    fn pderiv_bc_eps_empty() {
        assert_eq!(materialize(pderiv_bc(&Regex::Eps, 'a')), vec![]);
    }

    // pderiv_bc(Lit(c), c) = [(Eps, [])]
    #[test]
    fn pderiv_bc_lit_match() {
        assert_eq!(materialize(pderiv_bc(&Regex::lit('a'), 'a')), vec![(Regex::Eps, vec![])]);
    }
    #[test]
    fn pderiv_bc_lit_no_match() {
        assert_eq!(materialize(pderiv_bc(&Regex::lit('a'), 'b')), vec![]);
    }

    // pderiv_bc(a + ab, 'a') = [(Eps,[false]), (b,[true])]  (worked example, ch.3-style)
    #[test]
    fn pderiv_bc_alt_bits_tag_branch() {
        let r = Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b')));
        let d = materialize(pderiv_bc(&r, 'a'));
        assert_eq!(
            as_set(d),
            set(&[(Regex::Eps, vec![false]), (Regex::lit('b'), vec![true])])
        );
    }

    // pderiv_bc(Seq(r1,r2), c), r1 not nullable: bits pass through unchanged
    #[test]
    fn pderiv_bc_seq_non_nullable_no_extra_bits() {
        let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
        let d = materialize(pderiv_bc(&r, 'a'));
        assert_eq!(d, vec![(Regex::lit('b'), vec![])]);
    }

    // pderiv_bc(Seq(r1,r2), c), r1 nullable: both strands present
    #[test]
    fn pderiv_bc_seq_nullable_left_both_strands() {
        // (eps + a) . b on 'a': strand 2 (jump to r2=b) dies since 'b' != 'a', only strand 1 survives
        let r1 = Regex::alt(Regex::Eps, Regex::lit('a'));
        let r = Regex::seq(r1, Regex::lit('b'));
        let d = materialize(pderiv_bc(&r, 'a'));
        // smart_seq collapses Seq(Eps, Lit('b')) to Lit('b') alone; bits unchanged.
        assert_eq!(d, vec![(Regex::lit('b'), vec![true])]);
    }

    // pderiv_bc(Star(r), c) tags with a leading `false` (one more iteration)
    #[test]
    fn pderiv_bc_star_tags_iteration() {
        let r = Regex::star(Regex::lit('a'));
        let d = materialize(pderiv_bc(&r, 'a'));
        assert_eq!(d, vec![(Regex::star(Regex::lit('a')), vec![false])]);
    }
    #[test]
    fn pderiv_bc_star_wrong_char_empty() {
        let r = Regex::star(Regex::lit('a'));
        assert_eq!(materialize(pderiv_bc(&r, 'b')), vec![]);
    }

    // nub2 keeps the first occurrence, dropping a later duplicate's bits
    #[test]
    fn pderiv_bc_alt_dedups_identical_residuals_keeping_first() {
        // (a + a) on 'a': both branches produce Eps; left [false] must win
        let r = Regex::alt(Regex::lit('a'), Regex::lit('a'));
        let d = materialize(pderiv_bc(&r, 'a'));
        assert_eq!(d, vec![(Regex::Eps, vec![false])]);
    }

    // a + (a + b) on 'a', nested two Alt levels deep. Left's own 'a'
    // (prefix [false]) must win the tie over the nested 'a' (prefix
    // [true,false]); the nested 'b' never matches 'a' and contributes
    // nothing either way.
    #[test]
    fn nested_alt_three_leaves_prefix_matches_hand_trace_and_dedups_leftmost() {
        let r = Regex::alt(Regex::lit('a'), Regex::alt(Regex::lit('a'), Regex::lit('b')));
        let d = materialize(pderiv_bc(&r, 'a'));
        assert_eq!(d, vec![(Regex::Eps, vec![false])]);
    }

    // Four levels deep, no duplicates this time, to pin down that every
    // leaf's prefix (not just the first two) matches what per-level
    // nesting assigns: leaf i's prefix is i copies of `true` followed
    // by one `false`, except the last leaf, which has no trailing
    // `false` (it's reached by `true` at every level with nothing
    // nested inside it to add one).
    #[test]
    fn nested_alt_four_leaves_each_prefix_matches_hand_trace() {
        // a + (b + (c + d)): leaf i's prefix is i copies of `true`
        // followed by one `false`, except the last leaf (d), which has
        // no trailing `false` since nothing is nested inside it.
        let r = Regex::alt(
            Regex::lit('a'),
            Regex::alt(Regex::lit('b'), Regex::alt(Regex::lit('c'), Regex::lit('d'))),
        );
        // Only one leaf ever matches the single character being
        // derived by, so each case isolates that leaf's own prefix
        // directly in the (sole) surviving strand's bits.
        assert_eq!(materialize(pderiv_bc(&r, 'a')), vec![(Regex::Eps, vec![false])]);
        assert_eq!(materialize(pderiv_bc(&r, 'b')), vec![(Regex::Eps, vec![true, false])]);
        assert_eq!(materialize(pderiv_bc(&r, 'c')), vec![(Regex::Eps, vec![true, true, false])]);
        assert_eq!(materialize(pderiv_bc(&r, 'd')), vec![(Regex::Eps, vec![true, true, true])]);
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

    // pderiv_bc_timed must compute exactly the same residuals/bits as
    // plain pderiv_bc -- tag_us is purely an added side channel, never
    // a change to what's returned -- across both a flat and a nested
    // alternation, since flatten_alt (the only thing tag_us measures)
    // behaves differently at each depth.
    #[test]
    fn timed_matches_untimed_on_flat_alt() {
        let r = Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b')));
        for x in ['a', 'b'] {
            let mut tag_us = 0u64;
            assert_eq!(
                as_set(materialize(pderiv_bc(&r, x))),
                as_set(materialize(pderiv_bc_timed(&r, x, &mut tag_us))),
            );
        }
    }

    #[test]
    fn timed_matches_untimed_on_nested_alt() {
        let r = Regex::alt(
            Regex::lit('a'),
            Regex::alt(Regex::lit('b'), Regex::alt(Regex::lit('c'), Regex::lit('d'))),
        );
        for x in ['a', 'b', 'c', 'd'] {
            let mut tag_us = 0u64;
            assert_eq!(
                as_set(materialize(pderiv_bc(&r, x))),
                as_set(materialize(pderiv_bc_timed(&r, x, &mut tag_us))),
            );
        }
    }

    // Star/Seq/Lit/Eps/Phi never reach the Alt arm, so a pattern built
    // only from those must leave tag_us at exactly 0, not just small --
    // the Alt arm is the only code that ever increments it.
    #[test]
    fn tag_us_stays_zero_without_any_alt() {
        let r = Regex::seq(Regex::star(Regex::lit('a')), Regex::lit('b'));
        let mut tag_us = 0u64;
        let _ = pderiv_bc_timed(&r, 'a', &mut tag_us);
        let _ = pderiv_bc_timed(&r, 'b', &mut tag_us);
        assert_eq!(tag_us, 0);
    }
}
