// tests/common/mod.rs

use regex_engine::{Regex, ParseTree, flatten};


// =========================================================================
// Paper Regex Examples
// =========================================================================

// (a + ab)(b + ε) - running example from flops14, p. 4
pub fn paper_r1() -> Regex {
    Regex::seq(
        Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
        Regex::alt(Regex::lit('b'), Regex::Eps),
    )
}

/// (a + (b + ab))* - POSIX star-preference example from flops14 p.1
/// Structure: Star(Alt(a, Alt(b, ab))) - nested Alt, ab is Right(Right(...))
pub fn paper_r2() -> Regex {
    Regex::star(Regex::alt(
        Regex::lit('a'),
        Regex::alt(
            Regex::lit('b'),
            Regex::seq(Regex::lit('a'), Regex::lit('b')),
        ),
    ))
}


// =========================================================================
// Assertion helpers 
// =========================================================================

#[allow(dead_code)]
pub fn assert_round_trip(tree: &ParseTree, expected_word: &str) {
    assert_eq!(
        flatten(tree), expected_word,
        "flatten(tree) should equal the matched word"
    );
}

#[allow(dead_code)]
pub fn assert_parsers_agree(
    label_a: &str,
    tree_a: &Option<ParseTree>,
    label_b: &str,
    tree_b: &Option<ParseTree>,
) {
    assert_eq!(
        tree_a, tree_b,
        "{} and {} disagree:\n  {}: {:?}\n  {}: {:?}",
        label_a, label_b, label_a, tree_a, label_b, tree_b
    );
}