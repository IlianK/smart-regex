//! Tests for basic matchers (moved from old matcher_tests.rs)

use regex_engine::basic::Regex;
use regex_engine::basic::{match_naive, match_deriv, match_pderiv};

// Helper function to run all three matchers
fn check(input: &str, r: &Regex, expected: bool) {
    let naive = match_naive(input, r);
    let deriv = match_deriv(input, r);
    let pderiv = match_pderiv(input, r);

    assert_eq!(naive, expected, "naive: input={:?}, r={:?}", input, r);
    assert_eq!(deriv, expected, "deriv: input={:?}, r={:?}", input, r);
    assert_eq!(pderiv, expected, "pderiv: input={:?}, r={:?}", input, r);
}

#[test]
fn test_phi_matches_nothing() {
    check("", &Regex::Phi, false);
    check("a", &Regex::Phi, false);
    check("abc", &Regex::Phi, false);
}

#[test]
fn test_eps_matches_only_empty() {
    check("", &Regex::Eps, true);
    check("a", &Regex::Eps, false);
}

#[test]
fn test_literal() {
    let ra = Regex::lit('a');
    check("a", &ra, true);
    check("", &ra, false);
    check("b", &ra, false);
    check("aa", &ra, false);
}

#[test]
fn test_sequence() {
    let ab = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    check("ab", &ab, true);
    check("a", &ab, false);
    check("b", &ab, false);
    check("abc", &ab, false);
}

#[test]
fn test_alt() {
    let a_or_b = Regex::alt(Regex::lit('a'), Regex::lit('b'));
    check("a", &a_or_b, true);
    check("b", &a_or_b, true);
    check("c", &a_or_b, false);
    check("ab", &a_or_b, false);
}

#[test]
fn test_star() {
    let a_star = Regex::star(Regex::lit('a'));
    check("", &a_star, true);
    check("a", &a_star, true);
    check("aa", &a_star, true);
    check("aaa", &a_star, true);
    check("b", &a_star, false);
}

#[test]
fn test_ab_star() {
    let ab_star = Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')));
    check("", &ab_star, true);
    check("a", &ab_star, true);
    check("b", &ab_star, true);
    check("ab", &ab_star, true);
    check("ba", &ab_star, true);
    check("abba", &ab_star, true);
    check("abc", &ab_star, false);
}