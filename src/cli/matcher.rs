//! Matcher command logic -- always simple true/false, no diagnostics.

use regex_engine::matchers::MatcherType;
use regex_engine::frontend::parse_pattern;


// Runs with chosen matcher
pub fn run_match_single(regex_str: &str, input: &str, matcher: MatcherType) {
    let r = match parse_pattern(regex_str) {
        Ok(r)  => r,
        Err(e) => { eprintln!("Regex parse error: {}", e); std::process::exit(2); }
    };

    let matched = matcher.matcher()(input, &r);
    println!("{}", matched);
    if !matched { std::process::exit(1); }
}


// Runs with all matchers
pub fn run_match_all(regex_str: &str, input: &str) {
    let r = match parse_pattern(regex_str) {
        Ok(r)  => r,
        Err(e) => { eprintln!("Regex parse error: {}", e); std::process::exit(2); }
    };

    // Comparison table 
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
    if all_equal { println!("\nAll matchers agree"); }
    else         { println!("\nMATCHERS DISAGREE!"); }
}
