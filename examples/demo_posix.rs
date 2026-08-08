//! regex-engine/examples/demo_posix.rs
//! 
//! Demo: POSIX parsing with all three parsers
//!
//! ENVs:
//!   REGEX_PARSER       recursive (default), loop, bitcoded, all
//!   REGEX_DIAG         0=off (default), 1=basic, 2=verbose, 3=debug
//!   REGEX_DIAG_REPORT  (optional) override output file for Level 3
//!                      Default at Level 3: reports/demo_NN.txt per test case
//!
//! Examples:
//!   cargo run --example demo_posix
//!   REGEX_PARSER=loop                    
//!   REGEX_PARSER=all                      
//!   REGEX_DIAG=1                          
//!   REGEX_DIAG=2 REGEX_PARSER=bitcoded    
//!   REGEX_DIAG=3                         

use regex_engine::types::Regex;
use regex_engine::posix::{flatten, ParseTree};
use regex_engine::posix::{parse_recursive, parse_loop, parse_bitcoded};
use regex_engine::diagnostics::{DiagConfig, DiagLevel, run_parser};

// -------------------------------
// Test Case Structure
// -------------------------------

struct TestCase {
    name:     &'static str,
    pattern:  &'static str,
    regex:    Regex,
    input:    String,
    expected: Option<ParseTree>,
}

impl TestCase {
    fn new_match(
        name: &'static str,
        pattern: &'static str,
        regex: Regex,
        input: &str,
        expected: ParseTree,
    ) -> Self {
        Self { name, pattern, regex, input: input.to_string(), expected: Some(expected) }
    }

    fn new_no_match(
        name: &'static str,
        pattern: &'static str,
        regex: Regex,
        input: &str,
    ) -> Self {
        Self { name, pattern, regex, input: input.to_string(), expected: None }
    }

    // Run single parser with diagnostics (REGEX_PARSER=recursive|loop|bitcoded)
    fn run_parser_with_diagnostics(&self, config: &DiagConfig, index: usize) {
        println!("\n▶ {}", self.name);

        let effective_config = if config.level == DiagLevel::Debug {
            let path = std::env::var("REGEX_DIAG_REPORT")
                .unwrap_or_else(|_| format!("reports/demo_{:02}.txt", index));
            DiagConfig {
                level:       config.level,
                parser_type: config.parser_type,
                report_path: Some(path),
            }
        } else {
            config.clone()
        };

        run_parser(self.pattern, &self.regex, &self.input, &effective_config);
    }

    // Run single parser without diagnostics(REGEX_PARSER=recursive|loop|bitcoded) 
    fn run_parser_without_diag(
        &self,
        parser: fn(&str, &Regex) -> Option<ParseTree>,
        label: &str,
    ) -> Option<ParseTree> {
        let result = parser(&self.input, &self.regex);
        match &result {
            Some(tree) => println!("  {:12} | {} -> {:?}", label, tree, flatten(tree)),
            None       => println!("  {:12} | ✗ No match", label),
        }
        result
    }

    // Run all parsers for comparison (REGEX_PARSER=all) 
    fn run_all_parsers(&self) {
        println!("\n▶ {}", self.name);
        println!("  Regex: {}   Input: {:?}", self.pattern, self.input);
        println!();
        println!("  {:12} | {}", "Parser", "Result");
        println!("  {:-<12}-+-{:-<40}", "", "");

        let r1 = self.run_parser_without_diag(parse_recursive, "RECURSIVE");
        let r2 = self.run_parser_without_diag(parse_loop,      "LOOP");
        let r3 = self.run_parser_without_diag(parse_bitcoded,  "BITCODED");

        let all_equal = r1 == r2 && r2 == r3;
        if all_equal { println!("\n  ✓ All parsers agree"); }
        else         { println!("\n  ✗ PARSERS DISAGREE!"); }

        // Check no-match expectation
        if self.expected.is_none() && r1.is_some() {
            println!("  ✗ Expected no match but got: {}", r1.as_ref().unwrap());
        }

        // Check match expectation
        if let Some(ref expected) = self.expected {
            if r1.as_ref() == Some(expected) { println!("  ✓ Matches expected tree"); }
            else                             { println!("  ✗ Expected: {}", expected); }
        }
    }

    // Run either all parsers or single parser with diagnostics 
    fn run(&self, config: &DiagConfig, index: usize) {
        let parser_env = std::env::var("REGEX_PARSER").unwrap_or_default();
        if parser_env == "all" {
            self.run_all_parsers
    ();
        } else {
            self.run_parser_with_diagnostics(config, index);
        }
    }
}


// ---------------------------
// Paper Examples (Matching)
// ---------------------------

/// flops14 p.9-10: (a+ab)(b+ε) on "ab"
/// POSIX result: (Right (a, b), Right ())
fn test_paper_page_9_10() -> TestCase {
    let r = Regex::seq(
        Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
        Regex::alt(Regex::lit('b'), Regex::Eps),
    );
    let expected = ParseTree::Pair(
        Box::new(ParseTree::Right(Box::new(
            ParseTree::Pair(
                Box::new(ParseTree::Char('a')),
                Box::new(ParseTree::Char('b')),
            )
        ))),
        Box::new(ParseTree::Right(Box::new(ParseTree::Empty))),
    );
    TestCase::new_match(
        "Page 9-10: (a+ab)(b+ε) on \"ab\"",
        "(a+ab)(b+ε)",
        r,
        "ab",
        expected,
    )
}

/// flops14 p.3-4: (a+b+ab)* on "ab"
/// POSIX prefers [ab] (one iteration) over [a, b] (two iterations)
fn test_paper_page_3_4() -> TestCase {
    let r = Regex::star(Regex::alt(
        Regex::lit('a'),
        Regex::alt(Regex::lit('b'), Regex::seq(Regex::lit('a'), Regex::lit('b')))
    ));
    let expected = ParseTree::Star(vec![
        ParseTree::Right(Box::new(ParseTree::Right(Box::new(
            ParseTree::Pair(
                Box::new(ParseTree::Char('a')),
                Box::new(ParseTree::Char('b')),
            )
        ))))
    ]);
    TestCase::new_match(
        "Page 3-4: (a+b+ab)* on \"ab\" (POSIX prefers [ab])",
        "(a+b+ab)*",
        r,
        "ab",
        expected,
    )
}

// -------------------------------
// Ordering Rules: A1, A2, K1, K2
// -------------------------------

/// A1: when right alternative matches a strictly
/// longer string than the left, right is preferred over left
///
/// (a + aa)* on "aa" has two valid parses:
///   Greedy (left-first):  Star([Left(a), Left(a)])  - takes 'a' twice
///   POSIX  (A1):          Star([Right(aa)])         - takes 'aa' in one iteration
fn test_ordering_a1_longer_right_wins() -> TestCase {
    let r = Regex::star(Regex::alt(
        Regex::lit('a'),
        Regex::seq(Regex::lit('a'), Regex::lit('a')),
    ));
    TestCase::new_match(
        "POSIX A1: (a+aa)* on \"aa\" - right wins, matches longer (greedy picks [a,a])",
        "(a+aa)*",
        r,
        "aa",
        ParseTree::Star(vec![
            ParseTree::Right(Box::new(
                ParseTree::Pair(Box::new(ParseTree::Char('a')), Box::new(ParseTree::Char('a')))
            ))
        ]),
    )
}

/// A2: when both alternatives match strings of equal
/// length, the left alternative is preferred
///
/// (a+a) on "a": both Left and Right match "a" (length 1).
///     POSIX A2 (tiebreaker): left wins -> Left(Char('a'))
///     Greedy also picks left here (but doesn't know it's a tie, just picks left first)
fn test_ordering_a2_left_tiebreaker() -> TestCase {
    let r = Regex::alt(Regex::lit('a'), Regex::lit('a'));
    TestCase::new_match(
        "POSIX A2: (a+a) on \"a\" - equal length, left wins (tiebreaker)",
        "(a+a)",
        r,
        "a",
        ParseTree::Left(Box::new(ParseTree::Char('a'))),
    )
}

/// K1: a Kleene star always accepts zero iterations.
/// ε* on "" - the only valid parse is the empty star
///
/// This is "problematic" for naive greedy implementations because
/// Star(Eps) could loop infinitely taking ε at each iteration
/// 
/// K1 resolves this: [] >_{r*} v:vs (empty list beats any non-empty list
/// when the inner expression matches only ε). Result is always Star([])
fn test_ordering_k1_empty_star() -> TestCase {
    TestCase::new_match(
        "POSIX K1: ε* on \"\" - zero iterations (naive greedy loops forever)",
        "ε*",
        Regex::star(Regex::Eps),
        "",
        ParseTree::Star(vec![]),
    )
}

/// K2: when the inner expression is nullable, prefer non-empty star iterations over empty ones.
///     (ε+a)* on "a" - the inner Alt(ε, a) is nullable.
///     This is the "problematic expression" case discussed in greedy.pdf 1.3 and flops14 2.
///
/// Regex: (ε+a)*   Input: "a"
///     Naive greedy (left-first)       non-termination     - loops on ε forever
///     Xi [Xi01] no-progress fix       no full match       - cuts loop, "a" unconsumed
///     Harper/Kearns [Har99,Kea91]     depends on rewrite  - (ε+a)* -> a*, then match a* Star([a]) 
///     Greedy axiomatised (F&C)        full match          - Star([Left(()), Right(a)])
///         - reactive: runs left-first, detects empty loop, patches it (two iterations)
///     POSIX (flops14 Prop.1)          full match          - Star([Right(a)])
///         - proactive: evaluates all branches globally, never enters the loop (one iteration)
fn test_ordering_k2_nonempty_preferred() -> TestCase {
    let r = Regex::star(Regex::alt(Regex::Eps, Regex::lit('a')));
    TestCase::new_match(
        "Problematic (ε+a)* on \"a\": POSIX->[Right(a)]  greedy->[Left(),Right(a)]  Xi->no match",
        "(ε+a)*",
        r,
        "a",
        ParseTree::Star(vec![
            ParseTree::Right(Box::new(ParseTree::Char('a')))
        ]),
    )
}

// -------------------------------
// Injection preservation: verify inject correctly rebuilds a* -> [a, a, a]
// -------------------------------

fn test_injection_preservation() -> TestCase {
    TestCase::new_match(
        "Injection: a* on \"aaa\"",
        "a*",
        Regex::star(Regex::lit('a')),
        "aaa",
        ParseTree::Star(vec![
            ParseTree::Char('a'),
            ParseTree::Char('a'),
            ParseTree::Char('a'),
        ]),
    )
}

// -------------------------------
// No Match Examples
// -------------------------------

fn test_no_match_literal() -> TestCase {
    TestCase::new_no_match(
        "No match: a vs \"b\"",
        "a",
        Regex::lit('a'),
        "b",
    )
}

fn test_no_match_sequence_incomplete() -> TestCase {
    TestCase::new_no_match(
        "No match: a·b vs \"a\" (incomplete)",
        "a·b",
        Regex::seq(Regex::lit('a'), Regex::lit('b')),
        "a",
    )
}

fn test_no_match_sequence_wrong() -> TestCase {
    TestCase::new_no_match(
        "No match: a·b vs \"ac\" (wrong second char)",
        "a·b",
        Regex::seq(Regex::lit('a'), Regex::lit('b')),
        "ac",
    )
}

fn test_no_match_alternation() -> TestCase {
    TestCase::new_no_match(
        "No match: (a+b) vs \"c\"",
        "(a+b)",
        Regex::alt(Regex::lit('a'), Regex::lit('b')),
        "c",
    )
}

fn test_no_match_epsilon_alt_star() -> TestCase {
    TestCase::new_no_match(
        "No match: (ε+a)* vs \"b\"",
        "(ε+a)*",
        Regex::star(Regex::alt(Regex::Eps, Regex::lit('a'))),
        "b",
    )
}

fn test_no_match_complex_ambiguous() -> TestCase {
    TestCase::new_no_match(
        "No match: (ca+a+b)* vs \"aabcba\" (fails at pos 4)",
        "(ca+a+b)*",
        Regex::star(Regex::alt(
            Regex::seq(Regex::lit('c'), Regex::lit('a')),
            Regex::alt(Regex::lit('a'), Regex::lit('b')),
        )),
        "aabcba",
    )
}

fn test_no_match_deep_sequence() -> TestCase {
    TestCase::new_no_match(
        "No match: a·a·a·a vs \"aaa\" (too short)",
        "a·a·a·a",
        Regex::seq(
            Regex::lit('a'),
            Regex::seq(
                Regex::lit('a'),
                Regex::seq(Regex::lit('a'), Regex::lit('a')),
            ),
        ),
        "aaa",
    )
}

// -------------------------------
// Main
// -------------------------------

fn main() {
    let config     = DiagConfig::read_from_env();
    let parser_env = std::env::var("REGEX_PARSER").unwrap_or_default();

    println!("---------------------------------------------------");
    println!("POSIX Parsing Demo");
    match parser_env.as_str() {
        "all"      => println!("Mode:        All Parsers (Comparison)"),
        "loop"     => println!("Mode:        Loop Parser"),
        "bitcoded" => println!("Mode:        Bitcoded Parser"),
        _          => println!("Mode:        Recursive Parser (default)"),
    }
    println!(
        "Diagnostics: REGEX_DIAG={}",
        std::env::var("REGEX_DIAG").unwrap_or_else(|_| "0".into())
    );
    if config.level == DiagLevel::Debug {
        let dest = std::env::var("REGEX_DIAG_REPORT")
            .unwrap_or_else(|_| "reports/demo_NN.txt (per test case)".into());
        println!("Report:      {}", dest);
    }
    println!("---------------------------------------------------");

    let tests: Vec<TestCase> = vec![
        // Paper examples (matching)
        test_paper_page_9_10(),
        test_paper_page_3_4(),

        // Ordering rules A1, A2, K1, K2
        test_ordering_a1_longer_right_wins(),
        test_ordering_a2_left_tiebreaker(),
        test_ordering_k1_empty_star(),
        test_ordering_k2_nonempty_preferred(),  
        
        // Injection preservation
        test_injection_preservation(),
        
        // No match examples
        test_no_match_literal(),
        test_no_match_sequence_incomplete(),
        test_no_match_sequence_wrong(),
        test_no_match_alternation(),
        test_no_match_epsilon_alt_star(),
        test_no_match_complex_ambiguous(),
        test_no_match_deep_sequence(),
    ];

    for (index, test) in tests.iter().enumerate() {
        test.run(&config, index + 1);
    }

    println!();

    if config.level == DiagLevel::Debug && parser_env != "all" {
        let dest = std::env::var("REGEX_DIAG_REPORT")
            .unwrap_or_else(|_| "reports/demo_01.txt .. reports/demo_14.txt".into());
        println!("Reports written to: {}", dest);
    }
}