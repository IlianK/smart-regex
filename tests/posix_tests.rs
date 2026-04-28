//! Main POSIX parser tests

use regex_engine::basic::Regex;
use regex_engine::posix::{parse_posix, flatten, ParseTree};
use regex_engine::posix::tests::fmt_regex;

fn test_posix(input: &str, r: &Regex, expected: Option<ParseTree>, label: &str) {
    println!("\n▶ {}", label);
    println!("  - Regex: {}", fmt_regex(r));
    println!("  - Input: \"{}\"", input);
    
    match parse_posix(input, r) {
        Some(tree) => {
            if let Some(exp) = expected {
                assert_eq!(tree, exp);
                println!("  - Expected:   {}", exp);
            }
            println!("  - Parse tree: {}", tree);
            println!("  - Flattened:  \"{}\"", flatten(&tree));
        }
        None => {
            println!("  - Result:     X No match");
            if expected.is_some() {
                panic!("Expected match but got None for: {}", label);
            }
        }
    }
    println!();
}


#[test]
fn test_literal() {
    let r = Regex::lit('a');
    test_posix("a", &r, Some(ParseTree::Char('a')), "Literal 'a'");
    test_posix("b", &r, None, "Literal 'a' rejects 'b'");
}

#[test]
fn test_empty_string() {
    let r = Regex::Eps;
    test_posix("", &r, Some(ParseTree::Empty), "Empty ε");
}

#[test]
fn test_sequence() {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    let expected = ParseTree::Pair(
        Box::new(ParseTree::Char('a')),
        Box::new(ParseTree::Char('b'))
    );
    test_posix("ab", &r, Some(expected), "Sequence a·b");
}

#[test]
fn test_alternation() {
    let r = Regex::alt(Regex::lit('a'), Regex::lit('b'));
    test_posix("a", &r, Some(ParseTree::Left(Box::new(ParseTree::Char('a')))), "Alt a+b picks Left a");
    test_posix("b", &r, Some(ParseTree::Right(Box::new(ParseTree::Char('b')))), "Alt a+b picks Right b");
}

#[test]
fn test_alternation_longer_wins() {
    let r = Regex::alt(
        Regex::lit('a'),
        Regex::seq(Regex::lit('a'), Regex::lit('b'))
    );
    let expected = ParseTree::Right(Box::new(
        ParseTree::Pair(Box::new(ParseTree::Char('a')), Box::new(ParseTree::Char('b')))
    ));
    test_posix("ab", &r, Some(expected), "Alt (a + ab): longer 'ab' wins");
}

#[test]
fn test_star() {
    let r = Regex::star(Regex::lit('a'));
    test_posix("", &r, Some(ParseTree::Star(vec![])), "a* empty -> []");
    test_posix("a", &r, Some(ParseTree::Star(vec![ParseTree::Char('a')])), "a* one -> [a]");
    test_posix("aaa", &r, Some(ParseTree::Star(vec![ParseTree::Char('a'), ParseTree::Char('a'), ParseTree::Char('a')])), "a* three -> [a,a,a]");
}

#[test]
fn test_star_alternating() {
    let r = Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')));
    let expected = ParseTree::Star(vec![
        ParseTree::Left(Box::new(ParseTree::Char('a'))),
        ParseTree::Right(Box::new(ParseTree::Char('b'))),
    ]);
    test_posix("ab", &r, Some(expected), "(a|b)* with 'ab'");
}

#[test]
fn test_epsilon_alt_star() {
    let r = Regex::star(Regex::alt(Regex::Eps, Regex::lit('a')));
    let expected = ParseTree::Star(vec![
        ParseTree::Right(Box::new(ParseTree::Char('a')))
    ]);
    test_posix("a", &r, Some(expected), "(ε + a)* with 'a'");
}

#[test]
fn test_no_match() {
    let r = Regex::lit('a');
    test_posix("", &r, None, "Lit 'a' vs empty");
    test_posix("aa", &r, None, "Lit 'a' vs 'aa'");
}