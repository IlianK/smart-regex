//! benches/bench_corpus_time.rs
//!
//! Wall-clock cost per parser over the real corpus.
//! One pass per source, each entry timed individually.
//! No sampling, no per-pattern group.
//!
//! A plain reporting binary rather than a Criterion target.
//! Criterion's model assumes a cheap closure measured many times; a
//! corpus pass takes seconds to hours per parser, so its statistics
//! would report sample-count variance, not corpus-scale performance.
//!
//! Run: `cargo bench --bench bench_corpus_time`

use std::time::{Duration, Instant};

use regex_engine::data::load_prepared_cases;
use regex_engine::data::prepare::core_regex;
use regex_engine::data::types::{MatchIntent, SourceKind};
use regex_engine::parsers::{
    parse_deriv_bc, parse_deriv_std_loop, parse_deriv_std_rec, parse_pderiv_bc, parse_pderiv_std,
    ParserType,
};
use regex_engine::types::Regex;

const DATA_ROOT: &str = "data/processed";
/// Default when `BENCH_PATTERN_LIMIT` is unset: no cap.
const PATTERN_LIMIT_PER_SOURCE: usize = usize::MAX;

/// `pattern_limit` for display: `usize::MAX` reads as "unlimited".
fn fmt_limit(n: usize) -> String {
    if n == usize::MAX { "unlimited".to_string() } else { n.to_string() }
}

struct BenchSelection {
    sources: Vec<SourceKind>,
    pattern_limit: usize,
    match_intent: Option<MatchIntent>,
}

fn bench_selection() -> BenchSelection {
    let sources = std::env::var("BENCH_SOURCE")
        .ok()
        .map(|s| {
            s.split(',')
                .map(|part| match part.trim().to_lowercase().as_str() {
                    "suricata" | "snort" => SourceKind::Suricata,
                    "spamassassin" => SourceKind::SpamAssassin,
                    "regexlib" => SourceKind::RegexLib,
                    "s2-elab" | "s2elab" => SourceKind::S2ELab,
                    "innovatorved" => SourceKind::Innovatorved,
                    other => panic!(
                        "BENCH_SOURCE entries must be suricata, spamassassin, regexlib, s2-elab, or innovatorved, got {other:?}"
                    ),
                })
                .collect()
        })
        .unwrap_or_default();
    let pattern_limit = std::env::var("BENCH_PATTERN_LIMIT")
        .ok()
        .map(|s| {
            s.parse().unwrap_or_else(|_| panic!("BENCH_PATTERN_LIMIT must be a positive integer, got {s:?}"))
        })
        .unwrap_or(PATTERN_LIMIT_PER_SOURCE);
    let match_intent = match std::env::var("BENCH_MATCH_INTENT").ok() {
        None => None,
        Some(s) => match s.trim().to_lowercase().as_str() {
            "both" => None,
            "direct" => Some(MatchIntent::UnanchoredAmbiguous),
            "explicit" => Some(MatchIntent::ExplicitFullMatch),
            other => panic!("BENCH_MATCH_INTENT must be direct, explicit, or both, got {other:?}"),
        },
    };
    BenchSelection { sources, pattern_limit, match_intent }
}

struct Entry {
    regex: Regex,
    input: String,
    verified_match: bool,
}

type Corpora = (Vec<Entry>, Vec<Entry>, Vec<Entry>, Vec<Entry>, Vec<Entry>);

fn load_corpus(sel: &BenchSelection) -> Corpora {
    let cases = load_prepared_cases(std::path::Path::new(DATA_ROOT), &sel.sources)
        .unwrap_or_else(|e| panic!("cannot read {DATA_ROOT}: {e} -- run dataset_prepare first"));

    let mut suricata = Vec::new();
    let mut spamassassin = Vec::new();
    let mut regexlib = Vec::new();
    let mut s2_elab = Vec::new();
    let mut innovatorved = Vec::new();
    let mut suricata_patterns = std::collections::HashSet::new();
    let mut spamassassin_patterns = std::collections::HashSet::new();
    let mut regexlib_patterns = std::collections::HashSet::new();
    let mut s2_elab_patterns = std::collections::HashSet::new();
    let mut innovatorved_patterns = std::collections::HashSet::new();

    for case in cases {
        if let Some(wanted) = sel.match_intent {
            if case.match_intent != wanted {
                continue;
            }
        }
        let (bucket, patterns) = match case.source {
            SourceKind::Suricata => (&mut suricata, &mut suricata_patterns),
            SourceKind::SpamAssassin => (&mut spamassassin, &mut spamassassin_patterns),
            SourceKind::RegexLib => (&mut regexlib, &mut regexlib_patterns),
            SourceKind::S2ELab => (&mut s2_elab, &mut s2_elab_patterns),
            SourceKind::Innovatorved => (&mut innovatorved, &mut innovatorved_patterns),
        };
        let already_admitted = patterns.contains(&case.pattern);
        if !already_admitted && patterns.len() >= sel.pattern_limit {
            continue;
        }
        // `core_regex`, not `parse_pcre_rule`.
        // `case.pattern` was accepted by `core_regex_tagged` to reach
        // `prepared.jsonl` at all; an `ExplicitFullMatch` pattern only
        // translates through `core_regex`'s `^...$` stripping.
        let regex = match core_regex(&case.pattern) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipping pre-verified case: {} -- {e}", case.pattern);
                continue;
            }
        };
        patterns.insert(case.pattern.clone());
        bucket.push(Entry {
            regex,
            input: case.input,
            verified_match: case.verified_match,
        });
    }
    if suricata.is_empty()
        && spamassassin.is_empty()
        && regexlib.is_empty()
        && s2_elab.is_empty()
        && innovatorved.is_empty()
    {
        panic!(
            "no entries loaded from {DATA_ROOT} (sources={:?}, limit={}, match_intent={:?}) -- \
             run dataset_prepare first",
            sel.sources, fmt_limit(sel.pattern_limit), sel.match_intent
        );
    }
    (suricata, spamassassin, regexlib, s2_elab, innovatorved)
}

// -------------------------------
// Reporting
// -------------------------------

type ParserFn = fn(&str, &Regex) -> Option<regex_engine::types::ParseTree>;
const PARSERS: &[(ParserType, ParserFn)] = &[
    (ParserType::DerivStdRec, parse_deriv_std_rec),
    (ParserType::DerivStdLoop, parse_deriv_std_loop),
    (ParserType::DerivBc, parse_deriv_bc),
    (ParserType::PDerivBc, parse_pderiv_bc),
    (ParserType::PDerivStd, parse_pderiv_std),
];

struct Row {
    name: &'static str,
    total: Duration,
    all: Vec<Duration>,
    matches: Vec<Duration>,
    non_matches: Vec<Duration>,
}

fn report(label: &str, corpus: &[Entry], split_outcome: bool) {
    if corpus.is_empty() {
        eprintln!("skip {label}: no entries");
        return;
    }

    // One pass per parser: time every entry, keep the durations
    // partitioned by outcome for the split table below.
    let mut rows: Vec<Row> = Vec::new();
    for (parser_type, parser) in PARSERS {
        let mut all = Vec::with_capacity(corpus.len());
        let mut matches = Vec::new();
        let mut non_matches = Vec::new();
        let start = Instant::now();
        for entry in corpus {
            let t0 = Instant::now();
            let _ = parser(&entry.input, &entry.regex);
            let dt = t0.elapsed();
            all.push(dt);
            if entry.verified_match {
                matches.push(dt);
            } else {
                non_matches.push(dt);
            }
        }
        let total = start.elapsed();
        rows.push(Row { name: parser_type.name(), total, all, matches, non_matches });
    }

    println!("\n{label} ({} entries)", corpus.len());
    println!(
        "{:<16} {:>12} {:>12} {:>12} {:>12} {:>12}",
        "parser", "total (s)", "mean (ms)", "median (ms)", "p99 (ms)", "max (ms)"
    );
    println!(
        "{:-<16} {:->12} {:->12} {:->12} {:->12} {:->12}",
        "", "", "", "", "", ""
    );
    let ms = |d: Duration| d.as_secs_f64() * 1_000.0;
    for row in &rows {
        let mut sorted = row.all.clone();
        sorted.sort();
        let n = sorted.len();
        let mean = sorted.iter().sum::<Duration>() / n as u32;
        let median = sorted[n / 2];
        let p99 = sorted[(n * 99) / 100];
        let max = *sorted.last().expect("non-empty");
        println!(
            "{:<16} {:>12.3} {:>12.3} {:>12.3} {:>12.3} {:>12.3}",
            row.name,
            row.total.as_secs_f64(),
            ms(mean),
            ms(median),
            ms(p99),
            ms(max),
        );
    }

    if !split_outcome {
        return;
    }

    println!();
    println!("{label}, by outcome");
    println!(
        "{:<16} {:>12} {:>12} {:>12} {:>12}",
        "parser", "matches", "match (s)", "non-match", "non-match (s)"
    );
    println!(
        "{:-<16} {:->12} {:->12} {:->12} {:->12}",
        "", "", "", "", ""
    );
    for row in &rows {
        let mt = row.matches.iter().sum::<Duration>();
        let nt = row.non_matches.iter().sum::<Duration>();
        println!(
            "{:<16} {:>12} {:>12.3} {:>12} {:>12.3}",
            row.name,
            row.matches.len(),
            mt.as_secs_f64(),
            row.non_matches.len(),
            nt.as_secs_f64(),
        );
    }
}

fn main() {
    let sel = bench_selection();
    let split_outcome = std::env::var("BENCH_SPLIT_OUTCOME")
        .ok()
        .map(|s| match s.trim().to_lowercase().as_str() {
            "1" | "true" | "yes" => true,
            "0" | "false" | "no" => false,
            other => panic!("BENCH_SPLIT_OUTCOME must be 1 or 0, got {other:?}"),
        })
        .unwrap_or(false);
    let (suricata, spamassassin, regexlib, s2_elab, innovatorved) = load_corpus(&sel);
    eprintln!(
        "bench_corpus_time: {} suricata, {} spamassassin, {} regexlib, {} s2-elab, {} innovatorved \
         (limit {} each, sources={:?}, match_intent={:?}, split_outcome={})",
        suricata.len(),
        spamassassin.len(),
        regexlib.len(),
        s2_elab.len(),
        innovatorved.len(),
        fmt_limit(sel.pattern_limit),
        sel.sources,
        sel.match_intent,
        split_outcome,
    );
    report("dataset_suricata", &suricata, split_outcome);
    report("dataset_spamassassin", &spamassassin, split_outcome);
    report("dataset_regexlib", &regexlib, split_outcome);
    report("dataset_s2_elab", &s2_elab, split_outcome);
    report("dataset_innovatorved", &innovatorved, split_outcome);
}