//! examples/demo_growth_pderiv.rs
//!
//! cargo run --release --example demo_growth_pderiv
//!
//! Antimirov partial derivative:
//! frontier size against distinct residual count,
//! per input character, for (a+(b+ab))* on (ab)^n.
//! Deterministic; pinned by tests/test_thesis_figures.rs.
//!
//! Also saves its output to reports/demos/growth_pderiv.txt.

#[path = "common/thesis_figures.rs"]
mod common;
use common::*;
use std::fmt::Write as _;

fn main() {
    let mut buf = String::new();
    writeln!(buf, "Frontier size vs. distinct residual count.\n").unwrap();

    let rows = series_frontier(&expr_paper_r2(), &"ab".repeat(7));
    writeln!(buf, "{:>5} {:>10} {:>10}", "char", "strands", "distinct").unwrap();
    for (i, (s, d)) in rows.iter().enumerate() {
        writeln!(buf, "{:>5} {:>10} {:>10}", i, s, d).unwrap();
    }

    let strands: Vec<(usize, usize)> = rows.iter().enumerate().map(|(i, t)| (i, t.0)).collect();
    let distinct: Vec<(usize, usize)> = rows.iter().enumerate().map(|(i, t)| (i, t.1)).collect();
    writeln!(buf, "\n-- pgfplots coordinates --").unwrap();
    writeln!(buf, "% strands in the frontier").unwrap();
    writeln!(buf, "\\addplot coordinates {{{}}};", coords(&strands)).unwrap();
    writeln!(buf, "% distinct residuals among them").unwrap();
    writeln!(buf, "\\addplot coordinates {{{}}};", coords(&distinct)).unwrap();

    report("growth_pderiv", &buf);
}
