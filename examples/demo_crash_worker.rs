//! examples/demo_crash_worker.rs
//!
//! cargo build --example demo_crash_worker
//!
//! Worker for `demo_crash`: runs one `a*` probe on `value` chars, 
//! recursive or loop by argv, and exits with 0 on success, 1 on failure (stack overflow).

use regex_engine::types::Regex;
use regex_engine::parsers::{parse_deriv_std_rec, parse_deriv_std_loop};
use std::env;
use std::process;


fn main() {
    let args: Vec<String> = env::args().collect();
    
    // argv: <prog> <value> <use_loop>
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
        let _ = parse_deriv_std_loop(&input, &regex);
    } else {
        let _ = parse_deriv_std_rec(&input, &regex);
    }
}