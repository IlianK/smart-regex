//! examples/demo_simp_branches.rs
//!
//! cargo run --release --example demo_simp_branches
//!
//! Branch counts per derivative step for (a+(b+ab))*
//! Shows why simp's r + r = r rule never fires on it:
//! branches carry bits that survive simplification even when their shape is the same.
//! Deterministic pinned by tests/test_thesis_figures.rs.
//!
//! Saves the table to reports/demos/simp_branches.csv (columns: step,
//! branches, distinct, distinct_ignoring_bits, thesis_quoted -- the
//! last marks the six rows (steps 2,4,6,8,10,12) the thesis text
//! quotes directly), replacing any previous run.

#[path = "common/thesis_figures.rs"]
mod common;
use common::*;

fn main() {
    println!("Branch counts per derivative step.\n");

    let rows = series_branch_counts(&expr_paper_r2(), &"ab".repeat(6));
    let quoted_steps = [2usize, 4, 6, 8, 10, 12];
    println!("{:>5} {:>10} {:>10} {:>22}", "step", "branches", "distinct", "distinct ignoring bits");
    let mut csv = String::new();
    csv.push_str("step,branches,distinct,distinct_ignoring_bits,thesis_quoted\n");
    for (i, (b, d, s)) in rows.iter().enumerate() {
        let step = i + 1;
        println!("{:>5} {:>10} {:>10} {:>22}", step, b, d, s);
        csv.push_str(&format!("{},{},{},{},{}\n", step, b, d, s, quoted_steps.contains(&step)));
    }
    save("simp_branches", "csv", &csv);

    println!("\n-- the rows quoted in the thesis (steps 2,4,6,8,10,12) --");
    let picked: Vec<_> = quoted_steps.iter().map(|&k| rows[k - 1]).collect();
    println!("branches              : {:?}", picked.iter().map(|t| t.0).collect::<Vec<_>>());
    println!("distinct branches     : {:?}", picked.iter().map(|t| t.1).collect::<Vec<_>>());
    println!("distinct ignoring bits: {:?}", picked.iter().map(|t| t.2).collect::<Vec<_>>());
}
