//! Tests for mk_eps (empty parse tree construction)
//!
//! Based on Lemma 1: "mkEps_r is the POSIX parse tree of r for the empty word"

use regex_engine::basic::Regex;
use regex_engine::posix::{mk_eps, flatten, ParseTree};
use regex_engine::posix::tests::fmt_regex;

fn test_mk_eps(r: &Regex, label: &str, expected_tree: &str, expected_flat: &str) {
    println!("\n▶ mkEps: {}", label);
    println!("  - Regex: {}", fmt_regex(r));
    println!("  - Expected:   {} -> \"{}\"", expected_tree, expected_flat);
    let tree = mk_eps(r);
    println!("  - Parse tree: {}", tree);
    println!("  - Flattened:  \"{}\"", flatten(&tree));
    println!();
}


// BASIC MK_EPS TESTS
#[test]
fn test_mk_eps_epsilon() {
    test_mk_eps(&Regex::Eps, "ε", "()", "");
    let tree = mk_eps(&Regex::Eps);
    assert!(matches!(tree, ParseTree::Empty));
    assert_eq!(flatten(&tree), "");
}

#[test]
fn test_mk_eps_star() {
    let r = Regex::star(Regex::lit('a'));
    test_mk_eps(&r, "a*", "[]", "");
    let tree = mk_eps(&r);
    assert!(matches!(tree, ParseTree::Star(ref v) if v.is_empty()));
    assert_eq!(flatten(&tree), "");
}

#[test]
fn test_mk_eps_star_of_epsilon() {
    let r = Regex::star(Regex::Eps);
    test_mk_eps(&r, "ε*", "[]", "");
    let tree = mk_eps(&r);
    assert!(matches!(tree, ParseTree::Star(ref v) if v.is_empty()));
}


// ALTERNATION MK_EPS TESTS
#[test]
fn test_mk_eps_alt_left_nullable() {
    let r = Regex::alt(Regex::Eps, Regex::lit('a'));
    test_mk_eps(&r, "ε + a", "Left ()", "");
    let tree = mk_eps(&r);
    assert!(matches!(tree, ParseTree::Left(_)));
    assert_eq!(flatten(&tree), "");
}

#[test]
fn test_mk_eps_alt_right_nullable() {
    let r = Regex::alt(Regex::lit('a'), Regex::Eps);
    test_mk_eps(&r, "a + ε", "Right ()", "");
    let tree = mk_eps(&r);
    assert!(matches!(tree, ParseTree::Right(_)));
    assert_eq!(flatten(&tree), "");
}

#[test]
fn test_mk_eps_alt_both_nullable() {
    let r = Regex::alt(Regex::Eps, Regex::Eps);
    test_mk_eps(&r, "ε + ε", "Left ()", "");
    let tree = mk_eps(&r);
    assert!(matches!(tree, ParseTree::Left(_)));
}


// SEQUENCE MK_EPS TESTS
#[test]
fn test_mk_eps_sequence_both_nullable() {
    let r = Regex::seq(Regex::Eps, Regex::Eps);
    test_mk_eps(&r, "ε·ε", "((), ())", "");
    let tree = mk_eps(&r);
    assert!(matches!(tree, ParseTree::Pair(_, _)));
    assert_eq!(flatten(&tree), "");
}

#[test]
fn test_mk_eps_sequence_star_and_eps() {
    let r = Regex::seq(Regex::star(Regex::lit('a')), Regex::Eps);
    test_mk_eps(&r, "a*·ε", "([], ())", "");
    let tree = mk_eps(&r);
    assert!(matches!(tree, ParseTree::Pair(_, _)));
}


// NEGATIVE MK_EPS TESTS (Should panic)
#[test]
#[should_panic(expected = "mk_eps called on Lit")]
fn test_mk_eps_panics_on_literal() {
    let _ = mk_eps(&Regex::lit('a'));
}

#[test]
#[should_panic(expected = "mk_eps called on Phi")]
fn test_mk_eps_panics_on_phi() {
    let _ = mk_eps(&Regex::Phi);
}

#[test]
#[should_panic(expected = "mk_eps called on Lit")]
fn test_mk_eps_panics_on_non_nullable_sequence() {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    let _ = mk_eps(&r);
}