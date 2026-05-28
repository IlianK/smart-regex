//! Simplification for annotated ARegex (simp from Figure 6)
//!
//! Based on paper Figure 6:
//!   • Seq where one side is Phi → Phi
//!   • Seq(bs, Eps(bs'), ri) → fuse(bs++bs', ri)   (ε·r = r)
//!   • Alt with empty operand list → Phi
//!   • Nested Alt flattened into a list → flat list
//!   • Alt([ri]) → fuse(bs, simp(ri))
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