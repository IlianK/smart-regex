//! src/parsers/pderiv_bc/pderiv.rs
//! 
//! Bit-Coded Partial-Derivative-Based Parser
//! Antimirov Partial Derivative 
//! plain Regex + external per-strand bits, GREEDY.

use std::collections::HashSet;
use crate::types::Regex;
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
/// encounter order -- which for a plain `Vec<bool>` is also append
/// order, so push-before-recurse already builds it correctly).
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
fn flatten_alt<'a>(r: &'a Regex, prefix: &mut Vec<bool>, out: &mut Vec<(&'a Regex, Vec<bool>)>) {
    match r {
        Regex::Alt(r1, r2) => {
            prefix.push(false);
            flatten_alt(r1, prefix, out);
            prefix.pop();
            prefix.push(true);
            flatten_alt(r2, prefix, out);
            prefix.pop();
        }
        other => out.push((other, prefix.clone())),
    }
}

/// Bit-coded partial derivative (pDerivBC): one `(residual, bits)` pair per surviving strand.
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

        Regex::Alt(_, _) => {
            let mut leaves: Vec<(&Regex, Vec<bool>)> = Vec::new();
            let mut prefix = Vec::new();
            flatten_alt(r, &mut prefix, &mut leaves);

            let mut out: Vec<(Regex, Vec<bool>)> = Vec::new();
            for (leaf, leaf_prefix) in leaves {
                for (r_, bs) in pderiv_bc(leaf, x) {
                    let mut b = leaf_prefix.clone();
                    b.extend(bs);
                    out.push((r_, b));
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
                let mut out: Vec<(Regex, Vec<bool>)> = pderiv_bc(r1, x)
                    .into_iter()
                    .map(|(r1_, bs)| (smart_seq(r1_, r2), bs))
                    .collect();

                // Strand 2: r1 matched empty, jump straight into r2.
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


// Tests for pderiv_bc and mk_eps_bits

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
        // (eps + a) . b on 'a': strand 2 (jump to r2=b) dies since 'b' != 'a', only strand 1 survives
        let r1 = Regex::alt(Regex::Eps, Regex::lit('a'));
        let r = Regex::seq(r1, Regex::lit('b'));
        let d = pderiv_bc(&r, 'a');
        // smart_seq collapses Seq(Eps, Lit('b')) to Lit('b') alone; bits unchanged.
        assert_eq!(d, vec![(Regex::lit('b'), vec![true])]);
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

    // Hand-traced in the fix's own writeup: a + (a + b) on 'a', nested
    // two Alt levels deep. Without the flatten-first fix this would
    // compute (via the old per-level recursion) out = [(Eps,[false]),
    // (Eps,[true]++[false])] before nub2 -- same two candidates
    // flatten_alt now produces directly, confirming the prefix a
    // flattened leaf gets matches what per-level nesting would have
    // assigned. Left's own 'a' (prefix [false]) must still win the tie
    // over the nested 'a' (prefix [true,false]); the nested 'b' never
    // matches 'a' and contributes nothing either way.
    #[test]
    fn nested_alt_three_leaves_prefix_matches_hand_trace_and_dedups_leftmost() {
        let r = Regex::alt(Regex::lit('a'), Regex::alt(Regex::lit('a'), Regex::lit('b')));
        let d = pderiv_bc(&r, 'a');
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
        assert_eq!(pderiv_bc(&r, 'a'), vec![(Regex::Eps, vec![false])]);
        assert_eq!(pderiv_bc(&r, 'b'), vec![(Regex::Eps, vec![true, false])]);
        assert_eq!(pderiv_bc(&r, 'c'), vec![(Regex::Eps, vec![true, true, false])]);
        assert_eq!(pderiv_bc(&r, 'd'), vec![(Regex::Eps, vec![true, true, true])]);
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
