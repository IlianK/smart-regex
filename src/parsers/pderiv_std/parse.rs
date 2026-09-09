//! Plain-Regex partial-derivative parser (pderiv_std); traced variant in traced.rs reuses step_frontier/select.

use std::rc::Rc;
use crate::types::{ParseTree, Regex};
use crate::regex::nullable::nullable;
use crate::regex::mk_eps::mk_eps;
use super::inject::{Inj, identity_inj};
use super::pderiv_tree::pderiv_tree;

pub(super) fn step_frontier(frontier: Vec<(Regex, Inj)>, x: char) -> Vec<(Regex, Inj)> {
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

pub(super) fn select(frontier: &[(Regex, Inj)]) -> Option<ParseTree> {
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


// Unit tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::flatten;
    use crate::parsers::pderiv_bc::parse_pderiv_bc;
    use crate::parsers::deriv_std::parse_deriv_std_rec;

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
        assert_eq!(tree, parse_deriv_std_rec("aaa", &r).unwrap());
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
        assert_ne!(tree_std, parse_deriv_std_rec("ab", &r), "Greedy differs from POSIX here");
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
}
