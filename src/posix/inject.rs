//! inj_{r\l}: Injects a letter back into a parse tree of the derivative

use crate::basic::{Regex, nullable};
use super::parse_tree::ParseTree;
use super::mk_eps::mk_eps;
use crate::debug_println;
use crate::posix::debug::{indent_inc, indent_dec};


pub fn inject(r: &Regex, l: char, v: ParseTree) -> ParseTree {
    debug_println!("inject({:?}, '{}', {:?})", r, l, v);
    indent_inc();
    
    let result = match r {
        Regex::Lit(c) => {
            assert!(*c == l);
            assert!(matches!(v, ParseTree::Empty));
            debug_println!("-> Char('{}')", l);
            ParseTree::Char(l)
        }
        
        Regex::Star(r1) => {
            debug_println!("Star case: need to inject into first iteration");
            match v {
                ParseTree::Pair(v1, vs) => {
                    indent_inc();
                    let v1_inj = inject(r1, l, *v1);
                    let mut iterations = vec![v1_inj];
                    if let ParseTree::Star(rest) = *vs {
                        iterations.extend(rest);
                    }
                    indent_dec();
                    debug_println!("-> Star({:?})", iterations);
                    ParseTree::Star(iterations)
                }
                _ => panic!("inject on Star: expected Pair, got {:?}", v),
            }
        }
        
        Regex::Seq(r1, r2) => {
            let r1_nullable = nullable(r1);
            debug_println!("Seq case: r1 nullable? {}", r1_nullable);
            
            if !r1_nullable {
                match v {
                    ParseTree::Pair(v1, v2) => {
                        indent_inc();
                        let v1_inj = inject(r1, l, *v1);
                        indent_dec();
                        debug_println!("-> Pair({:?}, {:?})", v1_inj, v2);
                        ParseTree::Pair(Box::new(v1_inj), v2)
                    }
                    _ => panic!("inject on Seq (non-nullable r1): expected Pair, got {:?}", v),
                }
            } else {
                match v {
                    ParseTree::Left(v_pair) => {
                        debug_println!("Left branch: letter from r1");
                        match *v_pair {
                            ParseTree::Pair(v1, v2) => {
                                indent_inc();
                                let v1_inj = inject(r1, l, *v1);
                                indent_dec();
                                debug_println!("-> Pair({:?}, {:?})", v1_inj, v2);
                                ParseTree::Pair(Box::new(v1_inj), v2)
                            }
                            _ => panic!("inject on Seq (nullable r1, Left): expected Pair, got {:?}", v_pair),
                        }
                    }
                    ParseTree::Right(v2) => {
                        debug_println!("Right branch: letter from r2");
                        indent_inc();
                        let v2_inj = inject(r2, l, *v2);
                        let v1_eps = mk_eps(r1);
                        indent_dec();
                        debug_println!("-> Pair({:?}, {:?})", v1_eps, v2_inj);
                        ParseTree::Pair(Box::new(v1_eps), Box::new(v2_inj))
                    }
                    _ => panic!("inject on Seq (nullable r1): expected Left or Right, got {:?}", v),
                }
            }
        }
        
        Regex::Alt(r1, r2) => {
            debug_println!("Alt case");
            match v {
                ParseTree::Left(v1) => {
                    indent_inc();
                    let inj = inject(r1, l, *v1);
                    indent_dec();
                    debug_println!("-> Left({:?})", inj);
                    ParseTree::Left(Box::new(inj))
                }
                ParseTree::Right(v2) => {
                    indent_inc();
                    let inj = inject(r2, l, *v2);
                    indent_dec();
                    debug_println!("-> Right({:?})", inj);
                    ParseTree::Right(Box::new(inj))
                }
                _ => panic!("inject on Alt: expected Left or Right, got {:?}", v),
            }
        }
        
        Regex::Eps => panic!("inject called on Eps"),
        Regex::Phi => panic!("inject called on Phi"),
    };
    
    indent_dec();
    debug_println!("<- result: {:?}", result);
    result
}