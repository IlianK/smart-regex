//! src/parsers/deriv_bc/parse.rs
//! 
//! Bit-coded POSIX parser (deriv_bc): 
//! recursive and loop variants; traced counterpart in traced.rs.

use std::time::Instant;
use crate::types::{Regex, ParseTree, ARegex};
use crate::parsers::deriv_bc::nullable::nullable_bc;
use crate::parsers::deriv_bc::deriv::deriv_bc;
use crate::parsers::deriv_bc::simplify::simp;
use crate::parsers::BcTiming;
use super::internalize::internalize;
use super::mk_eps_bc::mk_eps_bc;
use crate::regex::decode::decode;


// RECURSIVE BITCODED PARSER
//
// ("Recursive" is this parser's long-standing name in ALL/the CLI/the
// benches, kept for compatibility -- the per-character stepping below
// is a loop, not recursion over position. An earlier version called
// itself once per input character; that put one stack frame per
// character on the call stack, a latent stack-overflow risk that the
// O(n^2) bits-materialization bug above it masked by failing on
// memory/time first, long before reaching the millions-of-characters
// depth where this would actually bite -- confirmed empirically: fixing
// that bug let deriv_bc reach ~1M characters, where recursing hit a real
// stack overflow at 5M that a plain loop does not.)
fn derive_and_simplify(ri: ARegex, chars: &[char]) -> ARegex {
    let mut cur = ri;
    for &l in chars {
        // simp absorbs the recorded iteration bit into the Star prefix
        cur = simp(deriv_bc(cur, l));
    }
    cur
}

pub fn parse_bitcoded_recursive(input: &str, r: &Regex) -> Option<ParseTree> {
    parse_bitcoded_recursive_timed(input, r).0
}

/// Same parse, plus how long encode (Regex -> ARegex), derive (the
/// per-character stepping), and decode (bits -> ParseTree) each took.
pub fn parse_bitcoded_recursive_timed(input: &str, r: &Regex) -> (Option<ParseTree>, BcTiming) {
    let t0 = Instant::now();
    let ri = internalize(r);
    let encode_us = t0.elapsed().as_micros() as u64;

    let t1 = Instant::now();
    let chars: Vec<char> = input.chars().collect();
    let final_ri = derive_and_simplify(ri, &chars);
    let derive_us = t1.elapsed().as_micros() as u64;

    if !nullable_bc(&final_ri) {
        return (None, BcTiming { encode_us, derive_us, decode_us: 0 });
    }

    let t2 = Instant::now();
    let bits = mk_eps_bc(&final_ri);
    let tree = decode(r, &bits);
    let decode_us = t2.elapsed().as_micros() as u64;

    (Some(tree), BcTiming { encode_us, derive_us, decode_us })
}


// LOOP BITCODED PARSER

pub fn parse_bitcoded_loop(input: &str, r: &Regex) -> Option<ParseTree> {
    let ri = internalize(r);
    let chars: Vec<char> = input.chars().collect();
    let final_ri = derive_and_simplify(ri, &chars);
    if !nullable_bc(&final_ri) {
        return None;
    }
    let bits = mk_eps_bc(&final_ri);
    Some(decode(r, &bits))
}


// DEFAULT

pub fn parse_deriv_bc(input: &str, r: &Regex) -> Option<ParseTree> {
    parse_bitcoded_recursive(input, r)
}


// Unit tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_matches_untimed_on_success() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_bitcoded_recursive("aaa", &r);
        let (timed, _) = parse_bitcoded_recursive_timed("aaa", &r);
        assert_eq!(plain, timed);
    }

    #[test]
    fn timed_matches_untimed_on_failure() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_bitcoded_recursive("aab", &r);
        let (timed, timing) = parse_bitcoded_recursive_timed("aab", &r);
        assert_eq!(plain, timed);
        assert_eq!(timing.decode_us, 0, "no decode happens on a non-match");
    }

    #[test]
    fn timed_reports_zero_decode_on_phi() {
        let (tree, timing) = parse_bitcoded_recursive_timed("a", &Regex::Phi);
        assert_eq!(tree, None);
        assert_eq!(timing.decode_us, 0);
    }
}
