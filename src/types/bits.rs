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
    /// A single bit prepended onto `rest`, O(1) with exactly one
    /// allocation (the `Node` itself). The cheap specialization of
    /// `Prepend` for the one-bit-at-a-time case `flatten_alt`'s `Alt`
    /// tagging and `Star`'s per-step marker both are: `Prepend` always
    /// pays for collecting a `Vec` and copying it into a fresh
    /// `Rc<[bool]>` slice before it ever gets here, pure overhead when
    /// the caller already knows there's only one bit. That overhead is
    /// paid once per `Alt`/`Star` level, every character, for any
    /// pattern that keeps one live in its current residual -- a tiny
    /// alternation inside a loop, run over a long input, pays it far
    /// more often than the wide-but-shallow alternations `Prepend`'s
    /// general form was built for.
    PrependOne(bool, Rc<Node>),
    /// `left` followed by `right`, both already-lazy chains grafted
    /// together in O(1): see `Bits::prepend_bits`.
    Chain(Rc<Node>, Rc<Node>),
}

thread_local! {
    /// The one shared empty chain. Every `Bits::empty()` call --
    /// directly, or via `AppendBits::empty()`/`prepend_one` composing
    /// with it -- used to allocate its own fresh `Node::Nil`; sharing
    /// one `Rc` here instead removes an allocation from what is
    /// otherwise a per-character hot path for any pattern with a live
    /// `Alt` or `Star`.
    static EMPTY_NODE: Rc<Node> = Rc::new(Node::Nil);
}

fn empty_rc() -> Rc<Node> {
    EMPTY_NODE.with(|n| n.clone())
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
            Node::Prepend(_, rest) => stack.push(std::mem::replace(rest, empty_rc())),
            Node::PrependOne(_, rest) => stack.push(std::mem::replace(rest, empty_rc())),
            Node::Chain(l, r) => {
                stack.push(std::mem::replace(l, empty_rc()));
                stack.push(std::mem::replace(r, empty_rc()));
            }
        }
        while let Some(rc) = stack.pop() {
            if let Ok(mut owned) = Rc::try_unwrap(rc) {
                match &mut owned {
                    Node::Nil => {}
                    Node::Prepend(_, rest) => stack.push(std::mem::replace(rest, empty_rc())),
                    Node::PrependOne(_, rest) => stack.push(std::mem::replace(rest, empty_rc())),
                    Node::Chain(l, r) => {
                        stack.push(std::mem::replace(l, empty_rc()));
                        stack.push(std::mem::replace(r, empty_rc()));
                    }
                }
                // `owned` drops here, trivially -- its children are now shared Nils.
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
        Bits(empty_rc())
    }

    pub fn is_empty(&self) -> bool {
        matches!(&*self.0, Node::Nil)
    }

    /// `prefix ++ self`, O(prefix.len()) regardless of how long `self`
    /// already is -- the existing chain is shared, not copied. When
    /// `prefix` is a single bit, `prepend_one` does the same thing
    /// without the `Rc<[bool]>` slice-copy this general form always
    /// pays for.
    pub fn prepend(&self, prefix: &[bool]) -> Bits {
        if prefix.is_empty() {
            return self.clone();
        }
        Bits(Rc::new(Node::Prepend(Rc::from(prefix), self.0.clone())))
    }

    /// `[bit] ++ self`, O(1) with exactly one allocation -- see
    /// `Node::PrependOne`.
    pub fn prepend_one(&self, bit: bool) -> Bits {
        Bits(Rc::new(Node::PrependOne(bit, self.0.clone())))
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
                Node::PrependOne(bit, rest) => {
                    out.push(*bit);
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
    /// already is. When `suffix` is a single bit, `append_one` does the
    /// same thing without the `Vec` collection and reversal this
    /// general form always pays for.
    pub fn append(&self, suffix: &[bool]) -> AppendBits {
        if suffix.is_empty() {
            return self.clone();
        }
        let mut rev_suffix: Vec<bool> = suffix.to_vec();
        rev_suffix.reverse();
        AppendBits(self.0.prepend(&rev_suffix))
    }

    /// `self ++ [bit]`, O(1) with exactly one allocation -- the
    /// single-bit specialization of `append`. This is `flatten_alt`'s
    /// own per-level `Alt` tagging: one bit appended per level, every
    /// character, for any pattern that keeps an `Alt` live in its
    /// current residual.
    pub fn append_one(&self, bit: bool) -> AppendBits {
        AppendBits(self.0.prepend_one(bit))
    }

    /// `[bit] ++ self`, O(1) with exactly two allocations (one for the
    /// single-bit chain, one to graft it onto `self`) -- the
    /// single-bit specialization of
    /// `AppendBits::from(vec![bit]).append_bits(self)`, which pays for
    /// a `Vec` and a slice-copying `Prepend` to represent that same
    /// one bit. This is `Star`'s own per-step marker: one fresh `false`
    /// prepended onto whatever bits its body's own derivative already
    /// carries, every character, for any pattern that keeps a `Star`
    /// live in its current residual.
    pub fn prepend_one(&self, bit: bool) -> AppendBits {
        let tiny = Bits::empty().prepend_one(bit);
        AppendBits(tiny.prepend_bits(&self.0))
    }

    /// Materializes the full bit sequence. O(total length); call once,
    /// not on every intermediate step.
    pub fn to_vec(&self) -> Vec<bool> {
        let mut v = self.0.to_vec();
        v.reverse();
        v
    }

    /// `self ++ suffix`, O(1) regardless of how long either side already
    /// is -- grafts the two existing chains together instead of
    /// flattening either one first. The `AppendBits` analogue of
    /// `Bits::prepend_bits`; use this (not `suffix.to_vec()` fed into
    /// `append`) whenever `suffix` is itself another `AppendBits` that
    /// may already carry its own accumulated history, e.g. one strand's
    /// own per-step delta out of a wide alternation (see
    /// `pderiv_bc::flatten_alt`, which builds exactly such per-leaf
    /// prefixes in O(1) each instead of the O(leaf depth) a `Vec<bool>`
    /// clone-at-every-leaf would cost, O(p^2) total over a p-leaf
    /// right-nested chain).
    pub fn append_bits(&self, suffix: &AppendBits) -> AppendBits {
        if suffix.is_empty() {
            return self.clone();
        }
        if self.is_empty() {
            return suffix.clone();
        }
        AppendBits(self.0.prepend_bits(&suffix.0))
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
    fn append_bits_chain_builds_in_order() {
        let base: AppendBits = vec![true, false].into();
        let suffix: AppendBits = vec![false, true].into();
        assert_eq!(base.append_bits(&suffix).to_vec(), vec![true, false, false, true]);
    }

    #[test]
    fn append_bits_chain_empty_suffix_is_identity() {
        let base: AppendBits = vec![true].into();
        assert_eq!(base.append_bits(&AppendBits::empty()).to_vec(), vec![true]);
    }

    #[test]
    fn append_bits_chain_empty_self_is_suffix() {
        let suffix: AppendBits = vec![true, false].into();
        assert_eq!(AppendBits::empty().append_bits(&suffix).to_vec(), vec![true, false]);
    }

    #[test]
    fn append_bits_chain_does_not_mutate_either_input() {
        let base: AppendBits = vec![true].into();
        let suffix: AppendBits = vec![false].into();
        let combined = base.append_bits(&suffix);
        assert_eq!(base.to_vec(), vec![true]);
        assert_eq!(suffix.to_vec(), vec![false]);
        assert_eq!(combined.to_vec(), vec![true, false]);
    }

    #[test]
    fn append_bits_chain_matches_vec_extend_semantics() {
        let mut v: Vec<bool> = Vec::new();
        let mut b = AppendBits::empty();
        for i in 0..2000 {
            let chunk: AppendBits = vec![i % 2 == 0, i % 3 == 0].into();
            v.extend_from_slice(&chunk.to_vec());
            b = b.append_bits(&chunk);
        }
        assert_eq!(b.to_vec(), v);
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

    // --- prepend_one / append_one: differential tests against the
    // general (slice-based) paths they specialize. Each one asserts
    // byte-for-byte equal `to_vec()` output, not just "looks right" --
    // these exist specifically so the cheap paths can never silently
    // drift in value from what they replace.

    #[test]
    fn bits_prepend_one_matches_prepend_single_slice() {
        let base: Bits = vec![true, false, true].into();
        for bit in [false, true] {
            assert_eq!(base.prepend_one(bit).to_vec(), base.prepend(&[bit]).to_vec());
        }
    }

    #[test]
    fn bits_prepend_one_matches_prepend_from_empty() {
        for bit in [false, true] {
            assert_eq!(
                Bits::empty().prepend_one(bit).to_vec(),
                Bits::empty().prepend(&[bit]).to_vec()
            );
        }
    }

    #[test]
    fn bits_prepend_one_chain_matches_vec_push_front_semantics() {
        let mut v: Vec<bool> = Vec::new();
        let mut b = Bits::empty();
        for i in 0..5000 {
            let bit = i % 3 == 0;
            v.insert(0, bit);
            b = b.prepend_one(bit);
        }
        assert_eq!(b.to_vec(), v);
    }

    #[test]
    fn append_one_matches_append_single_slice() {
        let base: AppendBits = vec![true, false, true].into();
        for bit in [false, true] {
            assert_eq!(base.append_one(bit).to_vec(), base.append(&[bit]).to_vec());
        }
    }

    #[test]
    fn append_one_matches_append_from_empty() {
        for bit in [false, true] {
            assert_eq!(
                AppendBits::empty().append_one(bit).to_vec(),
                AppendBits::empty().append(&[bit]).to_vec()
            );
        }
    }

    #[test]
    fn append_one_chain_matches_vec_push_semantics() {
        let mut v: Vec<bool> = Vec::new();
        let mut b = AppendBits::empty();
        for i in 0..5000 {
            let bit = i % 3 == 0;
            v.push(bit);
            b = b.append_one(bit);
        }
        assert_eq!(b.to_vec(), v);
    }

    /// `flatten_alt`'s own access pattern: branch on every level, not
    /// just a single straight-line chain -- `append_one` must stay
    /// correct (and independent per branch) when the same prefix
    /// handle is extended two different ways from the same point, same
    /// as `Bits::prepend`/`append` already are (see
    /// `prepend_does_not_mutate_the_original`).
    #[test]
    fn append_one_does_not_mutate_the_original_and_branches_independently() {
        let base: AppendBits = vec![true].into();
        let left = base.append_one(false);
        let right = base.append_one(true);
        assert_eq!(base.to_vec(), vec![true]);
        assert_eq!(left.to_vec(), vec![true, false]);
        assert_eq!(right.to_vec(), vec![true, true]);
    }

    #[test]
    fn prepend_one_matches_from_vec_append_bits_single_bit() {
        let bs: AppendBits = vec![true, false, true].into();
        for bit in [false, true] {
            let expected = AppendBits::from(vec![bit]).append_bits(&bs);
            assert_eq!(bs.prepend_one(bit).to_vec(), expected.to_vec());
        }
    }

    #[test]
    fn prepend_one_matches_from_vec_append_bits_onto_empty() {
        for bit in [false, true] {
            let expected = AppendBits::from(vec![bit]).append_bits(&AppendBits::empty());
            assert_eq!(AppendBits::empty().prepend_one(bit).to_vec(), expected.to_vec());
            assert_eq!(AppendBits::empty().prepend_one(bit).to_vec(), vec![bit]);
        }
    }

    #[test]
    fn prepend_one_chain_matches_star_iteration_semantics() {
        // Mirrors pderiv_bc's own Star arm: prepend_one(false) applied
        // repeatedly onto a growing tail, as repeated Star iterations would.
        let mut v: Vec<bool> = Vec::new();
        let mut b = AppendBits::empty();
        for _ in 0..5000 {
            v.insert(0, false);
            b = b.prepend_one(false);
        }
        assert_eq!(b.to_vec(), v);
    }

    #[test]
    fn prepend_one_does_not_mutate_the_original() {
        let base: AppendBits = vec![true].into();
        let extended = base.prepend_one(false);
        assert_eq!(base.to_vec(), vec![true]);
        assert_eq!(extended.to_vec(), vec![false, true]);
    }

    #[test]
    fn millions_prepend_one_and_append_one_drop_without_stack_overflow() {
        let mut b = Bits::empty();
        for _ in 0..3_000_000 {
            b = b.prepend_one(true);
        }
        assert_eq!(b.to_vec().len(), 3_000_000);
        drop(b);

        let mut a = AppendBits::empty();
        for _ in 0..3_000_000 {
            a = a.append_one(true);
        }
        assert_eq!(a.to_vec().len(), 3_000_000);
        drop(a);

        let mut p = AppendBits::empty();
        for _ in 0..3_000_000 {
            p = p.prepend_one(true);
        }
        assert_eq!(p.to_vec().len(), 3_000_000);
        drop(p);
    }

    /// Simulates `flatten_alt`'s real shape: a binary tree of
    /// `append_one` calls (not a straight chain), built and dropped
    /// many times over, the way one call per character over a long
    /// input would. Mainly a drop/leak-shape regression guard; value
    /// correctness at depth is already covered by the chain tests above.
    #[test]
    fn many_small_binary_append_one_trees_build_and_drop_correctly() {
        fn flatten(depth: u32, prefix: AppendBits, out: &mut Vec<Vec<bool>>) {
            if depth == 0 {
                out.push(prefix.to_vec());
                return;
            }
            flatten(depth - 1, prefix.append_one(false), out);
            flatten(depth - 1, prefix.append_one(true), out);
        }
        for _ in 0..10_000 {
            let mut out = Vec::new();
            flatten(4, AppendBits::empty(), &mut out);
            assert_eq!(out.len(), 16);
            assert!(out.contains(&vec![false, false, false, false]));
            assert!(out.contains(&vec![true, true, true, true]));
        }
    }

    #[test]
    fn empty_node_sharing_keeps_independent_values_independent() {
        // Bits::empty()/AppendBits::empty() now share one underlying
        // Rc<Node::Nil>; confirm that sharing the sentinel never leaks
        // into sharing content -- two chains built from separately
        // obtained "empty" handles must stay unrelated to each other.
        let e1 = Bits::empty();
        let e2 = Bits::empty();
        let b1 = e1.prepend_one(true);
        let b2 = e2.prepend_one(false);
        assert_eq!(b1.to_vec(), vec![true]);
        assert_eq!(b2.to_vec(), vec![false]);
        assert_eq!(e1.to_vec(), Vec::<bool>::new());
        assert_eq!(e2.to_vec(), Vec::<bool>::new());
        drop(e1);
        drop(e2);
        // Nothing else should have been affected by dropping these handles.
        assert_eq!(b1.to_vec(), vec![true]);
        assert_eq!(b2.to_vec(), vec![false]);
    }
}
