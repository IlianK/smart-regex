//! Bit-coded partial-derivative parser (pDerivBC / parsePDerivBC), GREEDY; traced variant in traced.rs.

use crate::types::{Regex, ParseTree};
use crate::regex::nullable::nullable;
use crate::parsers::pderiv_bc::pderiv::{pderiv_bc, mk_eps_bits};
use crate::regex::decode::decode;

/// One step of `parsePDerivBC2`: replace the frontier with union of `pderiv_bc(x, ·)`
pub(super) fn step_frontier(frontier: &[(Regex, Vec<bool>)], x: char) -> Vec<(Regex, Vec<bool>)> {
    let mut out = Vec::new();
    for (r, bits) in frontier {
        for (r_next, extra) in pderiv_bc(r, x) {
            let mut combined = bits.clone();
            combined.extend(extra);
            out.push((r_next, combined));
        }
    }
    out
}

/// `filter (\(r,_) -> nullable r) rs`, take head, and append `mkEpsBC`.
pub(super) fn select_bits(frontier: &[(Regex, Vec<bool>)]) -> Option<Vec<bool>> {
    for (r, bits) in frontier {
        if nullable(r) {
            let mut result = bits.clone();
            result.extend(mk_eps_bits(r));
            return Some(result);
        }
    }
    None
}

pub fn parse_pderiv_bc(input: &str, r: &Regex) -> Option<ParseTree> {
    let mut frontier = vec![(r.clone(), Vec::new())];
    for c in input.chars() {
        frontier = step_frontier(&frontier, c);
        if frontier.is_empty() {
            return None;
        }
    }
    let bits = select_bits(&frontier)?;
    Some(decode(r, &bits))
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
