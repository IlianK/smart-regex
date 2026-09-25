//! Compares the four parsers, and this crate's tree-free matchers, against
//! Rust's `regex` crate and Google's RE2, on real corpus patterns paired
//! with real, verified input (see `bench_dataset.rs`'s own doc comment for
//! why that matters). Input comes from `data/processed/*/prepared.jsonl`;
//! see `docs/DATASET_PREPARE.md`.
//!
//! Run: `cargo bench --bench bench_external --features external-engines`

use criterion::measurement::WallTime;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkGroup, Criterion};
use regex_engine::data::load_prepared_cases;
use regex_engine::data::types::Category;
use regex_engine::external::re2::Re2;
use regex_engine::external::rust_regex::RustRegex;
use regex_engine::frontend::{parse_pcre_rule, strip_pcre_delimiters};
use regex_engine::parsers::{
    parse_deriv_bc, parse_deriv_std_loop, parse_deriv_std_rec, parse_pderiv_bc, parse_pderiv_std,
    ParserType,
};
use regex_engine::types::Regex;
use regex_engine::{match_deriv, match_pderiv};

const DATA_ROOT: &str = "data/processed";
const SAMPLE_SIZE_PER_CATEGORY: usize = 30;

/// One prepared case, compiled for every engine this bench compares.
struct Entry {
    internal: Regex,
    input: String,
    verified_match: bool,
    rust_regex: Option<RustRegex>,
    re2_perl: Option<Re2>,
    re2_posix: Option<Re2>,
}

/// `/pattern/flags` -> the bare pattern body plus whether the `i` flag was
/// set. RE2's posix_syntax mode rejects an inline `(?i)` group outright.
fn external_pattern(pattern: &str) -> (String, bool) {
    let (body, case_insensitive) = strip_pcre_delimiters(pattern);
    (body.to_string(), case_insensitive)
}

fn load_corpus(limit: usize) -> (Vec<Entry>, Vec<Entry>, Vec<Entry>) {
    let cases = load_prepared_cases(std::path::Path::new(DATA_ROOT), &[], None).unwrap_or_else(|e| {
        panic!(
            "couldn't read prepared cases under {} ({e}) -- run `cargo run --release --example \
             prepare_dataset -- <suricata|spamassassin|regexlib> <file>...` first; see \
             docs/DATASET_PREPARE.md",
            DATA_ROOT
        )
    });

    let mut rust_regex_failures = 0usize;
    let mut re2_perl_failures = 0usize;
    let mut re2_posix_failures = 0usize;
    let mut best = Vec::new();
    let mut neutral = Vec::new();
    let mut worst = Vec::new();

    for case in cases {
        let full = match (&case.category, best.len(), neutral.len(), worst.len()) {
            (Category::Best, n, _, _) => n >= limit,
            (Category::Neutral, _, n, _) => n >= limit,
            (Category::Worst, _, _, n) => n >= limit,
        };
        if full {
            continue;
        }
        let internal = match parse_pcre_rule(&case.pattern) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipping prepared case (should have been pre-verified): {} -- {}", case.pattern, e);
                continue;
            }
        };

        let (body, case_insensitive) = external_pattern(&case.pattern);
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

        let entry = Entry {
            internal,
            input: case.input,
            verified_match: case.verified_match,
            rust_regex,
            re2_perl,
            re2_posix,
        };
        match case.category {
            Category::Best => best.push(entry),
            Category::Neutral => neutral.push(entry),
            Category::Worst => worst.push(entry),
        }
    }

    eprintln!(
        "bench_external: {} best, {} neutral, {} worst entries loaded ({} rejected by `regex`, \
         {} rejected by RE2 perl-mode, {} rejected by RE2 posix-mode -- each excluded from that \
         engine's own bench only)",
        best.len(),
        neutral.len(),
        worst.len(),
        rust_regex_failures,
        re2_perl_failures,
        re2_posix_failures,
    );
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

/// The tree-free Boolean matchers, unlike `PARSERS` above: there is no
/// POSIX/Greedy split here, since membership never distinguishes
/// disambiguation policy.
type MatcherFn = fn(&str, &Regex) -> bool;
const MATCHERS: &[(&str, MatcherFn)] = &[("deriv", match_deriv), ("pderiv", match_pderiv)];

/// Adds the three external-engine benchmark functions (`rust_regex`,
/// `re2_perl`, `re2_posix`) to `group`, each probed with its own entry's
/// verified input.
fn bench_external_engines(group: &mut BenchmarkGroup<WallTime>, corpus: &[Entry]) {
    group.bench_function("rust_regex", |b| {
        b.iter(|| {
            for entry in corpus {
                if let Some(re) = &entry.rust_regex {
                    black_box(re.is_match(black_box(&entry.input)));
                }
            }
        })
    });

    group.bench_function("re2_perl", |b| {
        b.iter(|| {
            for entry in corpus {
                if let Some(re) = &entry.re2_perl {
                    black_box(re.is_match(black_box(&entry.input)));
                }
            }
        })
    });

    group.bench_function("re2_posix", |b| {
        b.iter(|| {
            for entry in corpus {
                if let Some(re) = &entry.re2_posix {
                    black_box(re.is_match(black_box(&entry.input)));
                }
            }
        })
    });
}

fn bench_by_category(c: &mut Criterion) {
    let (best, neutral, worst) = load_corpus(SAMPLE_SIZE_PER_CATEGORY);

    for (label, corpus) in [
        ("external_best", &best),
        ("external_neutral", &neutral),
        ("external_worst", &worst),
    ] {
        if corpus.is_empty() {
            eprintln!("bench_external: skipping {label}, no entries loaded");
            continue;
        }
        let mut group = c.benchmark_group(label);
        group.sample_size(20);
        for (parser_type, parser) in PARSERS {
            group.bench_function(parser_type.name(), |b| {
                b.iter(|| {
                    for entry in corpus {
                        black_box(parser(black_box(&entry.input), black_box(&entry.internal)));
                    }
                })
            });
        }
        bench_external_engines(&mut group, corpus);
        group.finish();
    }
}

/// `regex`/RE2's `is_match` builds no parse tree; compares against this
/// crate's own tree-free matchers instead of the full parsers.
fn bench_matcher_by_category(c: &mut Criterion) {
    let (best, neutral, worst) = load_corpus(SAMPLE_SIZE_PER_CATEGORY);

    for (label, corpus) in [
        ("external_matcher_best", &best),
        ("external_matcher_neutral", &neutral),
        ("external_matcher_worst", &worst),
    ] {
        if corpus.is_empty() {
            continue;
        }
        let mut group = c.benchmark_group(label);
        group.sample_size(20);
        for (name, matcher) in MATCHERS {
            group.bench_function(*name, |b| {
                b.iter(|| {
                    for entry in corpus {
                        black_box(matcher(black_box(&entry.input), black_box(&entry.internal)));
                    }
                })
            });
        }
        bench_external_engines(&mut group, corpus);
        group.finish();
    }
}

/// One-time sanity check, printed to stderr, of how often this crate's own
/// membership decision agrees with `regex`-crate's and RE2's, on each
/// entry's own verified input and expected outcome.
fn bench_agreement_smoke(c: &mut Criterion) {
    let (best, neutral, worst) = load_corpus(SAMPLE_SIZE_PER_CATEGORY);
    let all: Vec<&Entry> = best.iter().chain(neutral.iter()).chain(worst.iter()).collect();

    let mut rust_regex_agree = 0usize;
    let mut rust_regex_total = 0usize;
    let mut re2_agree = 0usize;
    let mut re2_total = 0usize;
    let mut ours_correct = 0usize;

    for entry in &all {
        let ours = parse_deriv_std_loop(&entry.input, &entry.internal).is_some();
        if ours == entry.verified_match {
            ours_correct += 1;
        }
        if let Some(rr) = &entry.rust_regex {
            rust_regex_total += 1;
            if ours == rr.is_match(&entry.input) {
                rust_regex_agree += 1;
            }
        }
        if let Some(re2p) = &entry.re2_perl {
            re2_total += 1;
            if ours == re2p.is_match(&entry.input) {
                re2_agree += 1;
            }
        }
    }

    eprintln!(
        "bench_external agreement smoke check ({} entries, each against its own verified input): \
         this crate matches its own prior verification {}/{}; vs `regex` {}/{} agree; vs RE2 {}/{} agree",
        all.len(),
        ours_correct,
        all.len(),
        rust_regex_agree,
        rust_regex_total,
        re2_agree,
        re2_total
    );

    // Criterion still wants at least one measured function per group.
    let mut group = c.benchmark_group("external_agreement_smoke");
    group.sample_size(10);
    group.bench_function("noop", |b| b.iter(|| black_box(())));
    group.finish();
}

criterion_group!(benches, bench_by_category, bench_matcher_by_category, bench_agreement_smoke);
criterion_main!(benches);
