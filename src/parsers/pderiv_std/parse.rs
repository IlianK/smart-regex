//! src/parsers/pderiv_std/mod.rs
//! 
//! Plain-Regex partial-derivative parser (pderiv_std); 
//! traced variant in traced.rs reuses step_frontier/select.

use crate::types::{ParseTree, Regex};
use crate::regex::nullable::nullable;
use crate::regex::mk_eps::mk_eps;
use super::inject::{Inj, nub_tree};
use super::pderiv_tree::pderiv_tree;

/// One transition of the whole state set. `pderiv_tree` already dedups
/// the children of a single parent; a final `nub_tree` here dedups
/// *across* parents too, which is required for the frontier to stay
/// bounded (Sulzmann & Lu 2012, Def. 8: "duplicate states are removed,
/// only the left-most state is kept" -- applied to the combined
/// configuration, not per-origin). Without it, two strands that
/// independently derive to the same residual both survive, and the
/// frontier can double every character on an ambiguous pattern like
/// `(a|b|ab)*` even though the number of distinct residuals stays
/// bounded by the pattern's own size (Antimirov's finiteness result).
pub(super) fn step_frontier(frontier: Vec<(Regex, Inj)>, x: char) -> Vec<(Regex, Inj)> {
    let mut out = Vec::new();
    for (r, inj_parent) in frontier {
        for (r_next, inj_step) in pderiv_tree(&r, x) {
            out.push((r_next, inj_parent.then(inj_step)));
        }
    }
    nub_tree(out)
}

/// Reverses every `Star`'s items exactly once. `pderiv_tree`'s Star-case
/// closures (see that file) `push` a new iteration instead of
/// `insert(0, ..)`, O(1) amortized instead of O(current length) --
/// which leaves every Star's items in reverse order until this single
/// O(total output size) correction pass.
fn reverse_stars(v: ParseTree) -> ParseTree {
    match v {
        ParseTree::Empty | ParseTree::Char(_) => v,
        ParseTree::Pair(v1, v2) => ParseTree::Pair(Box::new(reverse_stars(*v1)), Box::new(reverse_stars(*v2))),
        ParseTree::Left(v1) => ParseTree::Left(Box::new(reverse_stars(*v1))),
        ParseTree::Right(v1) => ParseTree::Right(Box::new(reverse_stars(*v1))),
        ParseTree::Star(mut items) => {
            items.reverse();
            ParseTree::Star(items.into_iter().map(reverse_stars).collect())
        }
    }
}

pub(super) fn select(frontier: &[(Regex, Inj)]) -> Option<ParseTree> {
    for (r, inj) in frontier {
        if nullable(r) {
            return Some(reverse_stars(inj.apply(mk_eps(r))));
        }
    }
    None
}

pub fn parse_pderiv_std(input: &str, r: &Regex) -> Option<ParseTree> {
    let mut frontier: Vec<(Regex, Inj)> = vec![(r.clone(), Inj::identity())];
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
    fn star_iterations_are_in_left_to_right_order() {
        // (a|b)* on "abab": a bug in the push/reverse rewrite would
        // reverse this to [b,a,b,a] instead of the correct [a,b,a,b].
        let r = Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')));
        let tree = parse_pderiv_std("abab", &r).unwrap();
        assert_eq!(flatten(&tree), "abab");
        assert_eq!(
            tree,
            ParseTree::Star(vec![
                ParseTree::Left(Box::new(ParseTree::Char('a'))),
                ParseTree::Right(Box::new(ParseTree::Char('b'))),
                ParseTree::Left(Box::new(ParseTree::Char('a'))),
                ParseTree::Right(Box::new(ParseTree::Char('b'))),
            ])
        );
        assert_eq!(tree, parse_pderiv_bc("abab", &r).unwrap());
    }

    #[test]
    fn nested_star_iterations_match_bc_order() {
        // (a*)* on "aaaaa": a bug in the push/reverse rewrite at either
        // nesting level would still agree with pderiv_bc on *membership*
        // but disagree on the exact tree shape.
        let r = Regex::star(Regex::star(Regex::lit('a')));
        let tree_std = parse_pderiv_std("aaaaa", &r);
        let tree_bc = parse_pderiv_bc("aaaaa", &r);
        assert_eq!(tree_std, tree_bc);
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
