//! regex-engine/src/regex/simplify/annotated.rs
//! 
//! Simplification for annotated ARegex (simp from Figure 6)
//! 
//! Simplification rules:
//!   • Seq where one side is Phi -> Phi
//!   • Seq(bs, Eps(bs'), ri) -> fuse(bs++bs', ri)   (ε·r = r)
//!   • Alt with empty operand list -> Phi
//!   • Nested Alt flattened into a list -> flat list
//!   • Alt([ri]) -> fuse(bs, simp(ri))
//!   • Alt list: remove Phi, remove duplicates (keep first occurrence)

use crate::types::ARegex;
use crate::regex::nullable::annotated::is_phi;
use crate::posix::bitcoded::internalize::fuse;

/// Simplify an annotated expression to a fixpoint.
pub fn simp(ri: ARegex) -> ARegex {
    simp_once(ri)
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
                let mut combined = bs;
                combined.extend(bs2);
                return fuse(&combined, r2s);
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
                1 => fuse(&bs, branches.remove(0)),
                _ => {
                    let folded = fold_alt(branches);
                    fuse(&bs, folded)
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
                let r1f = fuse(&bs, *r1);
                let r2f = fuse(&bs, *r2);
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
        acc = ARegex::Alt(vec![], Box::new(b), Box::new(acc));
    }
    acc
}

fn dedup_first(branches: &mut Vec<ARegex>) {
    let mut i = 0;
    while i < branches.len() {
        let mut j = i + 1;
        while j < branches.len() {
            if branches[i] == branches[j] {
                branches.remove(j);
            } else {
                j += 1;
            }
        }
        i += 1;
    }
}


// -------------------------------
// Tests for simp
// -------------------------------

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
        let ri = ARegex::Eps(vec![false]);
        assert_eq!(simp(ri.clone()), ri);
    }

    // simp(Lit) = Lit  (unchanged)
    #[test]
    fn simp_lit_unchanged() {
        let ri = ARegex::Lit(vec![], 'a');
        assert_eq!(simp(ri.clone()), ri);
    }

    // Seq where left is Phi → Phi
    #[test]
    fn simp_seq_phi_left_gives_phi() {
        let ri = ARegex::Seq(vec![], Box::new(ARegex::Phi), Box::new(ARegex::lit('a')));
        assert_eq!(simp(ri), ARegex::Phi);
    }

    // Seq where right is Phi → Phi
    #[test]
    fn simp_seq_phi_right_gives_phi() {
        let ri = ARegex::Seq(vec![], Box::new(ARegex::lit('a')), Box::new(ARegex::Phi));
        assert_eq!(simp(ri), ARegex::Phi);
    }

    // Seq(bs, Eps(bs2), r) → fuse(bs++bs2, r) - ε·r = r with bit merge
    #[test]
    fn simp_seq_eps_left_fuses_bits() {
        // Seq([], Eps([false]), Lit([], 'a'))  →  Lit([false], 'a')
        let ri = ARegex::Seq(
            vec![],
            Box::new(ARegex::Eps(vec![false])),
            Box::new(ARegex::Lit(vec![], 'a')),
        );
        assert_eq!(simp(ri), ARegex::Lit(vec![false], 'a'));
    }

    // Alt where all branches are Phi → Phi
    #[test]
    fn simp_alt_all_phi_gives_phi() {
        let ri = ARegex::Alt(vec![], Box::new(ARegex::Phi), Box::new(ARegex::Phi));
        assert_eq!(simp(ri), ARegex::Phi);
    }

    // Alt where one branch is Phi → the other branch is kept (with outer bits fused)
    #[test]
    fn simp_alt_one_phi_drops_phi() {
        // Alt([], Phi, Lit([], 'a'))  →  Lit([], 'a')   (outer bits are [] so no change)
        let ri = ARegex::Alt(
            vec![],
            Box::new(ARegex::Phi),
            Box::new(ARegex::Lit(vec![], 'a')),
        );
        assert_eq!(simp(ri), ARegex::Lit(vec![], 'a'));
    }

    // Alt with a single non-Phi branch after filtering → fuse outer bits onto that branch
    #[test]
    fn simp_alt_single_branch_fuses_outer_bits() {
        // Alt([true], Phi, Eps([]))  →  fuse([true], Eps([])) = Eps([true])
        let ri = ARegex::Alt(
            vec![true],
            Box::new(ARegex::Phi),
            Box::new(ARegex::Eps(vec![])),
        );
        assert_eq!(simp(ri), ARegex::Eps(vec![true]));
    }

    // Duplicate branches are deduplicated (keep first occurrence)
    #[test]
    fn simp_alt_deduplicates_branches() {
        let branch = ARegex::Lit(vec![], 'a');
        let ri = ARegex::Alt(
            vec![],
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
        let inner = ARegex::Seq(vec![], Box::new(ARegex::Phi), Box::new(ARegex::lit('a')));
        let ri = ARegex::Star(vec![], Box::new(inner));
        let result = simp(ri);
        assert!(matches!(result, ARegex::Star(_, ref inner) if matches!(inner.as_ref(), ARegex::Phi)));
    }
}