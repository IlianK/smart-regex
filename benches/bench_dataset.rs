//! regex-engine/benches/bench_dataset.rs
//!
//! Run: `cargo bench --bench bench_dataset`

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use regex_engine::frontend::parse_pcre_rule;
use regex_engine::parsers::{parse_bitcoded, parse_loop, parse_pderiv_bc, parse_pderiv_std, parse_recursive};
use regex_engine::types::Regex;

const CORPUS_PATH: &str = "data/corpus_patterns.txt";
const CORPUS_SAMPLE_SIZE: usize = 60;
const SHORT_PROBES: &[&str] = &["", "abc123def456"];
const WORST_CASE_PROBE: &str = "the quick catty fox jumps over the sleeping dog 0123456789";

fn load_corpus(limit: usize) -> Vec<(String, Regex)> {
    let text = std::fs::read_to_string(CORPUS_PATH).unwrap_or_else(|e| {
        panic!(
            "couldn't read {} ({}) -- run `cargo run --example extract_dataset -- <format> <file>...` first",
            CORPUS_PATH, e
        )
    });
    let mut out = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match parse_pcre_rule(line) {
            Ok(r) => out.push((line.to_string(), r)),
            Err(e) => eprintln!("skipping corpus line (should have been pre-filtered): {} -- {}", line, e),
        }
        if out.len() >= limit {
            break;
        }
    }
    out
}

type ParserFn = fn(&str, &Regex) -> Option<regex_engine::types::ParseTree>;
const PARSERS: &[(&str, ParserFn)] = &[
    ("deriv_rec", parse_recursive),
    ("deriv_loop", parse_loop),
    ("deriv_bc", parse_bitcoded),
    ("pderiv_bc", parse_pderiv_bc),
    ("pderiv_standard", parse_pderiv_std),
];

fn bench_corpus_sample_sweep(c: &mut Criterion) {
    let corpus = load_corpus(CORPUS_SAMPLE_SIZE);
    eprintln!(
        "bench_dataset: {} patterns loaded (of {} requested) from {}",
        corpus.len(),
        CORPUS_SAMPLE_SIZE,
        CORPUS_PATH
    );

    let mut group = c.benchmark_group("dataset_corpus_sample_sweep");
    group.sample_size(20);

    for (name, parser) in PARSERS {
        group.bench_function(*name, |b| {
            b.iter(|| {
                for (_pattern, r) in &corpus {
                    for probe in SHORT_PROBES {
                        black_box(parser(black_box(probe), black_box(r)));
                    }
                }
            })
        });
    }

    group.finish();
}

fn bench_worst_case_probe(c: &mut Criterion) {
    let corpus = load_corpus(10);

    let mut group = c.benchmark_group("dataset_worst_case_probe");
    group.sample_size(10);

    for (name, parser) in [
        ("deriv_rec", PARSERS[0].1),
        ("deriv_bc", PARSERS[2].1), // simplifying, for contrast
    ] {
        group.bench_function(name, |b| {
            b.iter(|| {
                for (_pattern, r) in &corpus {
                    black_box(parser(black_box(WORST_CASE_PROBE), black_box(r)));
                }
            })
        });
    }

    group.finish();
}

fn bench_per_pattern_sample(c: &mut Criterion) {
    let corpus = load_corpus(5);

    let mut group = c.benchmark_group("dataset_per_pattern_sample");
    group.sample_size(10);

    for (i, (pattern, r)) in corpus.iter().enumerate() {
        let label = format!("{}_{}", i, truncate(pattern, 24));
        for (name, parser) in PARSERS {
            group.bench_with_input(
                BenchmarkId::new(*name, &label),
                r,
                |b, r| b.iter(|| parser(black_box(SHORT_PROBES[1]), black_box(r))),
            );
        }
    }

    group.finish();
}

fn truncate(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    t.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

criterion_group!(benches, bench_corpus_sample_sweep, bench_worst_case_probe, bench_per_pattern_sample);
criterion_main!(benches);
