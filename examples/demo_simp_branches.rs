//! examples/demo_simp_branches.rs
//!
//! cargo run --release --example demo_simp_branches
//!
//! Branch counts per derivative step for (a+(b+ab))*
//! Shows why simp's r + r = r rule never fires on it: 
//! branches carry bits that survive simplification even when their shape is the same. 
//! Deterministic pinned by tests/test_thesis_figures.rs.

#[path = "common/thesis_figures.rs"]
mod common;
use common::*;

fn main() {
    println!("Branch counts per derivative step.\n");

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