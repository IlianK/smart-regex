//! Matcher types and match command logic

use regex_engine::matchers::MatcherType;
use super::input::parse_regex_string;

pub fn run_match_single(regex_str: &str, input: &str, matcher: MatcherType) {
    let r = match parse_regex_string(regex_str) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Regex parse error: {}", e);
            std::process::exit(2);
        }
    };
    
    let matched = matcher.matcher()(input, &r);
    println!("{}", matched);
    
    if !matched {
        std::process::exit(1);
    }
}

pub fn run_match_all(regex_str: &str, input: &str) {
    let r = match parse_regex_string(regex_str) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Regex parse error: {}", e);
            std::process::exit(2);
        }
    };
    
    println!("Regex: {}", regex_str);
    println!("Input: {}", input);
    println!();
    println!("{:10} | {:6}", "Matcher", "Result");
    println!("{:-<10}-+-{:-<6}", "", "");
    
    let matchers = vec![
        MatcherType::Naive,
        MatcherType::Deriv,
        MatcherType::PDeriv,
    ];
    
    let mut results = Vec::new();
    
    for matcher in &matchers {
        let matched = matcher.matcher()(input, &r);
        results.push(matched);
        println!("{:10} | {:6}", matcher.display_name(), matched);
    }
    
    let all_equal = results.windows(2).all(|w| w[0] == w[1]);
    if all_equal {
        println!("\n✓ All matchers agree");
    } else {
        println!("\n✗ MATCHERS DISAGREE!");
    }
}