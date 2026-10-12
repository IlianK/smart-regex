//! src/parsers/pderiv_std/pderiv_tree.rs
//! 
//! Standard Partial-Derivative-Based Parser
//! Antimirov Partial Derivative 
//! 
//! One step of the tree-based partial derivative: 
//! `pderiv_bc`'s analogue, `(residual, Inj)` pairs instead of bits.

use std::rc::Rc;
use crate::types::{ParseTree, Regex};
use crate::regex::nullable::nullable;
use crate::regex::mk_eps::mk_eps;
use super::inject::{Inj, nub_tree};

pub(crate) fn pderiv_tree(r: &Regex, x: char) -> Vec<(Regex, Inj)> {
    match r {
        Regex::Eps | Regex::Phi => vec![],

        Regex::Lit(c) => {
            if *c == x {
                vec![(Regex::Eps, Rc::new(move |_: ParseTree| ParseTree::Char(x)) as Inj)]
            } else {
                vec![]
            }
        }

        Regex::Alt(r1, r2) => {
            let mut out: Vec<(Regex, Inj)> = pderiv_tree(r1, x)
                .into_iter()
                .map(|(r1_, inj1)| {
                    let f: Inj = Rc::new(move |v| ParseTree::Left(Box::new(inj1(v))));
                    (r1_, f)
                })
                .collect();
            out.extend(pderiv_tree(r2, x).into_iter().map(|(r2_, inj2)| {
                let f: Inj = Rc::new(move |v| ParseTree::Right(Box::new(inj2(v))));
                (r2_, f)
            }));
            nub_tree(out)
        }

        Regex::Seq(r1, r2) => {
            if nullable(r1) {
                let mut out: Vec<(Regex, Inj)> = pderiv_tree(r1, x)
                    .into_iter()
                    .map(|(r1_, inj1)| {
                        let r2c = (**r2).clone();
                        let f: Inj = Rc::new(move |v: ParseTree| match v {
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
                    let f: Inj = Rc::new(move |v2: ParseTree| {
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
                        let f: Inj = Rc::new(move |v: ParseTree| match v {
                            ParseTree::Pair(v1, v2) => ParseTree::Pair(Box::new(inj1(*v1)), v2),
                            other => panic!(
                                "pderiv_tree: Seq(non-nullable) strand expected Pair, got {:?}",
                                other
                            ),
                        });
                        (Regex::seq(r1_, r2c), f)
                    })
                    .collect()
            }
        }

        Regex::Star(r1) => {
            let out: Vec<(Regex, Inj)> = pderiv_tree(r1, x)
                .into_iter()
                .map(|(r1_, inj1)| {
                    let r1c = (**r1).clone();
                    let f: Inj = Rc::new(move |v: ParseTree| match v {
                        ParseTree::Pair(v1, vs) => match *vs {
                            ParseTree::Star(mut items) => {
                                items.insert(0, inj1(*v1));
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
                    (Regex::seq(r1_, Regex::star(r1c)), f)
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
/// stays `pub(crate)` since `Inj` is not exported.
pub fn pderiv_tree_residuals(r: &Regex, x: char) -> Vec<Regex> {
    pderiv_tree(r, x).into_iter().map(|(r, _)| r).collect()
}
