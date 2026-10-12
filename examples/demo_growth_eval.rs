//! examples/demo_growth_eval.rs
//!
//! cargo run --release --example demo_growth_eval
//!
//! Chapter 8 (Evaluation) memory-growth figure: all four parsers side by
//! side on the same two canonical expressions `demo_growth_deriv.rs` and
//! `demo_growth_pderiv.rs` already use for the chapter 5/6 figures, so
//! this is a superset comparison, not a new scenario. Those two files
//! and the tests pinning them are untouched; this one only adds the two
//! missing series (PartDeriv standard) and prints all four together.
//!
//! "Size" means different things for the two families: Deriv keeps one
//! expression, so its size is that expression's node count. PartDeriv
//! keeps a frontier (a set of residual terms), so its size is the
//! strand count and, for a size comparable to Deriv's, the total node
//! count summed across the whole frontier. Neither family's bitcoded
//! and standard sides are quite the same construction either: the
//! bitcoded side applies `smart_seq`/dedup the standard side does not
//! always match exactly (see `series_frontier_std`'s doc comment) --
//! printed here rather than smoothed over.

#[path = "common/thesis_figures.rs"]
mod common;
use common::*;

fn main() {
    let astar = expr_astar();
    let r2 = expr_paper_r2();
    let a16 = "a".repeat(16);
    let ab8 = "ab".repeat(8);

    println!("== a* on a^16 ==\n");
    let deriv_std = series_plain_unsimplified(&astar, &a16);
    let deriv_bc = series_bitcoded_simplified(&astar, &a16);
    let pderiv_std_total = series_frontier_std_total_size(&astar, &a16);
    let pderiv_bc_total = series_frontier_total_size(&astar, &a16);
    println!(
        "{:>5} {:>12} {:>12} {:>14} {:>14}",
        "step", "deriv_std", "deriv_bc", "pderiv_std", "pderiv_bc"
    );
    for i in 0..=a16.chars().count() {
        println!(
            "{:>5} {:>12} {:>12} {:>14} {:>14}",
            i, deriv_std[i], deriv_bc[i], pderiv_std_total[i], pderiv_bc_total[i]
        );
    }

    println!("\n== (a+(b+ab))* on (ab)^8 ==\n");
    let deriv_std2 = series_plain_unsimplified(&r2, &ab8);
    let deriv_bc2 = series_bitcoded_simplified(&r2, &ab8);
    let pderiv_std2 = series_frontier_std(&r2, &ab8);
    let pderiv_bc2 = series_frontier(&r2, &ab8);
    let pderiv_std2_total = series_frontier_std_total_size(&r2, &ab8);
    let pderiv_bc2_total = series_frontier_total_size(&r2, &ab8);
    println!(
        "{:>5} {:>10} {:>9} {:>16} {:>16} {:>12} {:>12}",
        "step", "deriv_std", "deriv_bc", "pderiv_std(strands,size)", "pderiv_bc(strands,size)", "", ""
    );
    for i in 0..=ab8.chars().count() {
        println!(
            "{:>5} {:>10} {:>9} {:>9},{:<6} {:>9},{:<6}",
            i,
            deriv_std2[i],
            deriv_bc2[i],
            pderiv_std2[i].0,
            pderiv_std2_total[i],
            pderiv_bc2[i].0,
            pderiv_bc2_total[i],
        );
    }

    println!("\n-- pgfplots coordinates --");
    println!("% deriv_std, a* on a^n");
    println!("\\addplot coordinates {{{}}};", coords(&enumerate_series(&deriv_std)));
    println!("% deriv_bc, a* on a^n");
    println!("\\addplot coordinates {{{}}};", coords(&enumerate_series(&deriv_bc)));
    println!("% pderiv_std total frontier size, a* on a^n");
    println!("\\addplot coordinates {{{}}};", coords(&enumerate_series(&pderiv_std_total)));
    println!("% pderiv_bc total frontier size, a* on a^n");
    println!("\\addplot coordinates {{{}}};", coords(&enumerate_series(&pderiv_bc_total)));
}
