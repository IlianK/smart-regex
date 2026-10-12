//! examples/demo_frontend_translation_sizes.rs
//!
//! cargo run --release --example demo_frontend_translation_sizes
//!
//! Sizes of the Regex produced by frontend translation.
//! Node count per pattern.
//! Case-folding's per-letter cost.
//! Deterministic; pinned by `tests/test_thesis_figures.rs`.

#[path = "common/thesis_figures.rs"]
mod common;
use common::*;

use regex_engine::frontend::parse_pcre_rule;

fn main() {
    println!("Node count of translated Regex, per pattern");
    println!();
    println!("  {:<16} {:>6}", "pattern", "nodes");
    for &pat in LOWERED_PATTERNS {
        match lowered_size(pat) {
            Ok(n) => println!("  {:<16} {:>6}", pat, n),
            Err(e) => println!("  {:<16}  ERROR: {}", pat, e),
        }
    }

    println!();
    println!("Other translated sizes");
    println!("  {:<16} {:>6}", "pattern", "nodes");
    for (label, pat) in [("a+", "a+"), ("a?", "a?"), ("a{2}", "a{2}"), ("a{2,}", "a{2,}")] {
        match lowered_size(pat) {
            Ok(n) => println!("  {:<16} {:>6}", label, n),
            Err(e) => println!("  {:<16}  ERROR: {}", label, e),
        }
    }

    // Case folding adds two nodes per folded letter.
    let plain = size_regex(&parse_pcre_rule("/abc/").unwrap());
    let folded = size_regex(&parse_pcre_rule("/abc/i").unwrap());
    println!();
    println!(
        "Case folding: /abc/ = {} nodes, /abc/i = {} nodes ({} extra, {} per letter)",
        plain,
        folded,
        folded - plain,
        (folded - plain) / 3
    );
}