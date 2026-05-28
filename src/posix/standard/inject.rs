//! inj_{r\l}: Injects a letter back into a parse tree of the derivative

use crate::types::{Regex, ParseTree};
use crate::regex::nullable::standard::nullable;
use crate::posix::standard::mk_eps;

pub fn inject(r: &Regex, l: char, v: ParseTree) -> ParseTree {
    match r {
        Regex::Lit(c) => {
            assert!(*c == l);
            assert!(matches!(v, ParseTree::Empty));
            ParseTree::Char(l)
        }
        
        Regex::Star(r1) => {
            match v {
                ParseTree::Pair(v1, vs) => {
                    let v1_inj = inject(r1, l, *v1);
                    let mut iterations = vec![v1_inj];
                    if let ParseTree::Star(rest) = *vs {
                        iterations.extend(rest);
                    }
                    ParseTree::Star(iterations)
                }
                _ => panic!("inject on Star: expected Pair, got {:?}", v),
            }
        }
        
        Regex::Seq(r1, r2) => {
            let r1_nullable = nullable(r1);
            
            if !r1_nullable {
                match v {
                    ParseTree::Pair(v1, v2) => {
                        let v1_inj = inject(r1, l, *v1);
                        ParseTree::Pair(Box::new(v1_inj), v2)
                    }
                    _ => panic!("inject on Seq (non-nullable r1): expected Pair, got {:?}", v),
                }
            } else {
                match v {
                    ParseTree::Left(v_pair) => {
                        match *v_pair {
                            ParseTree::Pair(v1, v2) => {
                                let v1_inj = inject(r1, l, *v1);
                                ParseTree::Pair(Box::new(v1_inj), v2)
                            }
                            _ => panic!("inject on Seq (nullable r1, Left): expected Pair, got {:?}", v_pair),
                        }
                    }
                    ParseTree::Right(v2) => {
                        let v2_inj = inject(r2, l, *v2);
                        let v1_eps = mk_eps(r1);
                        ParseTree::Pair(Box::new(v1_eps), Box::new(v2_inj))
                    }
                    _ => panic!("inject on Seq (nullable r1): expected Left or Right, got {:?}", v),
                }
            }
        }
        
        Regex::Alt(r1, r2) => {
            match v {
                ParseTree::Left(v1) => {
                    ParseTree::Left(Box::new(inject(r1, l, *v1)))
                }
                ParseTree::Right(v2) => {
                    ParseTree::Right(Box::new(inject(r2, l, *v2)))
                }
                _ => panic!("inject on Alt: expected Left or Right, got {:?}", v),
            }
        }
        
        Regex::Eps => panic!("inject called on Eps"),
        Regex::Phi => panic!("inject called on Phi"),
    }
}