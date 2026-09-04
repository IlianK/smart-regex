//! regex-engine/src/diagnostics/diag_1.rs
//! 
//! Diag 1 - Basic diagnostics output.
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