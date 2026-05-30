//! Demo: POSIX parsing with all three parsers
//!
//! Environment Variables:
//!   REGEX_PARSER       recursive (default), loop, bitcoded, all
//!   REGEX_DIAG         0=off (default), 1=basic, 2=verbose, 3=debug
//!   REGEX_DIAG_REPORT  (optional) override output file for Level 3
//!                      Default at Level 3: reports/demo_NN.txt per test case
//!
//! Examples:
//!   cargo run --example demo_posix
//!   REGEX_PARSER=loop       cargo run --example demo_posix
//!   REGEX_PARSER=all        cargo run --example demo_posix
//!   REGEX_DIAG=1            cargo run --example demo_posix
//!   REGEX_DIAG=2 REGEX_PARSER=bitcoded cargo run --example demo_posix
//!   REGEX_DIAG=3            cargo run --example demo_posix
//!   REGEX_DIAG=3 REGEX_DIAG_REPORT=reports/all.txt cargo run --example demo_posix

use regex_engine::types::Regex;
use regex_engine::posix::{flatten, ParseTree};
use regex_engine::posix::{parse_recursive, parse_loop, parse_bitcoded};
use regex_engine::diagnostics::{DiagConfig, DiagLevel, run_parser};

// ============================================================================
// Test Case
// ============================================================================

struct TestCase {
    /// Human-readable label shown in the demo terminal header (▶ ...)
    name:     &'static str,
    /// Actual regex pattern string shown in the Regex: field of diagnostic output
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

    // ── Comparison mode (REGEX_PARSER=all) ───────────────────────────────────

    fn run_parser_direct(
        &self,
        parser: fn(&str, &Regex) -> Option<ParseTree>,
        label: &str,
    ) -> Option<ParseTree> {
        let result = parser(&self.input, &self.regex);
        match &result {
            Some(tree) => println!("  {:12} | {} → {:?}", label, tree, flatten(tree)),
            None       => println!("  {:12} | ✗ No match", label),
        }
        result
    }

    fn run_comparison(&self) {
        println!("\n▶ {}", self.name);
        println!("  Regex: {}   Input: {:?}", self.pattern, self.input);
        println!();
        println!("  {:12} | {}", "Parser", "Result");
        println!("  {:-<12}-+-{:-<40}", "", "");

        let r1 = self.run_parser_direct(parse_recursive, "RECURSIVE");
        let r2 = self.run_parser_direct(parse_loop,      "LOOP");
        let r3 = self.run_parser_direct(parse_bitcoded,  "BITCODED");

        let all_equal = r1 == r2 && r2 == r3;
        if all_equal { println!("\n  ✓ All parsers agree"); }
        else         { println!("\n  ✗ PARSERS DISAGREE!"); }

        if let Some(ref expected) = self.expected {
            if r1.as_ref() == Some(expected) { println!("  ✓ Matches expected tree"); }
            else                             { println!("  ✗ Expected: {}", expected); }
        }
    }

    // ── Single parser mode ────────────────────────────────────────────────────

    fn run_single(&self, config: &DiagConfig, index: usize) {
        println!("\n▶ {}", self.name);

        // At Level 3 each test case gets its own report file so they do not
        // overwrite each other. Priority:
        //   1. REGEX_DIAG_REPORT env var (user override — single shared file)
        //   2. reports/demo_NN.txt       (default per-test file)
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

        // Pass self.pattern (the actual regex string) not self.name (the label)
        run_parser(self.pattern, &self.regex, &self.input, &effective_config);
    }

    fn run(&self, config: &DiagConfig, index: usize) {
        let parser_env = std::env::var("REGEX_PARSER").unwrap_or_default();
        if parser_env == "all" {
            self.run_comparison();
        } else {
            self.run_single(config, index);
        }
    }
}

// ============================================================================
// Paper Examples (Matching)
// ============================================================================

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

/// POSIX A1 left preference: Alt(Eps, Lit) — left branch nullable → Left wins
fn test_ordering_left() -> TestCase {
    let r = Regex::star(Regex::alt(Regex::Eps, Regex::lit('a')));
    TestCase::new_match(
        "POSIX left preference: (ε+a)* on \"\"",
        "(ε+a)*",
        r,
        "",
        ParseTree::Star(vec![]),
    )
}

/// POSIX A2 right fallback: Alt(Lit, Eps) — left not nullable → Right used
fn test_ordering_right() -> TestCase {
    let r = Regex::star(Regex::alt(Regex::lit('a'), Regex::Eps));
    TestCase::new_match(
        "POSIX right fallback: (a+ε)* on \"\"",
        "(a+ε)*",
        r,
        "",
        ParseTree::Star(vec![]),
    )
}

/// Problematic expression: ε* — Star of empty word
fn test_problematic_epsilon_star() -> TestCase {
    TestCase::new_match(
        "Problematic: ε* on \"\"",
        "ε*",
        Regex::star(Regex::Eps),
        "",
        ParseTree::Star(vec![]),
    )
}

/// Problematic expression: (ε+a)* — infinite empty chain risk
fn test_problematic_epsilon_alt_star() -> TestCase {
    let r = Regex::star(Regex::alt(Regex::Eps, Regex::lit('a')));
    TestCase::new_match(
        "Problematic: (ε+a)* on \"a\"",
        "(ε+a)*",
        r,
        "a",
        ParseTree::Star(vec![ParseTree::Right(Box::new(ParseTree::Char('a')))]),
    )
}

/// Injection preservation: verify inject correctly rebuilds a* → [a, a, a]
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

// ============================================================================
// No Match Examples
// ============================================================================

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

// ============================================================================
// Main
// ============================================================================

fn main() {
    let config     = DiagConfig::read_from_env();
    let parser_env = std::env::var("REGEX_PARSER").unwrap_or_default();

    println!("=========================================");
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
    println!("=========================================");

    let tests: Vec<TestCase> = vec![
        // Paper examples (matching)
        test_paper_page_9_10(),
        test_paper_page_3_4(),
        test_ordering_left(),
        test_ordering_right(),
        test_problematic_epsilon_star(),
        test_problematic_epsilon_alt_star(),
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