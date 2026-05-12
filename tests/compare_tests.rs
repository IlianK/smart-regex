//! Compare recursive and loop implementations - correctness 

use regex_engine::basic::Regex;
use regex_engine::posix::{parse_recursive, parse_loop};

fn compare(input: &str, r: &Regex, label: &str) {
    println!("\nTesting: {}", label);
    println!("  Input: \"{}\"", input);
    println!("  Regex: {:?}", r);
    
    let rec_result = parse_recursive(input, r);
    let loop_result = parse_loop(input, r);
    
    println!("  Recursive: {:?}", rec_result);
    println!("  Loop:      {:?}", loop_result);
    
    assert_eq!(rec_result, loop_result, "Results differ for {}", label);
    println!("  OK");
}

#[test]
fn test_correctness_literal() {
    let r = Regex::lit('a');
    compare("a", &r, "Literal 'a'");
    compare("b", &r, "Literal 'a' with 'b' (no match)");
}

#[test]
fn test_correctness_sequence() {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    compare("ab", &r, "Sequence a.b");
}

#[test]
fn test_correctness_star() {
    let r = Regex::star(Regex::lit('a'));
    compare("", &r, "Star a* (empty)");
    compare("aaa", &r, "Star a* (three a's)");
}

#[test]
fn test_correctness_alternation() {
    let r = Regex::alt(
        Regex::lit('a'),
        Regex::seq(Regex::lit('a'), Regex::lit('b'))
    );
    compare("ab", &r, "Alternation (a + ab)");
}

#[test]
fn test_correctness_epsilon_alt_star() {
    let r = Regex::star(Regex::alt(Regex::Eps, Regex::lit('a')));
    compare("a", &r, "(epsilon + a)*");
}