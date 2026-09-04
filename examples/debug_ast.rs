//! regex-engine/examples/debug_ast.rs
//!
//! How frontend reads a pattern string,
//! at each of its three stages: 
//! 1. ExtPat (surface AST) -> 
//! 2. translate (core Regex, no search padding) -> 
//! 3. parse_dataset_pattern (core Regex, WITH leading/trailing unanchored-search)
//!
//! Usage:
//!   cargo run --example debug_ast -- '<pattern>'
//!
//! Example:
//!   cargo run --example debug_ast -- '(a+b+ab+e3)*'

use regex_engine::frontend::{parse_ext_pattern, translate};

fn main() {
    let pattern = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: cargo run --example debug_ast -- '<pattern>'");
        std::process::exit(2);
    });

    match parse_ext_pattern(&pattern) {
        Ok(ep) => {
            println!("ExtPat (surface AST):\n{:#?}\n", ep);
            match translate(&ep) {
                Ok(core) => println!("translate() -- core Regex, no search padding:\n{:?}\n", core),
                Err(e) => println!("translate() failed: {}\n", e),
            }
        }
        Err(e) => {
            println!("parse_ext_pattern() failed: {}\n", e);
            return;
        }
    }

    match regex_engine::parse_dataset_pattern(&pattern) {
        Ok(full) => println!(
            "parse_dataset_pattern() -- what --syntax ext actually runs, WITH search padding:\n{:?}",
            full
        ),
        Err(e) => println!("parse_dataset_pattern() failed: {}", e),
    }
}
