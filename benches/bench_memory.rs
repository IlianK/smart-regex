//! benches/bench_memory.rs
//!
//! Memory consumption of the four parsers against real corpus patterns.
//!
//! Run: `cargo bench --bench bench_memory`

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use regex_engine::data::load_prepared_cases;
use regex_engine::data::prepare::core_regex;
use regex_engine::data::types::{MatchIntent, SourceKind};
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
/// Default when `BENCH_PATTERN_LIMIT` is unset: no cap. See the same
/// constant in `bench_dataset.rs` for why `usize::MAX` means that.
const PATTERN_LIMIT_PER_SOURCE: usize = usize::MAX;

/// `pattern_limit` for display: `usize::MAX` reads as "unlimited"
/// rather than a giant literal number.
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
}

/// One `Vec<Entry>` per source, each capped at `pattern_limit` distinct
/// patterns, optionally filtered to one `MatchIntent` tier first.
/// Source is the dimension examples group by.
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
        // An `ExplicitFullMatch` pattern only translates through
        // `core_regex`'s `^...$` stripping.
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

fn report(label: &str, corpus: &[Entry]) {
    if corpus.is_empty() {
        eprintln!("skip {label}: no entries");
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
    let (suricata, spamassassin, regexlib, s2_elab, innovatorved) = load_corpus(&sel);
    eprintln!(
        "bench_memory: {} suricata, {} spamassassin, {} regexlib, {} s2-elab, {} innovatorved \
         (limit {} each, sources={:?}, match_intent={:?})",
        suricata.len(),
        spamassassin.len(),
        regexlib.len(),
        s2_elab.len(),
        innovatorved.len(),
        fmt_limit(sel.pattern_limit),
        sel.sources,
        sel.match_intent,
    );
    report("dataset_suricata", &suricata);
    report("dataset_spamassassin", &spamassassin);
    report("dataset_regexlib", &regexlib);
    report("dataset_s2_elab", &s2_elab);
    report("dataset_innovatorved", &innovatorved);
}