//! examples/demo_growth_eval.rs
//!
//! cargo run --release --example demo_growth_eval
//!
//! Memory-growth figure: all four parsers side by side on the same two
//! canonical expressions the deriv/pderiv growth demos already use.
//! A superset comparison, not a new scenario.
//! Those files and their pinning tests are untouched.
//! This one adds the two missing series (PartDeriv standard) and prints
//! all four together.
//!
//! "Size" means different things per family.
//! Deriv keeps one expression: size is its node count.
//! PartDeriv keeps a frontier (a set of residual terms): size is the
//! strand count, plus total node count summed across the frontier for a
//! value comparable to Deriv's.
//! Bitcoded and standard sides are not quite the same construction
//! either: bitcoded applies `smart_seq`/dedup the standard side does not
//! always match.
//! Printed, not smoothed over.
//!
//! Saves the two tables separately, replacing any previous run:
//!   - reports/demos/growth_eval_astar.csv (step, deriv_std, deriv_bc,
//!     pderiv_std, pderiv_bc -- all single node-count values for this
//!     expression)
//!   - reports/demos/growth_eval_r2.csv (step, deriv_std, deriv_bc,
//!     pderiv_std_strands, pderiv_std_size, pderiv_bc_strands,
//!     pderiv_bc_size -- the partial-derivative columns are a
//!     (strand count, total size) pair here, unlike the a* table, so
//!     the two tables do not share one schema)

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
    println!("{:>5} {:>12} {:>12} {:>14} {:>14}", "step", "deriv_std", "deriv_bc", "pderiv_std", "pderiv_bc");
    let mut astar_csv = String::new();
    astar_csv.push_str("step,deriv_std,deriv_bc,pderiv_std,pderiv_bc\n");
    for i in 0..=a16.chars().count() {
        println!(
            "{:>5} {:>12} {:>12} {:>14} {:>14}",
            i, deriv_std[i], deriv_bc[i], pderiv_std_total[i], pderiv_bc_total[i]
        );
        astar_csv.push_str(&format!(
            "{},{},{},{},{}\n",
            i, deriv_std[i], deriv_bc[i], pderiv_std_total[i], pderiv_bc_total[i]
        ));
    }
    save("growth_eval_astar", "csv", &astar_csv);

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
    let mut r2_csv = String::new();
    r2_csv.push_str("step,deriv_std,deriv_bc,pderiv_std_strands,pderiv_std_size,pderiv_bc_strands,pderiv_bc_size\n");
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
        r2_csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            i, deriv_std2[i], deriv_bc2[i], pderiv_std2[i].0, pderiv_std2_total[i], pderiv_bc2[i].0, pderiv_bc2_total[i],
        ));
    }
    save("growth_eval_r2", "csv", &r2_csv);

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
