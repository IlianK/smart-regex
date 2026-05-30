//! Level 1 — Basic diagnostics output.
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
use crate::posix::selection::ParserType;
use crate::posix::standard::{parse_recursive, parse_loop};
use crate::posix::bitcoded::parse_bitcoded;
use crate::diagnostics::DiagConfig;
use crate::diagnostics::replay::{find_failure, caret_lines};

// ============================================================================
// Parser — Level 1
// ============================================================================

pub fn run_parser(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    let result = match config.parser_type {
        ParserType::Recursive => parse_recursive(input, r),
        ParserType::Loop      => parse_loop(input, r),
        ParserType::Bitcoded  => parse_bitcoded(input, r),
    };

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

// ============================================================================
// Matcher — Level 1
// ============================================================================

pub fn run_matcher(regex_str: &str, r: &Regex, input: &str) {
    use crate::matchers::MatcherType;
    use crate::matchers::{match_naive, match_deriv, match_pderiv};

    // MatcherType::from_env() returns Vec — take the first (default: Deriv)
    let matcher_type = MatcherType::from_env()
        .into_iter()
        .next()
        .unwrap_or(MatcherType::Deriv);

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