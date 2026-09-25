//! Benchmarks the four parsers against real corpus patterns paired with
//! real, verified input -- not a pattern list probed with the same fixed
//! generic string regardless of what each pattern actually is. That input
//! comes from `src/data/`'s prepare stage (`data/processed/*/prepared.jsonl`);
//! see `docs/DATASET_PREPARE.md` for how to produce it.
//!
//! Run: `cargo bench --bench bench_dataset`

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use regex_engine::data::load_prepared_cases;
use regex_engine::data::types::Category;
use regex_engine::frontend::parse_pcre_rule;
use regex_engine::parsers::{
    parse_deriv_bc, parse_deriv_std_loop, parse_deriv_std_rec, parse_pderiv_bc, parse_pderiv_std,
    ParserType,
};
use regex_engine::types::Regex;

const DATA_ROOT: &str = "src/data/processed";
const SAMPLE_SIZE_PER_CATEGORY: usize = 30;

/// One prepared case, with its pattern compiled once rather than per
/// benchmark iteration.
struct Entry {
    regex: Regex,
    input: String,
}

/// Loads up to `limit` entries per `Category` from every source under
/// `data/processed/`. A pattern that fails to compile here would mean
/// `prepare_dataset` wrote something `parse_pcre_rule` itself rejects,
/// which should never happen (prepare_dataset verifies every case against
/// this exact function before writing it) -- skipped with a warning rather
/// than panicking, so a bench run survives an unexpected mismatch.
fn load_corpus(limit: usize) -> (Vec<Entry>, Vec<Entry>, Vec<Entry>) {
    let cases = load_prepared_cases(std::path::Path::new(DATA_ROOT), &[], None).unwrap_or_else(|e| {
        panic!(
            "couldn't read prepared cases under {} ({e}) -- run `cargo run --release --example \
             prepare_dataset -- <suricata|spamassassin|regexlib> <file>...` first; see \
             docs/DATASET_PREPARE.md",
            DATA_ROOT
        )
    });

    let mut best = Vec::new();
    let mut neutral = Vec::new();
    let mut worst = Vec::new();
    for case in cases {
        let regex = match parse_pcre_rule(&case.pattern) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipping prepared case (should have been pre-verified): {} -- {}", case.pattern, e);
                continue;
            }
        };
        let bucket = match case.category {
            Category::Best if best.len() < limit => &mut best,
            Category::Neutral if neutral.len() < limit => &mut neutral,
            Category::Worst if worst.len() < limit => &mut worst,
            _ => continue,
        };
        bucket.push(Entry { regex, input: case.input });
    }
    (best, neutral, worst)
}

type ParserFn = fn(&str, &Regex) -> Option<regex_engine::types::ParseTree>;
const PARSERS: &[(ParserType, ParserFn)] = &[
    (ParserType::DerivStdRec, parse_deriv_std_rec),
    (ParserType::DerivStdLoop, parse_deriv_std_loop),
    (ParserType::DerivBc, parse_deriv_bc),
    (ParserType::PDerivBc, parse_pderiv_bc),
    (ParserType::PDerivStd, parse_pderiv_std),
];

/// One benchmark group per category: each parser runs against every entry
/// in that category, probed with *that entry's own* verified input, not a
/// string shared across the whole corpus.
fn bench_by_category(c: &mut Criterion) {
    let (best, neutral, worst) = load_corpus(SAMPLE_SIZE_PER_CATEGORY);
    eprintln!(
        "bench_dataset: {} best, {} neutral, {} worst entries loaded (limit {} each) from {}",
        best.len(),
        neutral.len(),
        worst.len(),
        SAMPLE_SIZE_PER_CATEGORY,
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

/// Per-pattern breakdown for the worst-case category specifically: which
/// individual patterns cost the most, not just the aggregate.
fn bench_worst_per_pattern(c: &mut Criterion) {
    let (_, _, worst) = load_corpus(5);

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
