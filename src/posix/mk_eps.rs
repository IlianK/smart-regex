//! mkEps_r: Constructs the POSIX parse tree for the empty word
//!
//! Based on Lemma 1: "mkEps_r is the POSIX parse tree of r for the empty word"

use crate::basic::{Regex, nullable};
use super::parse_tree::ParseTree;

/// Constructs parse tree for empty word
/// Assumes r is nullable (epsilon ∈ L(r))
pub fn mk_eps(r: &Regex) -> ParseTree {
    match r {
        // ε -> ()
        Regex::Eps => ParseTree::Empty,
        
        // r* -> [] (zero iterations)
        Regex::Star(_) => ParseTree::Star(Vec::new()),
        
        Regex::Seq(r1, r2) => {
            // Both must be nullable because Seq is nullable
            let v1 = mk_eps(r1);                // recursive call on r1
            let v2 = mk_eps(r2);                // recursive call on r2
            ParseTree::Pair(Box::new(v1), Box::new(v2))    // (v1, v2)
        }
        

        Regex::Alt(r1, r2) => {
            // Priority to left alternative if nullable (POSIX order longest leftmost)
            if nullable(r1) {
                ParseTree::Left(Box::new(mk_eps(r1)))
            } else {
                // r1 is not nullable, so r2 must be nullable (overall expr is nullable)
                ParseTree::Right(Box::new(mk_eps(r2)))
            }
        }

        // r1 = Seq(φ, Lit('b'))    -> nullable(r1) = false
        // r2 = Eps                 -> nullable(r2) = true
        // Take right branch        -> ParseTree::Right(Box::new(mk_eps(Eps)))
        // -> Eps
        
        Regex::Lit(c) => {
            panic!("mk_eps called on Lit('{}') which is not nullable", c)
        }
        
        Regex::Phi => {
            panic!("mk_eps called on Phi which is not nullable")
        }
    }
}