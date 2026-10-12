//! benches/bench_external.rs
//!
//! Compares the four parsers, and this crate's tree-free matchers, against
//! Rust's `regex` crate and Google's RE2
//!
//! Run: `cargo bench --bench bench_external --features external-engines`

use criterion::measurement::WallTime;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkGroup, Criterion};
use regex_engine::data::load_prepared_cases;
use regex_engine::data::prepare::core_regex;
use regex_engine::data::types::SourceKind;
use regex_engine::external::re2::Re2;
use regex_engine::external::rust_regex::RustRegex;
use regex_engine::frontend::strip_pcre_delimiters;
use regex_engine::parsers::{
    parse_deriv_bc, parse_deriv_std_loop, parse_deriv_std_rec, parse_pderiv_bc, parse_pderiv_std,
    ParserType,
};
use regex_engine::types::Regex;
use regex_engine::{match_deriv, match_pderiv};

const DATA_ROOT: &str = "data/processed";
const PATTERN_LIMIT_PER_SOURCE: usize = 30;

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
        .unwrap_or_default();
    let pattern_limit = std::env::var("BENCH_PATTERN_LIMIT")
        .ok()
        .map(|s| {
            s.parse().unwrap_or_else(|_| panic!("BENCH_PATTERN_LIMIT must be a positive integer, got {s:?}"))
        })
        .unwrap_or(PATTERN_LIMIT_PER_SOURCE);
    BenchSelection { sources, pattern_limit }
}

struct Entry {
    internal: Regex,
    input: String,
    verified_match: bool,
    rust_regex: Option<RustRegex>,
    re2_perl: Option<Re2>,
    re2_posix: Option<Re2>,
}

fn external_pattern(pattern: &str) -> (String, bool, bool) {
    let (body, flags) = strip_pcre_delimiters(pattern);
    (body.to_string(), flags.case_insensitive, flags.dot_all)
}

/// One `Vec<Entry>` per source. There is no difficulty-category split
/// left to bucket by (`data/generate.rs` no longer has one); source is
/// the dimension this project's examples group by instead.
fn load_corpus(sel: &BenchSelection, pattern_limit: usize) -> (Vec<Entry>, Vec<Entry>, Vec<Entry>) {
    let cases = load_prepared_cases(std::path::Path::new(DATA_ROOT), &sel.sources)
        .unwrap_or_else(|e| {
        panic!(
            "couldn't read prepared cases under {} ({e}) -- run `cargo run --release --example \
             dataset_prepare -- <suricata|spamassassin|regexlib> <file>...` first; see \
             docs/DATASET.md",
            DATA_ROOT
        )
    });

    let mut rust_regex_failures = 0usize;
    let mut re2_perl_failures = 0usize;
    let mut re2_posix_failures = 0usize;
    let mut suricata = Vec::new();
    let mut spamassassin = Vec::new();
    let mut regexlib = Vec::new();
    let mut suricata_patterns = std::collections::HashSet::new();
    let mut spamassassin_patterns = std::collections::HashSet::new();
    let mut regexlib_patterns = std::collections::HashSet::new();

    for case in cases {
        let patterns = match case.source {
            SourceKind::Suricata => &mut suricata_patterns,
            SourceKind::SpamAssassin => &mut spamassassin_patterns,
            SourceKind::RegexLib => &mut regexlib_patterns,
        };
        let already_admitted = patterns.contains(&case.pattern);
        if !already_admitted && patterns.len() >= pattern_limit {
            continue;
        }
        patterns.insert(case.pattern.clone());

        // `core_regex`, not `parse_pcre_rule`: `case.pattern` was accepted
        // by `core_regex_tagged` to reach `prepared.jsonl` at all, and an
        // `ExplicitFullMatch` pattern only translates through the
        // `^...$`-stripping `core_regex` applies, which plain
        // `parse_pcre_rule` does not.
        let internal = match core_regex(&case.pattern) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipping prepared case (should have been pre-verified): {} -- {}", case.pattern, e);
                continue;
            }
        };

        let (body, case_insensitive, dot_all) = external_pattern(&case.pattern);
        let rust_regex = RustRegex::new(&body, case_insensitive, dot_all).ok();
        if rust_regex.is_none() {
            rust_regex_failures += 1;
        }
        let re2_perl = Re2::new(&body, false, case_insensitive, dot_all).ok();
        if re2_perl.is_none() {
            re2_perl_failures += 1;
        }
        let re2_posix = Re2::new(&body, true, case_insensitive, dot_all).ok();
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
        match case.source {
            SourceKind::Suricata => suricata.push(entry),
            SourceKind::SpamAssassin => spamassassin.push(entry),
            SourceKind::RegexLib => regexlib.push(entry),
        }
    }

    eprintln!(
        "bench_external: {} suricata, {} spamassassin, {} regexlib (pattern, input) entries \
         loaded ({} rejected by `regex`, {} rejected by RE2 perl-mode, {} rejected by RE2 \
         posix-mode -- each excluded from that engine's own bench only)",
        suricata.len(),
        spamassassin.len(),
        regexlib.len(),
        rust_regex_failures,
        re2_perl_failures,
        re2_posix_failures,
    );
    if suricata.is_empty() && spamassassin.is_empty() && regexlib.is_empty() {
        panic!("{}", empty_corpus_diagnosis(sel));
    }
    (suricata, spamassassin, regexlib)
}


fn empty_corpus_diagnosis(sel: &BenchSelection) -> String {
    let root = std::path::Path::new(DATA_ROOT);
    let resolved = std::fs::canonicalize(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| format!("{DATA_ROOT} (relative to the process's current directory; \
             does not exist there)"));
    let mut lines = vec![format!(
        "bench: no entries loaded from any source under {DATA_ROOT} \
         (resolved: {resolved}). Active selection: sources={:?} (empty = all), \
         pattern_limit={}. Per-source check:",
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
    lines.push(format!(
        "Run `cargo run --release --example dataset_prepare -- <suricata|spamassassin|regexlib> \
         <file>...` for whichever source shows 0 lines or unreadable above; see \
         docs/DATASET.md. If a file shows nonzero lines here but the bench still reported \
         0 entries loaded, every case in it was rejected during loading (each rejection is \
         eprintln'd above this panic) rather than missing on disk."
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


type MatcherFn = fn(&str, &Regex) -> bool;
const MATCHERS: &[(&str, MatcherFn)] = &[("deriv", match_deriv), ("pderiv", match_pderiv)];

/// Calls each external engine's `full_match`, not `is_match`: `is_match`
/// is unanchored substring search (each wrapper's own doc comment says
/// so), which was a fair comparison while this crate's own parsers also
/// matched via search-padded patterns. They no longer do (Chapter 7 /
/// "Anchors and the Two Entry Points"): `entry.internal` is always a
/// full-string match now, so the external engines need their own
/// full-match entry point to be compared against it on the same
/// question, not a strictly easier one.
fn bench_external_engines(group: &mut BenchmarkGroup<WallTime>, corpus: &[Entry]) {
    group.bench_function("rust_regex", |b| {
        b.iter(|| {
            for entry in corpus {
                if let Some(re) = &entry.rust_regex {
                    black_box(re.full_match(black_box(&entry.input)));
                }
            }
        })
    });

    group.bench_function("re2_perl", |b| {
        b.iter(|| {
            for entry in corpus {
                if let Some(re) = &entry.re2_perl {
                    black_box(re.full_match(black_box(&entry.input)));
                }
            }
        })
    });

    group.bench_function("re2_posix", |b| {
        b.iter(|| {
            for entry in corpus {
                if let Some(re) = &entry.re2_posix {
                    black_box(re.full_match(black_box(&entry.input)));
                }
            }
        })
    });
}

fn bench_by_source(c: &mut Criterion) {
    let sel = bench_selection();
    let (suricata, spamassassin, regexlib) = load_corpus(&sel, sel.pattern_limit);

    for (label, corpus) in [
        ("external_suricata", &suricata),
        ("external_spamassassin", &spamassassin),
        ("external_regexlib", &regexlib),
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


fn bench_matcher_by_source(c: &mut Criterion) {
    let sel = bench_selection();
    let (suricata, spamassassin, regexlib) = load_corpus(&sel, sel.pattern_limit);

    for (label, corpus) in [
        ("external_matcher_suricata", &suricata),
        ("external_matcher_spamassassin", &spamassassin),
        ("external_matcher_regexlib", &regexlib),
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

fn bench_agreement_smoke(c: &mut Criterion) {
    let sel = bench_selection();
    let (suricata, spamassassin, regexlib) = load_corpus(&sel, sel.pattern_limit);
    let all: Vec<&Entry> = suricata.iter().chain(spamassassin.iter()).chain(regexlib.iter()).collect();

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
            if ours == rr.full_match(&entry.input) {
                rust_regex_agree += 1;
            }
        }
        if let Some(re2p) = &entry.re2_perl {
            re2_total += 1;
            if ours == re2p.full_match(&entry.input) {
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

    let mut group = c.benchmark_group("external_agreement_smoke");
    group.sample_size(10);
    group.bench_function("noop", |b| b.iter(|| black_box(())));
    group.finish();
}

criterion_group!(benches, bench_by_source, bench_matcher_by_source, bench_agreement_smoke);
criterion_main!(benches);
