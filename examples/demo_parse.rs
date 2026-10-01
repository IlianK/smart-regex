//! examples/demo_parse.rs
//!
//! POSIX ordering-rule examples (A1, A2, K1, K2) and paper examples,
//! against any one of the five parsers, or all five with `--parser all`.
//!
//! Flags: --parser <NAME>    deriv_std_rec (default), deriv_std_loop,
//!                           deriv_bc, pderiv_std, pderiv_bc, all
//!        --diag <0|1|2|3>   0=off (default), 1=basic, 2=verbose, 3=debug
//!        --diag-report PATH override output file for level 3
//!                           (default: reports/<parser>/demo_NN.txt per case)
//!                           --diag > 0 is single-parser only.

use regex_engine::types::Regex;
use regex_engine::parsers::{flatten, ParseTree, ParserType};
use regex_engine::parsers::{
    parse_deriv_std_rec, parse_deriv_std_loop, parse_deriv_bc, parse_pderiv_std, parse_pderiv_bc,
};
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

    // Single parser with diagnostics (any --parser value but "all").
    fn run_parser_with_diagnostics(&self, config: &DiagConfig, index: usize) {
        println!("\n▶ {}", self.name);

        let effective_config = if config.level == DiagLevel::Debug {
            let path = config
                .report_path
                .clone()
                .unwrap_or_else(|| {
                    format!("reports/{}/demo_{:02}.txt", config.parser_type.name(), index)
                });
            // Ensure the report's parent directory exists before handing
            // the path to run_parser, which may or may not create it.
            if let Some(parent) = std::path::Path::new(&path).parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!("could not create {}: {e}", parent.display());
                }
            }
            DiagConfig::new(config.level, config.parser_type, Some(path))
        } else {
            config.clone()
        };

        run_parser(self.pattern, &self.regex, &self.input, &effective_config);
    }

    // Run one parser, print one row; `label` is a ParserType::name() value.
    fn run_parser_row(
        &self,
        parser: fn(&str, &Regex) -> Option<ParseTree>,
        label: &str,
        family: &str,
    ) -> Option<ParseTree> {
        let result = parser(&self.input, &self.regex);
        match &result {
            Some(tree) => println!("  {:<14} | {:<6} | {:<8} | {:?}", label, family, flatten(tree), tree),
            None       => println!("  {:<14} | {:<6} | {:<8} | no match", label, family, ""),
        }
        result
    }

    fn print_table_header(&self) {
        println!("  {:<14} | {:<6} | {:<8} | {}", "parser", "family", "match", "tree");
        println!("  {:-<14}-+-{:-<6}-+-{:-<8}-+-{:-<30}", "", "", "", "");
    }

    // All five parsers in one table, then a summary block (--parser all).
    fn run_all_parsers(&self) {
        println!("\n▶ {}", self.name);
        println!("  Regex: {}   Input: {:?}", self.pattern, self.input);

        self.print_table_header();
        let rec  = self.run_parser_row(parse_deriv_std_rec,  ParserType::DerivStdRec.name(),  "POSIX");
        let loop_ = self.run_parser_row(parse_deriv_std_loop, ParserType::DerivStdLoop.name(), "POSIX");
        let bc   = self.run_parser_row(parse_deriv_bc,       ParserType::DerivBc.name(),       "POSIX");
        let pstd = self.run_parser_row(parse_pderiv_std,     ParserType::PDerivStd.name(),     "Greedy");
        let pbc  = self.run_parser_row(parse_pderiv_bc,      ParserType::PDerivBc.name(),      "Greedy");

        let posix_agree  = rec == loop_ && loop_ == bc;
        let greedy_agree = pstd == pbc;

        // Informational only: no Greedy `expected` tree exists by design --
        // Greedy's own correctness is checked in `examples/parser_agreement.rs`.
        let greedy_vs_posix = if pstd == self.expected { "agree" } else { "disagree" };

        let matched = [rec.is_some(), loop_.is_some(), bc.is_some(), pstd.is_some(), pbc.is_some()];
        let membership_agree = matched.iter().all(|&m| m == matched[0]);
        let membership = if membership_agree {
            if matched[0] { "[ok] all agree: match" } else { "[ok] all agree: no match" }
        } else {
            "[FAIL] parsers disagree on membership"
        };

        println!("\n  summary");
        println!("    membership       : {}", membership);
        println!("    POSIX parsers    : {}", if posix_agree  { "[ok] agree" } else { "[FAIL] DISAGREE" });
        println!("    Greedy parsers   : {}", if greedy_agree { "[ok] agree" } else { "[FAIL] DISAGREE" });
        println!("    Greedy vs POSIX  : {}", greedy_vs_posix);

        if !membership_agree {
            println!(
                "      {}={} {}={} {}={} {}={} {}={}",
                ParserType::DerivStdRec.name(),  matched[0],
                ParserType::DerivStdLoop.name(), matched[1],
                ParserType::DerivBc.name(),      matched[2],
                ParserType::PDerivStd.name(),    matched[3],
                ParserType::PDerivBc.name(),     matched[4],
            );
        }
    }

    fn run(&self, config: &DiagConfig, is_all: bool, index: usize) {
        if is_all {
            self.run_all_parsers();
        } else {
            self.run_parser_with_diagnostics(config, index);
        }
    }
}


// ---------------------------
// Paper Examples (Matching)
// ---------------------------

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

fn test_ordering_k1_empty_star() -> TestCase {
    TestCase::new_match(
        "POSIX K1: ε* on \"\" - zero iterations (naive greedy loops forever)",
        "ε*",
        Regex::star(Regex::Eps),
        "",
        ParseTree::Star(vec![]),
    )
}

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
// Injection preservation: a* -> [a, a, a]
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

struct Args {
    parser: String,
    diag: DiagLevel,
    diag_report: Option<String>,
}

fn parse_args() -> Args {
    let argv: Vec<String> = std::env::args().collect();
    let mut parser = "deriv_std_rec".to_string();
    let mut diag = DiagLevel::Off;
    let mut diag_report: Option<String> = None;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--parser" => {
                i += 1;
                parser = argv.get(i).cloned().unwrap_or_else(|| {
                    eprintln!(
                        "--parser needs a value (deriv_std_rec, deriv_std_loop, deriv_bc, \
                         pderiv_std, pderiv_bc, or all)"
                    );
                    std::process::exit(2);
                });
            }
            "--diag" => {
                i += 1;
                diag = match argv.get(i).map(String::as_str) {
                    Some("0") => DiagLevel::Off,
                    Some("1") => DiagLevel::Basic,
                    Some("2") => DiagLevel::Verbose,
                    Some("3") => DiagLevel::Debug,
                    _ => {
                        eprintln!("--diag needs 0, 1, 2, or 3");
                        std::process::exit(2);
                    }
                };
            }
            "--diag-report" => {
                i += 1;
                diag_report = Some(argv.get(i).cloned().unwrap_or_else(|| {
                    eprintln!("--diag-report needs a path");
                    std::process::exit(2);
                }));
            }
            other => {
                eprintln!(
                    "unknown argument {other:?}\n\
                     usage: cargo run --example demo_parse -- \
                     [--parser <NAME>] [--diag 0|1|2|3] [--diag-report <PATH>]"
                );
                std::process::exit(2);
            }
        }
        i += 1;
    }
    Args { parser, diag, diag_report }
}

fn main() {
    let args = parse_args();
    let is_all: bool = args.parser == "all";

    // --diag > 0 is single-parser only: in `all` mode the five parsers
    // are compared in one compact table, with no per-parser trace or
    // report. Refuse rather than silently ignore.
    if is_all && args.diag != DiagLevel::Off {
        eprintln!(
            "--diag {} is single-parser only; --parser all runs a comparison table with no \
             traces. Pick one parser (deriv_std_rec, deriv_std_loop, deriv_bc, pderiv_std, \
             or pderiv_bc) to get diagnostics, or drop --diag.",
            args.diag as u8
        );
        std::process::exit(2);
    }

    let parser_type = if is_all {
        // Not used when is_all, but DiagConfig still needs a value to be passed.
        ParserType::DerivStdRec
    } else {
        ParserType::parse(&args.parser).unwrap_or_else(|| {
            eprintln!(
                "unknown --parser value {:?} (expected deriv_std_rec, deriv_std_loop, deriv_bc, \
                 pderiv_std, pderiv_bc, or all)",
                args.parser
            );
            std::process::exit(2);
        })
    };
    let config = DiagConfig::new(args.diag, parser_type, args.diag_report.clone());

    println!("---------------------------------------------------");
    println!("Parsing Demo");
    if is_all {
        println!("Mode:        All Parsers (Comparison)");
    } else {
        println!("Mode:        {} parser", config.parser_type.name());
        if matches!(config.parser_type, ParserType::PDerivStd | ParserType::PDerivBc) {
            println!(
                "Note:        {} is a Greedy parser, not POSIX -- it may diverge from the POSIX \
                 behaviour the examples below are described in terms of (see --parser all)",
                config.parser_type.name()
            );
        }
    }
    println!("Diagnostics: --diag={}", args.diag as u8);
    if config.level == DiagLevel::Debug {
        let dest = config
            .report_path
            .clone()
            .unwrap_or_else(|| {
                format!("reports/{}/demo_NN.txt (per test case)", config.parser_type.name())
            });
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
        test.run(&config, is_all, index + 1);
    }

    println!();

    if config.level == DiagLevel::Debug && !is_all {
        let dest = config
            .report_path
            .clone()
            .unwrap_or_else(|| {
                format!(
                    "reports/{}/demo_01.txt .. reports/{}/demo_14.txt",
                    config.parser_type.name(),
                    config.parser_type.name(),
                )
            });
        println!("Reports written to: {}", dest);
    }
}