//! examples/demo_translate_sizes.rs
//!
//! cargo run --release --example demo_translate_sizes
//!
//! Sizes of the Regex produced by frontend translation.
//! Node count per pattern.
//! Case-folding's per-letter cost.
//! Deterministic; pinned by `tests/test_thesis_figures.rs`.
//!
//! Saves the table to reports/demos/translate_sizes.csv (columns:
//! group, pattern, nodes), replacing any previous run.

#[path = "common/thesis_figures.rs"]
mod common;
use common::*;
use std::fmt::Write as _;

use regex_engine::frontend::parse_pcre_rule;

fn main() {
    let mut csv = String::new();
    writeln!(csv, "group,pattern,nodes").unwrap();

    println!("Node count of translated Regex, per pattern\n");
    println!("  {:<16} {:>6}", "pattern", "nodes");
    for &pat in LOWERED_PATTERNS {
        match lowered_size(pat) {
            Ok(n) => {
                println!("  {:<16} {:>6}", pat, n);
                writeln!(csv, "lowered,{},{}", csv_quote(pat), n).unwrap();
            }
            Err(e) => {
                println!("  {:<16}  ERROR: {}", pat, e);
                eprintln!("{pat:?} failed to translate: {e}");
            }
        }
    }

    println!("\nOther translated sizes");
    println!("  {:<16} {:>6}", "pattern", "nodes");
    for (label, pat) in [("a+", "a+"), ("a?", "a?"), ("a{2}", "a{2}"), ("a{2,}", "a{2,}")] {
        match lowered_size(pat) {
            Ok(n) => {
                println!("  {:<16} {:>6}", label, n);
                writeln!(csv, "other,{},{}", csv_quote(pat), n).unwrap();
            }
            Err(e) => {
                println!("  {:<16}  ERROR: {}", label, e);
                eprintln!("{pat:?} failed to translate: {e}");
            }
        }
    }

    // Case folding adds two nodes per folded letter.
    let plain = size_regex(&parse_pcre_rule("/abc/").unwrap());
    let folded = size_regex(&parse_pcre_rule("/abc/i").unwrap());
    println!(
        "\nCase folding: /abc/ = {} nodes, /abc/i = {} nodes ({} extra, {} per letter)",
        plain,
        folded,
        folded - plain,
        (folded - plain) / 3
    );
    writeln!(csv, "case_folding,\"/abc/\",{plain}").unwrap();
    writeln!(csv, "case_folding,\"/abc/i\",{folded}").unwrap();

    save("translate_sizes", "csv", &csv);
}
