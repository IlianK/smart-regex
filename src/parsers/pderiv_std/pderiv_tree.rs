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

        Regex::Alt(r1, r2) => {
            let mut out: Vec<(Regex, InjStep)> = pderiv_tree(r1, x)
                .into_iter()
                .map(|(r1_, inj1)| {
                    let f: InjStep = Rc::new(move |v| ParseTree::Left(Box::new(inj1(v))));
                    (r1_, f)
                })
                .collect();
            out.extend(pderiv_tree(r2, x).into_iter().map(|(r2_, inj2)| {
                let f: InjStep = Rc::new(move |v| ParseTree::Right(Box::new(inj2(v))));
                (r2_, f)
            }));
            nub_tree(out)
        }

        Regex::Seq(r1, r2) => {
            if nullable(r1) {
                let mut out: Vec<(Regex, InjStep)> = pderiv_tree(r1, x)
                    .into_iter()
                    .map(|(r1_, inj1)| {
                        let r2c = (**r2).clone();
                        let f: InjStep = Rc::new(move |v: ParseTree| match v {
                            ParseTree::Pair(v1, v2) => ParseTree::Pair(Box::new(inj1(*v1)), v2),
                            other => panic!(
                                "pderiv_tree: Seq-continue strand expected Pair, got {:?}",
                                other
                            ),
                        });
                        (Regex::seq(r1_, r2c), f)
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
                        // smart_seq, not raw Seq, matching the reference's
                        // `smartC r' s` here (the nullable branch above
                        // deliberately keeps raw Seq, matching its own
                        // `Seq r' s` -- the asymmetry is the reference's,
                        // not a bug). Without it, residuals can carry a
                        // spurious `Eps . r` wrapper that defeats
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
                            (Regex::seq(r1_, r2c), f)
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
