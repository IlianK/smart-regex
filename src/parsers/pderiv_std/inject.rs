//! src/parsers/pderiv_std/inject.rs
//!
//! Injection-closure for tree-based partial-derivative parser:
//! each strand carries an `Inj` instead of a bit prefix.

use std::collections::HashSet;
use std::rc::Rc;
use crate::types::{ParseTree, Regex};

/// One character's own injection step, as `pderiv_tree` builds it --
/// nesting here is bounded by the *pattern's* size (e.g. `Alt`/`Seq`
/// wrapping), not the input length, so a plain closure is fine.
pub(crate) type InjStep = Rc<dyn Fn(ParseTree) -> ParseTree>;

/// A strand's full injection history, accumulated one `InjStep` per
/// character. Composing these as nested closures (`move |v|
/// parent(step(v))`) made both *calling* the result and *dropping* it
/// recurse one native stack frame per character -- fine at tens of
/// thousands of characters, a real stack-overflow risk beyond that
/// (confirmed empirically: a 100K-character input crashed on a
/// default-size stack). `Inj` instead stores the steps as a persistent
/// cons-list and applies them in an explicit loop.
enum InjNode {
    Identity,
    Step(InjStep, Rc<InjNode>),
}

/// Mirrors `Bits`'s own iterative `Drop` (src/types/bits.rs) for the
/// same reason: the derived recursive drop glue would walk this chain
/// one stack frame per character too.
impl Drop for InjNode {
    fn drop(&mut self) {
        let mut next = match self {
            InjNode::Identity => return,
            InjNode::Step(_, rest) => std::mem::replace(rest, Rc::new(InjNode::Identity)),
        };
        loop {
            match Rc::try_unwrap(next) {
                Ok(mut owned) => {
                    next = match &mut owned {
                        InjNode::Identity => return,
                        InjNode::Step(_, rest) => std::mem::replace(rest, Rc::new(InjNode::Identity)),
                    };
                }
                Err(_) => return,
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct Inj(Rc<InjNode>);

impl Inj {
    pub(crate) fn identity() -> Self {
        Inj(Rc::new(InjNode::Identity))
    }

    /// Adds one more character's step to the chain -- O(1), just wraps
    /// the existing chain in a new node, same as `Bits::prepend`.
    pub(crate) fn then(&self, step: InjStep) -> Self {
        Inj(Rc::new(InjNode::Step(step, self.0.clone())))
    }

    /// Applies every step in one iterative pass, in the order they were
    /// added (each character's step runs on the result of the ones
    /// added after it, matching the old `parent(step(v))` composition)
    /// -- O(chain length), no recursive calls regardless of how long
    /// the chain is.
    pub(crate) fn apply(&self, v: ParseTree) -> ParseTree {
        let mut v = v;
        let mut cur = &self.0;
        loop {
            match &**cur {
                InjNode::Identity => return v,
                InjNode::Step(step, rest) => {
                    v = step(v);
                    cur = rest;
                }
            }
        }
    }
}

/// Dedup on the residual alone, keeping the first (highest-priority,
/// GREEDY-ordered). Generic over the injection type: `pderiv_tree`'s
/// own call carries one step (`InjStep`), `step_frontier`'s call
/// carries the accumulated chain (`Inj`) -- dedup-by-residual doesn't
/// care which.
pub(crate) fn nub_tree<I>(strands: Vec<(Regex, I)>) -> Vec<(Regex, I)> {
    let mut seen: HashSet<Regex> = HashSet::with_capacity(strands.len());
    let mut out = Vec::with_capacity(strands.len());
    for (r, inj) in strands {
        if seen.insert(r.clone()) {
            out.push((r, inj));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_a_no_op() {
        assert_eq!(Inj::identity().apply(ParseTree::Empty), ParseTree::Empty);
    }

    #[test]
    fn steps_apply_in_the_order_they_were_added() {
        // then(wrap_a).then(wrap_b).apply(v) should equal wrap_a(wrap_b(v)),
        // matching the old `parent(step(v))` composition order.
        let inj = Inj::identity()
            .then(Rc::new(|v| ParseTree::Left(Box::new(v))))
            .then(Rc::new(|v| ParseTree::Right(Box::new(v))));
        let got = inj.apply(ParseTree::Empty);
        let want = ParseTree::Left(Box::new(ParseTree::Right(Box::new(ParseTree::Empty))));
        assert_eq!(got, want);
    }

    #[test]
    fn millions_deep_chain_drops_without_stack_overflow() {
        // Steps that don't themselves build a deeply nested ParseTree,
        // so this isolates Inj's own Drop from ParseTree's: real usage
        // never applies a chain this long to build a Box-nested value
        // anyway (a real Star's n iterations land in one flat Vec).
        let mut inj = Inj::identity();
        for _ in 0..3_000_000 {
            inj = inj.then(Rc::new(|v| v));
        }
        assert_eq!(inj.apply(ParseTree::Empty), ParseTree::Empty);
        drop(inj); // would overflow the default test-thread stack without the iterative Drop impl
    }
}
