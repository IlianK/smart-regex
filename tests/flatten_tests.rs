//! Tests for flatten function (parse tree to string conversion)

use regex_engine::posix::{flatten, ParseTree};

fn test_flatten(tree: ParseTree, expected: &str, label: &str) {
    println!("  - {} -> \"{}\"", label, flatten(&tree));
    assert_eq!(flatten(&tree), expected);
}


// BASIC FLATTEN TESTS
#[test]
fn test_flatten_empty() {
    println!("\n▶ Flatten: Empty");
    test_flatten(ParseTree::Empty, "", "Empty");
    println!();
}

#[test]
fn test_flatten_char() {
    println!("\n▶ Flatten: Char");
    test_flatten(ParseTree::Char('a'), "a", "Char('a')");
    test_flatten(ParseTree::Char('x'), "x", "Char('x')");
    println!();
}


// PAIR FLATTEN TESTS
#[test]
fn test_flatten_pair() {
    println!("\n▶ Flatten: Pair");
    
    let tree = ParseTree::Pair(
        Box::new(ParseTree::Char('a')),
        Box::new(ParseTree::Char('b')),
    );
    test_flatten(tree, "ab", "Pair(a,b)");
    
    let tree = ParseTree::Pair(
        Box::new(ParseTree::Char('a')),
        Box::new(ParseTree::Pair(
            Box::new(ParseTree::Char('b')),
            Box::new(ParseTree::Char('c')),
        )),
    );
    test_flatten(tree, "abc", "Pair(a, Pair(b,c))");
    println!();
}


// ALTERNATION FLATTEN TESTS
#[test]
fn test_flatten_alt() {
    println!("\n▶ Flatten: Alternation");
    
    let tree = ParseTree::Left(Box::new(ParseTree::Char('x')));
    test_flatten(tree, "x", "Left(x)");
    
    let tree = ParseTree::Right(Box::new(ParseTree::Char('y')));
    test_flatten(tree, "y", "Right(y)");
    println!();
}


// STAR FLATTEN TESTS
#[test]
fn test_flatten_star() {
    println!("\n▶ Flatten: Star");
    
    let tree = ParseTree::Star(vec![]);
    test_flatten(tree, "", "Star([])");
    
    let tree = ParseTree::Star(vec![
        ParseTree::Char('a'),
        ParseTree::Char('b'),
        ParseTree::Char('c'),
    ]);
    test_flatten(tree, "abc", "Star([a,b,c])");
    println!();
}


// NESTED FLATTEN TESTS
#[test]
fn test_flatten_nested() {
    println!("\n▶ Flatten: Nested structures");
    
    let tree = ParseTree::Star(vec![
        ParseTree::Pair(Box::new(ParseTree::Char('a')), Box::new(ParseTree::Char('b'))),
        ParseTree::Pair(Box::new(ParseTree::Char('c')), Box::new(ParseTree::Char('d'))),
    ]);
    test_flatten(tree, "abcd", "Star([Pair(a,b), Pair(c,d)])");
    
    let tree = ParseTree::Pair(
        Box::new(ParseTree::Star(vec![ParseTree::Char('a'), ParseTree::Char('b')])),
        Box::new(ParseTree::Char('c')),
    );
    test_flatten(tree, "abc", "Pair(Star([a,b]), c)");
    println!();
}