//! regex-engine/src/cli/parser.rs
//!
//! Parser command logic
//! 
use regex_engine::diagnostics::{DiagConfig, DiagLevel, run_parser};
use regex_engine::matchers::MatcherType;
use regex_engine::{parse_recursive, parse_loop, parse_bitcoded, parse_pderiv_bc, flatten};
use regex_engine::parsers::ParserType;
use regex_engine::types::ParseTree;
use super::input::parse_regex_string;

// Runs with chosen parser
pub fn run_parse_single(
    regex_str: &str,
    input: &str,
    parser: ParserType,
    diag: DiagLevel,
    diag_report: Option<String>,
) {
    let r = match parse_regex_string(regex_str) {
        Ok(r)  => r,
        Err(e) => { eprintln!("Regex parse error: {}", e); std::process::exit(2); }
    };

    // Default report path only kicks in at Debug level and only when
    // --diag-report wasn't given -- same rule DiagConfig::read_from_env
    // used for REGEX_DIAG_REPORT.
    let report_path = match (&diag_report, diag) {
        (Some(path), _)          => Some(path.clone()),
        (None, DiagLevel::Debug) => Some("reports/report.txt".to_string()),
        (None, _)                => None,
    };

    // matcher_type is irrelevant for the parse command's own diagnostics
    // (run_parser never reads it), Deriv is the harmless default.
    let config = DiagConfig::new(diag, parser, MatcherType::Deriv, report_path);
    run_parser(regex_str, &r, input, &config);
}

// Runs with all parsers: the three Brzozowski-derivative parsers (proven
// POSIX-equivalent, checked for full agreement) plus the bit-coded
// partial-derivative parser (pderiv/pderiv_bc -- both names alias the
// same construction, shown once) alongside them.
pub fn run_parse_all(regex_str: &str, input: &str) {
    let r = match parse_regex_string(regex_str) {
        Ok(r)  => r,
        Err(e) => { eprintln!("Regex parse error: {}", e); std::process::exit(2); }
    };

    // "all" mode: comparison table always, regardless of --diag
    println!("Regex: {}", regex_str);
    println!("Input: {:?}", input);
    println!();
    println!("{:12} | {}", "Parser", "Result");
    println!("{:-<12}-+-{:-<30}", "", "");

    type ParserFn = fn(&str, &regex_engine::Regex) -> Option<ParseTree>;
    // Only the three Brzozowski-derivative parsers here are checked for
    // mutual agreement -- all three are proven/verified POSIX-equivalent
    // (Theorem 1; also independently checked in
    // tests/posix_bruteforce_oracle.rs).
    let posix_parsers: Vec<(&str, ParserFn)> = vec![
        ("DERIV_REC",  parse_recursive),
        ("DERIV_LOOP", parse_loop),
        ("DERIV_BC",   parse_bitcoded),
    ];

    let mut posix_results: Vec<Option<ParseTree>> = Vec::new();
    for (name, parser) in &posix_parsers {
        let result = parser(input, &r);
        match &result {
            Some(tree) => println!("{:12} | {} → {:?}", name, tree, flatten(tree)),
            None       => println!("{:12} | ✗ No match", name),
        }
        posix_results.push(result);
    }

    // pderiv/pderiv_bc (regex::pderiv::annotated::pderiv_bc, Chapter 6):
    // computes GREEDY leftmost priority, not POSIX leftmost-longest, in
    // general -- shown alongside the three POSIX parsers above, not
    // folded into their agreement check, since disagreeing with them on
    // the exact parse tree is expected on any input with an
    // A1-relevant ambiguity, not a bug.
    let pderiv_result = parse_pderiv_bc(input, &r);
    match &pderiv_result {
        Some(tree) => println!("{:12} | {} → {:?}", "PDERIV_BC*", tree, flatten(tree)),
        None       => println!("{:12} | ✗ No match", "PDERIV_BC*"),
    }

    let posix_agree = posix_results.windows(2).all(|w| w[0] == w[1]);
    if posix_agree { println!("\n✓ All POSIX parsers (DERIV_REC/DERIV_LOOP/DERIV_BC) agree"); }
    else            { println!("\n✗ POSIX PARSERS DISAGREE!"); }

    // Membership (match/no-match) is a correctness property that holds
    // across every parser unconditionally, greedy or POSIX alike --
    // unlike tree shape, disagreement here would be a real bug.
    let membership_agrees = posix_results.iter().all(|t| t.is_some() == pderiv_result.is_some());
    if membership_agrees {
        println!("✓ PDERIV_BC agrees with the POSIX parsers on membership (match/no-match)");
    } else {
        println!("✗ PDERIV_BC DISAGREES WITH THE POSIX PARSERS ON MEMBERSHIP -- this is a bug, not the expected Greedy/POSIX gap");
    }

    println!();
    println!("* PDERIV_BC (alias: pderiv) computes Greedy leftmost priority, not POSIX --");
    println!("  its parse TREE may differ from the parsers above on ambiguous inputs.");
    println!("  Membership is unaffected: the two policies always agree on *whether*");
    println!("  the input matches, only (on ambiguous inputs) on which parse wins.");
}
