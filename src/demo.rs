//! Demo utilities for regex-engine

use crate::basic::Regex;
use crate::basic::{match_naive, match_deriv, match_pderiv};
use crate::posix::{parse_posix, flatten, ParseTree};

/// Run basic matching demo (naive, deriv, pderiv)
pub fn demo_basic_matching(r: &Regex, expr_str: &str, inputs: &[&str]) {
    println!(" ▶ Expression: {}", expr_str);
    for input in inputs {
        let naive = match_naive(input, r);
        let deriv = match_deriv(input, r);
        let pderiv = match_pderiv(input, r);
        println!("  \"{:4}\" -> n={} d={} p={}", input, naive, deriv, pderiv);
    }
    println!();
}

/// Run POSIX parsing demo
pub fn demo_posix_parse(r: &Regex, _expr_str: &str, input: &str) -> Option<ParseTree> {
    println!("  Input: \"{}\"", input);
    
    match parse_posix(input, r) {
        Some(tree) => {
            println!("  - Parse tree: {}", tree);
            println!("  - Debug:      {:?}", tree);
            println!("  - Flatten:     \"{}\"\n", flatten(&tree));
            Some(tree)
        }
        None => {
            println!("  [X] No match!\n");
            None
        }
    }
}
