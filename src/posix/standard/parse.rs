//! Standard POSIX parsers (recursive and loop)

use crate::types::{Regex, ParseTree};
use crate::regex::brzozowski::standard::deriv;
use crate::regex::nullable::standard::nullable;
use crate::posix::standard::{mk_eps, inject};

// ============================================================================
// RECURSIVE PARSER
// ============================================================================

fn parse_recursive_helper(r: &Regex, input: &str) -> Option<ParseTree> {
    let mut chars = input.chars();
    match chars.next() {
        None => {
            if nullable(r) { Some(mk_eps(r)) } else { None }
        }
        Some(l) => {
            let rest: String = chars.collect();
            let r_deriv = deriv(r, l);
            let subtree = parse_recursive_helper(&r_deriv, &rest)?;
            Some(inject(r, l, subtree))
        }
    }
}

pub fn parse_recursive(input: &str, r: &Regex) -> Option<ParseTree> {
    parse_recursive_helper(r, input)
}

// ============================================================================
// LOOP PARSER
// ============================================================================

pub fn parse_loop(input: &str, r: &Regex) -> Option<ParseTree> {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();

    let mut expressions = Vec::with_capacity(n + 1);
    expressions.push(r.clone());

    for &c in chars.iter() {
        let current = expressions.last().unwrap();
        let next = deriv(current, c);
        expressions.push(next);
    }

    let final_r = expressions.last().unwrap();
    if !nullable(final_r) {
        return None;
    }

    let mut tree = mk_eps(expressions.last().unwrap());
    for i in (0..n).rev() {
        tree = inject(&expressions[i], chars[i], tree);
    }

    Some(tree)
}

// ============================================================================
// LOOP PARSER — TRACED VARIANT (used by diagnostics Level 2/3 with REGEX_PARSER=loop)
// ============================================================================

use crate::trace::{DerivStep, InjectStep, MkEpsResult, ParseTrace};

/// Identical in behaviour to parse_loop but also returns a ParseTrace.
pub fn parse_loop_traced(input: &str, r: &Regex) -> (Option<ParseTree>, ParseTrace) {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();

    let mut expressions: Vec<Regex> = Vec::with_capacity(n + 1);
    expressions.push(r.clone());

    let mut deriv_steps: Vec<DerivStep> = Vec::with_capacity(n);
    let mut last_nullable_idx: Option<usize> = if nullable(r) { Some(0) } else { None };

    // ── Forward pass ─────────────────────────────────────────────────────────
    for (idx, &c) in chars.iter().enumerate() {
        let before = expressions.last().unwrap().clone();
        let after  = deriv(&before, c);
        let is_nullable = nullable(&after);

        deriv_steps.push(DerivStep {
            position:  idx + 1,
            character: c,
            before,
            after: after.clone(),
            nullable:  is_nullable,
        });

        if is_nullable {
            last_nullable_idx = Some(idx + 1);
        }

        expressions.push(after);
    }

    // ── Nullability check ─────────────────────────────────────────────────────
    let final_r = expressions.last().unwrap();
    if !nullable(final_r) {
        let trace = ParseTrace {
            expressions,
            deriv_steps,
            mk_eps_result:     None,
            inject_steps:      None,
            last_nullable_idx,
        };
        return (None, trace);
    }

    // ── Backward pass ─────────────────────────────────────────────────────────
    let mk_eps_tree = mk_eps(final_r);
    let mk_eps_result = Some(MkEpsResult {
        regex: final_r.clone(),
        tree:  mk_eps_tree.clone(),
    });

    let mut tree = mk_eps_tree;
    let mut inject_steps: Vec<InjectStep> = Vec::with_capacity(n);

    for i in (0..n).rev() {
        let before = tree.clone();
        tree = inject(&expressions[i], chars[i], tree);
        inject_steps.push(InjectStep {
            position:  i + 1,
            character: chars[i],
            before,
            after: tree.clone(),
        });
    }

    // Steps recorded n-1..0; reverse so index 0 = position 1 (forward order)
    inject_steps.reverse();

    let trace = ParseTrace {
        expressions,
        deriv_steps,
        mk_eps_result,
        inject_steps: Some(inject_steps),
        last_nullable_idx,
    };

    (Some(tree), trace)
}

// ============================================================================
// RECURSIVE PARSER — TRACED VARIANT (used by diagnostics Level 2/3 with REGEX_PARSER=recursive)
// ============================================================================

/// Identical in behaviour to parse_recursive but also returns a ParseTrace.
/// The trace is built by threading mutable accumulators through the recursion.
pub fn parse_recursive_traced(input: &str, r: &Regex) -> (Option<ParseTree>, ParseTrace) {
    let chars: Vec<char> = input.chars().collect();

    let mut expressions: Vec<Regex> = Vec::new();
    let mut deriv_steps: Vec<DerivStep> = Vec::new();
    let mut last_nullable_idx: Option<usize> = if nullable(r) { Some(0) } else { None };

    expressions.push(r.clone());

    let result = parse_recursive_traced_helper(
        r,
        &chars,
        0,
        &mut expressions,
        &mut deriv_steps,
        &mut last_nullable_idx,
    );

    match result {
        None => {
            let trace = ParseTrace {
                expressions,
                deriv_steps,
                mk_eps_result: None,
                inject_steps:  None,
                last_nullable_idx,
            };
            (None, trace)
        }
        Some((tree, mut inject_steps, mke)) => {
            // inject_steps are pushed during unwind (deepest first); reverse to forward order
            inject_steps.reverse();
            let trace = ParseTrace {
                expressions,
                deriv_steps,
                mk_eps_result: Some(mke),
                inject_steps:  Some(inject_steps),
                last_nullable_idx,
            };
            (Some(tree), trace)
        }
    }
}

fn parse_recursive_traced_helper(
    r: &Regex,
    chars: &[char],
    pos: usize,
    expressions: &mut Vec<Regex>,
    deriv_steps: &mut Vec<DerivStep>,
    last_nullable_idx: &mut Option<usize>,
) -> Option<(ParseTree, Vec<InjectStep>, MkEpsResult)> {

    if pos == chars.len() {
        if !nullable(r) { return None; }
        let mke_tree = mk_eps(r);
        let mke = MkEpsResult { regex: r.clone(), tree: mke_tree.clone() };
        return Some((mke_tree, Vec::new(), mke));
    }

    let c = chars[pos];
    let after = deriv(r, c);
    let is_nullable = nullable(&after);

    deriv_steps.push(DerivStep {
        position:  pos + 1,
        character: c,
        before:    r.clone(),
        after:     after.clone(),
        nullable:  is_nullable,
    });

    if is_nullable {
        *last_nullable_idx = Some(pos + 1);
    }

    expressions.push(after.clone());

    let (subtree, mut inject_steps, mke) = parse_recursive_traced_helper(
        &after,
        chars,
        pos + 1,
        expressions,
        deriv_steps,
        last_nullable_idx,
    )?;

    let before_inject = subtree.clone();
    let injected = inject(r, c, subtree);

    // Push in unwind order (deepest/last position first); caller reverses to get forward order
    inject_steps.push(InjectStep {
        position:  pos + 1,
        character: c,
        before:    before_inject,
        after:     injected.clone(),
    });

    Some((injected, inject_steps, mke))
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_loop_traced ────────────────────────────────────────────────────

    #[test]
    fn loop_traced_matches_untraced_on_success() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_loop("aaa", &r);
        let (traced, trace) = parse_loop_traced("aaa", &r);
        assert_eq!(plain, traced);
        assert!(traced.is_some());
        assert_eq!(trace.expression_count(), 4);
        assert!(trace.inject_steps.is_some());
    }

    #[test]
    fn loop_traced_matches_untraced_on_failure() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_loop("aab", &r);
        let (traced, trace) = parse_loop_traced("aab", &r);
        assert_eq!(plain, traced);
        assert!(traced.is_none());
        assert!(trace.inject_steps.is_none());
        assert_eq!(trace.last_nullable_idx, Some(2));
    }

    // ── parse_recursive_traced ───────────────────────────────────────────────

    #[test]
    fn recursive_traced_matches_untraced_on_success() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_recursive("aaa", &r);
        let (traced, trace) = parse_recursive_traced("aaa", &r);
        assert_eq!(plain, traced);
        assert!(traced.is_some());
        assert_eq!(trace.expression_count(), 4);
        assert!(trace.inject_steps.is_some());
    }

    #[test]
    fn recursive_traced_matches_untraced_on_failure() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_recursive("aab", &r);
        let (traced, trace) = parse_recursive_traced("aab", &r);
        assert_eq!(plain, traced);
        assert!(traced.is_none());
        assert!(trace.inject_steps.is_none());
        assert_eq!(trace.last_nullable_idx, Some(2));
    }

    #[test]
    fn recursive_traced_agrees_with_loop_traced() {
        let r = Regex::star(Regex::lit('a'));
        let (loop_tree, _)      = parse_loop_traced("aaa", &r);
        let (rec_tree,  _)      = parse_recursive_traced("aaa", &r);
        assert_eq!(loop_tree, rec_tree);
    }

    #[test]
    fn recursive_traced_inject_steps_are_in_forward_order() {
        // Steps should be position 1, 2, 3 — not 3, 2, 1
        let r = Regex::star(Regex::lit('a'));
        let (_, trace) = parse_recursive_traced("aaa", &r);
        let steps = trace.inject_steps.unwrap();
        let positions: Vec<usize> = steps.iter().map(|s| s.position).collect();
        assert_eq!(positions, vec![1, 2, 3]);
    }

    #[test]
    fn loop_traced_inject_steps_are_in_forward_order() {
        let r = Regex::star(Regex::lit('a'));
        let (_, trace) = parse_loop_traced("aaa", &r);
        let steps = trace.inject_steps.unwrap();
        let positions: Vec<usize> = steps.iter().map(|s| s.position).collect();
        assert_eq!(positions, vec![1, 2, 3]);
    }
}