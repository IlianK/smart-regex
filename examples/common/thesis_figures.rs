//! examples/common/thesis_figures.rs

//! Shared measurement helpers for the `demo_*` examples.
//! Included via `#[path = "common/thesis_figures.rs"] mod common;`.
//! Deterministic: no randomness, no timing, no environment dependence.
#![allow(dead_code)] // each demo uses a subset

use regex_engine::frontend::parse_pattern;
use regex_engine::parsers::deriv_bc::{deriv::deriv_bc, internalize::internalize, simplify::simp};
use regex_engine::parsers::pderiv_bc::pderiv::pderiv_bc;
use regex_engine::parsers::pderiv_std::pderiv_tree_residuals;
use regex_engine::regex::deriv::deriv;
use regex_engine::types::{ARegex, Regex};

// -------------------------------
// Size measures
// -------------------------------

/// Node count of a plain `Regex`, every constructor counted once.
pub fn size_regex(r: &Regex) -> usize {
    match r {
        Regex::Phi | Regex::Eps | Regex::Lit(_) => 1,
        Regex::Alt(a, b) | Regex::Seq(a, b) => 1 + size_regex(a) + size_regex(b),
        Regex::Star(a) => 1 + size_regex(a),
    }
}

/// Node count of an annotated `ARegex`.
/// Bit-lists not counted: measures the expression the derivative walks,
/// not the bits it carries.
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

/// `a*`, the case the standard simplification example uses.
pub fn expr_astar() -> Regex {
    Regex::star(Regex::lit('a'))
}

/// `(a + (b + ab))*`, right-associated.
///
/// The CLI spelling `"(a|b|ab)*"` parses to left-associated
/// `((a+b)+ab)*` instead.
/// Both produce identical size series; `tests/test_thesis_figures.rs`
/// asserts this.
pub fn expr_paper_r2() -> Regex {
    let ab = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    Regex::star(Regex::alt(Regex::lit('a'), Regex::alt(Regex::lit('b'), ab)))
}

// -------------------------------
// Derivative growth: expression size per step
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
// Branch counts: why simp's dedup never fires
// -------------------------------

/// Top-level alternation branches of `ri`.
/// Flattened through unannotated `Alt` nodes, matching `simp`'s own
/// `push_branches`.
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
// Frontier growth: strands vs. distinct residuals
// -------------------------------

/// Per character: (strands in the frontier, distinct residuals among them).
///
/// Drives the frontier as `pderiv_bc::parse::step_frontier` does.
/// `nub2` is applied inside `pderiv_bc` only, never across the flattened
/// frontier.
/// Step 0 is the one-strand starting state.
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

/// `series_frontier`'s standard (non-bit-coded) counterpart.
///
/// The two are not the same construction.
/// `pderiv_bc` builds each successor with `smart_seq` (drops a redundant
/// leading `Eps ·`).
/// `pderiv_tree` uses the plain, unconditional `Regex::seq`, so a
/// standard residual can carry a few extra `Eps ·` nodes.
/// Both sides dedup the frontier (`nub_tree` / `nub2`) before the next
/// step, so strand *counts* are directly comparable.
/// Per-residual *node* size is not, without accounting for that
/// `Eps ·` difference.
pub fn series_frontier_std(r: &Regex, input: &str) -> Vec<(usize, usize)> {
    let mut frontier: Vec<Regex> = vec![r.clone()];
    let mut out = vec![(1usize, 1usize)];
    for c in input.chars() {
        let mut next = Vec::new();
        for res in &frontier {
            next.extend(pderiv_tree_residuals(res, c));
        }
        frontier = next;
        let mut distinct: Vec<&Regex> = Vec::new();
        for res in frontier.iter() {
            if !distinct.contains(&res) {
                distinct.push(res);
            }
        }
        out.push((frontier.len(), distinct.len()));
    }
    out
}

/// Total `size_regex` summed across every strand in a `pderiv_bc`
/// frontier, per step.
/// Comparable to `series_bitcoded_simplified`'s single-tree size, since
/// a partial-derivative parser's live state is the whole frontier, not
/// any one strand.
pub fn series_frontier_total_size(r: &Regex, input: &str) -> Vec<usize> {
    let mut frontier: Vec<Regex> = vec![r.clone()];
    let mut out = vec![size_regex(r)];
    for c in input.chars() {
        let mut next = Vec::new();
        for res in &frontier {
            next.extend(pderiv_bc(res, c).into_iter().map(|(r, _)| r));
        }
        frontier = next;
        out.push(frontier.iter().map(size_regex).sum());
    }
    out
}

/// `series_frontier_total_size`'s standard (non-bit-coded) counterpart.
pub fn series_frontier_std_total_size(r: &Regex, input: &str) -> Vec<usize> {
    let mut frontier: Vec<Regex> = vec![r.clone()];
    let mut out = vec![size_regex(r)];
    for c in input.chars() {
        let mut next = Vec::new();
        for res in &frontier {
            next.extend(pderiv_tree_residuals(res, c));
        }
        frontier = next;
        out.push(frontier.iter().map(size_regex).sum());
    }
    out
}

// -------------------------------
// Frontend translation sizes
// -------------------------------

/// The patterns plotted in the translation-sizes figure.
/// `parse_pattern` is the only entry point left, so there is nothing to
/// pair a pattern against.
pub const LOWERED_PATTERNS: &[&str] =
    &["a*", "abc", "a{2,4}", r"\d", r"\d{2}", r"[a-c]+\d{2}", r"\w", "[^a]", ".", r"\w+"];

pub fn lowered_size(pattern: &str) -> Result<usize, String> {
    Ok(size_regex(&parse_pattern(pattern)?))
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

/// Prints `body` and also saves it to `reports/demos/<name>.txt`,
/// replacing any previous run -- the same "latest run, not a log"
/// convention the corpus benches use for their CSVs. These demos are
/// deterministic given the algorithms they measure (no randomness, no
/// timing, no environment dependence), so a saved copy only goes
/// stale when those algorithms themselves change.
pub fn report(name: &str, body: &str) {
    print!("{body}");
    let dir = "reports/demos";
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("could not create {dir}: {e}");
        return;
    }
    let path = format!("{dir}/{name}.txt");
    if let Err(e) = std::fs::write(&path, body) {
        eprintln!("could not write {path}: {e}");
    } else {
        eprintln!("wrote {path}");
    }
}

/// Saves `body` to `reports/demos/<name>.<ext>`, replacing any previous
/// run -- the save half of `report()`, generalized to an arbitrary
/// extension (`csv`/`json`) and decoupled from stdout: a CSV/JSON file
/// is for other tools to read, not for a terminal, so callers print
/// their own human-readable table with `println!` as before and call
/// this only to persist the compact machine-readable copy.
pub fn save(name: &str, ext: &str, body: &str) {
    let dir = "reports/demos";
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("could not create {dir}: {e}");
        return;
    }
    let path = format!("{dir}/{name}.{ext}");
    if let Err(e) = std::fs::write(&path, body) {
        eprintln!("could not write {path}: {e}");
    } else {
        eprintln!("wrote {path}");
    }
}

/// CSV-quotes a field: wraps in `"..."`, doubling any embedded `"`.
/// Same helper as `benches/bench_translate.rs`'s own `csv_quote`
/// (duplicated there rather than shared, per this codebase's
/// examples/benches split) -- needed here for the same reason: a real
/// pattern or a `Debug`-rendered `Regex`/error message routinely
/// contains a literal `,` (`a{2,5}`, `Seq(Lit('a'), Lit('b'))`, ...).
pub fn csv_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}