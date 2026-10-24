//! src/parsers/pderiv_bc/parse.rs
//! 
//! Bit-coded partial-derivative parser (pDerivBC / parsePDerivBC), GREEDY; traced variant in traced.rs.

use std::time::Instant;
use crate::types::{Regex, ParseTree, AppendBits};
use crate::regex::nullable::nullable;
use crate::parsers::pderiv_bc::pderiv::{pderiv_bc, mk_eps_bits, nub2};
use crate::parsers::BcTiming;
use crate::regex::decode::decode;

/// One step of `parsePDerivBC2`: replace the frontier with union of
/// `pderiv_bc(x, ·)`, then dedup across the whole combined frontier
/// (not just within one parent's own expansion). The reference slide's
/// `parsePDerivBC2` omits this, but its own `runBC_NFA`/`go` applies
/// `nub2` at exactly this point, matching Sulzmann & Lu 2012's Def. 8
/// ("duplicate states are removed, only the left-most state is kept")
/// and this project's own `demo_growth_pderiv` finding: without it, the
/// frontier doubles every character on `(a|b|ab)*` even though the
/// number of distinct residuals stays bounded.
///
/// Each strand's bits accumulate across the whole input so far; a plain
/// `Vec<bool>` clone-then-extend here is O(current length) per step,
/// O(n^2) over n characters -- `AppendBits::append` is O(extra.len())
/// regardless (see src/types/bits.rs).
pub(super) fn step_frontier(frontier: &[(Regex, AppendBits)], x: char) -> Vec<(Regex, AppendBits)> {
    let mut out = Vec::new();
    for (r, bits) in frontier {
        for (r_next, extra) in pderiv_bc(r, x) {
            out.push((r_next, bits.append(&extra)));
        }
    }
    nub2(out)
}

/// `filter (\(r,_) -> nullable r) rs`, take head, and append `mkEpsBC`.
pub(super) fn select_bits(frontier: &[(Regex, AppendBits)]) -> Option<Vec<bool>> {
    for (r, bits) in frontier {
        if nullable(r) {
            return Some(bits.append(&mk_eps_bits(r)).to_vec());
        }
    }
    None
}

pub fn parse_pderiv_bc(input: &str, r: &Regex) -> Option<ParseTree> {
    parse_pderiv_bc_timed(input, r).0
}

/// Same parse, plus how long derive (the per-character frontier
/// stepping) and decode (bits -> ParseTree) each took. pderiv_bc has no
/// encode step (it works on plain `Regex` directly, no ARegex
/// conversion), so `encode_us` is always 0.
pub fn parse_pderiv_bc_timed(input: &str, r: &Regex) -> (Option<ParseTree>, BcTiming) {
    let t0 = Instant::now();
    let mut frontier = vec![(r.clone(), AppendBits::empty())];
    for c in input.chars() {
        frontier = step_frontier(&frontier, c);
        if frontier.is_empty() {
            let derive_us = t0.elapsed().as_micros() as u64;
            return (None, BcTiming { encode_us: 0, derive_us, decode_us: 0 });
        }
    }
    let derive_us = t0.elapsed().as_micros() as u64;

    let t1 = Instant::now();
    let Some(bits) = select_bits(&frontier) else {
        return (None, BcTiming { encode_us: 0, derive_us, decode_us: 0 });
    };
    let tree = decode(r, &bits);
    let decode_us = t1.elapsed().as_micros() as u64;

    (Some(tree), BcTiming { encode_us: 0, derive_us, decode_us })
}


// Unit tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Regex, flatten};
    use crate::parsers::deriv_std::parse_deriv_std_rec;

    #[test]
    fn empty_regex_empty_input() {
        assert_eq!(parse_pderiv_bc("", &Regex::Eps), Some(ParseTree::Empty));
    }

    #[test]
    fn literal_matches() {
        let r = Regex::lit('a');
        assert_eq!(parse_pderiv_bc("a", &r), Some(ParseTree::Char('a')));
    }

    #[test]
    fn no_match_is_none() {
        assert_eq!(parse_pderiv_bc("b", &Regex::lit('a')), None);
    }

    #[test]
    fn timed_matches_untimed_and_has_no_encode_step() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_pderiv_bc("aaa", &r);
        let (timed, timing) = parse_pderiv_bc_timed("aaa", &r);
        assert_eq!(plain, timed);
        assert_eq!(timing.encode_us, 0, "pderiv_bc has no ARegex encode step");
    }

    #[test]
    fn timed_reports_zero_decode_on_no_match() {
        let (tree, timing) = parse_pderiv_bc_timed("b", &Regex::lit('a'));
        assert_eq!(tree, None);
        assert_eq!(timing.decode_us, 0);
    }

    #[test]
    fn star_three_iterations() {
        let r = Regex::star(Regex::lit('a'));
        let tree = parse_pderiv_bc("aaa", &r).unwrap();
        assert_eq!(flatten(&tree), "aaa");
        assert_eq!(tree, parse_deriv_std_rec("aaa", &r).unwrap());
    }

    #[test]
    fn paper_r1_computes_greedy_not_posix() {
        // (a + ab)(b + eps) on "ab"
        let r = Regex::seq(
            Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
            Regex::alt(Regex::lit('b'), Regex::Eps),
        );

        let posix = parse_deriv_std_rec("ab", &r);
        let greedy = parse_pderiv_bc("ab", &r);

        assert!(posix.is_some() && greedy.is_some(), "both should match");
        assert_eq!(flatten(posix.as_ref().unwrap()), "ab");
        assert_eq!(flatten(greedy.as_ref().unwrap()), "ab");

        // POSIX: the longer "ab" alternative wins (rule A1)
        assert_eq!(
            posix,
            Some(ParseTree::Pair(
                Box::new(ParseTree::Right(Box::new(ParseTree::Pair(
                    Box::new(ParseTree::Char('a')),
                    Box::new(ParseTree::Char('b')),
                )))),
                Box::new(ParseTree::Right(Box::new(ParseTree::Empty))),
            ))
        );
        // Greedy: the leftmost alternative 'a' wins, committed to immediately
        assert_eq!(
            greedy,
            Some(ParseTree::Pair(
                Box::new(ParseTree::Left(Box::new(ParseTree::Char('a')))),
                Box::new(ParseTree::Left(Box::new(ParseTree::Char('b')))),
            ))
        );
        assert_ne!(posix, greedy, "this is exactly the documented divergence");
    }

    // Both parsers still agree on Some/None; only which parse tree is selected differs.
    #[test]
    fn paper_r2_computes_greedy_not_posix() {
        // (a + (b + ab))* on "ab"
        let r = Regex::star(Regex::alt(
            Regex::lit('a'),
            Regex::alt(Regex::lit('b'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
        ));

        let posix = parse_deriv_std_rec("ab", &r);
        let greedy = parse_pderiv_bc("ab", &r);

        assert!(posix.is_some() && greedy.is_some(), "both should match");
        assert_eq!(flatten(posix.as_ref().unwrap()), "ab");
        assert_eq!(flatten(greedy.as_ref().unwrap()), "ab");

        // POSIX: one iteration, the longer "ab" alternative wins (rule A1)
        assert_eq!(
            posix,
            Some(ParseTree::Star(vec![ParseTree::Right(Box::new(ParseTree::Right(
                Box::new(ParseTree::Pair(
                    Box::new(ParseTree::Char('a')),
                    Box::new(ParseTree::Char('b')),
                ))
            )))]))
        );
        // Greedy: two iterations, leftmost alternative wins at each step
        assert_eq!(
            greedy,
            Some(ParseTree::Star(vec![
                ParseTree::Left(Box::new(ParseTree::Char('a'))),
                ParseTree::Right(Box::new(ParseTree::Left(Box::new(ParseTree::Char('b'))))),
            ]))
        );
        assert_ne!(posix, greedy, "this is exactly the documented divergence");
    }
}
