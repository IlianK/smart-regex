//! examples/demo_frontend_translation_sizes.rs
//!
//! cargo run --release --example demo_frontend_translation_sizes
//!
//! Sizes of the Regex produced by frontend translation.
//! Node count per pattern.
//! Case-folding's per-letter cost.
//! Deterministic; pinned by `tests/test_thesis_figures.rs`.
//!
//! Also saves its output to reports/demos/frontend_translation_sizes.txt.

#[path = "common/thesis_figures.rs"]
mod common;
use common::*;
use std::fmt::Write as _;

use regex_engine::frontend::parse_pcre_rule;

fn main() {
    let mut buf = String::new();
    writeln!(buf, "Node count of translated Regex, per pattern").unwrap();
    writeln!(buf).unwrap();
    writeln!(buf, "  {:<16} {:>6}", "pattern", "nodes").unwrap();
    for &pat in LOWERED_PATTERNS {
        match lowered_size(pat) {
            Ok(n) => writeln!(buf, "  {:<16} {:>6}", pat, n).unwrap(),
            Err(e) => writeln!(buf, "  {:<16}  ERROR: {}", pat, e).unwrap(),
        }
    }

    writeln!(buf).unwrap();
    writeln!(buf, "Other translated sizes").unwrap();
    writeln!(buf, "  {:<16} {:>6}", "pattern", "nodes").unwrap();
    for (label, pat) in [("a+", "a+"), ("a?", "a?"), ("a{2}", "a{2}"), ("a{2,}", "a{2,}")] {
        match lowered_size(pat) {
            Ok(n) => writeln!(buf, "  {:<16} {:>6}", label, n).unwrap(),
            Err(e) => writeln!(buf, "  {:<16}  ERROR: {}", label, e).unwrap(),
        }
    }

    // Case folding adds two nodes per folded letter.
    let plain = size_regex(&parse_pcre_rule("/abc/").unwrap());
    let folded = size_regex(&parse_pcre_rule("/abc/i").unwrap());
    writeln!(buf).unwrap();
    writeln!(
        buf,
        "Case folding: /abc/ = {} nodes, /abc/i = {} nodes ({} extra, {} per letter)",
        plain,
        folded,
        folded - plain,
        (folded - plain) / 3
    )
    .unwrap();

    report("frontend_translation_sizes", &buf);
}
