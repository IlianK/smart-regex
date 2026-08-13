//! regex-engine/src/diagnostics/level1.rs
//! 
//! Level 1 - Basic diagnostics output.
//!
//! On success:
//!   Regex:  a*
//!   Input:  "aa"
//!   Match:  true
//!   Tree:   [a, a]
//!
//! On failure:
//!   Regex:  a*
//!   Input:  "aab"
//!   Match:  false
//!   Error:  position 3: found 'b', expected 'a' or end of input
//!     aab
//!       ^

use crate::types::Regex;
use crate::matchers::MatcherType;
use crate::diagnostics::DiagConfig;
use crate::diagnostics::replay::{find_failure, caret_lines};


// -------------------------------
// Parser - Level 1
// -------------------------------

pub fn run_parser(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    let result = config.parser_type.parser()(input, r);

    println!("Regex:  {}", regex_str);
    println!("Input:  {:?}", input);

    match result {
        Some(tree) => {
            println!("Match:  true");
            println!("Tree:   {}", tree);
        }
        None => {
            println!("Match:  false");
            let info = find_failure(input, r);
            if info.found == '\0' {
                println!(
                    "Error:  position {}: unexpected end of input, expected {}",
                    info.position, info.expected
                );
            } else {
                println!(
                    "Error:  position {}: found '{}', expected {}",
                    info.position, info.found, info.expected
                );
            }
            println!("{}", caret_lines(input, info.position));
        }
    }
}


// -------------------------------
// Matcher - Level 1
// -------------------------------

pub fn run_matcher(regex_str: &str, r: &Regex, input: &str, matcher_type: MatcherType) {
    use crate::matchers::{match_naive, match_deriv, match_pderiv};

    let matched = match matcher_type {
        MatcherType::Naive  => match_naive(input, r),
        MatcherType::Deriv  => match_deriv(input, r),
        MatcherType::PDeriv => match_pderiv(input, r),
    };

    println!("Regex:  {}", regex_str);
    println!("Input:  {:?}", input);
    println!("Match:  {}", matched);

    if !matched {
        let info = find_failure(input, r);
        if info.found == '\0' {
            println!(
                "Error:  position {}: unexpected end of input, expected {}",
                info.position, info.expected
            );
        } else {
            println!(
                "Error:  position {}: found '{}', expected {}",
                info.position, info.found, info.expected
            );
        }
        println!("{}", caret_lines(input, info.position));
    }
}