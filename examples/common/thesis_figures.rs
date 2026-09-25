//! Shared measurement helpers for the `demo_*` example binaries. Not an
//! example itself (nested under `examples/common/`, so cargo's example
//! autodiscovery doesn't pick it up); included via
//! `#[path = "common/thesis_figures.rs"] mod common;` from each demo that
//! needs it. Deterministic: no randomness, no timing, no environment
//! dependence.
#![allow(dead_code)] // each demo_*.rs only uses a subset of these

use regex_engine::frontend::{parse_dataset_pattern, parse_pattern};
use regex_engine::parsers::deriv_bc::{deriv::deriv_bc, internalize::internalize, simplify::simp};
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
    Regex::star(Regex::alt(Regex::lit('a'), Regex::alt(Regex::lit('b'), ab)))
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
pub fn alt_branches(ri: &ARegex, out: &mut Vec<ARegex>) {
    match ri {
        ARegex::Alt(bs, a, b) if bs.is_empty() => {
            alt_branches(a, out);
            alt_branches(b, out);
        }
        other => out.push(other.clone()),
    }
}

/// Erase every bit annotation, keeping only structural shape.
pub fn strip_bits(ri: &ARegex) -> ARegex {
    match ri {
        ARegex::Phi => ARegex::Phi,
        ARegex::Eps(_) => ARegex::Eps(vec![]),
        ARegex::Lit(_, c) => ARegex::Lit(vec![], *c),
        ARegex::Alt(_, a, b) => ARegex::Alt(vec![], Box::new(strip_bits(a)), Box::new(strip_bits(b))),
        ARegex::Seq(_, a, b) => ARegex::Seq(vec![], Box::new(strip_bits(a)), Box::new(strip_bits(b))),
        ARegex::Star(_, a) => ARegex::Star(vec![], Box::new(strip_bits(a))),
    }
}

/// `ARegex` has no `Hash`, so distinctness is counted by linear scan.
pub fn count_distinct(items: &[ARegex]) -> usize {
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

pub fn coords(pairs: &[(usize, usize)]) -> String {
    pairs.iter().map(|(x, y)| format!("({},{})", x, y)).collect::<Vec<_>>().join(" ")
}

pub fn enumerate_series(v: &[usize]) -> Vec<(usize, usize)> {
    v.iter().enumerate().map(|(i, &y)| (i, y)).collect()
}
