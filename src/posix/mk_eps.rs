//! mkEps_r: Constructs the POSIX parse tree for the empty word
//!
//! Based on Lemma 1: "mkEps_r is the POSIX parse tree of r for the empty word"

use crate::basic::{Regex, nullable};
use super::parse_tree::ParseTree;
use crate::debug_println;

/// Constructs parse tree for empty word
/// Assumes r is nullable (epsilon ∈ L(r))
pub fn mk_eps(r: &Regex) -> ParseTree {
    debug_println!("[DEBUG]   mkEps({:?})", r);
    
    let result = match r {
        // ε -> ()
        Regex::Eps => {
            debug_println!("[DEBUG]     -> Empty (ε)");
            ParseTree::Empty
        }
        
        // r* -> [] (zero iterations)
        Regex::Star(_) => {
            debug_println!("[DEBUG]     -> Star([]) (zero iterations)");
            ParseTree::Star(Vec::new())
        }
        
        Regex::Seq(r1, r2) => {
            debug_println!("[DEBUG]     -> Seq case, recursing...");
            // Both must be nullable because Seq is nullable
            let v1 = mk_eps(r1);                // recursive call on r1
            let v2 = mk_eps(r2);                // recursive call on r2
            debug_println!("[DEBUG]     -> Pair({:?}, {:?})", v1, v2);
            ParseTree::Pair(Box::new(v1), Box::new(v2))    // (v1, v2)
        }
        

        Regex::Alt(r1, r2) => {
            // Priority to left alternative if nullable (POSIX order longest leftmost)
            debug_println!("[DEBUG]     -> Alt case: nullable(r1) = {}", nullable(r1));
            if nullable(r1) {
                debug_println!("[DEBUG]     -> Taking LEFT branch");
                let v1 = mk_eps(r1);
                debug_println!("[DEBUG]     -> Left({:?})", v1);
                ParseTree::Left(Box::new(v1))
            } else {
                debug_println!("[DEBUG]     -> Taking RIGHT branch (left not nullable)");
                let v2 = mk_eps(r2);
                debug_println!("[DEBUG]     -> Right({:?})", v2);
                ParseTree::Right(Box::new(v2))
            }
        }

        // r1 = Seq(φ, Lit('b'))    -> nullable(r1) = false
        // r2 = Eps                 -> nullable(r2) = true
        // Take right branch        -> ParseTree::Right(Box::new(mk_eps(Eps)))
        // -> Eps
        
        Regex::Lit(c) => {
            debug_println!("[DEBUG]     -> PANIC: Lit('{}') not nullable!", c);
            panic!("mk_eps called on Lit('{}') which is not nullable", c)
        }
        
        Regex::Phi => {
            debug_println!("[DEBUG]     -> PANIC: Phi not nullable!");
            panic!("mk_eps called on Phi which is not nullable")
        }
    };
    
    debug_println!("[DEBUG]   <- mkEps result: {:?}", result);
    result
}