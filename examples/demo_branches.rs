//! Section 5.5 table (doc/chapters/05_derivative_parser.tex): why simp's
//! r + r = r rule never fires on (a+(b+ab))*.
//!
//! Run: cargo run --release --example demo_branches
//!
//! Deterministic; pinned by tests/test_thesis_figures.rs.

#[path = "common/thesis_figures.rs"]
mod common;
use common::*;

fn main() {
    println!("Section 5.5: branch counts per derivative step.\n");

    let rows = series_branch_counts(&expr_paper_r2(), &"ab".repeat(6));
    println!("{:>5} {:>10} {:>10} {:>22}", "step", "branches", "distinct", "distinct ignoring bits");
    for (i, (b, d, s)) in rows.iter().enumerate() {
        println!("{:>5} {:>10} {:>10} {:>22}", i + 1, b, d, s);
    }

    println!("\n-- the rows quoted in the thesis (steps 2,4,6,8,10,12) --");
    let picked: Vec<_> = [2usize, 4, 6, 8, 10, 12].iter().map(|&k| rows[k - 1]).collect();
    println!("branches              : {:?}", picked.iter().map(|t| t.0).collect::<Vec<_>>());
    println!("distinct branches     : {:?}", picked.iter().map(|t| t.1).collect::<Vec<_>>());
    println!("distinct ignoring bits: {:?}", picked.iter().map(|t| t.2).collect::<Vec<_>>());
}
