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