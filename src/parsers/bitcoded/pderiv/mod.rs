//! regex-engine/src/posix/bitcoded/pderiv/mod.rs
//!
//! Bit-coded partial-derivative parser (pDerivBC / parsePDerivBC),
//! https://sulzmann.github.io/ProgrammingParadigms/pp-regular-expressions.html#(9)

use crate::types::{Regex, ParseTree};
use crate::regex::nullable::standard::nullable;
use crate::regex::pderiv::annotated::{pderiv_bc, mk_eps_bits};
use crate::parsers::bitcoded::deriv::decode::decode;
use crate::trace::{PDerivBitStep, PDerivBitTrace};


// -------------------------------
// Shared stepping logic
// -------------------------------

/// One step of `parsePDerivBC2`: replace the frontier with union of `pderiv_bc(x, ·)` 
fn step_frontier(frontier: &[(Regex, Vec<bool>)], x: char) -> Vec<(Regex, Vec<bool>)> {
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
fn select_bits(frontier: &[(Regex, Vec<bool>)]) -> Option<Vec<bool>> {
    for (r, bits) in frontier {
        if nullable(r) {
            let mut result = bits.clone();
            result.extend(mk_eps_bits(r));
            return Some(result);
        }
    }
    None
}


// -------------------------------
// Parser
// -------------------------------

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


// -------------------------------
// Traced variant (used by REGEX_DIAG=2/3)
// -------------------------------
pub fn parse_pderiv_bc_traced(input: &str, r: &Regex) -> (Option<ParseTree>, PDerivBitTrace) {
    let chars: Vec<char> = input.chars().collect();
    let initial = vec![(r.clone(), Vec::new())];

    let mut frontier = initial.clone();
    let mut steps: Vec<PDerivBitStep> = Vec::with_capacity(chars.len());
    let mut last_nullable_idx: Option<usize> = None;
    let mut bits_at_last_nullable: Option<Vec<bool>> = None;

    if let Some(bits) = select_bits(&frontier) {
        last_nullable_idx = Some(0);
        bits_at_last_nullable = Some(bits);
    }

    for (idx, &c) in chars.iter().enumerate() {
        let before = frontier.clone();
        frontier = step_frontier(&frontier, c);
        let bits_here = select_bits(&frontier);

        steps.push(PDerivBitStep {
            position: idx + 1,
            character: c,
            before,
            after: frontier.clone(),
            nullable: bits_here.is_some(),
        });

        if let Some(ref bits) = bits_here {
            last_nullable_idx = Some(idx + 1);
            bits_at_last_nullable = Some(bits.clone());
        }
    }

    let final_bits = select_bits(&frontier);
    let tree = final_bits.as_ref().map(|bits| decode(r, bits));

    let trace = PDerivBitTrace {
        initial,
        steps,
        final_bits,
        last_nullable_idx,
        bits_at_last_nullable,
    };

    (tree, trace)
}


// -------------------------------
// Unit tests
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Regex, flatten};
    use crate::parsers::standard::parse_recursive;

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
        assert_eq!(tree, parse_recursive("aaa", &r).unwrap());
    }

    // paper_r1 is also A1-relevant (Section 4.3.1): Greedy commits to the
    // first alternative 'a' of (a+ab) and never reconsiders it, while
    // POSIX prefers the longer 'ab' alternative. Same documented
    // divergence as paper_r2 below; both parsers still agree on Some/None.
    #[test]
    fn paper_r1_computes_greedy_not_posix() {
        // (a + ab)(b + eps) on "ab"
        let r = Regex::seq(
            Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
            Regex::alt(Regex::lit('b'), Regex::Eps),
        );

        let posix = parse_recursive("ab", &r);
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

    // Both parsers still agree on the *language membership* question
    // (Some vs None) only which parse tree is selected differs.
    #[test]
    fn paper_r2_computes_greedy_not_posix() {
        // (a + (b + ab))* on "ab"
        let r = Regex::star(Regex::alt(
            Regex::lit('a'),
            Regex::alt(Regex::lit('b'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
        ));

        let posix = parse_recursive("ab", &r);
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

    #[test]
    fn traced_matches_untraced_on_success() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_pderiv_bc("aaa", &r);
        let (traced, trace) = parse_pderiv_bc_traced("aaa", &r);
        assert_eq!(plain, traced);
        assert!(traced.is_some());
        assert!(trace.final_bits.is_some());
        assert_eq!(trace.steps.len(), 3);
    }

    #[test]
    fn traced_matches_untraced_on_failure() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_pderiv_bc("aab", &r);
        let (traced, trace) = parse_pderiv_bc_traced("aab", &r);
        assert_eq!(plain, traced);
        assert!(traced.is_none());
        assert!(trace.final_bits.is_none());
        assert_eq!(trace.last_nullable_idx, Some(2));
        assert!(trace.bits_at_last_nullable.is_some());
        // traced variant still records a step for every character, even
        // after the frontier has died out
        assert_eq!(trace.steps.len(), 3);
    }
}
