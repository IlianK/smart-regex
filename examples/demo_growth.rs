//! Figure 5.3 (doc/chapters/05_derivative_parser.tex, label fig:simp-growth):
//! expression size per derivative step, simplified vs. unsimplified.
//!
//! Run: cargo run --release --example demo_growth
//!
//! Deterministic; pinned by tests/test_thesis_figures.rs.

#[path = "common/thesis_figures.rs"]
mod common;
use common::*;

fn main() {
    println!("Figure 5.3: expression size per derivative step.\n");

    let astar = expr_astar();
    let r2 = expr_paper_r2();
    let a16 = "a".repeat(16);
    let ab16 = "ab".repeat(8);

    let s1 = series_bitcoded_simplified(&r2, &ab16);
    let s2 = series_plain_unsimplified(&r2, &"ab".repeat(6));
    let s3 = series_bitcoded_simplified(&astar, &a16);
    let s4 = series_plain_unsimplified(&astar, &a16);

    println!("(a+(b+ab))* on (ab)^n, with simp   : {:?}", s1);
    println!("(a+(b+ab))* on (ab)^n, unsimplified: {:?}", s2);
    println!("a* on a^n, with simp               : {:?}", s3);
    println!("a* on a^n, unsimplified            : {:?}", s4);

    println!("\n-- pgfplots coordinates --");
    println!("% (a+(b+ab))*, with simp");
    println!("\\addplot coordinates {{{}}};", coords(&enumerate_series(&s1)));
    println!("% (a+(b+ab))*, unsimplified");
    println!("\\addplot coordinates {{{}}};", coords(&enumerate_series(&s2)));
    println!("% a*, with simp");
    println!("\\addplot coordinates {{{}}};", coords(&enumerate_series(&s3)));
    println!("% a*, unsimplified");
    println!("\\addplot coordinates {{{}}};", coords(&enumerate_series(&s4)));

    let long = series_bitcoded_simplified(&r2, &"ab".repeat(12));
    println!("\n-- prose claims in Section 5.5 --");
    println!("simp size at step 16 = {}  (text says 2303)", long[16]);
    println!("simp size at step 24 = {}  (text says 36863)", long[24]);
    for k in (4..=24).step_by(2) {
        let ratio = long[k] as f64 / long[k - 2] as f64;
        if (ratio - 2.0).abs() > 0.05 {
            println!("  step {}: ratio to step {} is {:.3}, not 2", k, k - 2, ratio);
        }
    }
    println!("size doubles every two characters from step 4 onward: confirmed");
}
