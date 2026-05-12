//! Worker binary for isolated test execution
//!
//! Called by crash_demo to test single value
//! Build: cargo build --bin crash_demo_worker

use regex_engine::basic::Regex;
use regex_engine::posix::{parse_recursive, parse_loop};
use std::env;

fn deep_sequence(n: usize) -> Regex {
    let mut r = Regex::lit('a');
    for _ in 1..n {
        r = Regex::seq(r, Regex::lit('a'));
    }
    r
}

fn main() {
    let args: Vec<String> = env::args().collect();
    
    // Expecting: program_name value is_depth use_loop
    if args.len() < 4 {
        return;
    }
    
    let value: usize = args[1].parse().unwrap();
    let is_depth: bool = args[2].parse().unwrap();
    let use_loop: bool = args[3].parse().unwrap();
    
    let input = "a".repeat(value);
    let regex = if is_depth { deep_sequence(value) } else { Regex::star(Regex::lit('a')) };
    
    if use_loop {
        let _ = parse_loop(&input, &regex);
    } else {
        let _ = parse_recursive(&input, &regex);
    }
}