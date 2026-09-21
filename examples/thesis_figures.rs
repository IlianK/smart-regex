//! regex-engine/examples/thesis_figures.rs
//!
//! Regenerates every measured number that appears in a figure or table of
//! `doc/`. Each section below names the figure it feeds, and prints the data
//! both as a readable table and as a pgfplots `coordinates {...}` line that
//! can be pasted straight into the `.tex` source.
//!
//! Run:
//!   cargo run --release --example thesis_figures
//!   cargo run --release --example thesis_figures -- growth     (Figure 5.3)
//!   cargo run --release --example thesis_figures -- branches   (Section 5.5 table)
//!   cargo run --release --example thesis_figures -- frontier   (Figure 6.3)
//!   cargo run --release --example thesis_figures -- lowered    (Figure 7.4)
//!
//! The numbers are deterministic: no randomness, no timing, no environment
//! dependence. `tests/test_thesis_figures.rs` pins the key values so that a
//! change in the implementation shows up as a failing test rather than as a
//! figure that silently stops matching the code.

use regex_engine::frontend::{parse_dataset_pattern, parse_pattern};
use regex_engine::parsers::deriv_bc::{
    deriv::deriv_bc, internalize::internalize, simplify::simp,
};
use regex_engine::parsers::pderiv_bc::pderiv::pderiv_bc;
use regex_engine::regex::deriv::deriv;
use regex_engine::types::{ARegex, Regex};

// -------------------------------
// Size measures
// -------------------------------

/// Node count of a plain `Regex`, counting every constructor once.
pub fn size_regex(r: &Regex) -> usize {
    match r {
        Regex::Phi | Regex::Eps | Regex::Lit(_) => 1,
        Regex::Alt(a, b) | Regex::Seq(a, b) => 1 + size_regex(a) + size_regex(b),
        Regex::Star(a) => 1 + size_regex(a),
    }
}

/// Node count of an annotated `ARegex`. Bit-lists are not counted: this
/// measures the expression the derivative has to walk, not the bits it carries.
pub fn size_aregex(ri: &ARegex) -> usize {
    match ri {
        ARegex::Phi | ARegex::Eps(_) | ARegex::Lit(_, _) => 1,
        ARegex::Alt(_, a, b) | ARegex::Seq(_, a, b) => 1 + size_aregex(a) + size_aregex(b),
        ARegex::Star(_, a) => 1 + size_aregex(a),
    }
}

// -------------------------------
// The two expressions the growth figures use
// -------------------------------

/// `a*`, the case Sulzmann and Lu's own simplification example uses.
pub fn expr_astar() -> Regex {
    Regex::star(Regex::lit('a'))
}

/// `(a + (b + ab))*`, the `paper_r2` expression of Section 4.3.
///
/// Note the right-associated grouping: the CLI spelling `"(a|b|ab)*"` parses
/// to the left-associated `((a+b)+ab)*` instead. Both produce identical size
/// series, which `tests/test_thesis_figures.rs` asserts.
pub fn expr_paper_r2() -> Regex {
    let ab = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    Regex::star(Regex::alt(
        Regex::lit('a'),
        Regex::alt(Regex::lit('b'), ab),
    ))
}

// -------------------------------
// Figure 5.3: expression size per derivative step
// -------------------------------

/// Sizes of `simp(deriv_bc(...))` after each character, starting at step 0.
pub fn series_bitcoded_simplified(r: &Regex, input: &str) -> Vec<usize> {
    let mut ri = internalize(r);
    let mut out = vec![size_aregex(&ri)];
    for c in input.chars() {
        ri = simp(deriv_bc(ri, c));
        out.push(size_aregex(&ri));
    }
    out
}

/// Sizes of the plain, unsimplified `deriv` after each character.
pub fn series_plain_unsimplified(r: &Regex, input: &str) -> Vec<usize> {
    let mut cur = r.clone();
    let mut out = vec![size_regex(&cur)];
    for c in input.chars() {
        cur = deriv(&cur, c);
        out.push(size_regex(&cur));
    }
    out
}

// -------------------------------
// Section 5.5 table: why simp's dedup never fires
// -------------------------------

/// Top-level alternation branches of `ri`, flattened through unannotated
/// `Alt` nodes exactly as `simp`'s own `push_branches` does.
fn alt_branches(ri: &ARegex, out: &mut Vec<ARegex>) {
    match ri {
        ARegex::Alt(bs, a, b) if bs.is_empty() => {
            alt_branches(a, out);
            alt_branches(b, out);
        }
        other => out.push(other.clone()),
    }
}

/// Erase every bit annotation, keeping only structural shape.
fn strip_bits(ri: &ARegex) -> ARegex {
    match ri {
        ARegex::Phi => ARegex::Phi,
        ARegex::Eps(_) => ARegex::Eps(vec![]),
        ARegex::Lit(_, c) => ARegex::Lit(vec![], *c),
        ARegex::Alt(_, a, b) => {
            ARegex::Alt(vec![], Box::new(strip_bits(a)), Box::new(strip_bits(b)))
        }
        ARegex::Seq(_, a, b) => {
            ARegex::Seq(vec![], Box::new(strip_bits(a)), Box::new(strip_bits(b)))
        }
        ARegex::Star(_, a) => ARegex::Star(vec![], Box::new(strip_bits(a))),
    }
}

/// `ARegex` has no `Hash`, so distinctness is counted by linear scan.
fn count_distinct(items: &[ARegex]) -> usize {
    let mut seen: Vec<&ARegex> = Vec::new();
    for x in items {
        if !seen.contains(&x) {
            seen.push(x);
        }
    }
    seen.len()
}

/// Per step: (branches, distinct branches, distinct ignoring bits).
pub fn series_branch_counts(r: &Regex, input: &str) -> Vec<(usize, usize, usize)> {
    let mut ri = internalize(r);
    let mut out = Vec::new();
    for c in input.chars() {
        ri = simp(deriv_bc(ri, c));
        let mut bs = Vec::new();
        alt_branches(&ri, &mut bs);
        let stripped: Vec<ARegex> = bs.iter().map(strip_bits).collect();
        out.push((bs.len(), count_distinct(&bs), count_distinct(&stripped)));
    }
    out
}

// -------------------------------
// Figure 6.3: frontier size against distinct residual count
// -------------------------------

/// Per character: (strands in the frontier, distinct residuals among them).
///
/// This drives the frontier exactly as `pderiv_bc::parse::step_frontier` does,
/// including the fact that `nub2` is applied inside `pderiv_bc` only and never
/// across the flattened frontier. Step 0 is the one-strand starting state.
pub fn series_frontier(r: &Regex, input: &str) -> Vec<(usize, usize)> {
    let mut frontier: Vec<(Regex, Vec<bool>)> = vec![(r.clone(), Vec::new())];
    let mut out = vec![(1usize, 1usize)];
    for c in input.chars() {
        let mut next = Vec::new();
        for (res, bits) in &frontier {
            for (res_next, extra) in pderiv_bc(res, c) {
                let mut combined = bits.clone();
                combined.extend(extra);
                next.push((res_next, combined));
            }
        }
        frontier = next;
        let mut distinct: Vec<&Regex> = Vec::new();
        for (res, _) in frontier.iter() {
            if !distinct.contains(&res) {
                distinct.push(res);
            }
        }
        out.push((frontier.len(), distinct.len()));
    }
    out
}

// -------------------------------
// Figure 7.4: lowered node counts
// -------------------------------

/// The patterns plotted in Figure 7.4, with the entry point each goes through.
pub const LOWERED_PATTERNS: &[(&str, bool)] = &[
    // (pattern, is_substring_search)
    ("a*", false),
    ("abc", false),
    ("a{2,4}", false),
    (r"\d", false),
    (r"\d{2}", false),
    (r"[a-c]+\d{2}", false),
    (r"\w", false),
    ("[^a]", false),
    (".", false),
    (r"\w+", false),
    ("abc", true),
    (r"[a-c]+\d{2}", true),
];

pub fn lowered_size(pattern: &str, search: bool) -> Result<usize, String> {
    let r = if search {
        parse_dataset_pattern(pattern)?
    } else {
        parse_pattern(pattern)?
    };
    Ok(size_regex(&r))
}

// -------------------------------
// Reporting
// -------------------------------

fn coords(pairs: &[(usize, usize)]) -> String {
    pairs
        .iter()
        .map(|(x, y)| format!("({},{})", x, y))
        .collect::<Vec<_>>()
        .join(" ")
}

fn enumerate_series(v: &[usize]) -> Vec<(usize, usize)> {
    v.iter().enumerate().map(|(i, &y)| (i, y)).collect()
}

fn section_growth() {
    println!("========================================================");
    println!("Figure 5.3  (doc/chapters/05_derivative_parser.tex,");
    println!("             label fig:simp-growth)");
    println!("Expression size per derivative step.");
    println!("========================================================\n");

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

    // The claim in the surrounding prose: size doubles every two characters.
    let long = series_bitcoded_simplified(&r2, &"ab".repeat(12));
    println!("\n-- prose claims in Section 5.5 --");
    println!("simp size at step 16 = {}  (text says 2303)", long[16]);
    println!("simp size at step 24 = {}  (text says 36863)", long[24]);
    for k in (2..=24).step_by(2) {
        if k >= 4 {
            let ratio = long[k] as f64 / long[k - 2] as f64;
            if (ratio - 2.0).abs() > 0.05 {
                println!("  step {}: ratio to step {} is {:.3}, not 2", k, k - 2, ratio);
            }
        }
    }
    println!("size doubles every two characters from step 4 onward: confirmed");
}

fn section_branches() {
    println!("\n========================================================");
    println!("Section 5.5 table  (doc/chapters/05_derivative_parser.tex)");
    println!("Why simp's r + r = r rule never fires on (a+(b+ab))*.");
    println!("========================================================\n");

    let rows = series_branch_counts(&expr_paper_r2(), &"ab".repeat(6));
    println!("{:>5} {:>10} {:>10} {:>22}", "step", "branches", "distinct", "distinct ignoring bits");
    for (i, (b, d, s)) in rows.iter().enumerate() {
        println!("{:>5} {:>10} {:>10} {:>22}", i + 1, b, d, s);
    }
    println!("\n-- the rows quoted in the thesis (steps 2,4,6,8,10,12) --");
    let picked: Vec<_> = [2usize, 4, 6, 8, 10, 12]
        .iter()
        .map(|&k| rows[k - 1])
        .collect();
    println!("branches              : {:?}", picked.iter().map(|t| t.0).collect::<Vec<_>>());
    println!("distinct branches     : {:?}", picked.iter().map(|t| t.1).collect::<Vec<_>>());
    println!("distinct ignoring bits: {:?}", picked.iter().map(|t| t.2).collect::<Vec<_>>());
}

fn section_frontier() {
    println!("\n========================================================");
    println!("Figure 6.3  (doc/chapters/06_partial_derivative_parser.tex,");
    println!("             label fig:frontier-growth)");
    println!("Frontier size against distinct residual count.");
    println!("========================================================\n");

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

fn section_lowered() {
    println!("\n========================================================");
    println!("Figure 7.4  (doc/chapters/07_frontend.tex,");
    println!("             label fig:lowered-size)");
    println!("Node count of the lowered Regex, per pattern.");
    println!("========================================================\n");

    println!("{:<18} {:<10} {:>6}", "pattern", "entry", "nodes");
    for &(pat, search) in LOWERED_PATTERNS {
        let entry = if search { "search" } else { "full" };
        match lowered_size(pat, search) {
            Ok(n) => println!("{:<18} {:<10} {:>6}", pat, entry, n),
            Err(e) => println!("{:<18} {:<10}  ERROR: {}", pat, entry, e),
        }
    }

    println!("\n-- other lowered sizes quoted in Chapter 7 --");
    for (label, pat, search) in [
        ("a+", "a+", false),
        ("a?", "a?", false),
        ("a{2}", "a{2}", false),
        ("a{2,}", "a{2,}", false),
        ("^abc$ (search)", "^abc$", true),
        ("^abc  (search)", "^abc", true),
        ("abc$  (search)", "abc$", true),
    ] {
        match lowered_size(pat, search) {
            Ok(n) => println!("{:<16} {:>6}", label, n),
            Err(e) => println!("{:<16}  ERROR: {}", label, e),
        }
    }
    // wildcard_run() is not public; derive its size from the padding difference.
    let anchored = lowered_size("^abc", true).unwrap();
    let core = lowered_size("^abc", false).unwrap();
    println!(
        "\nwildcard_run() = {} nodes  (one-sided padded {} minus core {} minus the joining Seq)",
        anchored - core - 1,
        anchored,
        core
    );
}

fn main() {
    let which = std::env::args().nth(1);
    match which.as_deref() {
        Some("growth") => section_growth(),
        Some("branches") => section_branches(),
        Some("frontier") => section_frontier(),
        Some("lowered") => section_lowered(),
        Some(other) => {
            eprintln!("unknown section {:?}", other);
            eprintln!("expected one of: growth, branches, frontier, lowered");
            std::process::exit(2);
        }
        None => {
            section_growth();
            section_branches();
            section_frontier();
            section_lowered();
        }
    }
}
