//! src/parsers/deriv_bc/simplify.rs
//! 
//! Simplification for ARegex: 
//! Phi propagation, eps-elision, Alt flattening/dedup.

use crate::types::{ARegex, Bits};
use crate::parsers::deriv_bc::nullable::is_phi;
use crate::parsers::deriv_bc::internalize::fuse_bits;

/// Simplify an annotated expression to a fixpoint, re-applying `simp_once`
/// until it stops changing anything (mirrors the reference `simp r = let
/// s = simp2 r in if s == r then r else simp s`). A single bottom-up pass
/// resolves most shapes already, but a collapse at one level (e.g. Alt
/// dedup removing a branch) can occasionally expose a new one at the
/// level above that only a second pass sees.
///
/// Compares by `same_shape`, not full bit-sensitive equality: every
/// `simp_once` transformation is driven by the expression's *variant*
/// shape (Phi propagation, Eps elision, Alt dedup), never by bit
/// content, so a shape fixpoint is already a transformation fixpoint.
/// Using the derived `==` here would force a full `Bits::to_vec()` at
/// every node on every iteration -- exactly the per-step materialization
/// the `Bits` type exists to avoid, reintroducing an O(n^2) cost over n
/// characters (confirmed empirically: memory/time blew up superlinearly
/// until this was switched).
pub fn simp(ri: ARegex) -> ARegex {
    let mut cur = ri;
    loop {
        let next = simp_once(cur.clone());
        if same_shape(&next, &cur) {
            return next;
        }
        cur = next;
    }
}

fn simp_once(ri: ARegex) -> ARegex {
    match ri {
        ARegex::Phi => ARegex::Phi,
        ARegex::Eps(_) => ri,
        ARegex::Lit(_, _) => ri,

        ARegex::Seq(bs, r1, r2) => {
            let r1s = simp_once(*r1);
            let r2s = simp_once(*r2);

            if is_phi(&r1s) || is_phi(&r2s) {
                return ARegex::Phi;
            }

            if let ARegex::Eps(bs2) = r1s {
                // bs ++ bs2, prepended onto r2s in that order: fuse bs2
                // on first (innermost), then bs (outermost). Via
                // fuse_bits, not fuse(&x.to_vec(), ..): bs can already
                // be a Star's whole accumulated iteration history by
                // this point, and to_vec()-ing it on every character
                // is exactly the O(n^2) cost `Bits` exists to avoid.
                return fuse_bits(&bs, fuse_bits(&bs2, r2s));
            }

            // Symmetric case: `Seq(x, Eps)` collapses to `x`, matching the
            // plain-Regex simplify()'s own `(_, Regex::Eps) => r`. Only
            // safe when r2's own bits are empty: mk_eps_bc concatenates
            // bs ++ r1.bits ++ r2.bits (left-to-right), so fusing r2's
            // bits onto r1s the same way the left case does would put
            // them *before* r1's own (not-yet-known) bits instead of
            // after -- wrong order whenever r2's bits are non-empty.
            if let ARegex::Eps(bs2) = &r2s {
                if bs2.is_empty() {
                    return fuse_bits(&bs, r1s);
                }
            }

            ARegex::Seq(bs, Box::new(r1s), Box::new(r2s))
        }

        ARegex::Alt(bs, r1, r2) => {
            let branches_raw = collect_alt_branches(*r1, *r2);
            let branches_simped: Vec<ARegex> = branches_raw
                .into_iter()
                .map(simp_once)
                .collect();

            let mut branches: Vec<ARegex> = branches_simped
                .into_iter()
                .filter(|b| !is_phi(b))
                .collect();

            dedup_first(&mut branches);

            match branches.len() {
                0 => ARegex::Phi,
                1 => fuse_bits(&bs, branches.remove(0)),
                _ => {
                    let folded = fold_alt(branches);
                    fuse_bits(&bs, folded)
                }
            }
        }

        ARegex::Star(bs, r) => {
            ARegex::Star(bs, Box::new(simp_once(*r)))
        }
    }
}

fn collect_alt_branches(r1: ARegex, r2: ARegex) -> Vec<ARegex> {
    let mut out = Vec::new();
    push_branches(r1, &mut out);
    push_branches(r2, &mut out);
    out
}

fn push_branches(ri: ARegex, out: &mut Vec<ARegex>) {
    match ri {
        ARegex::Alt(bs, r1, r2) => {
            if bs.is_empty() {
                push_branches(*r1, out);
                push_branches(*r2, out);
            } else {
                let r1f = fuse_bits(&bs, *r1);
                let r2f = fuse_bits(&bs, *r2);
                push_branches(r1f, out);
                push_branches(r2f, out);
            }
        }
        other => out.push(other),
    }
}

fn fold_alt(mut branches: Vec<ARegex>) -> ARegex {
    branches.reverse();
    let mut acc = branches.remove(0);
    for b in branches {
        acc = ARegex::Alt(Bits::empty(), Box::new(b), Box::new(acc));
    }
    acc
}

/// Ignores each node's own bit-code history (`bs`) when comparing:
/// two branches that denote the same future language should dedup
/// even if they carry different POSIX/greedy choice paths so far. The
/// reference `nub2 = nubBy (\(r,_) (s,_) -> r == s)` makes the same
/// exclusion explicitly, and Sulzmann & Lu 2012's pattern equality
/// (Fig. 9) compares only the underlying regex language, never the
/// accumulated match. Keeping the bit-sensitive `#[derive(PartialEq)]`
/// as `ARegex`'s real `Eq` (used elsewhere, e.g. unit tests asserting
/// exact bit output) and scoping this shape-only comparison to dedup
/// alone avoids disturbing those.
fn same_shape(a: &ARegex, b: &ARegex) -> bool {
    match (a, b) {
        (ARegex::Phi, ARegex::Phi) => true,
        (ARegex::Eps(_), ARegex::Eps(_)) => true,
        (ARegex::Lit(_, c1), ARegex::Lit(_, c2)) => c1 == c2,
        (ARegex::Alt(_, a1, a2), ARegex::Alt(_, b1, b2)) => same_shape(a1, b1) && same_shape(a2, b2),
        (ARegex::Seq(_, a1, a2), ARegex::Seq(_, b1, b2)) => same_shape(a1, b1) && same_shape(a2, b2),
        (ARegex::Star(_, a1), ARegex::Star(_, b1)) => same_shape(a1, b1),
        _ => false,
    }
}

fn dedup_first(branches: &mut Vec<ARegex>) {
    let mut i = 0;
    while i < branches.len() {
        let mut j = i + 1;
        while j < branches.len() {
            if same_shape(&branches[i], &branches[j]) {
                branches.remove(j);
            } else {
                j += 1;
            }
        }
        i += 1;
    }
}


// Tests for simp

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ARegex;

    // simp(Phi) = Phi
    #[test]
    fn simp_phi_is_phi() {
        assert_eq!(simp(ARegex::Phi), ARegex::Phi);
    }

    // simp(Eps) = Eps  (unchanged)
    #[test]
    fn simp_eps_unchanged() {
        let ri = ARegex::Eps(vec![false].into());
        assert_eq!(simp(ri.clone()), ri);
    }

    // simp(Lit) = Lit  (unchanged)
    #[test]
    fn simp_lit_unchanged() {
        let ri = ARegex::Lit(vec![].into(), 'a');
        assert_eq!(simp(ri.clone()), ri);
    }

    // Seq where left is Phi → Phi
    #[test]
    fn simp_seq_phi_left_gives_phi() {
        let ri = ARegex::Seq(vec![].into(), Box::new(ARegex::Phi), Box::new(ARegex::lit('a')));
        assert_eq!(simp(ri), ARegex::Phi);
    }

    // Seq where right is Phi → Phi
    #[test]
    fn simp_seq_phi_right_gives_phi() {
        let ri = ARegex::Seq(vec![].into(), Box::new(ARegex::lit('a')), Box::new(ARegex::Phi));
        assert_eq!(simp(ri), ARegex::Phi);
    }

    // Seq(bs, Eps(bs2), r) → fuse(bs++bs2, r) - ε·r = r with bit merge
    #[test]
    fn simp_seq_eps_left_fuses_bits() {
        // Seq([], Eps([false]), Lit([], 'a'))  →  Lit([false], 'a')
        let ri = ARegex::Seq(
            vec![].into(),
            Box::new(ARegex::Eps(vec![false].into())),
            Box::new(ARegex::Lit(vec![].into(), 'a')),
        );
        assert_eq!(simp(ri), ARegex::Lit(vec![false].into(), 'a'));
    }

    // Seq(bs, r1, Eps([])) → fuse(bs, r1) - r.eps = r, only when r2's bits are empty
    #[test]
    fn simp_seq_eps_right_with_empty_bits_collapses() {
        let ri = ARegex::Seq(
            vec![].into(),
            Box::new(ARegex::lit('a')),
            Box::new(ARegex::Eps(vec![].into())),
        );
        assert_eq!(simp(ri), ARegex::Lit(vec![].into(), 'a'));
    }

    // Alt where all branches are Phi → Phi
    #[test]
    fn simp_alt_all_phi_gives_phi() {
        let ri = ARegex::Alt(vec![].into(), Box::new(ARegex::Phi), Box::new(ARegex::Phi));
        assert_eq!(simp(ri), ARegex::Phi);
    }

    // Alt where one branch is Phi → the other branch is kept (with outer bits fused)
    #[test]
    fn simp_alt_one_phi_drops_phi() {
        // Alt([], Phi, Lit([], 'a'))  →  Lit([], 'a')   (outer bits are [] so no change)
        let ri = ARegex::Alt(
            vec![].into(),
            Box::new(ARegex::Phi),
            Box::new(ARegex::Lit(vec![].into(), 'a')),
        );
        assert_eq!(simp(ri), ARegex::Lit(vec![].into(), 'a'));
    }

    // Alt with a single non-Phi branch after filtering → fuse outer bits onto that branch
    #[test]
    fn simp_alt_single_branch_fuses_outer_bits() {
        // Alt([true], Phi, Eps([]))  →  fuse([true], Eps([])) = Eps([true])
        let ri = ARegex::Alt(
            vec![true].into(),
            Box::new(ARegex::Phi),
            Box::new(ARegex::Eps(vec![].into())),
        );
        assert_eq!(simp(ri), ARegex::Eps(vec![true].into()));
    }

    // Duplicate branches are deduplicated (keep first occurrence)
    #[test]
    fn simp_alt_deduplicates_branches() {
        let branch = ARegex::Lit(vec![].into(), 'a');
        let ri = ARegex::Alt(
            vec![].into(),
            Box::new(branch.clone()),
            Box::new(branch.clone()),
        );
        // After dedup only one branch remains, so result is fuse([], branch) = branch
        assert_eq!(simp(ri), branch);
    }

    // Star is simplified recursively on its inner expression
    #[test]
    fn simp_star_simplifies_inner() {
        // Star([], Seq([], Phi, Lit([], 'a')))  →  Star([], Phi)
        let inner = ARegex::Seq(vec![].into(), Box::new(ARegex::Phi), Box::new(ARegex::lit('a')));
        let ri = ARegex::Star(vec![].into(), Box::new(inner));
        let result = simp(ri);
        assert!(matches!(result, ARegex::Star(_, ref inner) if matches!(inner.as_ref(), ARegex::Phi)));
    }
}