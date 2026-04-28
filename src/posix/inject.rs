//! inj_{r\l}: Injects a letter back into a parse tree of the derivative
//!
//! Based on Lemma 2 "inj preserves POSIX parse trees"

use crate::basic::{Regex, nullable};
use super::parse_tree::ParseTree;
use super::mk_eps::mk_eps;

/// Injects letter back into parse tree of derivative
/// Takes parse tree v of r\l and returns parse tree of r
pub fn inject(r: &Regex, l: char, v: ParseTree) -> ParseTree {
    match r {
        // Case: literal l
        // r = l, then r\l = eps, v must be Empty
        // Example: inj("a", 'a', ()) = 'a'
        Regex::Lit(c) => {
            assert!(*c == l, "Literal mismatch in inject: expected '{}', got '{}'", c, l);
            assert!(matches!(v, ParseTree::Empty), "inject on Lit: expected Empty, got {:?}", v);
            ParseTree::Char(l)
        }
        
        // Case: Kleene star r1*
        // (r1*)\l = (r1\l).r1*
        
        // v1 -> parse tree of  r1\l
        // v2 -> parse tree of  r1*

        // Forward: r0 = a*, r1 = deriv(a*, 'a') = ε·a*
        // Backward: v1 = mk_eps(ε·a*) = ?
        // Injection: inj(a*, 'a', v1) = [a]
        Regex::Star(r1) => {
            match v {
                // Normal case: we have a pair (v1, vs) where v1 is parse tree of r1\l
                ParseTree::Pair(v1, vs) => {
                    let v1_inj = inject(r1, l, *v1);
                    
                    let mut iterations = vec![v1_inj];
                    if let ParseTree::Star(mut rest) = *vs {
                        iterations.append(&mut rest);
                    }
                    ParseTree::Star(iterations)
                }
                // For empty star (no iterations), this shouldn't happen because
                // inject is only called when we consumed a letter
                _ => panic!("inject on Star: expected Pair, got {:?}", v),
            }
        }
        
        // Case: Concatenation r1.r2
        Regex::Seq(r1, r2) => {
            if !nullable(r1) {
                // Subcase 1: r1 is not nullable
                // Then (r1.r2)\l = (r1\l).r2
                // The parse tree v is a parse tree of (r1\l).r2, which is a Pair
                match v {
                    ParseTree::Pair(v1, v2) => {
                        let v1_inj = inject(r1, l, *v1);
                        ParseTree::Pair(Box::new(v1_inj), v2)
                    }
                    other => panic!("inject on Seq (non-nullable r1): expected Pair, got {:?}", other),
                }
                
            } else {
                // Subcase 2 & 3: r1 is nullable
                // (r1.r2)\l = (r1\l).r2 + r2\l
                // v can be either Left(v1, v2) or Right(v2)
                match v {
                    // Subcase 2: Left branch - letter came from r1
                    ParseTree::Left(v_pair) => {
                        match *v_pair {
                            ParseTree::Pair(v1, v2) => {
                                let v1_inj = inject(r1, l, *v1);
                                ParseTree::Pair(Box::new(v1_inj), v2)
                            }
                            _ => panic!("inject on Seq (nullable r1, Left): expected Pair, got {:?}", v_pair),
                        }
                    }
                    
                    // Subcase 3: Right branch - letter came from r2
                    ParseTree::Right(v2) => {
                        let v2_inj = inject(r2, l, *v2);
                        let v1_eps = mk_eps(r1);
                        ParseTree::Pair(Box::new(v1_eps), Box::new(v2_inj))
                    }
                    
                    other => panic!("inject on Seq (nullable r1): expected Left or Right, got {:?}", other),
                }
            }
        }
        
        // Case: Alternative r1 + r2
        Regex::Alt(r1, r2) => {
            match v {
                ParseTree::Left(v1) => {
                    ParseTree::Left(Box::new(inject(r1, l, *v1)))
                }
                ParseTree::Right(v2) => {
                    ParseTree::Right(Box::new(inject(r2, l, *v2)))
                }
                other => panic!("inject on Alt: expected Left or Right, got {:?}", other),
            }
        }
        
        Regex::Eps => panic!("inject called on Eps (should not happen)"),
        Regex::Phi => panic!("inject called on Phi (should not happen)"),
    }
}