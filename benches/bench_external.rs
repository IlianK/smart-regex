//! Run: `cargo bench --bench bench_external --features external-engines`
//!
//! Compares four parsers against Rust's `regex` crate and Google's RE2 
use criterion::measurement::WallTime;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkGroup, Criterion};
use regex_engine::external::re2::Re2;
use regex_engine::external::rust_regex::RustRegex;
use regex_engine::frontend::{parse_pcre_rule, strip_pcre_delimiters};
use regex_engine::parsers::{parse_deriv_bc, parse_deriv_std_loop, parse_pderiv_bc, parse_pderiv_std, parse_deriv_std_rec, ParserType};
use regex_engine::types::Regex;
use regex_engine::{match_deriv, match_pderiv};

const CORPUS_PATH: &str = "data/corpus_patterns.txt";
const CORPUS_SAMPLE_SIZE: usize = 60;
const SHORT_PROBES: &[&str] = &["", "abc123def456"];

/// One corpus line, compiled for every engine this bench compares
struct CorpusEntry {
    internal: Regex,
    rust_regex: Option<RustRegex>,
    re2_perl: Option<Re2>,
    re2_posix: Option<Re2>,
}

/// `/pattern/flags` -> the bare pattern body plus whether the `i` flag was set. 
/// RE2's posix_syntax mode rejects an inline `(?i)` group outright
fn external_pattern(line: &str) -> (String, bool) {
    let (body, case_insensitive) = strip_pcre_delimiters(line);
    (body.to_string(), case_insensitive)
}

fn load_corpus(limit: usize) -> Vec<CorpusEntry> {
    let text = std::fs::read_to_string(CORPUS_PATH).unwrap_or_else(|e| {
        panic!(
            "couldn't read {} ({}) -- run `cargo run --example extract_dataset -- <format> <file>...` first",
            CORPUS_PATH, e
        )
    });

    let mut rust_regex_failures = 0usize;
    let mut re2_perl_failures = 0usize;
    let mut re2_posix_failures = 0usize;
    let mut out = Vec::new();

    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let internal = match parse_pcre_rule(line) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipping corpus line (should have been pre-filtered): {} -- {}", line, e);
                continue;
            }
        };

        let (body, case_insensitive) = external_pattern(line);
        let rust_regex = RustRegex::new(&body, case_insensitive).ok();
        if rust_regex.is_none() {
            rust_regex_failures += 1;
        }
        let re2_perl = Re2::new(&body, false, case_insensitive).ok();
        if re2_perl.is_none() {
            re2_perl_failures += 1;
        }
        let re2_posix = Re2::new(&body, true, case_insensitive).ok();
        if re2_posix.is_none() {
            re2_posix_failures += 1;
        }

        out.push(CorpusEntry { internal, rust_regex, re2_perl, re2_posix });
        if out.len() >= limit {
            break;
        }
    }

    eprintln!(
        "bench_external: {} patterns loaded ({} rejected by `regex`, {} rejected by RE2 perl-mode, \
         {} rejected by RE2 posix-mode -- each excluded from that engine's own bench only)",
        out.len(),
        rust_regex_failures,
        re2_perl_failures,
        re2_posix_failures,
    );
    out
}

type ParserFn = fn(&str, &Regex) -> Option<regex_engine::types::ParseTree>;
const PARSERS: &[(ParserType, ParserFn)] = &[
    (ParserType::DerivStdRec, parse_deriv_std_rec),
    (ParserType::DerivStdLoop, parse_deriv_std_loop),
    (ParserType::DerivBc, parse_deriv_bc),
    (ParserType::PDerivBc, parse_pderiv_bc),
    (ParserType::PDerivStd, parse_pderiv_std),
];

/// The tree-free Boolean matchers unlike `PARSERS` above
/// there is no POSIX/Greedy split here: 
/// membership never distinguishes disambiguation policy, 
type MatcherFn = fn(&str, &Regex) -> bool;
const MATCHERS: &[(&str, MatcherFn)] = &[
    ("deriv", match_deriv),
    ("pderiv", match_pderiv),
];

/// Adds the three external-engine benchmark functions 
/// (`rust_regex`, `re2_perl`, `re2_posix`) to `group`, sweeping `corpus`
fn bench_external_engines(group: &mut BenchmarkGroup<WallTime>, corpus: &[CorpusEntry]) {
    group.bench_function("rust_regex", |b| {
        b.iter(|| {
            for entry in corpus {
                if let Some(re) = &entry.rust_regex {
                    for probe in SHORT_PROBES {
                        black_box(re.is_match(black_box(probe)));
                    }
                }
            }
        })
    });

    group.bench_function("re2_perl", |b| {
        b.iter(|| {
            for entry in corpus {
                if let Some(re) = &entry.re2_perl {
                    for probe in SHORT_PROBES {
                        black_box(re.is_match(black_box(probe)));
                    }
                }
            }
        })
    });

    group.bench_function("re2_posix", |b| {
        b.iter(|| {
            for entry in corpus {
                if let Some(re) = &entry.re2_posix {
                    for probe in SHORT_PROBES {
                        black_box(re.is_match(black_box(probe)));
                    }
                }
            }
        })
    });
}

fn bench_corpus_sample_sweep(c: &mut Criterion) {
    let corpus = load_corpus(CORPUS_SAMPLE_SIZE);

    let mut group = c.benchmark_group("external_corpus_sample_sweep");
    group.sample_size(20);

    for (parser_type, parser) in PARSERS {
        group.bench_function(parser_type.name(), |b| {
            b.iter(|| {
                for entry in &corpus {
                    for probe in SHORT_PROBES {
                        black_box(parser(black_box(probe), black_box(&entry.internal)));
                    }
                }
            })
        });
    }

    bench_external_engines(&mut group, &corpus);
    group.finish();
}

/// `regex`/RE2's `is_match` builds no parse tree
/// Here: compare against this crate's own tree-free matchers instead of the full parsers 
fn bench_matcher_sample_sweep(c: &mut Criterion) {
    let corpus = load_corpus(CORPUS_SAMPLE_SIZE);

    let mut group = c.benchmark_group("external_matcher_sample_sweep");
    group.sample_size(20);

    for (name, matcher) in MATCHERS {
        group.bench_function(*name, |b| {
            b.iter(|| {
                for entry in &corpus {
                    for probe in SHORT_PROBES {
                        black_box(matcher(black_box(probe), black_box(&entry.internal)));
                    }
                }
            })
        });
    }

    bench_external_engines(&mut group, &corpus);
    group.finish();
}

/// One-time sanity check, printed to stderr, of how often this crate's own membership decision 
/// (`parse_loop` POSIX/Greedy always agree on membership, they only differ on parse-tree/submatch structure) 
/// matches `regex`-crate's and RE2's, on the same corpus sample and the same unanchored substring-search semantics
fn bench_agreement_smoke(c: &mut Criterion) {
    let corpus = load_corpus(CORPUS_SAMPLE_SIZE);
    let probe = SHORT_PROBES[1];

    let mut rust_regex_agree = 0usize;
    let mut rust_regex_total = 0usize;
    let mut re2_agree = 0usize;
    let mut re2_total = 0usize;

    for entry in &corpus {
        let ours = parse_deriv_std_loop(probe, &entry.internal).is_some();
        if let Some(rr) = &entry.rust_regex {
            rust_regex_total += 1;
            if ours == rr.is_match(probe) {
                rust_regex_agree += 1;
            }
        }
        if let Some(re2p) = &entry.re2_perl {
            re2_total += 1;
            if ours == re2p.is_match(probe) {
                re2_agree += 1;
            }
        }
    }

    eprintln!(
        "bench_external agreement smoke check (probe {:?}): this crate vs `regex` {}/{} agree; \
         this crate vs RE2 {}/{} agree",
        probe, rust_regex_agree, rust_regex_total, re2_agree, re2_total
    );

    // Criterion still wants at least one measured function per group.
    let mut group = c.benchmark_group("external_agreement_smoke");
    group.sample_size(10);
    group.bench_function("noop", |b| b.iter(|| black_box(())));
    group.finish();
}

criterion_group!(benches, bench_corpus_sample_sweep, bench_matcher_sample_sweep, bench_agreement_smoke);
criterion_main!(benches);
