//! regex-engine/src/diagnostics/replay.rs
//! 
//! Replay utilities for failure diagnostics.
//!     Level 1 (error position only) 
//!     Level 2/3 (partial tree).

use crate::types::{Regex, ParseTree};
use crate::regex::deriv::standard::deriv;
use crate::regex::nullable::standard::nullable;
use crate::parsers::standard::mk_eps;
use crate::parsers::standard::inject;


// -------------------------------
// Error position (used by Level 1 for both standard and bitcoded)
// -------------------------------

/// Result of replaying the forward pass on a failing input
#[derive(Debug, Clone)]
pub struct FailureInfo {
    /// 1-indexed position where the match failed
    pub position: usize,
    /// The character that caused failure ('\\0' if input ended unexpectedly)
    pub found: char,
    /// Human-readable description of what was expected
    pub expected: String,
    /// Number of characters successfully matched before failure
    pub matched_prefix_len: usize,
}

/// Replay derivative forward pass and locate the first failure position
/// Called for both standard and bitcoded paths at Level 1 (caret error)
pub fn find_failure(input: &str, r: &Regex) -> FailureInfo {
    let chars: Vec<char> = input.chars().collect();
    let mut current = r.clone();
    let mut last_nullable_pos = 0; // how many chars matched successfully

    for (idx, &c) in chars.iter().enumerate() {
        let next = deriv(&current, c);
        if !nullable(&next) && is_dead(&next) {
            // This character caused transition into a dead (all-Phi) expression
            return FailureInfo {
                position:            idx + 1,
                found:               c,
                expected:            expected_description(&current),
                matched_prefix_len:  last_nullable_pos,
            };
        }
        if nullable(&next) {
            last_nullable_pos = idx + 1;
        }
        current = next;
    }

    // Input exhausted but final expression not nullable
    FailureInfo {
        position:           chars.len() + 1,
        found:              '\0', // signals "end of input"
        expected:           expected_description(&current),
        matched_prefix_len: last_nullable_pos,
    }
}

/// Returns true if r is equivalent to Phi (matches nothing)
fn is_dead(r: &Regex) -> bool {
    use Regex::*;
    match r {
        Phi          => true,
        Eps          => false,
        Lit(_)       => false,
        Star(_)      => false,
        Alt(r1, r2)  => is_dead(r1) && is_dead(r2),
        Seq(r1, _)   => is_dead(r1),
    }
}

/// Produce a human-readable description of what characters the expression
/// could still accept at this point, for the error message
fn expected_description(r: &Regex) -> String {
    let mut chars = collect_expected_chars(r);
    chars.sort();
    chars.dedup();

    let mut parts: Vec<String> = chars.iter().map(|c| format!("'{}'", c)).collect();

    if nullable(r) {
        parts.push("end of input".to_string());
    }

    match parts.len() {
        0 => "nothing (dead state)".to_string(),
        1 => parts.remove(0),
        2 => format!("{} or {}", parts[0], parts[1]),
        _ => {
            let last = parts.pop().unwrap();
            format!("{}, or {}", parts.join(", "), last)
        }
    }
}

/// Collect the set of characters that r can currently accept
fn collect_expected_chars(r: &Regex) -> Vec<char> {
    use Regex::*;
    match r {
        Phi         => vec![],
        Eps         => vec![],
        Lit(c)      => vec![*c],
        Star(r1)    => collect_expected_chars(r1),
        Alt(r1, r2) => {
            let mut v = collect_expected_chars(r1);
            v.extend(collect_expected_chars(r2));
            v
        }
        Seq(r1, r2) => {
            let mut v = collect_expected_chars(r1);
            if nullable(r1) {
                v.extend(collect_expected_chars(r2));
            }
            v
        }
    }
}


// -------------------------------
// Partial tree recovery (used by Level 2 and Level 3 on failure)
// -------------------------------

/// Recover partial parse tree from the last nullable derivative expression in forward pass
/// Returns None if no prefix matched at all.
///
/// Takes stored expression sequence from ParseTrace (so no need rerun forward pass)
pub fn partial_tree_standard(
    expressions: &[Regex],
    chars: &[char],
    last_nullable_idx: Option<usize>,
) -> Option<ParseTree> {
    let idx = last_nullable_idx?;
    if idx == 0 {
        // Only r0 was nullable (empty input prefix) - tree is mk_eps(r0)
        return Some(mk_eps(&expressions[0]));
    }

    // Run mkEps on the last nullable expression
    let mut tree = mk_eps(&expressions[idx]);

    // Replay inject steps backward from position idx-1 down to 0
    for i in (0..idx).rev() {
        tree = inject(&expressions[i], chars[i], tree);
    }

    Some(tree)
}


// -------------------------------
// Caret line builder (shared across all levels)
// -------------------------------

/// Build the two-line caret display:
///   "  aab"
///   "    ^"
/// position is 1-indexed.
pub fn caret_lines(input: &str, position: usize) -> String {
    let display_input = format!("  {}", input);
    // position 1 means first char; offset = 2 (indent) + position - 1
    let caret_offset = 2 + position.saturating_sub(1);
    let caret_line   = format!("{}^", " ".repeat(caret_offset));
    format!("{}\n{}", display_input, caret_line)
}

/// Full failure block: "Error: position N: found/expected ..." followed by
/// the caret lines. Used identically by every parser family at Levels 1-3
/// (previously duplicated verbatim at each call site); callers print the
/// result directly (Levels 1-2) or append it via ReportWriter (Level 3).
pub fn error_report(input: &str, r: &Regex) -> String {
    let info = find_failure(input, r);
    let headline = if info.found == '\0' {
        format!(
            "Error:  position {}: unexpected end of input, expected {}",
            info.position, info.expected
        )
    } else {
        format!(
            "Error:  position {}: found '{}', expected {}",
            info.position, info.found, info.expected
        )
    };
    format!("{}\n{}", headline, caret_lines(input, info.position))
}



// Unit tests
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_failure_correct_position() {
        let r = Regex::star(Regex::lit('a'));
        let info = find_failure("aab", &r);
        assert_eq!(info.position, 3);
        assert_eq!(info.found, 'b');
        assert_eq!(info.matched_prefix_len, 2);
    }

    #[test]
    fn find_failure_first_char() {
        let r = Regex::lit('a');
        let info = find_failure("b", &r);
        assert_eq!(info.position, 1);
        assert_eq!(info.found, 'b');
    }

    #[test]
    fn caret_lines_correct_offset() {
        let result = caret_lines("aab", 3);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[0], "  aab");
        assert_eq!(lines[1], "    ^");
    }

    #[test]
    fn error_report_combines_headline_and_caret() {
        let r = Regex::star(Regex::lit('a'));
        let report = error_report("aab", &r);
        let lines: Vec<&str> = report.lines().collect();
        assert_eq!(lines[0], "Error:  position 3: found 'b', expected 'a' or end of input");
        assert_eq!(lines[1], "  aab");
        assert_eq!(lines[2], "    ^");
    }

    #[test]
    fn error_report_end_of_input() {
        let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
        let report = error_report("a", &r);
        assert!(report.starts_with("Error:  position 2: unexpected end of input"));
    }

    #[test]
    fn partial_tree_recovers_matched_prefix() {
        use crate::types::flatten;
        let r = Regex::star(Regex::lit('a'));
        let chars = vec!['a', 'a', 'b'];
        let mut exprs = vec![r.clone()];
        let mut last_nullable = None;
        for (i, &c) in chars.iter().enumerate() {
            let next = deriv(exprs.last().unwrap(), c);
            if nullable(&next) { last_nullable = Some(i + 1); }
            exprs.push(next);
        }
        let partial = partial_tree_standard(&exprs, &chars, last_nullable);
        assert!(partial.is_some());
        assert_eq!(flatten(&partial.unwrap()), "aa");
    }
}