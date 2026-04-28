//! Tests for POSIX parser

use regex_engine::basic::Regex;
use regex_engine::posix::{parse_posix, flatten, mk_eps};

#[test]
fn test_mk_eps_on_eps() {
    let tree = mk_eps(&Regex::Eps);
    assert!(matches!(tree, regex_engine::posix::ParseTree::Empty));
    assert_eq!(flatten(&tree), "");
}

#[test]
fn test_mk_eps_on_star() {
    let r = Regex::star(Regex::lit('a'));
    let tree = mk_eps(&r);
    // Use ref to avoid moving the value
    assert!(matches!(tree, regex_engine::posix::ParseTree::Star(ref v) if v.is_empty()));
    assert_eq!(flatten(&tree), "");
}

#[test]
fn test_mk_eps_on_alt() {
    // (eps + a) - left branch is nullable
    let r = Regex::alt(Regex::Eps, Regex::lit('a'));
    let tree = mk_eps(&r);
    assert!(matches!(tree, regex_engine::posix::ParseTree::Left(_)));
    assert_eq!(flatten(&tree), "");
}

#[test]
fn test_parse_posix_literal() {
    let r = Regex::lit('a');
    let tree = parse_posix("a", &r).unwrap();
    assert_eq!(flatten(&tree), "a");
    assert!(matches!(tree, regex_engine::posix::ParseTree::Char('a')));
}

#[test]
fn test_parse_posix_empty() {
    let r = Regex::Eps;
    let tree = parse_posix("", &r).unwrap();
    assert_eq!(flatten(&tree), "");
}

#[test]
fn test_parse_posix_sequence() {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    let tree = parse_posix("ab", &r).unwrap();
    assert_eq!(flatten(&tree), "ab");
}


#[test]
fn test_parse_posix_star() {
    let r = Regex::star(Regex::lit('a'));
    
    let tree = parse_posix("", &r).unwrap();
    assert!(matches!(tree, regex_engine::posix::ParseTree::Star(ref v) if v.is_empty()));
    
    let tree = parse_posix("aaa", &r).unwrap();
    match &tree {
        regex_engine::posix::ParseTree::Star(v) => assert_eq!(v.len(), 3),
        _ => panic!("Expected Star"),
    }
    assert_eq!(flatten(&tree), "aaa");
}

#[test]
fn test_parse_posix_running_example() {
    // (a + ab)(b + c) with "ab"
    let r = Regex::seq(
        Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
        Regex::alt(Regex::lit('b'), Regex::lit('c')),
    );
    
    let tree = parse_posix("ab", &r);
    assert!(tree.is_some());
    let tree = tree.unwrap();
    assert_eq!(flatten(&tree), "ab");
}

#[test]
fn test_parse_posix_no_match() {
    let r = Regex::lit('a');
    assert!(parse_posix("b", &r).is_none());
    assert!(parse_posix("", &r).is_none());
    assert!(parse_posix("aa", &r).is_none());
}

#[test]
fn test_flatten_various_trees() {
    use regex_engine::posix::ParseTree;
    
    let tree = ParseTree::Star(vec![
        ParseTree::Char('a'),
        ParseTree::Char('b'),
        ParseTree::Char('c'),
    ]);
    assert_eq!(flatten(&tree), "abc");
    
    let tree = ParseTree::Pair(
        Box::new(ParseTree::Char('a')),
        Box::new(ParseTree::Pair(
            Box::new(ParseTree::Char('b')),
            Box::new(ParseTree::Char('c')),
        )),
    );
    assert_eq!(flatten(&tree), "abc");
}


#[test]
fn test_debug_sequence() {
    use regex_engine::basic::Regex;
    use regex_engine::posix::{parse_posix, flatten};
    
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    println!("\n=== Debug: Testing a·b with 'ab' ===");
    let tree = parse_posix("ab", &r);
    println!("Result: {:?}", tree);
    if let Some(t) = tree {
        println!("Flattened: {}", flatten(&t));
    }
}
