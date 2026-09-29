//! Benchmarks the four parsers against real corpus patterns 
//!
//! Run: `cargo bench --bench bench_dataset`

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use regex_engine::data::load_prepared_cases;
use regex_engine::data::types::{Category, SourceKind};
use regex_engine::frontend::parse_pcre_rule;
use regex_engine::parsers::{
    parse_deriv_bc, parse_deriv_std_loop, parse_deriv_std_rec, parse_pderiv_bc, parse_pderiv_std,
    ParserType,
};
use regex_engine::types::Regex;

const DATA_ROOT: &str = "data/processed";
const PATTERN_LIMIT_PER_CATEGORY: usize = 30;
struct Entry {
    regex: Regex,
    input: String,
}
struct BenchSelection {
    category: Option<Category>,
    sources: Vec<SourceKind>,
    pattern_limit: usize,
}

fn bench_selection() -> BenchSelection {
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
        .unwrap_or_default(); // empty Vec -> load_prepared_cases's own "all sources" default
    let pattern_limit = std::env::var("BENCH_PATTERN_LIMIT")
        .ok()
        .map(|s| {
            s.parse().unwrap_or_else(|_| panic!("BENCH_PATTERN_LIMIT must be a positive integer, got {s:?}"))
        })
        .unwrap_or(PATTERN_LIMIT_PER_CATEGORY);
    BenchSelection { category, sources, pattern_limit }
}

fn load_corpus(sel: &BenchSelection, pattern_limit: usize) -> (Vec<Entry>, Vec<Entry>, Vec<Entry>) {
    let cases = load_prepared_cases(std::path::Path::new(DATA_ROOT), &sel.sources, sel.category)
        .unwrap_or_else(|e| {
            panic!(
                "couldn't read prepared cases under {} ({e}) -- run `cargo run --release --example \
                 prepare_dataset -- <suricata|spamassassin|regexlib> <file>...` first; see \
                 docs/DATASETS.md",
                DATA_ROOT
            )
        });

    let mut best = Vec::new();
    let mut neutral = Vec::new();
    let mut worst = Vec::new();
    let mut best_patterns = std::collections::HashSet::new();
    let mut neutral_patterns = std::collections::HashSet::new();
    let mut worst_patterns = std::collections::HashSet::new();

    for case in cases {
        let (bucket, patterns) = match case.category {
            Category::Best => (&mut best, &mut best_patterns),
            Category::Neutral => (&mut neutral, &mut neutral_patterns),
            Category::Worst => (&mut worst, &mut worst_patterns),
        };
        let already_admitted = patterns.contains(&case.pattern);
        if !already_admitted && patterns.len() >= pattern_limit {
            continue;
        }
        let regex = match parse_pcre_rule(&case.pattern) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipping prepared case (should have been pre-verified): {} -- {}", case.pattern, e);
                continue;
            }
        };
        patterns.insert(case.pattern.clone());
        bucket.push(Entry { regex, input: case.input });
    }
    if best.is_empty() && neutral.is_empty() && worst.is_empty() {
        panic!("{}", empty_corpus_diagnosis(sel));
    }
    (best, neutral, worst)
}

fn empty_corpus_diagnosis(sel: &BenchSelection) -> String {
    let root = std::path::Path::new(DATA_ROOT);
    let resolved = std::fs::canonicalize(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| format!("{DATA_ROOT} (relative to the process's current directory; \
             does not exist there)"));
    let mut lines = vec![format!(
        "bench: no entries loaded from any category, for any source, under {DATA_ROOT} \
         (resolved: {resolved}). Active selection: category={:?}, sources={:?} (empty = all), \
         pattern_limit={}. Per-source check:",
        sel.category, sel.sources, sel.pattern_limit
    )];
    for source in [SourceKind::Suricata, SourceKind::SpamAssassin, SourceKind::RegexLib] {
        let path = root.join(source.dir_name()).join("prepared.jsonl");
        let status = match std::fs::read_to_string(&path) {
            Ok(text) => format!("{} lines", text.lines().filter(|l| !l.trim().is_empty()).count()),
            Err(e) => format!("unreadable ({e})"),
        };
        lines.push(format!("  {} -> {}", path.display(), status));
    }
    lines.push(format!(
        "Run `cargo run --release --example prepare_dataset -- <suricata|spamassassin|regexlib> \
         <file>...` for whichever source shows 0 lines or unreadable above; see \
         docs/DATASETS.md. If a file shows nonzero lines here but the bench still reported \
         0 entries loaded, the entries were filtered out after loading (see the eprintln above \
         this panic for that count) rather than missing on disk."
    ));
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

fn bench_by_category(c: &mut Criterion) {
    let sel = bench_selection();
    let (best, neutral, worst) = load_corpus(&sel, sel.pattern_limit);
    eprintln!(
        "bench_dataset: {} best, {} neutral, {} worst (pattern, input) entries loaded (up to {} \
         distinct patterns each, category={:?}, sources={:?} [empty = all]) from {}",
        best.len(),
        neutral.len(),
        worst.len(),
        sel.pattern_limit,
        sel.category,
        sel.sources,
        DATA_ROOT
    );

    for (label, corpus) in [("dataset_best", &best), ("dataset_neutral", &neutral), ("dataset_worst", &worst)] {
        if corpus.is_empty() {
            eprintln!("bench_dataset: skipping {label}, no entries loaded");
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


fn bench_worst_per_pattern(c: &mut Criterion) {
    let sel = bench_selection();
    let (_, _, worst) = load_corpus(&sel, 5);

    let mut group = c.benchmark_group("dataset_worst_per_pattern");
    group.sample_size(10);

    for (i, entry) in worst.iter().enumerate() {
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

criterion_group!(benches, bench_by_category, bench_worst_per_pattern);
criterion_main!(benches);
