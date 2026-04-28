//! mkEps_r: Constructs the POSIX parse tree for the empty word

use crate::basic::{Regex, nullable};
use super::parse_tree::ParseTree;
use crate::debug_println;
use crate::posix::debug::{indent_inc, indent_dec};


pub fn mk_eps(r: &Regex) -> ParseTree {
    debug_println!("mkEps({:?})", r);
    indent_inc();
    
    let result = match r {
        Regex::Eps => {
            debug_println!("ε -> ()");
            ParseTree::Empty
        }
        
        Regex::Star(r1) => {
            debug_println!("{:?}* -> [] (zero iterations)", r1);
            ParseTree::Star(Vec::new())
        }
        
        Regex::Seq(r1, r2) => {
            debug_println!("Seq({:?}, {:?})", r1, r2);
            indent_inc();
            let v1 = mk_eps(r1);
            let v2 = mk_eps(r2);
            indent_dec();
            debug_println!("-> Pair({:?}, {:?})", v1, v2);
            ParseTree::Pair(Box::new(v1), Box::new(v2))
        }
        
        Regex::Alt(r1, r2) => {
            let left_nullable = nullable(r1);
            debug_println!("Alt({:?}, {:?}): left nullable? {}", r1, r2, left_nullable);
            indent_inc();
            if left_nullable {
                let v1 = mk_eps(r1);
                indent_dec();
                debug_println!("-> Left({:?})", v1);
                ParseTree::Left(Box::new(v1))
            } else {
                let v2 = mk_eps(r2);
                indent_dec();
                debug_println!("-> Right({:?})", v2);
                ParseTree::Right(Box::new(v2))
            }
        }
        
        Regex::Lit(c) => panic!("mk_eps called on Lit('{}')", c),
        Regex::Phi => panic!("mk_eps called on Phi"),
    };
    
    indent_dec();
    result
}