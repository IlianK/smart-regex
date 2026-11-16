//! src/parsers/pderiv_std/pderiv_tree.rs
//! 
//! Standard Partial-Derivative-Based Parser
//! Antimirov Partial Derivative 
//! 
//! One step of the tree-based partial derivative:
//! `pderiv_bc`'s analogue, `(residual, InjStep)` pairs instead of bits.

use std::rc::Rc;
use crate::types::{ParseTree, Regex};
use crate::regex::nullable::nullable;
use crate::regex::mk_eps::mk_eps;
use crate::regex::simplify::smart_seq;
use super::inject::{InjStep, nub_tree};

/// Flattens a chain of nested `Alt`s into its leaves, pairing each leaf
/// with the composed injection closure its position in the chain
/// assigns (wrapping in `Left`/`Right` down the spine). Same fix, same
/// reasoning, as `pderiv_bc::pderiv::flatten_alt`: recursing through
/// `Alt` one level at a time and calling `nub_tree` at every level is
/// semantically fine (same "keep first occurrence" composition
/// argument) but costs O(p) redundant dedup passes for a p-wide
/// alternation. Flattening first and deduping once gives the identical
/// result for a fraction of the cost.
///
/// `wrap` composes outer-applied-last: each new level's `Left`/`Right`
/// goes *inside* the accumulated `wrap`, i.e. `w(Left(v))`, not
/// `Left(w(v))` -- the opposite order from how `prefix` accumulates for
/// `pderiv_bc`'s bits, where append order already matches encounter
/// order. Composing a closure the wrong way round doesn't panic or
/// drop anything; it silently builds the wrong parse tree, so this is
/// pinned down by `nested_alt_three_leaves_matches_hand_traced_wraps`
/// below rather than left to be caught by inspection alone.
fn flatten_alt<'a>(r: &'a Regex, wrap: &InjStep, out: &mut Vec<(&'a Regex, InjStep)>) {
    match r {
        Regex::Alt(r1, r2) => {
            let w = wrap.clone();
            let left_wrap: InjStep = Rc::new(move |v| w(ParseTree::Left(Box::new(v))));
            flatten_alt(r1, &left_wrap, out);
            let w = wrap.clone();
            let right_wrap: InjStep = Rc::new(move |v| w(ParseTree::Right(Box::new(v))));
            flatten_alt(r2, &right_wrap, out);
        }
        other => out.push((other, wrap.clone())),
    }
}

pub(crate) fn pderiv_tree(r: &Regex, x: char) -> Vec<(Regex, InjStep)> {
    match r {
        Regex::Eps | Regex::Phi => vec![],

        Regex::Lit(c) => {
            if *c == x {
                vec![(Regex::Eps, Rc::new(move |_: ParseTree| ParseTree::Char(x)) as InjStep)]
            } else {
                vec![]
            }
        }

        Regex::Alt(_, _) => {
            let mut leaves: Vec<(&Regex, InjStep)> = Vec::new();
            let identity: InjStep = Rc::new(|v| v);
            flatten_alt(r, &identity, &mut leaves);

            let mut out: Vec<(Regex, InjStep)> = Vec::new();
            for (leaf, wrap) in leaves {
                for (r_, inj) in pderiv_tree(leaf, x) {
                    let w = wrap.clone();
                    let combined: InjStep = Rc::new(move |v| w(inj(v)));
                    out.push((r_, combined));
                }
            }
            nub_tree(out)
        }

        Regex::Seq(r1, r2) => {
            if nullable(r1) {
                let mut out: Vec<(Regex, InjStep)> = pderiv_tree(r1, x)
                    .into_iter()
                    .map(|(r1_, inj1)| {
                        let r2c = (**r2).clone();
                        // smart_seq, not raw Seq: a nullable r1's own
                        // derivative can itself be Eps, and without
                        // collapsing that wrapper this strand and the
                        // "r1 matched empty, jump to r2" strand below can
                        // reach the same residual in two syntactically
                        // different shapes, defeating nub_tree's
                        // structural-equality dedup. PPDP12 licenses
                        // exactly this collapse ("we omit some obvious
                        // simplifications, e.g. (eps,A*) -> A*", Fig. 10),
                        // so this is not just a style choice.
                        if r1_ == Regex::Eps {
                            let f: InjStep = Rc::new(move |v2: ParseTree| {
                                ParseTree::Pair(Box::new(inj1(ParseTree::Empty)), Box::new(v2))
                            });
                            (r2c, f)
                        } else {
                            let f: InjStep = Rc::new(move |v: ParseTree| match v {
                                ParseTree::Pair(v1, v2) => ParseTree::Pair(Box::new(inj1(*v1)), v2),
                                other => panic!(
                                    "pderiv_tree: Seq-continue strand expected Pair, got {:?}",
                                    other
                                ),
                            });
                            (smart_seq(r1_, &r2c), f)
                        }
                    })
                    .collect();

                let v1_eps = mk_eps(r1);
                out.extend(pderiv_tree(r2, x).into_iter().map(|(r2_, inj2)| {
                    let v1c = v1_eps.clone();
                    let f: InjStep = Rc::new(move |v2: ParseTree| {
                        ParseTree::Pair(Box::new(v1c.clone()), Box::new(inj2(v2)))
                    });
                    (r2_, f)
                }));

                nub_tree(out)
            } else {
                pderiv_tree(r1, x)
                    .into_iter()
                    .map(|(r1_, inj1)| {
                        let r2c = (**r2).clone();
                        // smart_seq, not raw Seq: without it, residuals can
                        // carry a spurious `Eps . r` wrapper that defeats
                        // nub_tree's structural-equality dedup.
                        //
                        // smart_seq(Eps, r2c) collapses to r2c alone, so
                        // the downstream injected value is r2's own value
                        // directly, not wrapped in a Pair -- the closure
                        // must match that shape, injecting r1 with the
                        // canonical empty value instead of destructuring
                        // one out of a Pair that no longer exists.
                        if r1_ == Regex::Eps {
                            let f: InjStep = Rc::new(move |v2: ParseTree| {
                                ParseTree::Pair(Box::new(inj1(ParseTree::Empty)), Box::new(v2))
                            });
                            (r2c, f)
                        } else {
                            let f: InjStep = Rc::new(move |v: ParseTree| match v {
                                ParseTree::Pair(v1, v2) => ParseTree::Pair(Box::new(inj1(*v1)), v2),
                                other => panic!(
                                    "pderiv_tree: Seq(non-nullable) strand expected Pair, got {:?}",
                                    other
                                ),
                            });
                            (smart_seq(r1_, &r2c), f)
                        }
                    })
                    .collect()
            }
        }

        Regex::Star(r1) => {
            let out: Vec<(Regex, InjStep)> = pderiv_tree(r1, x)
                .into_iter()
                .map(|(r1_, inj1)| {
                    let r1c = (**r1).clone();
                    // smart_seq here too, matching the reference's
                    // `smartC r' (Star r)`. Same collapse-aware closure
                    // split as the Seq case above: when r1_ is Eps,
                    // smart_seq drops the Seq wrapper entirely, so the
                    // downstream value is a bare Star(items), not
                    // Pair(v1, Star(items)).
                    // `items.push` here, not `insert(0, ..)`: injection
                    // unwinds in reverse temporal order (Inj::apply runs
                    // the most-recently-added step first), so building
                    // left-to-right needs each new iteration prepended.
                    // `insert(0, ..)` is O(current length) per call, so
                    // reconstructing an n-iteration Star costs O(n^2)
                    // total. `push` is O(1) amortized; `select` reverses
                    // every Star's items exactly once at the very end
                    // instead (see `reverse_stars`), O(n) total.
                    if r1_ == Regex::Eps {
                        let f: InjStep = Rc::new(move |vs: ParseTree| match vs {
                            ParseTree::Star(mut items) => {
                                items.push(inj1(ParseTree::Empty));
                                ParseTree::Star(items)
                            }
                            other => panic!(
                                "pderiv_tree: Star strand expected Star tail, got {:?}",
                                other
                            ),
                        });
                        (Regex::star(r1c), f)
                    } else {
                        let f: InjStep = Rc::new(move |v: ParseTree| match v {
                            ParseTree::Pair(v1, vs) => match *vs {
                                ParseTree::Star(mut items) => {
                                    items.push(inj1(*v1));
                                    ParseTree::Star(items)
                                }
                                other => panic!(
                                    "pderiv_tree: Star strand expected Star tail, got {:?}",
                                    other
                                ),
                            },
                            other => panic!(
                                "pderiv_tree: Star strand expected Pair, got {:?}",
                                other
                            ),
                        });
                        (smart_seq(r1_, &Regex::star(r1c)), f)
                    }
                })
                .collect();
            nub_tree(out)
        }
    }
}

/// `pderiv_tree`'s residual `Regex`es alone, discarding the injection
/// closures: for measurement code that only needs the frontier's shape
/// (size, strand count), not parse-tree reconstruction. `pderiv_bc`'s
/// one-step primitive is `pub` for the same reason; `pderiv_tree` itself
/// stays `pub(crate)` since `InjStep` is not exported.
pub fn pderiv_tree_residuals(r: &Regex, x: char) -> Vec<Regex> {
    pderiv_tree(r, x).into_iter().map(|(r, _)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // (eps+a)b | ab, derived by 'a': the nullable Seq branch's first strand
    // used to build a raw Seq(Eps, Lit('b')) instead of collapsing it via
    // smart_seq, so this and the second alternative's own Lit('b') survived
    // nub_tree as two syntactically distinct residuals for the same state.
    #[test]
    fn seq_nullable_branch_dedups_against_sibling_alternative() {
        let left = Regex::seq(Regex::alt(Regex::Eps, Regex::lit('a')), Regex::lit('b'));
        let right = Regex::seq(Regex::lit('a'), Regex::lit('b'));
        let r = Regex::alt(left, right);
        let residuals = pderiv_tree_residuals(&r, 'a');
        assert_eq!(residuals, vec![Regex::lit('b')]);
    }

    // Same hand-traced case as pderiv_bc's own
    // nested_alt_three_leaves_prefix_matches_hand_trace_and_dedups_leftmost,
    // checked here on the actual parse tree the surviving closure
    // produces rather than on a bit prefix. This is the case that
    // caught the composition-order bug (`w(Left(v))` vs `Left(w(v))`)
    // before it ever reached the real implementation: getting the
    // order backwards doesn't panic, it silently builds a tree shaped
    // like Left(Right(Char('a'))) instead of Left(Char('a')), so this
    // has to be checked against a known tree, not just "does it run".
    #[test]
    fn nested_alt_three_leaves_dedups_leftmost_and_wraps_correctly() {
        // a + (a + b) on 'a': the left 'a' must win the tie over the
        // nested 'a', and its closure must wrap in exactly one Left
        // (not Right(Left(..)) or any other order), since it's reached
        // by taking the outer Alt's own left branch, one level deep.
        let r = Regex::alt(Regex::lit('a'), Regex::alt(Regex::lit('a'), Regex::lit('b')));
        let d = pderiv_tree(&r, 'a');
        assert_eq!(d.len(), 1, "the nested 'a' must be deduped away, leaving only the left one");
        let (residual, inj) = &d[0];
        assert_eq!(*residual, Regex::Eps);
        assert_eq!(inj(ParseTree::Empty), ParseTree::Left(Box::new(ParseTree::Char('a'))));
    }

    // Four levels deep, no duplicates, mirroring pderiv_bc's own
    // nested_alt_four_leaves_each_prefix_matches_hand_trace: leaf i's
    // wrap is i Rights then one Left, except the last leaf, which is
    // all Rights with no trailing Left.
    #[test]
    fn nested_alt_four_leaves_each_wraps_correctly() {
        let r = Regex::alt(
            Regex::lit('a'),
            Regex::alt(Regex::lit('b'), Regex::alt(Regex::lit('c'), Regex::lit('d'))),
        );
        let check = |c: char, expected: ParseTree| {
            let d = pderiv_tree(&r, c);
            assert_eq!(d.len(), 1);
            assert_eq!((d[0].1)(ParseTree::Empty), expected);
        };
        check('a', ParseTree::Left(Box::new(ParseTree::Char('a'))));
        check('b', ParseTree::Right(Box::new(ParseTree::Left(Box::new(ParseTree::Char('b'))))));
        check(
            'c',
            ParseTree::Right(Box::new(ParseTree::Right(Box::new(ParseTree::Left(Box::new(
                ParseTree::Char('c'),
            )))))),
        );
        check(
            'd',
            ParseTree::Right(Box::new(ParseTree::Right(Box::new(ParseTree::Right(Box::new(
                ParseTree::Char('d'),
            )))))),
        );
    }
}
