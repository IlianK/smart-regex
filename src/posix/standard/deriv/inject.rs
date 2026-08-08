//! regex-engine/src/posix/standard/inject.rs
//! 
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


// -------------------------------
// Tests for inject
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Regex, ParseTree};
 
    #[test]
    fn inject_lit_produces_char() {
        let t = inject(&Regex::lit('a'), 'a', ParseTree::Empty);
        assert_eq!(t, ParseTree::Char('a'));
    }
    #[test]
    fn inject_alt_left_case() {
        let r = Regex::alt(Regex::lit('a'), Regex::lit('b'));
        let t = inject(&r, 'a', ParseTree::Left(Box::new(ParseTree::Empty)));
        assert_eq!(t, ParseTree::Left(Box::new(ParseTree::Char('a'))));
    }
    #[test]
    fn inject_alt_right_case() {
        let r = Regex::alt(Regex::lit('a'), Regex::lit('b'));
        let t = inject(&r, 'b', ParseTree::Right(Box::new(ParseTree::Empty)));
        assert_eq!(t, ParseTree::Right(Box::new(ParseTree::Char('b'))));
    }
    #[test]
    fn inject_star_one_step() {
        let r = Regex::star(Regex::lit('a'));
        let v = ParseTree::Pair(Box::new(ParseTree::Empty), Box::new(ParseTree::Star(vec![])));
        let t = inject(&r, 'a', v);
        assert_eq!(t, ParseTree::Star(vec![ParseTree::Char('a')]));
    }
    #[test]
    #[should_panic(expected = "inject called on Eps")]
    fn inject_eps_panics() { inject(&Regex::Eps, 'a', ParseTree::Empty); }
    #[test]
    #[should_panic(expected = "inject called on Phi")]
    fn inject_phi_panics() { inject(&Regex::Phi, 'a', ParseTree::Empty); }
}
