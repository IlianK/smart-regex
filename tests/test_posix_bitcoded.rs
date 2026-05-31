// tests/test_posix_bitcoded.rs
//
// Integration tests for src/posix/bitcoded/
//   - parse_bitcoded (bitcoded/parse.rs)
//
// Correctness property (Theorem 1 applied to bitcoded path):
//   parse_bitcoded(w, r) == parse_recursive(w, r)   for all w, r
//
// Run:  cargo test --test test_posix_bitcoded

mod common;
use common::{assert_round_trip, assert_parsers_agree, paper_r1, paper_r2};

use regex_engine::Regex;
use regex_engine::posix::{parse_recursive, parse_bitcoded};


// ============================================================================
// parse_bitcoded 
// ============================================================================

fn bitcoded_agrees_with_recursive(input: &str, r: &Regex) {
    let rec = parse_recursive(input, r);
    let bc  = parse_bitcoded(input, r);
    assert_parsers_agree("recursive", &rec, "bitcoded", &bc);
}

#[test]
fn bitcoded_agrees_on_eps()         { bitcoded_agrees_with_recursive("",    &Regex::Eps); }
#[test]
fn bitcoded_agrees_on_literal()     { bitcoded_agrees_with_recursive("a",   &Regex::lit('a')); }
#[test]
fn bitcoded_agrees_on_no_match()    { bitcoded_agrees_with_recursive("b",   &Regex::lit('a')); }
#[test]
fn bitcoded_agrees_on_phi()         { bitcoded_agrees_with_recursive("a",   &Regex::Phi); }
#[test]
fn bitcoded_agrees_on_star_empty()  { bitcoded_agrees_with_recursive("",    &Regex::star(Regex::lit('a'))); }
#[test]
fn bitcoded_agrees_on_star_one()    { bitcoded_agrees_with_recursive("a",   &Regex::star(Regex::lit('a'))); }
#[test]
fn bitcoded_agrees_on_star_three()  { bitcoded_agrees_with_recursive("aaa", &Regex::star(Regex::lit('a'))); }

#[test]
fn bitcoded_agrees_on_paper_r1_ab() { bitcoded_agrees_with_recursive("ab",  &paper_r1()); }
#[test]
fn bitcoded_agrees_on_paper_r1_a()  { bitcoded_agrees_with_recursive("a",   &paper_r1()); }
#[test]
fn bitcoded_agrees_on_paper_r2_ab() { bitcoded_agrees_with_recursive("ab",  &paper_r2()); }

#[test]
fn bitcoded_agrees_on_seq() {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    bitcoded_agrees_with_recursive("ab", &r);
    bitcoded_agrees_with_recursive("a",  &r);
}

#[test]
fn bitcoded_agrees_on_alt() {
    let r = Regex::alt(Regex::lit('a'), Regex::lit('b'));
    for w in &["a", "b", "c", ""] {
        bitcoded_agrees_with_recursive(w, &r);
    }
}

#[test]
fn bitcoded_agrees_on_complex_star() {
    let r = Regex::star(Regex::seq(Regex::lit('a'), Regex::lit('b')));
    for w in &["", "ab", "abab", "ababab", "a", "b"] {
        bitcoded_agrees_with_recursive(w, &r);
    }
}

#[test]
fn bitcoded_round_trip_flatten() {
    // Whatever parse_bitcoded returns, flatten should reproduce the input
    let r = Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')));
    for w in &["", "a", "b", "ab", "ba", "aabb", "baba"] {
        if let Some(tree) = parse_bitcoded(w, &r) {
            assert_round_trip(&tree, w);
        }
    }
}

// ============================================================================
// POSIX ordering rules 
// ============================================================================

#[test]
fn bitcoded_agrees_on_a1_longer_right_wins() {
    // A1: (a+aa)* on "aa" - POSIX picks Star([Right(aa)]) not Star([Left(a), Left(a)])
    let r = Regex::star(Regex::alt(
        Regex::lit('a'),
        Regex::seq(Regex::lit('a'), Regex::lit('a')),
    ));
    bitcoded_agrees_with_recursive("aa", &r);
}

#[test]
fn bitcoded_agrees_on_a2_left_tiebreaker() {
    // A2: (a+a) on "a" - equal length, left wins → Left(Char('a'))
    let r = Regex::alt(Regex::lit('a'), Regex::lit('a'));
    bitcoded_agrees_with_recursive("a", &r);
}

#[test]
fn bitcoded_agrees_on_k1_empty_star() {
    // K1: ε* on "" - zero iterations, Star([])
    bitcoded_agrees_with_recursive("", &Regex::star(Regex::Eps));
}

#[test]
fn bitcoded_agrees_on_k2_nonempty_preferred() {
    // K2: (ε+a)* on "a" - non-empty iteration preferred, Star([Right(a)])
    let r = Regex::star(Regex::alt(Regex::Eps, Regex::lit('a')));
    bitcoded_agrees_with_recursive("a", &r);
}