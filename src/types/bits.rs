//! src/types/bits.rs
//!
//! Persistent, O(1)-amortized-prepend bit sequence for `ARegex`'s own
//! per-node annotations. `fuse`/`combine` prepend a small prefix onto a
//! node's existing bits at every derivative step; with a plain
//! `Vec<bool>` that existing content gets copied every time, which is
//! O(current length) per step and O(n^2) over n characters once a node
//! survives many steps (confirmed empirically: node count stays bounded
//! but one node's own bit length grew linearly with input length, and
//! wall-clock time went clearly superlinear). `Bits::prepend` instead
//! wraps the existing chain in a new `Rc` node -- O(prefix length) only,
//! never touching what's already there. The chain is only walked (and
//! materialized to `Vec<bool>`) when the bits are actually consumed, by
//! `mk_eps_bc`/`decode` at the end of a parse.

use std::rc::Rc;

enum Node {
    Nil,
    Prepend(Rc<[bool]>, Rc<Node>),
    /// `left` followed by `right`, both already-lazy chains grafted
    /// together in O(1): see `Bits::prepend_bits`.
    Chain(Rc<Node>, Rc<Node>),
}

/// Rust's derived drop glue for this enum would recurse through a
/// chained node's children one stack frame per node -- fine at hundreds
/// of thousands of characters (confirmed empirically), but a genuine
/// stack-overflow risk at the millions-of-characters scale this type
/// exists to support. Unwinds exclusively-owned descendants iteratively
/// via an explicit work-list instead (a plain single `next` pointer
/// isn't enough once `Chain` gives a node two children, not one); a
/// node still shared with another live `Bits` is left alone, since
/// something else still owns it and will drop it in its own time.
impl Drop for Node {
    fn drop(&mut self) {
        let mut stack: Vec<Rc<Node>> = Vec::new();
        match self {
            Node::Nil => return,
            Node::Prepend(_, rest) => stack.push(std::mem::replace(rest, Rc::new(Node::Nil))),
            Node::Chain(l, r) => {
                stack.push(std::mem::replace(l, Rc::new(Node::Nil)));
                stack.push(std::mem::replace(r, Rc::new(Node::Nil)));
            }
        }
        while let Some(rc) = stack.pop() {
            if let Ok(mut owned) = Rc::try_unwrap(rc) {
                match &mut owned {
                    Node::Nil => {}
                    Node::Prepend(_, rest) => stack.push(std::mem::replace(rest, Rc::new(Node::Nil))),
                    Node::Chain(l, r) => {
                        stack.push(std::mem::replace(l, Rc::new(Node::Nil)));
                        stack.push(std::mem::replace(r, Rc::new(Node::Nil)));
                    }
                }
                // `owned` drops here, trivially -- its children are now fresh Nils.
            }
            // else: still shared elsewhere; leave it for its other owner.
        }
    }
}

#[derive(Clone)]
pub struct Bits(Rc<Node>);

/// Formats exactly like `Vec<bool>`'s own `{:?}` (e.g. `[false, true]`),
/// so existing Display/Debug-based assertions keep working unchanged.
impl std::fmt::Debug for Bits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_vec().fmt(f)
    }
}

impl Bits {
    pub fn empty() -> Self {
        Bits(Rc::new(Node::Nil))
    }

    pub fn is_empty(&self) -> bool {
        matches!(&*self.0, Node::Nil)
    }

    /// `prefix ++ self`, O(prefix.len()) regardless of how long `self`
    /// already is -- the existing chain is shared, not copied.
    pub fn prepend(&self, prefix: &[bool]) -> Bits {
        if prefix.is_empty() {
            return self.clone();
        }
        Bits(Rc::new(Node::Prepend(Rc::from(prefix), self.0.clone())))
    }

    /// `prefix ++ self`, O(1) regardless of how long `prefix` or `self`
    /// already are -- grafts the two existing chains together instead
    /// of flattening either one. Use this (not `prefix.to_vec()` fed
    /// into `prepend`) whenever `prefix` is itself another `Bits`
    /// that may already carry a parse's whole accumulated history:
    /// `to_vec()`-ing it first to call `prepend` would materialize
    /// all of it on every call, reintroducing the O(n) per-step cost
    /// this type exists to avoid.
    pub fn prepend_bits(&self, prefix: &Bits) -> Bits {
        if prefix.is_empty() {
            return self.clone();
        }
        if self.is_empty() {
            return prefix.clone();
        }
        Bits(Rc::new(Node::Chain(prefix.0.clone(), self.0.clone())))
    }

    /// Materializes the full bit sequence. O(total length); call once,
    /// at the point the bits are actually needed (mk_eps_bc/decode),
    /// not on every intermediate derivative step. Iterative (an explicit
    /// work-list, not recursion) for the same reason `Drop` is: a chain
    /// built from millions of `prepend`/`prepend_bits` calls must not
    /// walk the call stack one frame per node.
    pub fn to_vec(&self) -> Vec<bool> {
        let mut out = Vec::new();
        let mut stack: Vec<Rc<Node>> = vec![self.0.clone()];
        while let Some(node) = stack.pop() {
            match &*node {
                Node::Nil => {}
                Node::Prepend(seg, rest) => {
                    out.extend_from_slice(seg);
                    stack.push(rest.clone());
                }
                Node::Chain(l, r) => {
                    stack.push(r.clone());
                    stack.push(l.clone());
                }
            }
        }
        out
    }
}

impl From<Vec<bool>> for Bits {
    fn from(v: Vec<bool>) -> Self {
        Bits::empty().prepend(&v)
    }
}

impl PartialEq for Bits {
    fn eq(&self, other: &Self) -> bool {
        self.to_vec() == other.to_vec()
    }
}
impl Eq for Bits {}

/// Mirror of `Bits` for the opposite access pattern: `pderiv_bc`'s
/// `step_frontier` APPENDS a small suffix onto a potentially-long
/// per-strand bit history at every character (the partial-derivative
/// analogue of `fuse`'s prepend). A plain `Vec<bool>` clone-then-extend
/// is O(current length) per step there too, same O(n^2) over n
/// characters. Stores the REVERSE of the real sequence in a `Bits`
/// chain, so appending a suffix becomes prepending its reverse --
/// `Bits::prepend`'s existing O(suffix length) guarantee -- and
/// `to_vec` reverses back just once, at the point the bits are
/// actually consumed (`decode`'s final call).
#[derive(Clone)]
pub struct AppendBits(Bits);

impl std::fmt::Debug for AppendBits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_vec().fmt(f)
    }
}

impl AppendBits {
    pub fn empty() -> Self {
        AppendBits(Bits::empty())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// `self ++ suffix`, O(suffix.len()) regardless of how long `self`
    /// already is.
    pub fn append(&self, suffix: &[bool]) -> AppendBits {
        if suffix.is_empty() {
            return self.clone();
        }
        let mut rev_suffix: Vec<bool> = suffix.to_vec();
        rev_suffix.reverse();
        AppendBits(self.0.prepend(&rev_suffix))
    }

    /// Materializes the full bit sequence. O(total length); call once,
    /// not on every intermediate step.
    pub fn to_vec(&self) -> Vec<bool> {
        let mut v = self.0.to_vec();
        v.reverse();
        v
    }
}

impl From<Vec<bool>> for AppendBits {
    fn from(v: Vec<bool>) -> Self {
        AppendBits::empty().append(&v)
    }
}

impl PartialEq for AppendBits {
    fn eq(&self, other: &Self) -> bool {
        self.to_vec() == other.to_vec()
    }
}
impl Eq for AppendBits {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_empty() {
        assert!(Bits::empty().is_empty());
        assert_eq!(Bits::empty().to_vec(), Vec::<bool>::new());
    }

    #[test]
    fn prepend_builds_in_order() {
        let b = Bits::empty().prepend(&[true, false]).prepend(&[false]);
        assert_eq!(b.to_vec(), vec![false, true, false]);
    }

    #[test]
    fn prepend_empty_prefix_is_identity() {
        let b: Bits = vec![true, true].into();
        let b2 = b.prepend(&[]);
        assert_eq!(b2.to_vec(), vec![true, true]);
    }

    #[test]
    fn equality_is_by_value_not_by_construction_path() {
        let a = Bits::empty().prepend(&[true]).prepend(&[false]);
        let b: Bits = vec![false, true].into();
        assert_eq!(a, b);
    }

    #[test]
    fn from_vec_round_trips() {
        let v = vec![true, false, true, true];
        let b: Bits = v.clone().into();
        assert_eq!(b.to_vec(), v);
    }

    #[test]
    fn prepend_does_not_mutate_the_original() {
        let base: Bits = vec![true].into();
        let extended = base.prepend(&[false]);
        assert_eq!(base.to_vec(), vec![true]);
        assert_eq!(extended.to_vec(), vec![false, true]);
    }

    #[test]
    fn many_small_prepends_stay_cheap_and_correct() {
        let mut b = Bits::empty();
        for i in 0..5000 {
            b = b.prepend(&[i % 2 == 0]);
        }
        let v = b.to_vec();
        assert_eq!(v.len(), 5000);
        assert_eq!(v[0], false); // last prepended bit (i=4999, odd) ends up at the front... see below
    }

    #[test]
    fn prepend_bits_builds_in_order() {
        let tail: Bits = vec![true, false].into();
        let prefix: Bits = vec![false, true].into();
        assert_eq!(tail.prepend_bits(&prefix).to_vec(), vec![false, true, true, false]);
    }

    #[test]
    fn prepend_bits_empty_prefix_is_identity() {
        let tail: Bits = vec![true].into();
        assert_eq!(tail.prepend_bits(&Bits::empty()).to_vec(), vec![true]);
    }

    #[test]
    fn prepend_bits_empty_self_is_prefix() {
        let prefix: Bits = vec![true, false].into();
        assert_eq!(Bits::empty().prepend_bits(&prefix).to_vec(), vec![true, false]);
    }

    #[test]
    fn prepend_bits_does_not_mutate_either_input() {
        let tail: Bits = vec![true].into();
        let prefix: Bits = vec![false].into();
        let combined = tail.prepend_bits(&prefix);
        assert_eq!(tail.to_vec(), vec![true]);
        assert_eq!(prefix.to_vec(), vec![false]);
        assert_eq!(combined.to_vec(), vec![false, true]);
    }

    #[test]
    fn many_chained_prepend_bits_drop_without_stack_overflow() {
        // Right-leaning chain of Chain nodes, mimicking repeated
        // fuse_bits(&big_accumulated_history, small_piece) calls.
        let mut b = Bits::empty();
        for _ in 0..1_000_000 {
            let small: Bits = vec![true].into();
            b = b.prepend_bits(&small);
        }
        assert_eq!(b.to_vec().len(), 1_000_000);
        drop(b);
    }

    #[test]
    fn append_bits_empty_is_empty() {
        assert!(AppendBits::empty().is_empty());
        assert_eq!(AppendBits::empty().to_vec(), Vec::<bool>::new());
    }

    #[test]
    fn append_bits_builds_in_order() {
        let b = AppendBits::empty().append(&[true, false]).append(&[false]);
        assert_eq!(b.to_vec(), vec![true, false, false]);
    }

    #[test]
    fn append_bits_empty_suffix_is_identity() {
        let b: AppendBits = vec![true, true].into();
        assert_eq!(b.append(&[]).to_vec(), vec![true, true]);
    }

    #[test]
    fn append_bits_from_vec_round_trips() {
        let v = vec![true, false, true, true];
        let b: AppendBits = v.clone().into();
        assert_eq!(b.to_vec(), v);
    }

    #[test]
    fn append_bits_does_not_mutate_the_original() {
        let base: AppendBits = vec![true].into();
        let extended = base.append(&[false]);
        assert_eq!(base.to_vec(), vec![true]);
        assert_eq!(extended.to_vec(), vec![true, false]);
    }

    #[test]
    fn append_bits_matches_vec_extend_semantics() {
        let mut v: Vec<bool> = Vec::new();
        let mut b = AppendBits::empty();
        for i in 0..2000 {
            let chunk = [i % 2 == 0, i % 3 == 0];
            v.extend_from_slice(&chunk);
            b = b.append(&chunk);
        }
        assert_eq!(b.to_vec(), v);
    }

    #[test]
    fn millions_deep_chain_drops_without_stack_overflow() {
        let mut b = Bits::empty();
        for _ in 0..3_000_000 {
            b = b.prepend(&[true]);
        }
        assert_eq!(b.to_vec().len(), 3_000_000);
        drop(b); // would overflow the default test-thread stack without the iterative Drop impl

        let mut a = AppendBits::empty();
        for _ in 0..3_000_000 {
            a = a.append(&[true]);
        }
        assert_eq!(a.to_vec().len(), 3_000_000);
        drop(a);
    }
}
