//! examples/demo_parser_agreement.rs
//!
//! cargo run --release --example demo_parser_agreement
//!
//! Cross-parser agreement over the prepared dataset 
//! (`data/processed/`, from `dataset_prepare`). 
//! For every (pattern, input) entry, all five parsers run and three separate checks apply: 
//! - POSIX among themselves, 
//! - Greedy among themselves 
//! - Membership agreement on all

use std::sync::mpsc;
use std::time::Duration;

use regex_engine::data::load_prepared_cases;
use regex_engine::data::types::{Category, PreparedCase, SourceKind};
use regex_engine::frontend::parse_pcre_rule;
use regex_engine::parsers::{
    parse_deriv_bc, parse_deriv_std_loop, parse_deriv_std_rec, parse_pderiv_bc, parse_pderiv_std,
    ParseTree,
};
use regex_engine::types::Regex;

const DATA_ROOT: &str = "data/processed";
const DEFAULT_TIMEOUT_MS: u64 = 2000;
/// Cap on printed instances per bug kind; beyond this, tally only -- a
/// corpus-wide bug would otherwise flood the terminal with the same finding.
const MAX_PRINTED_PER_BUG_KIND: usize = 20;

const SOURCES: [SourceKind; 3] = [SourceKind::Suricata, SourceKind::SpamAssassin, SourceKind::RegexLib];
const CATEGORIES: [Category; 3] = [Category::Best, Category::Neutral, Category::Worst];

/// `Category` derives Eq but not Hash, so grouping uses a positional index.
fn source_idx(s: SourceKind) -> usize {
    SOURCES.iter().position(|&x| x == s).expect("exhaustive")
}
fn category_idx(c: Category) -> usize {
    CATEGORIES.iter().position(|&x| x == c).expect("exhaustive")
}

struct Selection {
    category: Option<Category>,
    sources: Vec<SourceKind>,
    pattern_limit: Option<usize>,
    timeout: Duration,
}

fn selection() -> Selection {
    let category = std::env::var("BENCH_CATEGORY").ok().map(|s| match s.to_lowercase().as_str() {
        "best" => Category::Best,
        "neutral" => Category::Neutral,
        "worst" => Category::Worst,
        other => panic!("BENCH_CATEGORY must be best, neutral, or worst, got {other:?}"),
    });
    let sources = std::env::var("BENCH_SOURCE")
        .ok()
        .map(|s| {
            s.split(',')
                .map(|part| match part.trim().to_lowercase().as_str() {
                    "suricata" | "snort" => SourceKind::Suricata,
                    "spamassassin" => SourceKind::SpamAssassin,
                    "regexlib" => SourceKind::RegexLib,
                    other => panic!(
                        "BENCH_SOURCE entries must be suricata, spamassassin, or regexlib, got {other:?}"
                    ),
                })
                .collect()
        })
        .unwrap_or_default();
    let pattern_limit = std::env::var("BENCH_PATTERN_LIMIT").ok().map(|s| {
        s.parse().unwrap_or_else(|_| panic!("BENCH_PATTERN_LIMIT must be a positive integer, got {s:?}"))
    });
    let timeout_ms = std::env::var("AGREEMENT_TIMEOUT_MS")
        .ok()
        .map(|s| {
            s.parse().unwrap_or_else(|_| panic!("AGREEMENT_TIMEOUT_MS must be a positive integer, got {s:?}"))
        })
        .unwrap_or(DEFAULT_TIMEOUT_MS);
    Selection { category, sources, pattern_limit, timeout: Duration::from_millis(timeout_ms) }
}

/// Runs `f` on its own thread with a wall-clock budget. `None` means the
/// call did not return in time; that thread runs to completion (or forever)
/// in the background, since Rust cannot cancel it.
fn with_timeout<F, T>(timeout: Duration, f: F) -> Option<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(timeout).ok()
}

/// One parser's outcome for one entry.
enum Outcome {
    Done(Option<ParseTree>),
    TimedOut,
}

fn run_parser(
    timeout: Duration,
    parser: fn(&str, &Regex) -> Option<ParseTree>,
    input: &str,
    regex: &Regex,
) -> Outcome {
    let input = input.to_string();
    let regex = regex.clone();
    match with_timeout(timeout, move || parser(&input, &regex)) {
        Some(result) => Outcome::Done(result),
        None => Outcome::TimedOut,
    }
}

/// Loads `sel`'s cases; `pattern_limit` caps distinct patterns per category,
/// matching `bench_dataset.rs`'s `load_corpus`. Unset loads everything.
fn select_cases(sel: &Selection) -> Vec<PreparedCase> {
    let cases = load_prepared_cases(std::path::Path::new(DATA_ROOT), &sel.sources, sel.category)
        .unwrap_or_else(|e| {
            panic!(
                "couldn't read prepared cases under {} ({e}) -- run `cargo run --release --example \
                 dataset_prepare -- <suricata|spamassassin|regexlib> <file>...` first; see \
                 docs/DATASETS.md",
                DATA_ROOT
            )
        });
    let Some(limit) = sel.pattern_limit else {
        return cases;
    };
    let mut admitted: [[std::collections::HashSet<String>; 3]; 3] = Default::default();
    cases
        .into_iter()
        .filter(|case| {
            let patterns = &mut admitted[source_idx(case.source)][category_idx(case.category)];
            let already_admitted = patterns.contains(&case.pattern);
            if !already_admitted && patterns.len() >= limit {
                return false;
            }
            patterns.insert(case.pattern.clone());
            true
        })
        .collect()
}

#[derive(Default)]
struct Tally {
    entries: usize,
    /// At least one parser timed out -- not a bug.
    timeouts: usize,
    membership_bugs: usize,
    posix_shape_bugs: usize,
    greedy_shape_bugs: usize,
    /// Entries where both families were internally consistent and matched,
    /// so the POSIX-vs-Greedy shape comparison was meaningful.
    cross_compared: usize,
    cross_same_shape: usize,
    cross_different_shape: usize,
}

const PARSER_NAMES: [&str; 5] = ["rec", "loop", "bc", "pstd", "pbc"];

/// Names of whichever parsers timed out, in call order; `None` if none did.
fn timed_out_names(outcomes: &[Outcome; 5]) -> Option<Vec<&'static str>> {
    let names: Vec<&'static str> = outcomes
        .iter()
        .zip(PARSER_NAMES)
        .filter(|(o, _)| matches!(o, Outcome::TimedOut))
        .map(|(_, name)| name)
        .collect();
    if names.is_empty() { None } else { Some(names) }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "..."
    }
}

fn main() {
    let sel = selection();
    let cases = select_cases(&sel);
    if cases.is_empty() {
        panic!(
            "no prepared cases loaded from {DATA_ROOT} for category={:?} sources={:?} -- see \
             docs/DATASETS.md",
            sel.category, sel.sources
        );
    }

    let mut tallies: [[Tally; 3]; 3] = Default::default();
    let mut printed_membership = 0usize;
    let mut printed_posix = 0usize;
    let mut printed_greedy = 0usize;
    let mut printed_timeout = 0usize;

    for case in &cases {
        let regex = match parse_pcre_rule(&case.pattern) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipping prepared case (should have been pre-verified): {} -- {}", case.pattern, e);
                continue;
            }
        };

        let outcomes = [
            run_parser(sel.timeout, parse_deriv_std_rec, &case.input, &regex),
            run_parser(sel.timeout, parse_deriv_std_loop, &case.input, &regex),
            run_parser(sel.timeout, parse_deriv_bc, &case.input, &regex),
            run_parser(sel.timeout, parse_pderiv_std, &case.input, &regex),
            run_parser(sel.timeout, parse_pderiv_bc, &case.input, &regex),
        ];

        let tally = &mut tallies[source_idx(case.source)][category_idx(case.category)];
        tally.entries += 1;

        if let Some(names) = timed_out_names(&outcomes) {
            tally.timeouts += 1;
            if printed_timeout < MAX_PRINTED_PER_BUG_KIND {
                println!(
                    "TIMEOUT (>{}ms)  {:?}/{:?}  pattern={:?}  input={:?}  timed_out=[{}]",
                    sel.timeout.as_millis(), case.source, case.category, truncate(&case.pattern, 60),
                    truncate(&case.input, 40), names.join(",")
                );
                printed_timeout += 1;
            }
            continue;
        }
        let [rec, loop_, bc, pstd, pbc] = outcomes.map(|o| match o {
            Outcome::Done(r) => r,
            Outcome::TimedOut => unreachable!("checked by timed_out_names above"),
        });

        let all_matched = [rec.is_some(), loop_.is_some(), bc.is_some(), pstd.is_some(), pbc.is_some()];
        let membership_ok = all_matched.iter().all(|&m| m == case.verified_match);
        if !membership_ok {
            tally.membership_bugs += 1;
            if printed_membership < MAX_PRINTED_PER_BUG_KIND {
                println!(
                    "MEMBERSHIP BUG  {:?}/{:?}  pattern={:?}  input={:?}  expected_match={}  \
                     got=[rec={} loop={} bc={} pstd={} pbc={}]",
                    case.source, case.category, truncate(&case.pattern, 60), truncate(&case.input, 40),
                    case.verified_match, all_matched[0], all_matched[1], all_matched[2], all_matched[3],
                    all_matched[4]
                );
                printed_membership += 1;
            }
            continue;
        }

        if !case.verified_match {
            // Verified non-match: every parser returned None; no trees to compare.
            continue;
        }

        let posix_shape_ok = rec == loop_ && loop_ == bc;
        if !posix_shape_ok {
            tally.posix_shape_bugs += 1;
            if printed_posix < MAX_PRINTED_PER_BUG_KIND {
                println!(
                    "POSIX SHAPE BUG  {:?}/{:?}  pattern={:?}  input={:?}\n  rec:  {:?}\n  loop: {:?}\n  bc:   {:?}",
                    case.source, case.category, truncate(&case.pattern, 60), truncate(&case.input, 40),
                    rec, loop_, bc
                );
                printed_posix += 1;
            }
        }

        let greedy_shape_ok = pstd == pbc;
        if !greedy_shape_ok {
            tally.greedy_shape_bugs += 1;
            if printed_greedy < MAX_PRINTED_PER_BUG_KIND {
                println!(
                    "GREEDY SHAPE BUG  {:?}/{:?}  pattern={:?}  input={:?}\n  pstd: {:?}\n  pbc:  {:?}",
                    case.source, case.category, truncate(&case.pattern, 60), truncate(&case.input, 40),
                    pstd, pbc
                );
                printed_greedy += 1;
            }
        }

        if posix_shape_ok && greedy_shape_ok {
            tally.cross_compared += 1;
            let posix_tree: &Option<ParseTree> = &bc;
            let greedy_tree: &Option<ParseTree> = &pbc;
            if posix_tree == greedy_tree {
                tally.cross_same_shape += 1;
            } else {
                tally.cross_different_shape += 1;
            }
        }
    }

    println!();
    println!(
        "{:<13} {:<8} {:>8} {:>9} {:>10} {:>11} {:>12} {:>11} {:>11} {:>9}",
        "Source", "Category", "Entries", "Timeouts", "Mem.bugs", "POSIX-bugs", "Greedy-bugs", "Compared",
        "Same", "Diff"
    );
    println!("{}", "-".repeat(117));

    let mut total = Tally::default();
    for &source in &SOURCES {
        for &category in &CATEGORIES {
            let t = &tallies[source_idx(source)][category_idx(category)];
            if t.entries == 0 {
                continue;
            }
            let diff_pct = if t.cross_compared > 0 {
                100.0 * t.cross_different_shape as f64 / t.cross_compared as f64
            } else {
                0.0
            };
            println!(
                "{:<13} {:<8} {:>8} {:>9} {:>10} {:>11} {:>12} {:>11} {:>11} {:>8.1}%",
                format!("{:?}", source), format!("{:?}", category), t.entries, t.timeouts,
                t.membership_bugs, t.posix_shape_bugs, t.greedy_shape_bugs, t.cross_compared,
                t.cross_same_shape, diff_pct
            );
            total.entries += t.entries;
            total.timeouts += t.timeouts;
            total.membership_bugs += t.membership_bugs;
            total.posix_shape_bugs += t.posix_shape_bugs;
            total.greedy_shape_bugs += t.greedy_shape_bugs;
            total.cross_compared += t.cross_compared;
            total.cross_same_shape += t.cross_same_shape;
            total.cross_different_shape += t.cross_different_shape;
        }
    }
    println!("{}", "-".repeat(117));
    let total_diff_pct = if total.cross_compared > 0 {
        100.0 * total.cross_different_shape as f64 / total.cross_compared as f64
    } else {
        0.0
    };
    println!(
        "{:<13} {:<8} {:>8} {:>9} {:>10} {:>11} {:>12} {:>11} {:>11} {:>8.1}%",
        "Total", "-", total.entries, total.timeouts, total.membership_bugs, total.posix_shape_bugs,
        total.greedy_shape_bugs, total.cross_compared, total.cross_same_shape, total_diff_pct
    );
    println!();
    println!(
        "\"Compared\" = matching entries where both families were internally consistent, so a \
         POSIX-vs-Greedy shape comparison was meaningful; \"Same\"/\"Diff\" split that count. \
         Diff is not a bug: Chapter 6 only requires membership agreement across policies. \
         \"Timeouts\" is also not a bug: deriv_std_rec/deriv_std_loop run unsimplified, so a \
         large search-padded pattern can make them infeasible regardless of category -- see the \
         module doc comment."
    );

    let bugs = total.membership_bugs + total.posix_shape_bugs + total.greedy_shape_bugs;
    if bugs > 0 {
        println!(
            "\n{} BUG(S) FOUND across {} entries -- see the lines above (capped at {} per kind).",
            bugs, total.entries, MAX_PRINTED_PER_BUG_KIND
        );
        std::process::exit(1);
    } else {
        println!(
            "\nNo membership or within-policy shape bugs found across {} entries ({} timed out, \
             not counted as bugs).",
            total.entries, total.timeouts
        );
    }
}