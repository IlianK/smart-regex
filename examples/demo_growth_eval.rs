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
//! Also saves its output to reports/demos/growth_eval.txt.

#[path = "common/thesis_figures.rs"]
mod common;
use common::*;
use std::fmt::Write as _;

fn main() {
    let mut buf = String::new();
    let astar = expr_astar();
    let r2 = expr_paper_r2();
    let a16 = "a".repeat(16);
    let ab8 = "ab".repeat(8);

    writeln!(buf, "== a* on a^16 ==\n").unwrap();
    let deriv_std = series_plain_unsimplified(&astar, &a16);
    let deriv_bc = series_bitcoded_simplified(&astar, &a16);
    let pderiv_std_total = series_frontier_std_total_size(&astar, &a16);
    let pderiv_bc_total = series_frontier_total_size(&astar, &a16);
    writeln!(
        buf,
        "{:>5} {:>12} {:>12} {:>14} {:>14}",
        "step", "deriv_std", "deriv_bc", "pderiv_std", "pderiv_bc"
    )
    .unwrap();
    for i in 0..=a16.chars().count() {
        writeln!(
            buf,
            "{:>5} {:>12} {:>12} {:>14} {:>14}",
            i, deriv_std[i], deriv_bc[i], pderiv_std_total[i], pderiv_bc_total[i]
        )
        .unwrap();
    }

    writeln!(buf, "\n== (a+(b+ab))* on (ab)^8 ==\n").unwrap();
    let deriv_std2 = series_plain_unsimplified(&r2, &ab8);
    let deriv_bc2 = series_bitcoded_simplified(&r2, &ab8);
    let pderiv_std2 = series_frontier_std(&r2, &ab8);
    let pderiv_bc2 = series_frontier(&r2, &ab8);
    let pderiv_std2_total = series_frontier_std_total_size(&r2, &ab8);
    let pderiv_bc2_total = series_frontier_total_size(&r2, &ab8);
    writeln!(
        buf,
        "{:>5} {:>10} {:>9} {:>16} {:>16} {:>12} {:>12}",
        "step", "deriv_std", "deriv_bc", "pderiv_std(strands,size)", "pderiv_bc(strands,size)", "", ""
    )
    .unwrap();
    for i in 0..=ab8.chars().count() {
        writeln!(
            buf,
            "{:>5} {:>10} {:>9} {:>9},{:<6} {:>9},{:<6}",
            i,
            deriv_std2[i],
            deriv_bc2[i],
            pderiv_std2[i].0,
            pderiv_std2_total[i],
            pderiv_bc2[i].0,
            pderiv_bc2_total[i],
        )
        .unwrap();
    }

    writeln!(buf, "\n-- pgfplots coordinates --").unwrap();
    writeln!(buf, "% deriv_std, a* on a^n").unwrap();
    writeln!(buf, "\\addplot coordinates {{{}}};", coords(&enumerate_series(&deriv_std))).unwrap();
    writeln!(buf, "% deriv_bc, a* on a^n").unwrap();
    writeln!(buf, "\\addplot coordinates {{{}}};", coords(&enumerate_series(&deriv_bc))).unwrap();
    writeln!(buf, "% pderiv_std total frontier size, a* on a^n").unwrap();
    writeln!(buf, "\\addplot coordinates {{{}}};", coords(&enumerate_series(&pderiv_std_total))).unwrap();
    writeln!(buf, "% pderiv_bc total frontier size, a* on a^n").unwrap();
    writeln!(buf, "\\addplot coordinates {{{}}};", coords(&enumerate_series(&pderiv_bc_total))).unwrap();

    report("growth_eval", &buf);
}
