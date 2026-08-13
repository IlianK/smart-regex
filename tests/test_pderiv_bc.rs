// tests/test_pderiv_bc.rs
//
// Run:  cargo test --test test_pderiv_bc

mod common;
use common::{assert_round_trip, paper_r1, paper_r2};

use regex_engine::Regex;
use regex_engine::parsers::{parse_recursive, parse_pderiv_bc};


// -------------------------------
// Unambiguous cases: exact agreement expected
// -------------------------------

fn pderiv_bc_agrees_with_recursive(input: &str, r: &Regex) {
    let rec = parse_recursive(input, r);
    let pd = parse_pderiv_bc(input, r);
    assert_eq!(
        rec, pd,
        "recursive and pderiv_bc disagree on unambiguous input {:?} for {:?}:\n  recursive: {:?}\n  pderiv_bc: {:?}",
        input, r, rec, pd
    );
}

#[test]
fn pderiv_bc_agrees_on_eps()        { pderiv_bc_agrees_with_recursive("",    &Regex::Eps); }
#[test]
fn pderiv_bc_agrees_on_literal()    { pderiv_bc_agrees_with_recursive("a",   &Regex::lit('a')); }
#[test]
fn pderiv_bc_agrees_on_no_match()   { pderiv_bc_agrees_with_recursive("b",   &Regex::lit('a')); }
#[test]
fn pderiv_bc_agrees_on_phi()        { pderiv_bc_agrees_with_recursive("a",   &Regex::Phi); }
#[test]
fn pderiv_bc_agrees_on_star_empty() { pderiv_bc_agrees_with_recursive("",    &Regex::star(Regex::lit('a'))); }
#[test]
fn pderiv_bc_agrees_on_star_one()   { pderiv_bc_agrees_with_recursive("a",   &Regex::star(Regex::lit('a'))); }
#[test]
fn pderiv_bc_agrees_on_star_three() { pderiv_bc_agrees_with_recursive("aaa", &Regex::star(Regex::lit('a'))); }

#[test]
fn pderiv_bc_agrees_on_seq() {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    pderiv_bc_agrees_with_recursive("ab", &r);
    pderiv_bc_agrees_with_recursive("a",  &r);
}

#[test]
fn pderiv_bc_agrees_on_disjoint_alt() {
    // No shared prefix, no length ambiguity -> Greedy and POSIX coincide
    let r = Regex::alt(Regex::lit('a'), Regex::lit('b'));
    for w in &["a", "b", "c", ""] {
        pderiv_bc_agrees_with_recursive(w, &r);
    }
}

#[test]
fn pderiv_bc_round_trip_flatten() {
    let r = Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')));
    for w in &["", "a", "b", "ab", "ba", "aabb", "baba"] {
        if let Some(tree) = parse_pderiv_bc(w, &r) {
            assert_round_trip(&tree, w);
        }
    }
}


// -------------------------------
// Ambiguous cases (paper_r1, paper_r2): membership must still agree
// -------------------------------

#[test]
fn pderiv_bc_membership_agrees_on_paper_r1() {
    let r = paper_r1();
    for w in &["ab", "a", "b", ""] {
        let rec = parse_recursive(w, &r);
        let pd = parse_pderiv_bc(w, &r);
        assert_eq!(rec.is_some(), pd.is_some(), "membership disagreement on {:?}", w);
        if let Some(tree) = pd {
            assert_round_trip(&tree, w);
        }
    }
}

#[test]
fn pderiv_bc_membership_agrees_on_paper_r2() {
    let r = paper_r2();
    for w in &["ab", "a", "b", "aab", "abab", ""] {
        let rec = parse_recursive(w, &r);
        let pd = parse_pderiv_bc(w, &r);
        assert_eq!(rec.is_some(), pd.is_some(), "membership disagreement on {:?}", w);
        if let Some(tree) = pd {
            assert_round_trip(&tree, w);
        }
    }
}
