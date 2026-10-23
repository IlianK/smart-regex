//! examples/demo_translate.rs
//!
//! cargo run --release --example demo_translate -- <s2-elab-json-file...> [--diag 0|1|2|3]
//!
//! Translates real S2ELab patterns and runs a spread of them against
//! their own real match/non-match examples. A compact run summary is
//! saved to reports/demos/translate.csv; every pattern this run
//! translates successfully (not just the few shown on stdout) -- raw
//! text plus its translated `Regex` -- is a separate listing, saved to
//! reports/demos/translate_patterns.csv. Breadth-of-categories coverage
//! lives in `demo_translate_categories` (synthetic examples) instead --
//! this file only ever shows what the given dataset happens to contain.

#[path = "common/thesis_figures.rs"]
mod common;

use std::path::PathBuf;

use common::{csv_quote, save};

use regex_engine::data::extract::extract_s2_elab;
use regex_engine::data::prepare::core_regex;
use regex_engine::data::types::RuleContext;
use regex_engine::diagnostics::{run_parser, DiagConfig, DiagLevel};
use regex_engine::parsers::ParserType;
use regex_engine::parsers::deriv_bc::deriv::deriv_bc;
use regex_engine::parsers::deriv_bc::internalize::internalize;
use regex_engine::parsers::deriv_bc::nullable::{is_phi, nullable_bc};
use regex_engine::parsers::deriv_bc::simplify::simp;
use regex_engine::types::{ARegex, Regex};

/// Node-count cap so picking a real example can't itself blow up on a
/// pathological (pattern, input) pair -- this tool runs no process isolation.
const MAX_CHECK_NODES: usize = 100_000;

/// How many translated-and-verified patterns to show running on stdout.
const SAMPLE_COUNT: usize = 5;

fn aregex_node_count(r: &ARegex) -> usize {
    let mut stack = vec![r];
    let mut count = 0usize;
    while let Some(node) = stack.pop() {
        count += 1;
        if count > MAX_CHECK_NODES {
            return count;
        }
        match node {
            ARegex::Phi | ARegex::Eps(_) | ARegex::Lit(_, _) => {}
            ARegex::Alt(_, a, b) | ARegex::Seq(_, a, b) => {
                stack.push(a);
                stack.push(b);
            }
            ARegex::Star(_, a) => stack.push(a),
        }
    }
    count
}

/// Whether `input` matches `r`, via the bit-coded engine; `None` past the
/// node cap rather than risking the blowup an unbounded check could hit.
fn matches_bounded(input: &str, r: &Regex) -> Option<bool> {
    let mut ri = internalize(r);
    if aregex_node_count(&ri) > MAX_CHECK_NODES {
        return None;
    }
    for c in input.chars() {
        ri = simp(deriv_bc(ri, c));
        if aregex_node_count(&ri) > MAX_CHECK_NODES {
            return None;
        }
        if is_phi(&ri) {
            return Some(false);
        }
    }
    Some(nullable_bc(&ri))
}

struct RealPattern {
    raw_pattern: String,
    matches: Vec<String>,
    non_matches: Vec<String>,
}

/// One of `pattern`'s own given examples that the real parser agrees
/// with; `None` if none of them do. Never generated, never re-labeled.
fn real_example<'a>(core: &Regex, pool: &'a [String], want_match: bool) -> Option<&'a str> {
    pool.iter().map(String::as_str).find(|t| matches_bounded(t, core) == Some(want_match))
}

fn has_real_example(core: &Regex, pattern: &RealPattern) -> bool {
    real_example(core, &pattern.matches, true).is_some()
        && real_example(core, &pattern.non_matches, false).is_some()
}

/// Shows `core` (already translated from `pattern.raw_pattern`) run
/// against one of the pattern's own real matches and real non-matches.
fn demo_accepted(pattern: &RealPattern, core: &Regex, diag: DiagLevel) {
    let config = DiagConfig::new(diag, ParserType::DerivStdRec, None);
    for (text, label) in [
        (real_example(core, &pattern.matches, true), "match"),
        (real_example(core, &pattern.non_matches, false), "non-match"),
    ]
    .into_iter()
    .filter_map(|(t, label)| t.map(|t| (t, label)))
    {
        println!("  real {label}: {:?}", text);
        run_parser(&pattern.raw_pattern, core, text, &config);
    }
}

fn load_all_patterns(files: &[PathBuf]) -> Vec<RealPattern> {
    let mut out = Vec::new();
    for path in files {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("skipping {}: {e}", path.display());
                continue;
            }
        };
        for rule in extract_s2_elab(&text) {
            if let RuleContext::S2ELab { matches, non_matches } = rule.context {
                out.push(RealPattern { raw_pattern: rule.raw_pattern, matches, non_matches });
            }
        }
    }
    out
}

/// `want` indices spread evenly across `0..len`; all of them if `len <= want`.
fn spread_indices(len: usize, want: usize) -> Vec<usize> {
    if len == 0 {
        return Vec::new();
    }
    if len <= want || want <= 1 {
        return (0..len.min(want.max(1))).collect();
    }
    (0..want).map(|i| i * (len - 1) / (want - 1)).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "usage: cargo run --release --example demo_translate -- \
             <s2-elab-json-file> [file...] [--diag 0|1|2|3]"
        );
        std::process::exit(2);
    }

    let mut diag = DiagLevel::Basic;
    let mut files: Vec<PathBuf> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--diag" => {
                i += 1;
                diag = match args.get(i).map(String::as_str) {
                    Some("0") => DiagLevel::Off,
                    Some("1") => DiagLevel::Basic,
                    Some("2") => DiagLevel::Verbose,
                    Some("3") => DiagLevel::Debug,
                    _ => {
                        eprintln!("--diag needs 0, 1, 2, or 3");
                        std::process::exit(2);
                    }
                };
            }
            other => files.push(PathBuf::from(other)),
        }
        i += 1;
    }

    if files.is_empty() {
        eprintln!("no dataset files given");
        std::process::exit(2);
    }

    let patterns = load_all_patterns(&files);
    println!("{} patterns from {} file(s)", patterns.len(), files.len());

    let translated: Vec<(&RealPattern, Regex)> = patterns
        .iter()
        .filter_map(|p| core_regex(&p.raw_pattern).ok().map(|r| (p, r)))
        .collect();
    println!("{} of {} translate successfully", translated.len(), patterns.len());
    println!();

    let mut verified: Vec<&(&RealPattern, Regex)> =
        translated.iter().filter(|(p, r)| has_real_example(r, p)).collect();
    verified.sort_by_key(|(p, _)| p.raw_pattern.len());

    let picks = spread_indices(verified.len(), SAMPLE_COUNT);
    println!(
        "Showing {} translated patterns (a size spread) run against their own real input:",
        picks.len()
    );
    println!();
    for &idx in &picks {
        let (pattern, core) = verified[idx];
        println!("raw: {}", pattern.raw_pattern);
        demo_accepted(pattern, core, diag);
        println!();
    }

    let mut summary_csv = String::new();
    summary_csv.push_str("files,total_patterns,translated,translate_rate_pct\n");
    summary_csv.push_str(&format!(
        "{},{},{},{:.1}\n",
        files.len(),
        patterns.len(),
        translated.len(),
        if patterns.is_empty() { 0.0 } else { 100.0 * translated.len() as f64 / patterns.len() as f64 },
    ));
    save("translate", "csv", &summary_csv);

    let mut patterns_csv = String::new();
    patterns_csv.push_str("raw_pattern,regex\n");
    for (pattern, core) in &translated {
        patterns_csv.push_str(&format!("{},{}\n", csv_quote(&pattern.raw_pattern), csv_quote(&format!("{:?}", core))));
    }
    save("translate_patterns", "csv", &patterns_csv);
}
