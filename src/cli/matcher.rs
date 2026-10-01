//! src/cli/matcher.rs
//!
//! `match` subcommand: boolean match only, no diagnostics. `--matcher all`

use regex_engine::matchers::MatcherType;
use regex_engine::frontend::{parse_dataset_pattern, parse_pattern};

fn parse(regex_str: &str, search: bool) -> Result<regex_engine::types::Regex, String> {
    if search { parse_dataset_pattern(regex_str) } else { parse_pattern(regex_str) }
}

/// Single matcher: print `true`/`false`, exit 0 on match, 1 on no match.
pub fn run_match_single(regex_str: &str, input: &str, matcher: MatcherType, search: bool) {
    let r = match parse(regex_str, search) {
        Ok(r)  => r,
        Err(e) => { eprintln!("Regex parse error: {}", e); std::process::exit(2); }
    };

    let matched = matcher.matcher()(input, &r);
    println!("{}", matched);
    if !matched { std::process::exit(1); }
}

/// All three matchers: one row per matcher, then an agreement line.
pub fn run_match_all(regex_str: &str, input: &str, search: bool) {
    let r = match parse(regex_str, search) {
        Ok(r)  => r,
        Err(e) => { eprintln!("Regex parse error: {}", e); std::process::exit(2); }
    };

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
    println!();
    if all_equal {
        println!("All matchers agree");
    } else {
        println!("MATCHERS DISAGREE");
    }
}