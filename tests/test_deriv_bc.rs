//! src/tests/test_deriv_bc.rs
//!
//!  Integration tests for parsers/deriv_bc/: parse_bitcoded(w, r) == parse_recursive(w, r) for all w, r.

mod common;
use common::{assert_round_trip, assert_parsers_agree, paper_r1, paper_r2};

use regex_engine::Regex;
use regex_engine::parsers::{parse_deriv_std_rec, parse_deriv_bc};

// parse_bitcoded

fn bitcoded_agrees_with_recursive(input: &str, r: &Regex) {
    let rec = parse_deriv_std_rec(input, r);
    let bc  = parse_deriv_bc(input, r);
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
        if let Some(tree) = parse_deriv_bc(w, &r) {
            assert_round_trip(&tree, w);
        }
    }
}

// POSIX ordering rules

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

// Differential fuzz: parse_deriv_bc vs. parse_deriv_std_rec
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 { self.next() % n }
}

fn gen_regex(rng: &mut Rng, depth: u32, alphabet: &[char]) -> Regex {
    if depth == 0 {
        match rng.below(2) {
            0 => Regex::Eps,
            _ => Regex::lit(alphabet[rng.below(alphabet.len() as u64) as usize]),
        }
    } else {
        match rng.below(6) {
            0 => Regex::Eps,
            1 => Regex::lit(alphabet[rng.below(alphabet.len() as u64) as usize]),
            2 => Regex::seq(gen_regex(rng, depth - 1, alphabet), gen_regex(rng, depth - 1, alphabet)),
            3 => Regex::alt(gen_regex(rng, depth - 1, alphabet), gen_regex(rng, depth - 1, alphabet)),
            4 => Regex::star(gen_regex(rng, depth - 1, alphabet)),
            _ => Regex::seq(gen_regex(rng, depth - 1, alphabet), gen_regex(rng, depth - 1, alphabet)),
        }
    }
}

fn gen_word(rng: &mut Rng, len: u32, alphabet: &[char]) -> String {
    (0..len).map(|_| alphabet[rng.below(alphabet.len() as u64) as usize]).collect()
}

#[test]
fn fuzz_bc_matches_std_exactly() {
    let alphabet = ['a', 'b', 'c'];
    let mut rng = Rng(0xD1B54A32D192ED03);
    let mut checked = 0u32;
    let mut mismatches = Vec::new();

    for _ in 0..20_000 {
        let r = gen_regex(&mut rng, 6, &alphabet);
        let wlen = rng.below(9) as u32;
        let w = gen_word(&mut rng, wlen, &alphabet);
        checked += 1;

        let std_tree = parse_deriv_std_rec(&w, &r);
        let bc_tree = parse_deriv_bc(&w, &r);

        if std_tree != bc_tree {
            mismatches.push((r.clone(), w.clone(), std_tree.clone(), bc_tree.clone()));
            if mismatches.len() >= 5 { break; }
        }
    }

    if !mismatches.is_empty() {
        let mut msg = format!("{} mismatches out of {} checked:\n", mismatches.len(), checked);
        for (r, w, std_tree, bc_tree) in &mismatches {
            msg.push_str(&format!(
                "regex = {:?}\n  word = {:?}\n  std = {:?}\n  bitcoded = {:?}\n\n",
                r, w, std_tree, bc_tree
            ));
        }
        panic!("{}", msg);
    }
    assert!(checked > 0);
}