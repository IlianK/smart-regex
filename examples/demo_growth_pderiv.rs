//! examples/demo_growth_pderiv.rs
//!
//! cargo run --release --example demo_growth_pderiv
//!
//! Antimirov partial derivative:
//! frontier size against distinct residual count,
//! per input character, for (a+(b+ab))* on (ab)^n.
//! Deterministic; pinned by tests/test_thesis_figures.rs.
//!
//! Saves the table to reports/demos/growth_pderiv.csv (columns: char,
//! strands, distinct), replacing any previous run.

#[path = "common/thesis_figures.rs"]
mod common;
use common::*;

fn main() {
    println!("Frontier size vs. distinct residual count.\n");

    let rows = series_frontier(&expr_paper_r2(), &"ab".repeat(7));
    println!("{:>5} {:>10} {:>10}", "char", "strands", "distinct");
    let mut csv = String::new();
    csv.push_str("char,strands,distinct\n");
    for (i, (s, d)) in rows.iter().enumerate() {
        println!("{:>5} {:>10} {:>10}", i, s, d);
        csv.push_str(&format!("{i},{s},{d}\n"));
    }
    save("growth_pderiv", "csv", &csv);

    let strands: Vec<(usize, usize)> = rows.iter().enumerate().map(|(i, t)| (i, t.0)).collect();
    let distinct: Vec<(usize, usize)> = rows.iter().enumerate().map(|(i, t)| (i, t.1)).collect();
    println!("\n-- pgfplots coordinates --");
    println!("% strands in the frontier");
    println!("\\addplot coordinates {{{}}};", coords(&strands));
    println!("% distinct residuals among them");
    println!("\\addplot coordinates {{{}}};", coords(&distinct));

    // The thesis's growth table (tab:growth) reports a different metric for
    // these two parsers than the strands/distinct counts above: the total
    // *node count* summed across the deduplicated frontier, not the number
    // of strands in it. series_frontier_total_size/_std_total_size already
    // exist for exactly this; this demo just hadn't been calling them.
    let total_bc = series_frontier_total_size(&expr_paper_r2(), &"ab".repeat(16));
    let total_std = series_frontier_std_total_size(&expr_paper_r2(), &"ab".repeat(16));
    println!("\n-- prose claims --");
    println!(
        "pderiv_bc total node count at n=1,8,16 = {}, {}, {}  (text says 18, 8, 8)",
        total_bc[1], total_bc[8], total_bc[16]
    );
    println!(
        "pderiv_std total node count at n=1,8,16 = {}, {}, {}  (text says 18, 8, 8)",
        total_std[1], total_std[8], total_std[16]
    );
}
