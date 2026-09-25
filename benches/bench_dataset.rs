//! Benchmarks the four parsers against real corpus patterns paired with
//! real, verified input -- not a pattern list probed with the same fixed
//! generic string regardless of what each pattern actually is. That input
//! comes from `src/data/`'s prepare stage (`data/processed/*/prepared.jsonl`);
//! see `docs/DATASETS.md` for how to produce it.
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
/// Distinct patterns admitted per category, not rows: see `load_corpus`.
const PATTERN_LIMIT_PER_CATEGORY: usize = 30;

/// One prepared case, with its pattern compiled once rather than per
/// benchmark iteration.
struct Entry {
    regex: Regex,
    input: String,
}

/// Loads every verified variant from up to `pattern_limit` distinct
/// patterns per `Category`, from every source under `data/processed/`.
/// `pattern_limit` caps the number of *patterns* represented, not the
/// number of rows: `src/data/generate.rs` now produces several verified
/// inputs per (pattern, category) rather than one, so capping rows
/// directly would let one pattern's several variants crowd out another
/// pattern's coverage entirely. A pattern already admitted for a category
/// contributes every one of its variants for that category; the cap only
/// ever decides whether a *new* pattern gets in.
///
/// A pattern that fails to compile here would mean `prepare_dataset`
/// wrote something `parse_pcre_rule` itself rejects, which should never
/// happen (prepare_dataset verifies every case against this exact
/// function before writing it) -- skipped with a warning rather than
/// panicking, so a bench run survives an unexpected mismatch.
fn load_corpus(pattern_limit: usize) -> (Vec<Entry>, Vec<Entry>, Vec<Entry>) {
    let cases = load_prepared_cases(std::path::Path::new(DATA_ROOT), &[], None).unwrap_or_else(|e| {
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
        panic!("{}", empty_corpus_diagnosis());
    }
    (best, neutral, worst)
}

/// Explains *why* nothing loaded, rather than letting the caller run every
/// benchmark on a silently empty corpus (0 entries, but no failure) -- the
/// exact symptom this diagnostic exists to make impossible to miss. Checked
/// per source, since `load_prepared_cases` itself treats a missing
/// `<source>/prepared.jsonl` as "no cases from that source" rather than an
/// error, which is right for a benchmark that only wants whichever sources
/// happen to be prepared, but wrong for silently explaining nothing here.
fn empty_corpus_diagnosis() -> String {
    let root = std::path::Path::new(DATA_ROOT);
    let resolved = std::fs::canonicalize(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| format!("{DATA_ROOT} (relative to the process's current directory; \
             does not exist there)"));
    let mut lines = vec![format!(
        "bench: no entries loaded from any category, for any source, under {DATA_ROOT} \
         (resolved: {resolved}). Per-source check:"
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

/// One benchmark group per category: each parser runs against every entry
/// in that category, probed with *that entry's own* verified input, not a
/// string shared across the whole corpus.
fn bench_by_category(c: &mut Criterion) {
    let (best, neutral, worst) = load_corpus(PATTERN_LIMIT_PER_CATEGORY);
    eprintln!(
        "bench_dataset: {} best, {} neutral, {} worst (pattern, input) entries loaded (up to {} \
         distinct patterns each) from {}",
        best.len(),
        neutral.len(),
        worst.len(),
        PATTERN_LIMIT_PER_CATEGORY,
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
