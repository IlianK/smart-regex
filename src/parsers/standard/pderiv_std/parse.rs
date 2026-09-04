//! Plain-Regex partial-derivative parser (pderiv_std)

use std::rc::Rc;
use crate::types::{ParseTree, Regex};
use crate::trace::{PDerivStdStep, PDerivStdTrace};
use crate::regex::standard::nullable::nullable;
use crate::parsers::standard::mk_eps::mk_eps;
use super::inject::{Inj, identity_inj};
use super::pderiv_tree::pderiv_tree;

fn step_frontier(frontier: Vec<(Regex, Inj)>, x: char) -> Vec<(Regex, Inj)> {
    let mut out = Vec::new();
    for (r, inj_parent) in frontier {
        for (r_next, inj_step) in pderiv_tree(&r, x) {
            let parent = inj_parent.clone();
            let combined: Inj = Rc::new(move |v| parent(inj_step(v)));
            out.push((r_next, combined));
        }
    }
    out
}

fn select(frontier: &[(Regex, Inj)]) -> Option<ParseTree> {
    for (r, inj) in frontier {
        if nullable(r) {
            return Some(inj(mk_eps(r)));
        }
    }
    None
}

pub fn parse_pderiv_std(input: &str, r: &Regex) -> Option<ParseTree> {
    let mut frontier: Vec<(Regex, Inj)> = vec![(r.clone(), identity_inj())];
    for c in input.chars() {
        frontier = step_frontier(frontier, c);
        if frontier.is_empty() {
            return None;
        }
    }
    select(&frontier)
}

pub fn parse_pderiv_standard_traced(input: &str, r: &Regex) -> (Option<ParseTree>, PDerivStdTrace) {
    let chars: Vec<char> = input.chars().collect();
    let mut frontier: Vec<(Regex, Inj)> = vec![(r.clone(), identity_inj())];
    let initial: Vec<Regex> = frontier.iter().map(|(r, _)| r.clone()).collect();

    let mut steps: Vec<PDerivStdStep> = Vec::with_capacity(chars.len());
    let mut last_nullable_idx: Option<usize> = None;
    let mut tree_at_last_nullable: Option<ParseTree> = None;

    if let Some(tree) = select(&frontier) {
        last_nullable_idx = Some(0);
        tree_at_last_nullable = Some(tree);
    }

    for (idx, &c) in chars.iter().enumerate() {
        let before: Vec<Regex> = frontier.iter().map(|(r, _)| r.clone()).collect();
        frontier = step_frontier(frontier, c);
        let tree_here = select(&frontier);
        let after: Vec<Regex> = frontier.iter().map(|(r, _)| r.clone()).collect();

        steps.push(PDerivStdStep {
            position: idx + 1,
            character: c,
            before,
            after,
            nullable: tree_here.is_some(),
        });

        if let Some(ref tree) = tree_here {
            last_nullable_idx = Some(idx + 1);
            tree_at_last_nullable = Some(tree.clone());
        }
    }

    let final_tree = select(&frontier);

    let trace = PDerivStdTrace {
        initial,
        steps,
        final_tree: final_tree.clone(),
        last_nullable_idx,
        tree_at_last_nullable,
    };

    (final_tree, trace)
}

// Unit tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::flatten;
    use crate::parsers::bitcoded::pderiv_bc::parse_pderiv_bc;
    use crate::parsers::standard::parse_recursive;

    #[test]
    fn empty_regex_empty_input() {
        assert_eq!(parse_pderiv_std("", &Regex::Eps), Some(ParseTree::Empty));
    }

    #[test]
    fn literal_matches() {
        let r = Regex::lit('a');
        assert_eq!(parse_pderiv_std("a", &r), Some(ParseTree::Char('a')));
    }

    #[test]
    fn no_match_is_none() {
        assert_eq!(parse_pderiv_std("b", &Regex::lit('a')), None);
    }

    #[test]
    fn star_three_iterations() {
        let r = Regex::star(Regex::lit('a'));
        let tree = parse_pderiv_std("aaa", &r).unwrap();
        assert_eq!(flatten(&tree), "aaa");
        assert_eq!(tree, parse_recursive("aaa", &r).unwrap());
    }

    #[test]
    fn seq_matches() {
        let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
        let tree = parse_pderiv_std("ab", &r).unwrap();
        assert_eq!(flatten(&tree), "ab");
    }

    #[test]
    fn paper_r1_agrees_with_pderiv_bc_greedy_answer() {
        let r = Regex::seq(
            Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
            Regex::alt(Regex::lit('b'), Regex::Eps),
        );
        let tree_std = parse_pderiv_std("ab", &r);
        let tree_bc = parse_pderiv_bc("ab", &r);
        assert_eq!(tree_std, tree_bc, "standard and bitcoded pderiv must agree exactly");

        assert_eq!(
            tree_std,
            Some(ParseTree::Pair(
                Box::new(ParseTree::Left(Box::new(ParseTree::Char('a')))),
                Box::new(ParseTree::Left(Box::new(ParseTree::Char('b')))),
            ))
        );
        assert_ne!(tree_std, parse_recursive("ab", &r), "Greedy differs from POSIX here");
    }

    #[test]
    fn paper_r2_agrees_with_pderiv_bc_greedy_answer() {
        let r = Regex::star(Regex::alt(
            Regex::lit('a'),
            Regex::alt(Regex::lit('b'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
        ));
        let tree_std = parse_pderiv_std("ab", &r);
        let tree_bc = parse_pderiv_bc("ab", &r);
        assert_eq!(tree_std, tree_bc, "standard and bitcoded pderiv must agree exactly");
        assert_eq!(
            tree_std,
            Some(ParseTree::Star(vec![
                ParseTree::Left(Box::new(ParseTree::Char('a'))),
                ParseTree::Right(Box::new(ParseTree::Left(Box::new(ParseTree::Char('b'))))),
            ]))
        );
    }

    #[test]
    fn nested_star_all_agree() {
        let r = Regex::star(Regex::star(Regex::lit('a')));
        let tree_std = parse_pderiv_std("aaa", &r);
        let tree_bc = parse_pderiv_bc("aaa", &r);
        assert_eq!(tree_std, tree_bc);
        assert!(tree_std.is_some());
    }

    #[test]
    fn ambiguous_star_dedup_matches_bitcoded() {
        let r = Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('a')));
        let tree_std = parse_pderiv_std("aaaa", &r);
        let tree_bc = parse_pderiv_bc("aaaa", &r);
        assert_eq!(tree_std, tree_bc);
    }

    #[test]
    fn traced_matches_untraced_on_success() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_pderiv_std("aaa", &r);
        let (traced, trace) = parse_pderiv_standard_traced("aaa", &r);
        assert_eq!(plain, traced);
        assert!(traced.is_some());
        assert!(trace.final_tree.is_some());
        assert_eq!(trace.final_tree, plain);
        assert_eq!(trace.steps.len(), 3);
    }

    #[test]
    fn traced_matches_untraced_on_failure() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_pderiv_std("aab", &r);
        let (traced, trace) = parse_pderiv_standard_traced("aab", &r);
        assert_eq!(plain, traced);
        assert!(traced.is_none());
        assert!(trace.final_tree.is_none());
        assert_eq!(trace.last_nullable_idx, Some(2));
        assert!(trace.tree_at_last_nullable.is_some());
        assert_eq!(trace.steps.len(), 3);
    }

    #[test]
    fn traced_matches_bc_traced_frontier_sizes() {
        use crate::parsers::bitcoded::pderiv_bc::parse_pderiv_bc_traced;
        let r = Regex::star(Regex::alt(
            Regex::lit('a'),
            Regex::alt(Regex::lit('b'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
        ));
        let (_, trace_std) = parse_pderiv_standard_traced("aab", &r);
        let (_, trace_bc) = parse_pderiv_bc_traced("aab", &r);
        assert_eq!(trace_std.initial.len(), trace_bc.initial.len());
        for (s, b) in trace_std.steps.iter().zip(trace_bc.steps.iter()) {
            assert_eq!(s.after.len(), b.after.len());
        }
    }
}
