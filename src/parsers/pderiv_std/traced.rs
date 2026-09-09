//! Traced pderiv_std parser (REGEX_DIAG=2/3): wraps parse.rs's step_frontier/select, recording a PDerivStdTrace.

use crate::types::{ParseTree, Regex};
use crate::trace::{PDerivStdStep, PDerivStdTrace};
use super::inject::{Inj, identity_inj};
use super::parse::{step_frontier, select};

pub fn parse_pderiv_std_traced(input: &str, r: &Regex) -> (Option<ParseTree>, PDerivStdTrace) {
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
    use super::super::parse::parse_pderiv_std;

    #[test]
    fn traced_matches_untraced_on_success() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_pderiv_std("aaa", &r);
        let (traced, trace) = parse_pderiv_std_traced("aaa", &r);
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
        let (traced, trace) = parse_pderiv_std_traced("aab", &r);
        assert_eq!(plain, traced);
        assert!(traced.is_none());
        assert!(trace.final_tree.is_none());
        assert_eq!(trace.last_nullable_idx, Some(2));
        assert!(trace.tree_at_last_nullable.is_some());
        assert_eq!(trace.steps.len(), 3);
    }

    #[test]
    fn traced_matches_bc_traced_frontier_sizes() {
        use crate::parsers::pderiv_bc::parse_pderiv_bc_traced;
        let r = Regex::star(Regex::alt(
            Regex::lit('a'),
            Regex::alt(Regex::lit('b'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
        ));
        let (_, trace_std) = parse_pderiv_std_traced("aab", &r);
        let (_, trace_bc) = parse_pderiv_bc_traced("aab", &r);
        assert_eq!(trace_std.initial.len(), trace_bc.initial.len());
        for (s, b) in trace_std.steps.iter().zip(trace_bc.steps.iter()) {
            assert_eq!(s.after.len(), b.after.len());
        }
    }
}
