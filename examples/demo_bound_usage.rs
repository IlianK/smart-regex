//! examples/demo_bound_usage.rs
//!
//! How large a `{m,n}` bound real-world rules actually use, as evidence
//! for why `MAX_BOUND` (`src/frontend/translate.rs`) is set where it is:
//! walks every S2ELab pattern's `ExtPat`, collects every `Bound(_, lo, hi)`
//! node found anywhere in it (not just top-level), and reports the
//! distribution of `hi.unwrap_or(lo)` -- the largest repeat count each
//! bound actually asks for.
//!
//! cargo run --release --example demo_bound_usage

use regex_engine::data::extract::extract_s2_elab;
use regex_engine::frontend::{parse_ext_pattern, strip_pcre_delimiters, ExtPat};

fn collect_bounds(ep: &ExtPat, out: &mut Vec<u32>) {
    match ep {
        ExtPat::Bound(inner, lo, hi) => {
            out.push(hi.unwrap_or(*lo).max(*lo));
            collect_bounds(inner, out);
        }
        ExtPat::Group(inner)
        | ExtPat::GroupNonMarking(inner)
        | ExtPat::Opt(inner)
        | ExtPat::Plus(inner)
        | ExtPat::Star(inner) => collect_bounds(inner, out),
        ExtPat::Or(parts) | ExtPat::Concat(parts) => {
            for p in parts {
                collect_bounds(p, out);
            }
        }
        _ => {}
    }
}

fn main() {
    let raw = std::fs::read_to_string("data/raw/s2-elab/RegexEval.json")
        .expect("reading data/raw/s2-elab/RegexEval.json (run from the repo root)");
    let rules = extract_s2_elab(&raw);
    println!("S2ELab: {} raw patterns", rules.len());

    let mut all_bounds: Vec<u32> = Vec::new();
    let mut patterns_with_bound = 0usize;
    let mut parse_errors = 0usize;

    for rule in &rules {
        let (body, _flags) = strip_pcre_delimiters(&rule.raw_pattern);
        let ep = match parse_ext_pattern(body) {
            Ok(ep) => ep,
            Err(_) => {
                parse_errors += 1;
                continue;
            }
        };
        let mut bounds = Vec::new();
        collect_bounds(&ep, &mut bounds);
        if !bounds.is_empty() {
            patterns_with_bound += 1;
            all_bounds.extend(bounds);
        }
    }

    all_bounds.sort_unstable();
    println!("patterns that failed to parse: {parse_errors}");
    println!("patterns containing at least one {{m,n}}/{{m,}}/{{m}} bound: {patterns_with_bound}");
    println!("total bound occurrences: {}", all_bounds.len());
    if let Some(&max) = all_bounds.last() {
        println!("largest single bound value (hi, or lo if unbounded above): {max}");
    }
    for pct in [50, 90, 95, 99] {
        if all_bounds.is_empty() {
            break;
        }
        let idx = (all_bounds.len() * pct / 100).min(all_bounds.len() - 1);
        println!("p{pct}: {}", all_bounds[idx]);
    }
    let over_100 = all_bounds.iter().filter(|&&v| v > 100).count();
    let over_1000 = all_bounds.iter().filter(|&&v| v > 1000).count();
    println!("bound occurrences > 100: {over_100}");
    println!("bound occurrences > 1000 (would be rejected by MAX_BOUND): {over_1000}");
}
