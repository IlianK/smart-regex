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
    let final_ri = parse_bitcoded_recursive_helper(ri, &chars, 0);
    let derive_us = t1.elapsed().as_micros() as u64;

    let Some(final_ri) = final_ri else {
        return (None, BcTiming { encode_us, derive_us, decode_us: 0 });
    };
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
