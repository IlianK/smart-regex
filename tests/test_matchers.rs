// tests/test_matchers.rs
//
// Integration tests for src/matchers/
//   - match_naive    (matchers/match_naive.rs)
//   - match_deriv    (matchers/match_deriv.rs)
//   - match_pderiv   (matchers/match_pderiv.rs)
//
// Central property:  ∀ w r.  match_naive(w,r) = match_deriv(w,r) = match_pderiv(w,r)
//
// Run:  cargo test --test test_matchers

mod common;
use common::{paper_r1, paper_r2};

use regex_engine::{Regex, match_naive, match_deriv, match_pderiv};

// -------------------------------
// Helpers
// -------------------------------

fn all_agree(w: &str, r: &Regex) {
    let n = match_naive(w, r);
    let d = match_deriv(w, r);
    let p = match_pderiv(w, r);
    assert_eq!(n, d, "naive/deriv disagree on {:?}", w);
    assert_eq!(d, p, "deriv/pderiv disagree on {:?}", w);
}

fn expect_match(w: &str, r: &Regex) {
    assert!(match_naive(w, r),   "naive: {:?} should match", w);
    assert!(match_deriv(w, r),   "deriv: {:?} should match", w);
    assert!(match_pderiv(w, r),  "pderiv: {:?} should match", w);
}

fn expect_no_match(w: &str, r: &Regex) {
    assert!(!match_naive(w, r),  "naive: {:?} should NOT match", w);
    assert!(!match_deriv(w, r),  "deriv: {:?} should NOT match", w);
    assert!(!match_pderiv(w, r), "pderiv: {:?} should NOT match", w);
}


// -------------------------------
// Phi: empty language, matches nothing
// -------------------------------

#[test]
fn phi_matches_nothing() {
    expect_no_match("",  &Regex::Phi);
    expect_no_match("a", &Regex::Phi);
}


// -------------------------------
// Eps: matches only the empty word
// -------------------------------

#[test]
fn eps_matches_empty_word() {
    expect_match("", &Regex::Eps);
}

#[test]
fn eps_does_not_match_nonempty() {
    expect_no_match("a",  &Regex::Eps);
    expect_no_match("ab", &Regex::Eps);
}


// -------------------------------
// Lit: matches exactly one character
// -------------------------------

#[test]
fn lit_matches_its_character() {
    expect_match("a", &Regex::lit('a'));
}

#[test]
fn lit_does_not_match_wrong_char() {
    expect_no_match("b", &Regex::lit('a'));
}

#[test]
fn lit_does_not_match_empty() {
    expect_no_match("", &Regex::lit('a'));
}

#[test]
fn lit_does_not_match_two_chars() {
    expect_no_match("aa", &Regex::lit('a'));
}


// -------------------------------
// Alt: union of two languages
// -------------------------------

#[test]
fn alt_matches_left_branch() {
    let r = Regex::alt(Regex::lit('a'), Regex::lit('b'));
    expect_match("a", &r);
}

#[test]
fn alt_matches_right_branch() {
    let r = Regex::alt(Regex::lit('a'), Regex::lit('b'));
    expect_match("b", &r);
}

#[test]
fn alt_does_not_match_neither() {
    let r = Regex::alt(Regex::lit('a'), Regex::lit('b'));
    expect_no_match("c", &r);
    expect_no_match("",  &r);
}

#[test]
fn alt_with_eps_makes_nullable() {
    let r = Regex::alt(Regex::lit('a'), Regex::Eps);
    expect_match("",  &r);
    expect_match("a", &r);
    expect_no_match("b", &r);
}


// -------------------------------
// Seq: concatenation
// -------------------------------

#[test]
fn seq_matches_concatenation() {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    expect_match("ab", &r);
}

#[test]
fn seq_does_not_match_prefix_only() {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    expect_no_match("a",   &r);
    expect_no_match("",    &r);
    expect_no_match("abc", &r);
}

#[test]
fn seq_with_eps_is_identity() {
    let r = Regex::seq(Regex::Eps, Regex::lit('a'));
    expect_match("a",  &r);
    expect_no_match("", &r);
}


// -------------------------------
// Star: Kleene closure
// -------------------------------

#[test]
fn star_matches_empty() {
    expect_match("", &Regex::star(Regex::lit('a')));
}

#[test]
fn star_matches_one_repetition() {
    expect_match("a", &Regex::star(Regex::lit('a')));
}

#[test]
fn star_matches_many_repetitions() {
    let r = Regex::star(Regex::lit('a'));
    expect_match("aaaa", &r);
}

#[test]
fn star_does_not_match_wrong_char() {
    expect_no_match("b",  &Regex::star(Regex::lit('a')));
    expect_no_match("ab", &Regex::star(Regex::lit('a')));
}

#[test]
fn star_of_phi_matches_only_empty() {
    // Star(Phi) = Eps
    let r = Regex::star(Regex::Phi);
    expect_match("",  &r);
    expect_no_match("a", &r);
}

#[test]
fn star_of_eps_matches_only_empty() {
    // Star(Eps) = Eps (simplification)
    let r = Regex::star(Regex::Eps);
    // All three matchers should handle this without infinite-looping
    let n = match_naive("", &r);
    let d = match_deriv("", &r);
    let p = match_pderiv("", &r);
    assert!(n && d && p, "Star(Eps) must match empty");
}


// -------------------------------
// Paper examples (flops14-extended)
// -------------------------------

#[test]
fn paper_r1_matches_ab()    { expect_match("ab", &paper_r1()); }
#[test]
fn paper_r1_matches_a()     { expect_match("a",  &paper_r1()); }
#[test]
fn paper_r1_no_match_b()    { expect_no_match("b",   &paper_r1()); }
#[test]
fn paper_r1_no_match_aa()   { expect_no_match("aa",  &paper_r1()); }

#[test]
fn paper_r2_matches_a()     { expect_match("a",  &paper_r2()); }
#[test]
fn paper_r2_matches_b()     { expect_match("b",  &paper_r2()); }
#[test]
fn paper_r2_matches_ab()    { expect_match("ab", &paper_r2()); }
#[test]
fn paper_r2_matches_empty() { expect_match("",   &paper_r2()); }


// -------------------------------
// Agreement of all matchers
// -------------------------------

#[test]
fn agreement_star_of_lit() {
    let r = Regex::star(Regex::lit('a'));
    for w in &["", "a", "aa", "aaa", "b", "ba", "ab"] {
        all_agree(w, &r);
    }
}

#[test]
fn agreement_seq_of_stars() {
    let r = Regex::seq(
        Regex::star(Regex::lit('a')),
        Regex::star(Regex::lit('b')),
    );
    for w in &["", "a", "b", "ab", "aab", "abb", "aabb", "ba"] {
        all_agree(w, &r);
    }
}

#[test]
fn agreement_nested_alt_seq() {
    let r = Regex::alt(
        Regex::seq(Regex::lit('a'), Regex::lit('b')),
        Regex::seq(Regex::lit('a'), Regex::lit('c')),
    );
    for w in &["ab", "ac", "a", "b", "bc", "abc"] {
        all_agree(w, &r);
    }
}

#[test]
fn agreement_paper_r1() {
    for w in &["ab", "a", "b", "", "abb", "aa", "abc"] {
        all_agree(w, &paper_r1());
    }
}

#[test]
fn agreement_paper_r2() {
    for w in &["", "a", "b", "ab", "ba", "aab", "abb"] {
        all_agree(w, &paper_r2());
    }
}

// -------------------------------
// Edge cases
// -------------------------------

#[test]
fn empty_word_on_all_base_cases() {
    all_agree("", &Regex::Phi);
    all_agree("", &Regex::Eps);
    all_agree("", &Regex::lit('a'));
}

#[test]
fn long_word_on_star() {
    let r = Regex::star(Regex::lit('x'));
    let w: String = "x".repeat(50);
    all_agree(&w, &r);
}

#[test]
fn nested_star_all_agree() {
    // (a*)* - problematic for naive (exponential) but correct for all
    let r = Regex::star(Regex::star(Regex::lit('a')));
    let d = match_deriv("aaa", &r);
    let p = match_pderiv("aaa", &r);
    assert_eq!(d, p, "(a*)* deriv/pderiv should agree");
    assert!(d, "(a*)* should match 'aaa'");
}