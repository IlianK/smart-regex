//! Plain-Regex POSIX parsers (deriv_std): recursive and loop variants; traced counterparts in traced.rs.

use crate::types::{Regex, ParseTree};
use crate::regex::deriv::deriv;
use crate::regex::nullable::nullable;
use crate::regex::mk_eps::mk_eps;
use crate::parsers::deriv_std::inject::inject;


// RECURSIVE PARSER

fn parse_recursive_helper(r: &Regex, input: &str) -> Option<ParseTree> {
    let mut chars = input.chars();
    match chars.next() {
        None => {
            if nullable(r) { Some(mk_eps(r)) } else { None }
        }
        Some(l) => {
            let rest: String = chars.collect();
            let r_deriv = deriv(r, l);
            let subtree = parse_recursive_helper(&r_deriv, &rest)?;
            Some(inject(r, l, subtree))
        }
    }
}

pub fn parse_deriv_std_rec(input: &str, r: &Regex) -> Option<ParseTree> {
    parse_recursive_helper(r, input)
}


// LOOP PARSER

pub fn parse_deriv_std_loop(input: &str, r: &Regex) -> Option<ParseTree> {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();

    let mut expressions = Vec::with_capacity(n + 1);
    expressions.push(r.clone());

    for &c in chars.iter() {
        let current = expressions.last().unwrap();
        let next = deriv(current, c);
        expressions.push(next);
    }

    let final_r = expressions.last().unwrap();
    if !nullable(final_r) {
        return None;
    }

    let mut tree = mk_eps(expressions.last().unwrap());
    for i in (0..n).rev() {
        tree = inject(&expressions[i], chars[i], tree);
    }

    Some(tree)
}
