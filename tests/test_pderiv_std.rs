// tests/test_pderiv_standard.rs
//
// Integration tests for parsers::standard::pderiv_std::parse_pderiv_standard
//
// Run:  cargo test --test test_pderiv_standard

mod common;
use common::{assert_round_trip, paper_r1, paper_r2};

use regex_engine::Regex;
use regex_engine::parsers::{parse_pderiv_bc, parse_pderiv_std, parse_recursive};

// -------------------------------
// parse_pderiv_standard == parse_pderiv_bc
// -------------------------------

fn pderiv_standard_agrees_with_bc(input: &str, r: &Regex) {
    let std_tree = parse_pderiv_std(input, r);
    let bc_tree = parse_pderiv_bc(input, r);
    assert_eq!(
        std_tree, bc_tree,
        "parse_pderiv_standard and parse_pderiv_bc disagree on {:?} for {:?}:\n  standard: {:?}\n  bitcoded: {:?}",
        input, r, std_tree, bc_tree
    );
}

#[test]
fn agrees_on_eps()        { pderiv_standard_agrees_with_bc("",    &Regex::Eps); }
#[test]
fn agrees_on_literal()    { pderiv_standard_agrees_with_bc("a",   &Regex::lit('a')); }
#[test]
fn agrees_on_no_match()   { pderiv_standard_agrees_with_bc("b",   &Regex::lit('a')); }
#[test]
fn agrees_on_phi()        { pderiv_standard_agrees_with_bc("a",   &Regex::Phi); }
#[test]
fn agrees_on_star_empty() { pderiv_standard_agrees_with_bc("",    &Regex::star(Regex::lit('a'))); }
#[test]
fn agrees_on_star_one()   { pderiv_standard_agrees_with_bc("a",   &Regex::star(Regex::lit('a'))); }
#[test]
fn agrees_on_star_three() { pderiv_standard_agrees_with_bc("aaa", &Regex::star(Regex::lit('a'))); }

#[test]
fn agrees_on_seq() {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    pderiv_standard_agrees_with_bc("ab", &r);
    pderiv_standard_agrees_with_bc("a",  &r);
}

#[test]
fn agrees_on_paper_r1() {
    let r = paper_r1();
    for w in &["ab", "a", "b", ""] {
        pderiv_standard_agrees_with_bc(w, &r);
    }
}

#[test]
fn agrees_on_paper_r2() {
    let r = paper_r2();
    for w in &["ab", "a", "b", "aab", "abab", ""] {
        pderiv_standard_agrees_with_bc(w, &r);
    }
}

#[test]
fn round_trip_flatten() {
    let r = Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')));
    for w in &["", "a", "b", "ab", "ba", "aabb", "baba"] {
        if let Some(tree) = parse_pderiv_std(w, &r) {
            assert_round_trip(&tree, w);
        }
    }
}

// Membership must still agree with the POSIX parser 
#[test]
fn membership_agrees_with_posix_on_paper_r1() {
    let r = paper_r1();
    for w in &["ab", "a", "b", ""] {
        let rec = parse_recursive(w, &r);
        let pd = parse_pderiv_std(w, &r);
        assert_eq!(rec.is_some(), pd.is_some(), "membership disagreement on {:?}", w);
    }
}

// -------------------------------
// Differential fuzz: parse_pderiv_standard vs. parse_pderiv_bc 
// -------------------------------

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
fn fuzz_standard_matches_bitcoded_exactly() {
    let alphabet = ['a', 'b', 'c'];
    let mut rng = Rng(0xD1B54A32D192ED03);
    let mut checked = 0u32;
    let mut mismatches = Vec::new();

    for _ in 0..20_000 {
        let r = gen_regex(&mut rng, 6, &alphabet);
        let wlen = rng.below(9) as u32;
        let w = gen_word(&mut rng, wlen, &alphabet);
        checked += 1;

        let std_tree = parse_pderiv_std(&w, &r);
        let bc_tree = parse_pderiv_bc(&w, &r);

        if std_tree != bc_tree {
            mismatches.push((r.clone(), w.clone(), std_tree.clone(), bc_tree.clone()));
            if mismatches.len() >= 5 { break; }
        }
    }

    if !mismatches.is_empty() {
        let mut msg = format!("{} mismatches out of {} checked:\n", mismatches.len(), checked);
        for (r, w, std_tree, bc_tree) in &mismatches {
            msg.push_str(&format!(
                "regex = {:?}\n  word = {:?}\n  standard = {:?}\n  bitcoded = {:?}\n\n",
                r, w, std_tree, bc_tree
            ));
        }
        panic!("{}", msg);
    }
}

#[test]
fn fuzz_membership_agrees_with_posix() {
    let alphabet = ['a', 'b'];
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let mut checked = 0u32;

    for _ in 0..10_000 {
        let r = gen_regex(&mut rng, 5, &alphabet);
        let wlen = rng.below(7) as u32;
        let w = gen_word(&mut rng, wlen, &alphabet);
        checked += 1;

        let rec = parse_recursive(&w, &r);
        let pd = parse_pderiv_std(&w, &r);
        assert_eq!(
            rec.is_some(), pd.is_some(),
            "membership disagreement: r={:?} w={:?}\n  recursive: {:?}\n  pderiv_standard: {:?}",
            r, w, rec, pd
        );
        if let Some(ref tree) = pd {
            assert_eq!(regex_engine::types::flatten(tree), w, "round-trip failure");
        }
    }
    assert!(checked > 0);
}
