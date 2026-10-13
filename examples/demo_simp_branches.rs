//! examples/demo_simp_branches.rs
//!
//! cargo run --release --example demo_simp_branches
//!
//! Branch counts per derivative step for (a+(b+ab))*
//! Shows why simp's r + r = r rule never fires on it:
//! branches carry bits that survive simplification even when their shape is the same.
//! Deterministic pinned by tests/test_thesis_figures.rs.
//!
//! Also saves its output to reports/demos/simp_branches.txt.

#[path = "common/thesis_figures.rs"]
mod common;
use common::*;
use std::fmt::Write as _;

fn main() {
    let mut buf = String::new();
    writeln!(buf, "Branch counts per derivative step.\n").unwrap();

    let rows = series_branch_counts(&expr_paper_r2(), &"ab".repeat(6));
    writeln!(buf, "{:>5} {:>10} {:>10} {:>22}", "step", "branches", "distinct", "distinct ignoring bits").unwrap();
    for (i, (b, d, s)) in rows.iter().enumerate() {
        writeln!(buf, "{:>5} {:>10} {:>10} {:>22}", i + 1, b, d, s).unwrap();
    }

    writeln!(buf, "\n-- the rows quoted in the thesis (steps 2,4,6,8,10,12) --").unwrap();
    let picked: Vec<_> = [2usize, 4, 6, 8, 10, 12].iter().map(|&k| rows[k - 1]).collect();
    writeln!(buf, "branches              : {:?}", picked.iter().map(|t| t.0).collect::<Vec<_>>()).unwrap();
    writeln!(buf, "distinct branches     : {:?}", picked.iter().map(|t| t.1).collect::<Vec<_>>()).unwrap();
    writeln!(buf, "distinct ignoring bits: {:?}", picked.iter().map(|t| t.2).collect::<Vec<_>>()).unwrap();

    report("simp_branches", &buf);
}
