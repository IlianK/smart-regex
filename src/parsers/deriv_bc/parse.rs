//! src/parsers/deriv_bc/parse.rs
//! 
//! Bit-coded POSIX parser (deriv_bc): 
//! recursive and loop variants; traced counterpart in traced.rs.

use std::time::Instant;
use crate::types::{Regex, ParseTree, ARegex};
use crate::parsers::deriv_bc::nullable::nullable_bc;
use crate::parsers::deriv_bc::deriv::deriv_bc;
use crate::parsers::deriv_bc::simplify::simp;
use crate::parsers::PhaseTiming;
use super::internalize::internalize;
use super::mk_eps_bc::mk_eps_bc;
use crate::regex::decode::decode;


// RECURSIVE BITCODED PARSER
//
// Named "Recursive" for compatibility with ALL/the CLI/the benches; the
// per-character stepping below is actually a loop, not recursion over
// position, so it stays stack-safe on multi-million-character inputs.
fn derive_and_simplify(ri: ARegex, chars: &[char]) -> ARegex {
    let mut cur = ri;
    for &l in chars {
        // simp absorbs the recorded iteration bit into the Star prefix
        cur = simp(deriv_bc(&cur, l));
    }
    cur
}

pub fn parse_bitcoded_recursive(input: &str, r: &Regex) -> Option<ParseTree> {
    parse_bitcoded_recursive_timed(input, r).0
}

/// Same parse, plus `PhaseTiming`'s phases. For deriv_bc these are
/// genuinely encode (Regex -> ARegex via `internalize`), derive (the
/// per-character stepping), and decode (bits -> ParseTree) -- unlike
/// pderiv_bc/pderiv_std, whose `setup_us`/`reconstruct_us` measure a
/// plain clone and a closure application respectively, not an actual
/// encode/decode. `chars` and `final_ri` are dropped explicitly right
/// after `decode` is done with them, inside the same timed window
/// charged to `reconstruct_us`, so that cost isn't attributed to
/// whatever runs after this function returns.
pub fn parse_bitcoded_recursive_timed(input: &str, r: &Regex) -> (Option<ParseTree>, PhaseTiming) {
    let t0 = Instant::now();
    let ri = internalize(r);
    let setup_us = t0.elapsed().as_micros() as u64;

    let t1 = Instant::now();
    let chars: Vec<char> = input.chars().collect();
    let final_ri = derive_and_simplify(ri, &chars);
    let derive_us = t1.elapsed().as_micros() as u64;

    if !nullable_bc(&final_ri) {
        return (None, PhaseTiming { setup_us, derive_us, reconstruct_us: 0, teardown_us: 0 });
    }

    let t2 = Instant::now();
    let bits = mk_eps_bc(&final_ri);
    let tree = decode(r, &bits);
    drop(chars);
    drop(final_ri);
    let reconstruct_us = t2.elapsed().as_micros() as u64;

    (Some(tree), PhaseTiming { setup_us, derive_us, reconstruct_us, teardown_us: 0 })
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
        let (timed, timing) = parse_bitcoded_recursive_timed("aaa", &r);
        assert_eq!(plain, timed);
        // teardown_us is always 0 as directly returned from any of the
        // five parser functions -- only bench_worker fills it in.
        assert_eq!(timing.teardown_us, 0);
    }

    #[test]
    fn timed_matches_untimed_on_failure() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_bitcoded_recursive("aab", &r);
        let (timed, timing) = parse_bitcoded_recursive_timed("aab", &r);
        assert_eq!(plain, timed);
        assert_eq!(timing.reconstruct_us, 0, "no decode happens on a non-match");
        assert_eq!(timing.teardown_us, 0);
    }

    #[test]
    fn timed_reports_zero_decode_on_phi() {
        let (tree, timing) = parse_bitcoded_recursive_timed("a", &Regex::Phi);
        assert_eq!(tree, None);
        assert_eq!(timing.reconstruct_us, 0);
        assert_eq!(timing.teardown_us, 0);
    }

    // Exercises the new `drop(chars); drop(final_ri);` on a pattern
    // with Seq/Alt/Star all nested, including a Star iteration so the
    // Seq-nullable/Star bit-construction `simp`/`fuse` paths all run --
    // confirms dropping both early, right after `decode` is done
    // reading from `final_ri` (not `chars`, which `decode` never
    // touches at all), changes nothing about the decoded tree.
    #[test]
    fn timed_matches_untimed_on_nested_pattern() {
        let pair = Regex::seq(
            Regex::alt(Regex::lit('a'), Regex::lit('b')),
            Regex::alt(Regex::lit('a'), Regex::lit('b')),
        );
        let r = Regex::star(pair);
        let plain = parse_bitcoded_recursive("abba", &r);
        let (timed, timing) = parse_bitcoded_recursive_timed("abba", &r);
        assert_eq!(plain, timed);
        assert_eq!(timing.teardown_us, 0);
    }
}
