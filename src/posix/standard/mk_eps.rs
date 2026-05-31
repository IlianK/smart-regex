//! regex-engine/src/posix/standard/mk_eps.rs
//! 
//! mkEps_r: Constructs the POSIX parse tree for the empty word

use crate::types::{Regex, ParseTree};
use crate::regex::nullable::standard::nullable;

pub fn mk_eps(r: &Regex) -> ParseTree {
    match r {
        Regex::Eps => ParseTree::Empty,
        
        Regex::Star(_) => ParseTree::Star(Vec::new()),
        
        Regex::Seq(r1, r2) => {
            let v1 = mk_eps(r1);
            let v2 = mk_eps(r2);
            ParseTree::Pair(Box::new(v1), Box::new(v2))
        }
        
        Regex::Alt(r1, r2) => {
            if nullable(r1) {
                ParseTree::Left(Box::new(mk_eps(r1)))
            } else {
                ParseTree::Right(Box::new(mk_eps(r2)))
            }
        }
        
        Regex::Lit(c) => panic!("mk_eps called on Lit('{}')", c),
        Regex::Phi => panic!("mk_eps called on Phi"),
    }
}


// ============================================================================
// Tests for mk_eps
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Regex, ParseTree};
 
    #[test]
    fn eps_gives_empty() {
        assert_eq!(mk_eps(&Regex::Eps), ParseTree::Empty);
    }
    #[test]
    fn star_gives_empty_list() {
        let t = mk_eps(&Regex::star(Regex::lit('a')));
        assert!(matches!(t, ParseTree::Star(ref v) if v.is_empty()));
    }
    #[test]
    fn alt_left_nullable_gives_left() {
        let r = Regex::alt(Regex::Eps, Regex::lit('a'));
        assert!(matches!(mk_eps(&r), ParseTree::Left(_)));
    }
    #[test]
    fn alt_right_nullable_gives_right() {
        let r = Regex::alt(Regex::lit('a'), Regex::Eps);
        assert!(matches!(mk_eps(&r), ParseTree::Right(_)));
    }
    #[test]
    fn seq_gives_pair() {
        let r = Regex::seq(Regex::Eps, Regex::Eps);
        assert!(matches!(mk_eps(&r), ParseTree::Pair(_, _)));
    }
    #[test]
    #[should_panic(expected = "mk_eps called on Lit")]
    fn lit_panics() { mk_eps(&Regex::lit('a')); }
    #[test]
    #[should_panic(expected = "mk_eps called on Phi")]
    fn phi_panics() { mk_eps(&Regex::Phi); }
}
