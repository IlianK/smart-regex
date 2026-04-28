//! Tests verifying examples from flops14-extended.pdf

use regex_engine::basic::Regex;
use regex_engine::posix::{parse_posix, flatten, ParseTree};
use regex_engine::posix::tests::fmt_regex;

fn test_paper_example(input: &str, r: &Regex, expected_tree: ParseTree, name: &str) {
    println!("\n▶ {}", name);
    println!("  - Regex: {}", fmt_regex(r));
    println!("  - Input: \"{}\"", input);
    
    let tree = parse_posix(input, r).expect("Should match");
    let flat = flatten(&tree);
    
    println!("  - Expected:   {}", expected_tree);
    println!("  - Parse tree: {}", tree);
    println!("  - Flattened:  \"{}\"\n", flat);

    assert_eq!(tree, expected_tree);
}

// EXAMPLE FROM PAGE 9-10
#[test]
fn test_paper_example_page_9_10() {
    let r = Regex::seq(
        Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
        Regex::alt(Regex::lit('b'), Regex::Eps),
    );
    
    let expected = ParseTree::Pair(
        Box::new(ParseTree::Right(Box::new(
            ParseTree::Pair(Box::new(ParseTree::Char('a')), Box::new(ParseTree::Char('b')))
        ))),
        Box::new(ParseTree::Right(Box::new(ParseTree::Empty))),
    );
    
    test_paper_example("ab", &r, expected, "Page 9-10: (a+ab)(b+ε)");
}

// EXAMPLE FROM PAGE 3-4
#[test]
fn test_paper_example_page_3_4() {
    let r = Regex::star(Regex::alt(
        Regex::lit('a'),
        Regex::alt(Regex::lit('b'), Regex::seq(Regex::lit('a'), Regex::lit('b')))
    ));
    
    let expected = ParseTree::Star(vec![
        ParseTree::Right(Box::new(
            ParseTree::Right(Box::new(
                ParseTree::Pair(Box::new(ParseTree::Char('a')), Box::new(ParseTree::Char('b')))
            ))
        ))
    ]);
    
    test_paper_example("ab", &r, expected, "Page 3-4: (a+b+ab)*");
}

// ORDERING RULES
#[test]
fn test_paper_example_ordering_left() {
    let r = Regex::alt(
        Regex::seq(Regex::lit('a'), Regex::lit('b')),
        Regex::lit('a')
    );
    
    let expected = ParseTree::Left(Box::new(
        ParseTree::Pair(Box::new(ParseTree::Char('a')), Box::new(ParseTree::Char('b')))
    ));
    
    test_paper_example("ab", &r, expected, "Ordering: (ab + a)");
}

#[test]
fn test_paper_example_ordering_right() {
    let r = Regex::alt(
        Regex::lit('a'),
        Regex::seq(Regex::lit('a'), Regex::lit('b'))
    );
    
    let expected = ParseTree::Right(Box::new(
        ParseTree::Pair(Box::new(ParseTree::Char('a')), Box::new(ParseTree::Char('b')))
    ));
    
    test_paper_example("ab", &r, expected, "Ordering: (a + ab)");
}

// PROBLEMATIC EXPRESSIONS
#[test]
fn test_paper_example_epsilon_star() {
    let r = Regex::star(Regex::Eps);
    let expected = ParseTree::Star(vec![]);
    test_paper_example("", &r, expected, "ε*");
}

#[test]
fn test_paper_example_epsilon_alt_star() {
    let r = Regex::star(Regex::alt(Regex::Eps, Regex::lit('a')));
    let expected = ParseTree::Star(vec![
        ParseTree::Right(Box::new(ParseTree::Char('a')))
    ]);
    test_paper_example("a", &r, expected, "(ε + a)*");
}

// INJECTION
#[test]
fn test_paper_example_injection() {
    let r = Regex::star(Regex::lit('a'));
    let expected = ParseTree::Star(vec![
        ParseTree::Char('a'),
        ParseTree::Char('a'),
        ParseTree::Char('a'),
    ]);
    test_paper_example("aaa", &r, expected, "a* injection");
}