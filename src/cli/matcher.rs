//! regex-engine/src/cli/matcher.rs
//! 
//! Matcher command logic
//!
//! Reads DiagConfig and delegates to diagnostics::run_matcher,
//! or runs the all-matchers comparison table when --matcher all is used

use regex_engine::matchers::MatcherType;
use regex_engine::diagnostics::{DiagConfig, DiagLevel, run_matcher};
use super::input::parse_regex_string;

pub fn run_match_single(regex_str: &str, input: &str, matcher: MatcherType) {
    let r = match parse_regex_string(regex_str) {
        Ok(r)  => r,
        Err(e) => { eprintln!("Regex parse error: {}", e); std::process::exit(2); }
    };

    let config = DiagConfig::read_from_env();

    if config.level == DiagLevel::Off {
        // Original behaviour: print true/false, exit 1 on no match
        let matched = matcher.matcher()(input, &r);
        println!("{}", matched);
        if !matched { std::process::exit(1); }
    } else {
        run_matcher(regex_str, &r, input, &config);
        // Still set exit code correctly
        let matched = matcher.matcher()(input, &r);
        if !matched { std::process::exit(1); }
    }
}

pub fn run_match_all(regex_str: &str, input: &str) {
    let r = match parse_regex_string(regex_str) {
        Ok(r)  => r,
        Err(e) => { eprintln!("Regex parse error: {}", e); std::process::exit(2); }
    };

    // "all" mode: comparison table always, regardless of REGEX_DIAG
    println!("Regex: {}", regex_str);
    println!("Input: {}", input);
    println!();
    println!("{:10} | {:6}", "Matcher", "Result");
    println!("{:-<10}-+-{:-<6}", "", "");

    let matchers = vec![MatcherType::Naive, MatcherType::Deriv, MatcherType::PDeriv];
    let mut results = Vec::new();

    for m in &matchers {
        let matched = m.matcher()(input, &r);
        results.push(matched);
        println!("{:10} | {:6}", m.display_name(), matched);
    }

    let all_equal = results.windows(2).all(|w| w[0] == w[1]);
    if all_equal { println!("\n✓ All matchers agree"); }
    else         { println!("\n✗ MATCHERS DISAGREE!"); }
}