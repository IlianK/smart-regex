//! benches/bench_dataset.rs
//!
//! Benchmarks the four parsers against real corpus patterns.
//!
//! Run: `cargo bench --bench bench_dataset`

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use regex_engine::data::load_prepared_cases;
use regex_engine::data::prepare::core_regex;
use regex_engine::data::types::SourceKind;
use regex_engine::parsers::{
    parse_deriv_bc, parse_deriv_std_loop, parse_deriv_std_rec, parse_pderiv_bc, parse_pderiv_std,
    ParserType,
};
use regex_engine::types::Regex;

const DATA_ROOT: &str = "data/processed";
const PATTERN_LIMIT_PER_SOURCE: usize = 30;
struct Entry {
    regex: Regex,
    input: String,
}
struct BenchSelection {
    sources: Vec<SourceKind>,
    pattern_limit: usize,
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
                    other => panic!(
                        "BENCH_SOURCE entries must be suricata, spamassassin, or regexlib, got {other:?}"
                    ),
                })
                .collect()
        })
        .unwrap_or_default(); // empty Vec -> load_prepared_cases's own default sources (Suricata, SpamAssassin)
    let pattern_limit = std::env::var("BENCH_PATTERN_LIMIT")
        .ok()
        .map(|s| {
            s.parse().unwrap_or_else(|_| panic!("BENCH_PATTERN_LIMIT must be a positive integer, got {s:?}"))
        })
        .unwrap_or(PATTERN_LIMIT_PER_SOURCE);
    BenchSelection { sources, pattern_limit }
}

/// One `Vec<Entry>` per source, each capped at `pattern_limit` distinct
/// patterns.
/// Surveys pattern diversity within a source, not one pattern's candidates.
/// Source is the dimension examples like `demo_parser_agreement` group by.
fn load_corpus(sel: &BenchSelection, pattern_limit: usize) -> (Vec<Entry>, Vec<Entry>, Vec<Entry>) {
    let cases = load_prepared_cases(std::path::Path::new(DATA_ROOT), &sel.sources)
        .unwrap_or_else(|e| panic!("cannot read {DATA_ROOT}: {e} -- run dataset_prepare first"));

    let mut suricata = Vec::new();
    let mut spamassassin = Vec::new();
    let mut regexlib = Vec::new();
    let mut suricata_patterns = std::collections::HashSet::new();
    let mut spamassassin_patterns = std::collections::HashSet::new();
    let mut regexlib_patterns = std::collections::HashSet::new();

    for case in cases {
        let (bucket, patterns) = match case.source {
            SourceKind::Suricata => (&mut suricata, &mut suricata_patterns),
            SourceKind::SpamAssassin => (&mut spamassassin, &mut spamassassin_patterns),
            SourceKind::RegexLib => (&mut regexlib, &mut regexlib_patterns),
        };
        let already_admitted = patterns.contains(&case.pattern);
        if !already_admitted && patterns.len() >= pattern_limit {
            continue;
        }
        // `core_regex`, not `parse_pcre_rule`.
        // `case.pattern` was accepted by `core_regex_tagged` to reach
        // `prepared.jsonl` at all.
        // Some accepted patterns (`MatchIntent::ExplicitFullMatch`) only
        // translate through `core_regex`'s `^...$`-stripping.
        // Plain `parse_pcre_rule` would silently drop them as unparseable.
        let regex = match core_regex(&case.pattern) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipping pre-verified case: {} -- {e}", case.pattern);
                continue;
            }
        };
        patterns.insert(case.pattern.clone());
        bucket.push(Entry { regex, input: case.input });
    }
    if suricata.is_empty() && spamassassin.is_empty() && regexlib.is_empty() {
        panic!("{}", empty_corpus_diagnosis(sel));
    }
    (suricata, spamassassin, regexlib)
}

fn empty_corpus_diagnosis(sel: &BenchSelection) -> String {
    let root = std::path::Path::new(DATA_ROOT);
    let resolved = std::fs::canonicalize(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| format!("{DATA_ROOT} (missing)"));
    let mut lines = vec![format!(
        "no entries loaded from {DATA_ROOT} ({resolved}); \
         sources={:?} (empty = default: suricata, spamassassin) limit={}",
        sel.sources, sel.pattern_limit
    )];
    for source in [SourceKind::Suricata, SourceKind::SpamAssassin, SourceKind::RegexLib] {
        let path = root.join(source.dir_name()).join("prepared.jsonl");
        let status = match std::fs::read_to_string(&path) {
            Ok(text) => format!("{} lines", text.lines().filter(|l| !l.trim().is_empty()).count()),
            Err(e) => format!("unreadable ({e})"),
        };
        lines.push(format!("  {} -> {}", path.display(), status));
    }
    lines.push(
        "run dataset_prepare for any source above showing 0 lines or unreadable".to_string(),
    );
    lines.join("\n")
}

type ParserFn = fn(&str, &Regex) -> Option<regex_engine::types::ParseTree>;
const PARSERS: &[(ParserType, ParserFn)] = &[
    (ParserType::DerivStdRec, parse_deriv_std_rec),
    (ParserType::DerivStdLoop, parse_deriv_std_loop),
    (ParserType::DerivBc, parse_deriv_bc),
    (ParserType::PDerivBc, parse_pderiv_bc),
    (ParserType::PDerivStd, parse_pderiv_std),
];

fn bench_by_source(c: &mut Criterion) {
    let sel = bench_selection();
    let (suricata, spamassassin, regexlib) = load_corpus(&sel, sel.pattern_limit);
    eprintln!(
        "bench_dataset: {} suricata, {} spamassassin, {} regexlib (limit {} each, sources={:?})",
        suricata.len(),
        spamassassin.len(),
        regexlib.len(),
        sel.pattern_limit,
        sel.sources,
    );

    for (label, corpus) in [
        ("dataset_suricata", &suricata),
        ("dataset_spamassassin", &spamassassin),
        ("dataset_regexlib", &regexlib),
    ] {
        if corpus.is_empty() {
            eprintln!("skip {label}: no entries");
            continue;
        }
        let mut group = c.benchmark_group(label);
        group.sample_size(20);
        for (parser_type, parser) in PARSERS {
            group.bench_function(parser_type.name(), |b| {
                b.iter(|| {
                    for entry in corpus {
                        black_box(parser(black_box(&entry.input), black_box(&entry.regex)));
                    }
                })
            });
        }
        group.finish();
    }
}


fn bench_per_pattern(c: &mut Criterion) {
    let sel = bench_selection();
    let (suricata, spamassassin, regexlib) = load_corpus(&sel, 5);
    let combined: Vec<Entry> = suricata.into_iter().chain(spamassassin).chain(regexlib).collect();

    let mut group = c.benchmark_group("dataset_per_pattern");
    group.sample_size(10);

    for (i, entry) in combined.iter().enumerate() {
        let label = format!("{}_{}", i, truncate(&entry.input, 24));
        for (parser_type, parser) in PARSERS {
            group.bench_with_input(BenchmarkId::new(parser_type.name(), &label), entry, |b, entry| {
                b.iter(|| parser(black_box(&entry.input), black_box(&entry.regex)))
            });
        }
    }

    group.finish();
}

fn truncate(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    t.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

criterion_group!(benches, bench_by_source, bench_per_pattern);
criterion_main!(benches);