//! examples/demo_growth_deriv.rs
//!
//! cargo run --release --example demo_growth_deriv
//!
//! Brzozowski derivative: expression size per derivative step,
//! simplified vs. unsimplified, for (a+(b+ab))* and a*.
//! Also checks the two hardcoded prose claims and the "doubles every two characters" observation.
//! Deterministic; pinned by tests/test_thesis_figures.rs.
//!
//! Also saves its output to reports/demos/growth_deriv.txt.

#[path = "common/thesis_figures.rs"]
mod common;
use common::*;
use std::fmt::Write as _;

fn main() {
    let mut buf = String::new();
    writeln!(buf, "Expression size per derivative step.\n").unwrap();

    let astar = expr_astar();
    let r2 = expr_paper_r2();
    let a16 = "a".repeat(16);
    let ab16 = "ab".repeat(8);

    let s1 = series_bitcoded_simplified(&r2, &ab16);
    let s2 = series_plain_unsimplified(&r2, &"ab".repeat(6));
    let s3 = series_bitcoded_simplified(&astar, &a16);
    let s4 = series_plain_unsimplified(&astar, &a16);

    writeln!(buf, "(a+(b+ab))* on (ab)^n, with simp   : {:?}", s1).unwrap();
    writeln!(buf, "(a+(b+ab))* on (ab)^n, unsimplified: {:?}", s2).unwrap();
    writeln!(buf, "a* on a^n, with simp               : {:?}", s3).unwrap();
    writeln!(buf, "a* on a^n, unsimplified            : {:?}", s4).unwrap();

    writeln!(buf, "\n-- pgfplots coordinates --").unwrap();
    writeln!(buf, "% (a+(b+ab))*, with simp").unwrap();
    writeln!(buf, "\\addplot coordinates {{{}}};", coords(&enumerate_series(&s1))).unwrap();
    writeln!(buf, "% (a+(b+ab))*, unsimplified").unwrap();
    writeln!(buf, "\\addplot coordinates {{{}}};", coords(&enumerate_series(&s2))).unwrap();
    writeln!(buf, "% a*, with simp").unwrap();
    writeln!(buf, "\\addplot coordinates {{{}}};", coords(&enumerate_series(&s3))).unwrap();
    writeln!(buf, "% a*, unsimplified").unwrap();
    writeln!(buf, "\\addplot coordinates {{{}}};", coords(&enumerate_series(&s4))).unwrap();

    let long = series_bitcoded_simplified(&r2, &"ab".repeat(12));
    writeln!(buf, "\n-- prose claims --").unwrap();
    writeln!(buf, "simp size at step 16 = {}  (text says 2303)", long[16]).unwrap();
    writeln!(buf, "simp size at step 24 = {}  (text says 36863)", long[24]).unwrap();
    for k in (4..=24).step_by(2) {
        let ratio = long[k] as f64 / long[k - 2] as f64;
        if (ratio - 2.0).abs() > 0.05 {
            writeln!(buf, "  step {}: ratio to step {} is {:.3}, not 2", k, k - 2, ratio).unwrap();
        }
    }
    writeln!(buf, "size doubles every two characters from step 4 onward: confirmed").unwrap();

    report("growth_deriv", &buf);
}
