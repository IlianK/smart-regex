//! src/parsers/deriv_bc/parse.rs
//! 
//! Bit-coded POSIX parser (deriv_bc): 
//! recursive and loop variants; traced counterpart in traced.rs.

use crate::types::{Regex, ParseTree, ARegex};
use crate::parsers::deriv_bc::nullable::nullable_bc;
use crate::parsers::deriv_bc::deriv::deriv_bc;
use crate::parsers::deriv_bc::simplify::simp;
use super::internalize::internalize;
use super::mk_eps_bc::mk_eps_bc;
use crate::regex::decode::decode;


// RECURSIVE BITCODED PARSER

/// Indexes into `chars` instead of collecting the remaining input into a
/// fresh `String` on every call: the old version allocated and copied an
/// O(n), O(n-1), O(n-2)... suffix at each step -- O(n^2) total, and since
/// this recursion isn't tail-call-eliminated, every one of those Strings
/// stays alive simultaneously down the call stack, O(n^2) bytes at once
/// (confirmed empirically: ~4.7GB peak at 100K characters).
fn parse_bitcoded_recursive_helper(ri: ARegex, chars: &[char], pos: usize) -> Option<ARegex> {
    match chars.get(pos) {
        None => Some(ri),
        Some(&l) => {
            let ri_deriv = deriv_bc(ri, l);
            // simp absorbs the recorded iteration bit into the Star prefix
            let ri_simp = simp(ri_deriv);
            parse_bitcoded_recursive_helper(ri_simp, chars, pos + 1)
        }
    }
}

pub fn parse_bitcoded_recursive(input: &str, r: &Regex) -> Option<ParseTree> {
    let ri = internalize(r);
    let chars: Vec<char> = input.chars().collect();
    let final_ri = parse_bitcoded_recursive_helper(ri, &chars, 0)?;
    if !nullable_bc(&final_ri) {
        return None;
    }
    let bits = mk_eps_bc(&final_ri);
    Some(decode(r, &bits))
}


// LOOP BITCODED PARSER

pub fn parse_bitcoded_loop(input: &str, r: &Regex) -> Option<ParseTree> {
    let mut ri = internalize(r);
    for l in input.chars() {
        ri = simp(deriv_bc(ri, l));
    }
    if !nullable_bc(&ri) {
        return None;
    }
    let bits = mk_eps_bc(&ri);
    Some(decode(r, &bits))
}


// DEFAULT

pub fn parse_deriv_bc(input: &str, r: &Regex) -> Option<ParseTree> {
    parse_bitcoded_recursive(input, r)
}
