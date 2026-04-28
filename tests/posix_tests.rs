//! Tests for POSIX parser with clear output

use regex_engine::basic::Regex;
use regex_engine::posix::{parse_posix, flatten, mk_eps};

/// Helper to format Regex for display (simplified)
fn fmt_regex(r: &Regex) -> String {
    match r {
        Regex::Phi => "∅".to_string(),
        Regex::Eps => "ε".to_string(),
        Regex::Lit(c) => format!("'{}'", c),
        Regex::Seq(r1, r2) => format!("({}·{})", fmt_regex(r1), fmt_regex(r2)),
        Regex::Alt(r1, r2) => format!("({}+{})", fmt_regex(r1), fmt_regex(r2)),
        Regex::Star(r1) => format!("({})*", fmt_regex(r1)),
    }
}

/// Clean POSIX test function (like demo_posix_parse but for tests)
fn test_posix(input: &str, r: &Regex, expected: Option<&str>, label: &str) {
    println!("\n▶ {}", label);
    println!("  Regex: {}", fmt_regex(r));
    println!("  Input: \"{}\"", input);
    
    match parse_posix(input, r) {
        Some(tree) => {
            println!("  Parse tree: {}", tree);
            println!("  Output:     \"{}\"", flatten(&tree));
            if let Some(exp) = expected {
                assert_eq!(flatten(&tree), exp);
                println!("  True");
            }
        }
        None => {
            println!("  False");
            if expected.is_some() {
                panic!("Expected match but got None for: {}", label);
            }
        }
    }
}

/// Clean mk_eps test function
fn test_mk_eps(r: &Regex, label: &str) {
    println!("\n▶ mkEps: {}", label);
    println!("  Regex: {}", fmt_regex(r));
    let tree = mk_eps(r);
    println!("  Parse tree: {}", tree);
    println!("  Output:     \"{}\"", flatten(&tree));
}

// ============================================================================
// MK_EPS TESTS
// ============================================================================

#[test]
fn test_mk_eps_on_eps() {
    test_mk_eps(&Regex::Eps, "ε");
    let tree = mk_eps(&Regex::Eps);
    assert!(matches!(tree, regex_engine::posix::ParseTree::Empty));
    assert_eq!(flatten(&tree), "");
}

#[test]
fn test_mk_eps_on_star() {
    let r = Regex::star(Regex::lit('a'));
    test_mk_eps(&r, "a*");
    let tree = mk_eps(&r);
    assert!(matches!(tree, regex_engine::posix::ParseTree::Star(ref v) if v.is_empty()));
    assert_eq!(flatten(&tree), "");
}

#[test]
fn test_mk_eps_on_alt() {
    // (ε + a) - left branch is nullable
    let r = Regex::alt(Regex::Eps, Regex::lit('a'));
    test_mk_eps(&r, "ε + a");
    let tree = mk_eps(&r);
    assert!(matches!(tree, regex_engine::posix::ParseTree::Left(_)));
    assert_eq!(flatten(&tree), "");
}

#[test]
fn test_mk_eps_on_alt_right() {
    // (a + ε) - left is NOT nullable
    let r = Regex::alt(Regex::lit('a'), Regex::Eps);
    test_mk_eps(&r, "a + ε");
    let tree = mk_eps(&r);
    assert!(matches!(tree, regex_engine::posix::ParseTree::Right(_)));
    assert_eq!(flatten(&tree), "");
}

// ============================================================================
// PARSE_POSIX TESTS
// ============================================================================

#[test]
fn test_parse_posix_literal() {
    let r = Regex::lit('a');
    test_posix("a", &r, Some("a"), "Literal");
    let tree = parse_posix("a", &r).unwrap();
    assert!(matches!(tree, regex_engine::posix::ParseTree::Char('a')));
}

#[test]
fn test_parse_posix_empty() {
    let r = Regex::Eps;
    test_posix("", &r, Some(""), "Empty string");
}

#[test]
fn test_parse_posix_sequence() {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    test_posix("ab", &r, Some("ab"), "Sequence");
}

#[test]
fn test_parse_posix_star_empty() {
    let r = Regex::star(Regex::lit('a'));
    test_posix("", &r, Some(""), "Star (empty)");
}

#[test]
fn test_parse_posix_star_multiple() {
    let r = Regex::star(Regex::lit('a'));
    test_posix("aaa", &r, Some("aaa"), "Star (three a's)");
}

#[test]
fn test_parse_posix_star_alternating() {
    let r = Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')));
    test_posix("ab", &r, Some("ab"), "(a|b)*");
}

#[test]
fn test_parse_posix_running_example() {
    let r = Regex::seq(
        Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
        Regex::alt(Regex::lit('b'), Regex::lit('c')),
    );
    test_posix("ab", &r, Some("ab"), "Running Example (a+ab)(b+c)");
}

#[test]
fn test_parse_posix_ambiguous_star() {
    let r = Regex::star(Regex::alt(
        Regex::lit('a'),
        Regex::alt(Regex::lit('b'), Regex::seq(Regex::lit('a'), Regex::lit('b')))
    ));
    test_posix("ab", &r, Some("ab"), "(a+b+ab)* ");
}

#[test]
fn test_parse_posix_longest_match() {
    let r = Regex::star(Regex::lit('a'));
    test_posix("aaa", &r, Some("aaa"), "Longest match");
}

#[test]
fn test_parse_posix_epsilon_star() {
    let r = Regex::star(Regex::Eps);
    test_posix("", &r, Some(""), "ε*");
}

#[test]
fn test_parse_posix_epsilon_alt_star() {
    let r = Regex::star(Regex::alt(Regex::Eps, Regex::lit('a')));
    test_posix("a", &r, Some("a"), "(ε + a)*");
}

#[test]
fn test_parse_posix_no_match() {
    let r = Regex::lit('a');
    println!("\n▶ No match tests:");
    test_posix("b", &r, None, "  'a' vs 'b'");
    test_posix("", &r, None, "  'a' vs ''");
    test_posix("aa", &r, None, "  'a' vs 'aa'");
}

// ============================================================================
// FLATTEN TESTS
// ============================================================================

#[test]
fn test_flatten_trees() {
    use regex_engine::posix::ParseTree;
    
    println!("\n▶ Flatten tests:");
    
    let tree1 = ParseTree::Star(vec![
        ParseTree::Char('a'),
        ParseTree::Char('b'),
        ParseTree::Char('c'),
    ]);
    println!("  Star [a,b,c] → \"{}\"", flatten(&tree1));
    assert_eq!(flatten(&tree1), "abc");
    
    let tree2 = ParseTree::Pair(
        Box::new(ParseTree::Char('a')),
        Box::new(ParseTree::Pair(
            Box::new(ParseTree::Char('b')),
            Box::new(ParseTree::Char('c')),
        )),
    );
    println!("  Pair(a, Pair(b,c)) → \"{}\"", flatten(&tree2));
    assert_eq!(flatten(&tree2), "abc");
    
    let tree3 = ParseTree::Left(Box::new(ParseTree::Char('x')));
    println!("  Left(x) → \"{}\"", flatten(&tree3));
    assert_eq!(flatten(&tree3), "x");
    
    let tree4 = ParseTree::Right(Box::new(ParseTree::Char('y')));
    println!("  Right(y) → \"{}\"", flatten(&tree4));
    assert_eq!(flatten(&tree4), "y");
}

