//! Parse command logic using shared parser selection

use regex_engine::posix::{parse_posix, flatten, ParserType};
use super::input::parse_regex_string;

pub fn run_parse_single(regex_str: &str, input: &str) {
    let r = match parse_regex_string(regex_str) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Regex parse error: {}", e);
            std::process::exit(2);
        }
    };
    
    match parse_posix(input, &r) {
        Some(tree) => {
            println!("{}", flatten(&tree));
        }
        None => {
            std::process::exit(1);
        }
    }
}

pub fn run_parse_all(regex_str: &str, input: &str) {
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
    println!("{:12} | {:12}", "Parser", "Result");
    println!("{:-<12}-+-{:-<12}", "", "");
    
    let parsers = vec![
        ParserType::Recursive,
        ParserType::Loop,
        ParserType::Bitcoded,
    ];
    
    let mut results = Vec::new();
    
    for parser in &parsers {
        let result = parser.parser()(input, &r);
        results.push(result.clone());
        
        match result {
            Some(tree) => {
                let flat = flatten(&tree);
                println!("{:12} | {} -> \"{}\"", parser.display_name(), tree, flat);
            }
            None => {
                println!("{:12} | ✗ No match", parser.display_name());
            }
        }
    }
    
    let all_equal = results.windows(2).all(|w| w[0] == w[1]);
    if all_equal {
        println!("\n✓ All parsers agree");
    } else {
        println!("\n✗ PARSERS DISAGREE!");
    }
}