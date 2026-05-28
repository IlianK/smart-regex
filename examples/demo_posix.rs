//! Demo: POSIX parsing with all three parsers
//!
//! Environment Variables:
//!   REGEX_PARSER    recursive (default), loop, bitcoded, all
//!   REGEX_DIAG      0=off, 1=basic, 2=diagnostic, 3=debug
//!
//! Examples:
//!   cargo run --example demo_posix
//!   REGEX_PARSER=loop cargo run --example demo_posix
//!   REGEX_PARSER=all cargo run --example demo_posix
//!   REGEX_DIAG=2 cargo run --example demo_posix

use regex_engine::types::Regex;
use regex_engine::posix::{parse_posix, flatten, ParseTree};
use regex_engine::posix::{parse_recursive, parse_loop, parse_bitcoded};

// ============================================================================
// Test Case 
// ============================================================================

struct TestCase {
    name: &'static str,
    regex: Regex,
    input: String,
    expected: Option<ParseTree>,
}

impl TestCase {
    fn new_match(name: &'static str, regex: Regex, input: &str, expected: ParseTree) -> Self {
        Self {
            name,
            regex,
            input: input.to_string(),
            expected: Some(expected),
        }
    }
    
    fn new_no_match(name: &'static str, regex: Regex, input: &str) -> Self {
        Self {
            name,
            regex,
            input: input.to_string(),
            expected: None,
        }
    }
    
    /// Run a single parser directly (for comparison mode)
    fn run_parser(&self, parser: fn(&str, &Regex) -> Option<ParseTree>, name: &str) -> Option<ParseTree> {
        let result = parser(&self.input, &self.regex);
        match &result {
            Some(tree) => {
                let flat = flatten(tree);
                println!("  {:12} | {} -> \"{}\"", name, tree, flat);
            }
            None => {
                println!("  {:12} | ✗ No match", name);
            }
        }
        result
    }
    
    /// Run all three parsers and compare results
    fn run_comparison(&self) {
        println!("\n▶ {}", self.name);
        println!("  Input: \"{}\"", self.input);
        println!();
        println!("  {:12} | {:12}", "Parser", "Result");
        println!("  {:-<12}-+-{:-<12}", "", "");
        
        let r1 = self.run_parser(parse_recursive, "RECURSIVE");
        let r2 = self.run_parser(parse_loop, "LOOP");
        let r3 = self.run_parser(parse_bitcoded, "BITCODED");
        
        let all_equal = r1 == r2 && r2 == r3;
        if all_equal {
            println!("\n  ✓ All parsers agree");
        } else {
            println!("\n  ✗ PARSERS DISAGREE!");
        }
        
        if let Some(expected) = &self.expected {
            if r1.as_ref() == Some(expected) {
                println!("  ✓ Matches expected tree");
            } else {
                println!("  ✗ Expected: {}", expected);
            }
        }
    }
    
    /// Run single parser (uses REGEX_PARSER environment variable via parse_posix)
    fn run_single(&self) {
        println!("\n▶ {}", self.name);
        println!("  Input: \"{}\"", self.input);
        
        match parse_posix(&self.input, &self.regex) {
            Some(tree) => {
                let flat = flatten(&tree);
                println!("  Parse tree: {}", tree);
                println!("  Flattened:  \"{}\"", flat);
                if let Some(expected) = &self.expected {
                    if &tree == expected {
                        println!("  ✓ OK");
                    } else {
                        println!("  ✗ TREE MISMATCH!");
                        println!("  Expected:   {}", expected);
                    }
                }
            }
            None => {
                match &self.expected {
                    Some(_) => println!("  ✗ NO MATCH (expected match!)"),
                    None => println!("  ✓ No match (as expected)"),
                }
            }
        }
    }
    
    fn run(&self) {
        let parser_env = std::env::var("REGEX_PARSER").unwrap_or_default();
        
        if parser_env == "all" {
            self.run_comparison();
        } else {
            self.run_single();
        }
    }
}

// ============================================================================
// Paper Examples (Matching)
// ============================================================================

/// Page 9-10: (a+ab)(b+ε) with "ab"
/// Expected: (Right (a, b), Right ())
fn test_paper_page_9_10() -> TestCase {
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
    
    TestCase::new_match("Page 9-10: (a+ab)(b+ε)", r, "ab", expected)
}

/// Page 3-4: (a+b+ab)* with "ab"
/// POSIX prefers [ab] over [a, b]
fn test_paper_page_3_4() -> TestCase {
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
    
    TestCase::new_match("Page 3-4: (a+b+ab)* (POSIX prefers [ab])", r, "ab", expected)
}

/// Ordering: (ab + a) with "ab" - longer match wins (left branch)
fn test_ordering_left() -> TestCase {
    let r = Regex::alt(
        Regex::seq(Regex::lit('a'), Regex::lit('b')),
        Regex::lit('a')
    );
    
    let expected = ParseTree::Left(Box::new(
        ParseTree::Pair(Box::new(ParseTree::Char('a')), Box::new(ParseTree::Char('b')))
    ));
    
    TestCase::new_match("Ordering: (ab + a) - longer match wins (left)", r, "ab", expected)
}

/// Ordering: (a + ab) with "ab" - longer match wins (right branch)
fn test_ordering_right() -> TestCase {
    let r = Regex::alt(
        Regex::lit('a'),
        Regex::seq(Regex::lit('a'), Regex::lit('b'))
    );
    
    let expected = ParseTree::Right(Box::new(
        ParseTree::Pair(Box::new(ParseTree::Char('a')), Box::new(ParseTree::Char('b')))
    ));
    
    TestCase::new_match("Ordering: (a + ab) - longer match wins (right)", r, "ab", expected)
}

/// Problematic ε* with "" - POSIX picks [] over infinite chain
fn test_problematic_epsilon_star() -> TestCase {
    let r = Regex::star(Regex::Eps);
    let expected = ParseTree::Star(vec![]);
    TestCase::new_match("Problematic ε* (POSIX picks [] over infinite chain)", r, "", expected)
}

/// Problematic (ε+a)* with "a" - POSIX picks [Right a] over ε chains
fn test_problematic_epsilon_alt_star() -> TestCase {
    let r = Regex::star(Regex::alt(Regex::Eps, Regex::lit('a')));
    let expected = ParseTree::Star(vec![
        ParseTree::Right(Box::new(ParseTree::Char('a')))
    ]);
    TestCase::new_match("Problematic (ε+a)* (POSIX picks [Right a] over ε chains)", r, "a", expected)
}

/// Injection: a* with "aaa" - produces three iterations
fn test_injection_preservation() -> TestCase {
    let r = Regex::star(Regex::lit('a'));
    let expected = ParseTree::Star(vec![
        ParseTree::Char('a'),
        ParseTree::Char('a'),
        ParseTree::Char('a'),
    ]);
    TestCase::new_match("Injection: a* with 'aaa'", r, "aaa", expected)
}

// ============================================================================
// No Match Examples (For Diagnostics)
// ============================================================================

/// Simple literal mismatch: "a" vs "b"
fn test_no_match_literal() -> TestCase {
    TestCase::new_no_match("Literal 'a' vs 'b'", Regex::lit('a'), "b")
}

/// Sequence incomplete: a·b vs "a"
fn test_no_match_sequence_incomplete() -> TestCase {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    TestCase::new_no_match("Sequence a·b vs 'a' (incomplete)", r, "a")
}

/// Wrong character in sequence: a·b vs "ac"
fn test_no_match_sequence_wrong() -> TestCase {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    TestCase::new_no_match("Sequence a·b vs 'ac' (wrong second char)", r, "ac")
}

/// Wrong character in alternation: (a|b) vs "c"
fn test_no_match_alternation() -> TestCase {
    let r = Regex::alt(Regex::lit('a'), Regex::lit('b'));
    TestCase::new_no_match("Alternation (a|b) vs 'c'", r, "c")
}

/// Epsilon alt star with wrong character: (ε+a)* vs "b"
fn test_no_match_epsilon_alt_star() -> TestCase {
    let r = Regex::star(Regex::alt(Regex::Eps, Regex::lit('a')));
    TestCase::new_no_match("(ε+a)* vs 'b'", r, "b")
}

/// Complex ambiguous no match: (ca|a|b)* vs "aabcba" (fails at position 4)
fn test_no_match_complex_ambiguous() -> TestCase {
    let r = Regex::star(Regex::alt(
        Regex::seq(Regex::lit('c'), Regex::lit('a')),
        Regex::alt(Regex::lit('a'), Regex::lit('b'))
    ));
    TestCase::new_no_match("(ca|a|b)* vs 'aabcba' (fails at position 4)", r, "aabcba")
}

/// Deep expression no match: a·a·a·a vs "aaa" (needs 4 a's, only has 3)
fn test_no_match_deep_sequence() -> TestCase {
    let r = Regex::seq(
        Regex::lit('a'),
        Regex::seq(
            Regex::lit('a'),
            Regex::seq(Regex::lit('a'), Regex::lit('a'))
        )
    );
    TestCase::new_no_match("Deep sequence a·a·a·a vs 'aaa' (too short)", r, "aaa")
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    let diag = std::env::var("REGEX_DIAG").unwrap_or_default();
    let parser_env = std::env::var("REGEX_PARSER").unwrap_or_default();
    
    println!("=========================================");
    println!("POSIX Parsing Demo");
    match parser_env.as_str() {
        "all" => println!("Mode: All Parsers (Comparison)"),
        "loop" => println!("Mode: Loop Parser"),
        "bitcoded" => println!("Mode: Bitcoded Parser"),
        _ => println!("Mode: Recursive Parser (default)"),
    }
    if !diag.is_empty() && diag != "0" {
        println!("Diagnostic level: {}", diag);
    }
    println!("=========================================");
    
    // Collect all test cases
    let tests: Vec<TestCase> = vec![
        // Paper Examples (Matching)
        test_paper_page_9_10(),
        test_paper_page_3_4(),
        test_ordering_left(),
        test_ordering_right(),
        test_problematic_epsilon_star(),
        test_problematic_epsilon_alt_star(),
        test_injection_preservation(),
        
        // No Match Examples
        test_no_match_literal(),
        test_no_match_sequence_incomplete(),
        test_no_match_sequence_wrong(),
        test_no_match_alternation(),
        test_no_match_epsilon_alt_star(),
        test_no_match_complex_ambiguous(),
        test_no_match_deep_sequence(),
    ];
    
    // Run all tests
    for test in &tests {
        test.run();
    }
    
    println!();
}