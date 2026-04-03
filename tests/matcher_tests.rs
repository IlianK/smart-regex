/// regex_engine/tests/matcher_tests.rs

/* 
Tests for three matchers: match_naive, match_deriv, match_pderiv
*/

use regex_engine::{Regex, match_deriv, match_naive, match_pderiv};


// Runs all three matchers and asserts each equals expected
fn check(input: &str, r: &Regex, expected: bool) {
    let naive  = match_naive(input, r);
    let deriv  = match_deriv(input, r);
    let pderiv = match_pderiv(input, r);

    assert_eq!(naive,  expected, "naive:  input={:?}, r={:?}", input, r);
    assert_eq!(deriv,  expected, "deriv:  input={:?}, r={:?}", input, r);
    assert_eq!(pderiv, expected, "pderiv: input={:?}, r={:?}", input, r);
}

// Phi matches nothing
#[test]
fn test_phi_matches_nothing() {
    check("",    &Regex::Phi, false);
    check("a",   &Regex::Phi, false);
    check("abc", &Regex::Phi, false);
}

// Eps matches only empty string
#[test]
fn test_eps_matches_only_empty() {
    check("",  &Regex::Eps, true);
    check("a", &Regex::Eps, false);
}

// Lit matches exactly one char
#[test]
fn test_literal() {
    let ra = Regex::lit('a');
    check("a",  &ra, true);
    check("",   &ra, false);
    check("b",  &ra, false);
    check("aa", &ra, false);
}

// Seq of two literals: a·b
#[test]
fn test_sequence() {
    let ab = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    check("ab",  &ab, true);
    check("a",   &ab, false);
    check("b",   &ab, false);
    check("abc", &ab, false);
    check("",    &ab, false);
}

// Seq nested inside Seq three times: a·a·a
#[test]
fn test_sequence_three() {
    let abc = Regex::seq(
        Regex::seq(Regex::lit('a'), Regex::lit('b')),
        Regex::lit('c'),
    );
    check("abc",  &abc, true);
    check("ab",   &abc, false);
    check("abcd", &abc, false);
}

// Alternative base case
#[test]
fn test_alt() {
    let a_or_b = Regex::alt(Regex::lit('a'), Regex::lit('b'));
    check("a",  &a_or_b, true);
    check("b",  &a_or_b, true);
    check("c",  &a_or_b, false);
    check("ab", &a_or_b, false);
    check("",   &a_or_b, false);
}

// Star base case
#[test]
fn test_star_empty() {
    let a_star = Regex::star(Regex::lit('a'));
    check("",    &a_star, true);
    check("a",   &a_star, true);
    check("aa",  &a_star, true);
    check("aaa", &a_star, true);
    check("b",   &a_star, false);
    check("ab",  &a_star, false);
}


// Star over Alt
#[test]
fn test_ab_star() {
    let ab_star = Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')));
    check("",     &ab_star, true);
    check("a",    &ab_star, true);
    check("b",    &ab_star, true);
    check("ab",   &ab_star, true);
    check("ba",   &ab_star, true);
    check("abba", &ab_star, true);
    check("abc",  &ab_star, false);
    check("c",    &ab_star, false);
}

// Star over Seq
#[test]
fn test_even_as() {
    // (a·a)* -- accepts strings with an even number of a's
    let even_a = Regex::star(Regex::seq(Regex::lit('a'), Regex::lit('a')));
    check("",     &even_a, true);
    check("aa",   &even_a, true);
    check("aaaa", &even_a, true);
    check("a",    &even_a, false);
    check("aaa",  &even_a, false);
    check("b",    &even_a, false);
}

// Seq containing Star
#[test]
fn test_seq_with_star() {
    // a·(b*)·c
    let r = Regex::seq(
        Regex::seq(Regex::lit('a'), Regex::star(Regex::lit('b'))),
        Regex::lit('c'),
    );
    check("ac",    &r, true);  // b* matches epsilon
    check("abc",   &r, true);
    check("abbc",  &r, true);
    check("abbbc", &r, true);
    check("a",     &r, false);
    check("bc",    &r, false);
    check("abbd",  &r, false);
}

// Nullable Alt with Eps
#[test]
fn test_optional() {
    // a? = a | epsilon
    let a_opt = Regex::alt(Regex::lit('a'), Regex::Eps);
    check("",   &a_opt, true);
    check("a",  &a_opt, true);
    check("b",  &a_opt, false);
    check("aa", &a_opt, false);
}

// Nullable left side of Seq
#[test]
fn test_nullable_in_seq() {
    // (a*) · b -- since a* is nullable, "b" alone must match
    let r = Regex::seq(Regex::star(Regex::lit('a')), Regex::lit('b'));
    check("b",   &r, true);
    check("ab",  &r, true);
    check("aab", &r, true);
    check("",    &r, false);
    check("a",   &r, false);
}

