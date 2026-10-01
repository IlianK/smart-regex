//! examples/demo_growth_pderiv.rs
//!
//! cargo run --release --example demo_growth_pderiv
//!
//! Antimirov partial derivative: 
//! frontier size against distinct residual count, 
//! per input character, for (a+(b+ab))* on (ab)^n. 
//! Deterministic; pinned by tests/test_thesis_figures.rs.

#[path = "common/thesis_figures.rs"]
mod common;
use common::*;

fn main() {
    println!("Frontier size vs. distinct residual count.\n");

    let rows = series_frontier(&expr_paper_r2(), &"ab".repeat(7));
    println!("{:>5} {:>10} {:>10}", "char", "strands", "distinct");
    for (i, (s, d)) in rows.iter().enumerate() {
        println!("{:>5} {:>10} {:>10}", i, s, d);
    }

    let strands: Vec<(usize, usize)> = rows.iter().enumerate().map(|(i, t)| (i, t.0)).collect();
    let distinct: Vec<(usize, usize)> = rows.iter().enumerate().map(|(i, t)| (i, t.1)).collect();
    println!("\n-- pgfplots coordinates --");
    println!("% strands in the frontier");
    println!("\\addplot coordinates {{{}}};", coords(&strands));
    println!("% distinct residuals among them");
    println!("\\addplot coordinates {{{}}};", coords(&distinct));
}