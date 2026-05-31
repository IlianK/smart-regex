//! regex-engine/src/cli/parser.rs
//! 
//! Parser command logic
//!
//! Reads DiagConfig and delegates to diagnostics::run_parser,
//! or runs the all-parsers comparison table when REGEX_PARSER=all is used.

use regex_engine::diagnostics::{DiagConfig, run_parser};
use regex_engine::{parse_recursive, parse_loop, parse_bitcoded, flatten};
use regex_engine::types::ParseTree;
use super::input::parse_regex_string;

pub fn run_parse_single(regex_str: &str, input: &str) {
    let r = match parse_regex_string(regex_str) {
        Ok(r)  => r,
        Err(e) => { eprintln!("Regex parse error: {}", e); std::process::exit(2); }
    };

    let config = DiagConfig::read_from_env();
    run_parser(regex_str, &r, input, &config);
}

pub fn run_parse_all(regex_str: &str, input: &str) {
    let r = match parse_regex_string(regex_str) {
        Ok(r)  => r,
        Err(e) => { eprintln!("Regex parse error: {}", e); std::process::exit(2); }
    };

    // "all" mode: comparison table always, regardless of REGEX_DIAG
    println!("Regex: {}", regex_str);
    println!("Input: {:?}", input);
    println!();
    println!("{:12} | {}", "Parser", "Result");
    println!("{:-<12}-+-{:-<30}", "", "");

    type ParserFn = fn(&str, &regex_engine::Regex) -> Option<ParseTree>;
    let parsers: Vec<(&str, ParserFn)> = vec![
        ("RECURSIVE", parse_recursive),
        ("LOOP",      parse_loop),
        ("BITCODED",  parse_bitcoded),
    ];

    let mut results: Vec<Option<ParseTree>> = Vec::new();
    for (name, parser) in &parsers {
        let result = parser(input, &r);
        match &result {
            Some(tree) => println!("{:12} | {} → {:?}", name, tree, flatten(tree)),
            None       => println!("{:12} | ✗ No match", name),
        }
        results.push(result);
    }

    let all_equal = results.windows(2).all(|w| w[0] == w[1]);
    if all_equal { println!("\n✓ All parsers agree"); }
    else         { println!("\n✗ PARSERS DISAGREE!"); }
}