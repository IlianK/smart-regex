//! Figure 7.4 (doc/chapters/07_frontend.tex, label fig:lowered-size): node
//! count of the lowered Regex, per pattern. Also reports wildcard_run()'s
//! size and case-folding's per-letter cost, both quoted in Chapter 7.
//!
//! Run: cargo run --release --example demo_lowered
//!
//! Deterministic; pinned by tests/test_thesis_figures.rs.

#[path = "common/thesis_figures.rs"]
mod common;
use common::*;

use regex_engine::frontend::parse_pcre_rule;

fn main() {
    println!("Figure 7.4: node count of the lowered Regex, per pattern.\n");

    println!("{:<18} {:<10} {:>6}", "pattern", "entry", "nodes");
    for &(pat, search) in LOWERED_PATTERNS {
        let entry = if search { "search" } else { "full" };
        match lowered_size(pat, search) {
            Ok(n) => println!("{:<18} {:<10} {:>6}", pat, entry, n),
            Err(e) => println!("{:<18} {:<10}  ERROR: {}", pat, entry, e),
        }
    }

    println!("\n-- other lowered sizes quoted in Chapter 7 --");
    for (label, pat, search) in [
        ("a+", "a+", false),
        ("a?", "a?", false),
        ("a{2}", "a{2}", false),
        ("a{2,}", "a{2,}", false),
        ("^abc$ (search)", "^abc$", true),
        ("^abc  (search)", "^abc", true),
        ("abc$  (search)", "abc$", true),
    ] {
        match lowered_size(pat, search) {
            Ok(n) => println!("{:<16} {:>6}", label, n),
            Err(e) => println!("{:<16}  ERROR: {}", label, e),
        }
    }

    // wildcard_run() is not public; derive its size from the padding difference.
    let anchored = lowered_size("^abc", true).unwrap();
    let core = lowered_size("^abc", false).unwrap();
    println!(
        "\nwildcard_run() = {} nodes  (one-sided padded {} minus core {} minus the joining Seq)",
        anchored - core - 1,
        anchored,
        core
    );

    // Section 7.5: case folding adds two nodes per folded letter.
    let plain = size_regex(&parse_pcre_rule("/abc/").unwrap());
    let folded = size_regex(&parse_pcre_rule("/abc/i").unwrap());
    println!(
        "\ncase folding: /abc/ = {} nodes, /abc/i = {} nodes ({} extra, {} per letter)",
        plain,
        folded,
        folded - plain,
        (folded - plain) / 3
    );
}
