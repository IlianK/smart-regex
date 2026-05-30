// tests/common/mod.rs
//
// Shared helpers for integration tests.
// Placed in tests/common/mod.rs (not tests/common.rs) so Cargo does NOT
// treat this file as a standalone integration-test crate and add a spurious
// "running 0 tests" section in the output.
//
// Usage in any integration test file:
//   mod common;
//   use common::*;

use regex_engine::{Regex, ParseTree, flatten};

// Regex constructors (short aliases) 
pub fn lit(c: char)           -> Regex { Regex::lit(c) }
pub fn seq(a: Regex, b: Regex) -> Regex { Regex::seq(a, b) }
pub fn alt(a: Regex, b: Regex) -> Regex { Regex::alt(a, b) }
pub fn star(r: Regex)         -> Regex { Regex::star(r) }

// Paper regexes 
/// (a + ab)(b + ε)  — running example from flops14, p. 4
pub fn paper_r1() -> Regex {
    seq(
        alt(lit('a'), seq(lit('a'), lit('b'))),
        alt(lit('b'), Regex::Eps),
    )
}

/// (a + b + ab)*  — POSIX star-preference example from flops14
pub fn paper_r2() -> Regex {
    star(alt(alt(lit('a'), lit('b')), seq(lit('a'), lit('b'))))
}

// =========================================================================
// Assertion helpers 
// =========================================================================

#[allow(dead_code)]
pub fn assert_round_trip(tree: &ParseTree, expected_word: &str) {
    assert_eq!(
        flatten(tree), expected_word,
        "flatten(tree) should equal the matched word"
    );
}

#[allow(dead_code)]
pub fn assert_parsers_agree(
    label_a: &str,
    tree_a: &Option<ParseTree>,
    label_b: &str,
    tree_b: &Option<ParseTree>,
) {
    assert_eq!(
        tree_a, tree_b,
        "{} and {} disagree:\n  {}: {:?}\n  {}: {:?}",
        label_a, label_b, label_a, tree_a, label_b, tree_b
    );
}