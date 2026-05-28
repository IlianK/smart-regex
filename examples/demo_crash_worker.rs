//! Worker for isolated test execution
//!
//! Build with:
//!   cargo build --example demo_crash_worker
//!   cargo build --example demo_crash_worker --release

use regex_engine::types::Regex;
use regex_engine::posix::{parse_recursive, parse_loop};
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    // Expecting: program_name value use_loop
    if args.len() < 3 {
        process::exit(1);
    }
    
    let value: usize = match args[1].parse() {
        Ok(v) => v,
        Err(_) => process::exit(1),
    };
    let use_loop: bool = match args[2].parse() {
        Ok(v) => v,
        Err(_) => process::exit(1),
    };
    
    let input = "a".repeat(value);
    let regex = Regex::star(Regex::lit('a'));
    
    if use_loop {
        let _ = parse_loop(&input, &regex);
    } else {
        let _ = parse_recursive(&input, &regex);
    }
}