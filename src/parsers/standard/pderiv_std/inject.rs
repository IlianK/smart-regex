//! Injection-closure for tree-based partial-derivative parser: 
//! each strand carries an `Inj` instead of a bit prefix

use std::collections::HashSet;
use std::rc::Rc;
use crate::types::{ParseTree, Regex};

/// Reconstructs a strand's `ParseTree` from the eventual tree of its residual
/// `Rc`, not `Box`, since one parent's injection is reused by several child strands 
/// when a step fans out (e.g. `Alt`)
pub(crate) type Inj = Rc<dyn Fn(ParseTree) -> ParseTree>;

pub(crate) fn identity_inj() -> Inj {
    Rc::new(|v| v)
}

/// Dedup on the residual alone, keeping the first (highest-priority, GREEDY-ordered)
pub(crate) fn nub_tree(strands: Vec<(Regex, Inj)>) -> Vec<(Regex, Inj)> {
    let mut seen: HashSet<Regex> = HashSet::with_capacity(strands.len());
    let mut out = Vec::with_capacity(strands.len());
    for (r, inj) in strands {
        if seen.insert(r.clone()) {
            out.push((r, inj));
        }
    }
    out
}
