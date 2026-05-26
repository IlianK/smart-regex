//! Tests for bit-coded parser

use regex_engine::types::Regex;
use regex_engine::posix::{parse_recursive, parse_loop, parse_bitcoded, flatten};

fn compare_parsers(input: &str, r: &Regex, label: &str) {
    println!("\n=== Testing: {} ===", label);
    println!("Input: \"{}\"", input);
    
    let rec_result = parse_recursive(input, r);
    let loop_result = parse_loop(input, r);
    let bit_result = parse_bitcoded(input, r);
    
    println!("  Recursive: {:?} -> \"{}\"", rec_result, rec_result.as_ref().map_or("".to_string(), |t| flatten(t)));
    println!("  Loop:      {:?} -> \"{}\"", loop_result, loop_result.as_ref().map_or("".to_string(), |t| flatten(t)));
    println!("  Bit-coded: {:?} -> \"{}\"", bit_result, bit_result.as_ref().map_or("".to_string(), |t| flatten(t)));
    
    assert_eq!(rec_result, bit_result, "Recursive vs Bit-coded mismatch for {}", label);
    assert_eq!(loop_result, bit_result, "Loop vs Bit-coded mismatch for {}", label);
    println!("  ✓ All parsers agree");
}

#[test]
fn test_bitcoded_literal() {
    let r = Regex::lit('a');
    compare_parsers("a", &r, "Literal 'a'");
    compare_parsers("b", &r, "Literal 'a' with 'b' (no match)");
}

#[test]
fn test_bitcoded_sequence() {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    compare_parsers("ab", &r, "Sequence a·b");
    compare_parsers("a", &r, "Sequence a·b with 'a' (no match)");
}

#[test]
fn test_bitcoded_star() {
    let r = Regex::star(Regex::lit('a'));
    compare_parsers("", &r, "Star a* (empty)");
    compare_parsers("a", &r, "Star a* (one)");
    compare_parsers("aaa", &r, "Star a* (three)");
}

#[test]
fn test_bitcoded_epsilon_alt_star() {
    let r = Regex::star(Regex::alt(Regex::Eps, Regex::lit('a')));
    compare_parsers("a", &r, "(ε + a)*");
    compare_parsers("", &r, "(ε + a)* (empty)");
}

#[test]
fn test_bitcoded_paper_example() {
    let r = Regex::seq(
        Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
        Regex::alt(Regex::lit('b'), Regex::Eps),
    );
    compare_parsers("ab", &r, "(a+ab)(b+ε)");
}

#[test]
fn test_bitcoded_ambiguous() {
    let r = Regex::star(Regex::alt(
        Regex::lit('a'),
        Regex::alt(Regex::lit('b'), Regex::seq(Regex::lit('a'), Regex::lit('b')))
    ));
    compare_parsers("ab", &r, "(a+b+ab)*");
}