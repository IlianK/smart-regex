//! benches/bench_match.rs
//! 
//! Investigates memory consumption of the four parsers against real corpus patterns

//! Run: `cargo bench --bench bench_memory`

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use regex_engine::data::load_prepared_cases;
use regex_engine::data::types::{Category, SourceKind};
use regex_engine::frontend::parse_pcre_rule;
use regex_engine::parsers::{
    parse_deriv_bc, parse_deriv_std_loop, parse_deriv_std_rec, parse_pderiv_bc, parse_pderiv_std,
    ParserType,
};
use regex_engine::types::Regex;

// -------------------------------
// Counting global allocator
// -------------------------------
struct CountingAllocator;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static TOTAL: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            let now = CURRENT.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
            PEAK.fetch_max(now, Ordering::SeqCst);
            TOTAL.fetch_add(layout.size(), Ordering::SeqCst);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        CURRENT.fetch_sub(layout.size(), Ordering::SeqCst);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = System.realloc(ptr, layout, new_size);
        if !new_ptr.is_null() {
            if new_size > layout.size() {
                let grew = new_size - layout.size();
                let now = CURRENT.fetch_add(grew, Ordering::SeqCst) + grew;
                PEAK.fetch_max(now, Ordering::SeqCst);
                TOTAL.fetch_add(grew, Ordering::SeqCst);
            } else {
                CURRENT.fetch_sub(layout.size() - new_size, Ordering::SeqCst);
            }
        }
        new_ptr
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn measure<T>(f: impl FnOnce() -> T) -> (T, usize, usize) {
    let baseline = CURRENT.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    TOTAL.store(0, Ordering::SeqCst);
    let result = f();
    let total = TOTAL.load(Ordering::SeqCst);
    let peak = PEAK.load(Ordering::SeqCst).saturating_sub(baseline);
    (result, total, peak)
}

const DATA_ROOT: &str = "data/processed";
const PATTERN_LIMIT_PER_CATEGORY: usize = 30;

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
        .unwrap_or_default();
    let pattern_limit = std::env::var("BENCH_PATTERN_LIMIT")
        .ok()
        .map(|s| {
            s.parse().unwrap_or_else(|_| panic!("BENCH_PATTERN_LIMIT must be a positive integer, got {s:?}"))
        })
        .unwrap_or(PATTERN_LIMIT_PER_CATEGORY);
    BenchSelection { category, sources, pattern_limit }
}

struct Entry {
    regex: Regex,
    input: String,
}

fn load_corpus(sel: &BenchSelection) -> (Vec<Entry>, Vec<Entry>, Vec<Entry>) {
    let cases = load_prepared_cases(std::path::Path::new(DATA_ROOT), &sel.sources, sel.category)
        .unwrap_or_else(|e| {
            panic!(
                "couldn't read prepared cases under {} ({e}) -- run `cargo run --release --example \
                 dataset_prepare -- <suricata|spamassassin|regexlib> <file>...` first; see \
                 docs/DATASET.md",
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
        if !already_admitted && patterns.len() >= sel.pattern_limit {
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
        panic!(
            "bench_memory: no entries loaded under {DATA_ROOT} with category={:?}, sources={:?} \
             (empty = all), pattern_limit={} -- run dataset_prepare first; see docs/DATASET.md",
            sel.category, sel.sources, sel.pattern_limit
        );
    }
    (best, neutral, worst)
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

fn report(label: &str, corpus: &[Entry]) {
    if corpus.is_empty() {
        eprintln!("bench_memory: skipping {label}, no entries loaded");
        return;
    }
    println!("\n{label} ({} entries)", corpus.len());
    println!("{:<16} {:>14} {:>14} {:>14}", "parser", "total bytes", "avg/entry", "max peak");
    println!("{:-<16} {:->14} {:->14} {:->14}", "", "", "", "");
    for (parser_type, parser) in PARSERS {
        let mut total_sum = 0usize;
        let mut peak_max = 0usize;
        for entry in corpus {
            let (_, total, peak) = measure(|| parser(&entry.input, &entry.regex));
            total_sum += total;
            peak_max = peak_max.max(peak);
        }
        let avg = total_sum / corpus.len();
        println!("{:<16} {:>14} {:>14} {:>14}", parser_type.name(), total_sum, avg, peak_max);
    }
}

fn main() {
    let sel = bench_selection();
    let (best, neutral, worst) = load_corpus(&sel);
    eprintln!(
        "bench_memory: {} best, {} neutral, {} worst (pattern, input) entries loaded (up to {} \
         distinct patterns each, category={:?}, sources={:?} [empty = all]) from {}",
        best.len(),
        neutral.len(),
        worst.len(),
        sel.pattern_limit,
        sel.category,
        sel.sources,
        DATA_ROOT
    );
    println!("bytes allocated per parser per category (total = cumulative during the call, \
               including freed churn; max peak = largest single entry's high-water mark above \
               baseline)");
    report("dataset_best", &best);
    report("dataset_neutral", &neutral);
    report("dataset_worst", &worst);
}
