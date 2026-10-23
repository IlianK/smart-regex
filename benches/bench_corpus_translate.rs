//! benches/bench_translate.rs
//!
//! Cost of frontend translation (`core_regex`: PCRE string -> `Regex`)
//! over the real corpus, grouped by distinct pattern.
//!
//! Other benches in this family (`bench_corpus_time`, `bench_memory`)
//! measure the *parser* call per *entry* (pattern+input), translation
//! excluded. Translation cost depends only on the pattern, so timing it
//! per entry would re-measure the same pattern once per input. This file
//! dedups by pattern -- the same dedup the other benches do while
//! loading, but keeping the grouping.
//!
//! No worker-process isolation, safe here specifically because:
//!   1. Every pattern already went through `core_regex` once during
//!      `dataset_prepare`; this is a repeat of a proven-safe call.
//!   2. `core_regex` enforces `MAX_CORE_DEPTH`/`MAX_CORE_NODES` before
//!      the expensive part runs. The parsers need isolation precisely
//!      because they lack such a cap; translation does not.
//! A per-pattern `catch_unwind` stands in for crash containment. Consequence:
//! `micros` here is a clean in-process number, no spawn/pipe overhead.
//!
//! Outputs:
//!   - `reports/benches/bench_translate.csv` -- per source:
//!     mean/median/p99/max time, alloc (total, peak), nodes. Always
//!     this exact name, ignoring `BENCH_RUN_LABEL` -- appended to, not
//!     overwritten, so one run per source (or one run covering all of
//!     them) accumulates into a single overview instead of N separate
//!     one-line files. `run_label`/`pattern_limit` columns record what
//!     each row's own invocation actually was.
//!   - `reports/benches/bench_translate{label}_patterns.csv` -- per
//!     distinct pattern: nodes/micros/bytes plus the `Regex` via its
//!     `Debug`, when nodes <= `BENCH_TRANSLATE_RENDER_MAX_NODES`
//!     (default 150). Catalogue for thesis examples.
//!   - `reports/demos/translation_examples.txt` -- the
//!     `BENCH_TRANSLATE_EXAMPLES` (default 5) smallest patterns per
//!     source, fully rendered. Always overwritten, never labelled.
//!
//! `BENCH_SOURCE`, `BENCH_PATTERN_LIMIT`, `BENCH_MATCH_INTENT`,
//! `BENCH_RUN_LABEL` as in `bench_corpus_time.rs` (CSVs only). No
//! `BENCH_ENTRY_TIMEOUT_MS`: nothing to bound.
//!
//! cargo bench --bench bench_translate

use std::fmt::Write as _;
use std::io::Write as _;
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use regex_engine::data::load_prepared_cases;
use regex_engine::data::prepare::core_regex;
use regex_engine::data::types::{MatchIntent, SourceKind};
use regex_engine::types::Regex;

const DATA_ROOT: &str = "data/processed";
const PATTERN_LIMIT_PER_SOURCE: usize = usize::MAX;
const REPORTS_DIR: &str = "reports/benches";
const DEMOS_DIR: &str = "reports/demos";
const DEFAULT_RENDER_MAX_NODES: usize = 150;
const DEFAULT_EXAMPLES_PER_SOURCE: usize = 5;

// -------------------------------
// Counting allocator (one process, many patterns -- see per-pattern
// reset in `measure_one`, mirroring `examples/bench_worker.rs`)
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

// -------------------------------
// Selection (mirrors bench_corpus_time.rs/bench_memory.rs)
// -------------------------------

struct BenchSelection {
    sources: Vec<SourceKind>,
    pattern_limit: usize,
    match_intent: Option<MatchIntent>,
}

fn fmt_limit(n: usize) -> String {
    if n == usize::MAX { "unlimited".to_string() } else { n.to_string() }
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
        None => Some(MatchIntent::ExplicitFullMatch),
        Some(s) => match s.trim().to_lowercase().as_str() {
            "both" => None,
            "direct" => Some(MatchIntent::UnanchoredAmbiguous),
            "explicit" => Some(MatchIntent::ExplicitFullMatch),
            other => panic!("BENCH_MATCH_INTENT must be direct, explicit, or both, got {other:?}"),
        },
    };
    BenchSelection { sources, pattern_limit, match_intent }
}

fn run_label_suffix() -> String {
    std::env::var("BENCH_RUN_LABEL").ok().map(|l| format!("_{l}")).unwrap_or_default()
}

fn render_max_nodes() -> usize {
    std::env::var("BENCH_TRANSLATE_RENDER_MAX_NODES")
        .ok()
        .map(|s| {
            s.parse().unwrap_or_else(|_| {
                panic!("BENCH_TRANSLATE_RENDER_MAX_NODES must be a positive integer, got {s:?}")
            })
        })
        .unwrap_or(DEFAULT_RENDER_MAX_NODES)
}

fn examples_per_source() -> usize {
    std::env::var("BENCH_TRANSLATE_EXAMPLES")
        .ok()
        .map(|s| {
            s.parse().unwrap_or_else(|_| {
                panic!("BENCH_TRANSLATE_EXAMPLES must be a non-negative integer, got {s:?}")
            })
        })
        .unwrap_or(DEFAULT_EXAMPLES_PER_SOURCE)
}

// -------------------------------
// Corpus: distinct patterns only, no inputs needed
// -------------------------------

type Corpora = (Vec<String>, Vec<String>, Vec<String>, Vec<String>, Vec<String>);

fn load_patterns(sel: &BenchSelection) -> Corpora {
    let cases = load_prepared_cases(Path::new(DATA_ROOT), &sel.sources)
        .unwrap_or_else(|e| panic!("cannot read {DATA_ROOT}: {e} -- run dataset_prepare first"));

    let mut suricata = Vec::new();
    let mut spamassassin = Vec::new();
    let mut regexlib = Vec::new();
    let mut s2_elab = Vec::new();
    let mut innovatorved = Vec::new();
    let mut suricata_seen = std::collections::HashSet::new();
    let mut spamassassin_seen = std::collections::HashSet::new();
    let mut regexlib_seen = std::collections::HashSet::new();
    let mut s2_elab_seen = std::collections::HashSet::new();
    let mut innovatorved_seen = std::collections::HashSet::new();

    for case in cases {
        if let Some(wanted) = sel.match_intent {
            if case.match_intent != wanted {
                continue;
            }
        }
        let (bucket, seen) = match case.source {
            SourceKind::Suricata => (&mut suricata, &mut suricata_seen),
            SourceKind::SpamAssassin => (&mut spamassassin, &mut spamassassin_seen),
            SourceKind::RegexLib => (&mut regexlib, &mut regexlib_seen),
            SourceKind::S2ELab => (&mut s2_elab, &mut s2_elab_seen),
            SourceKind::Innovatorved => (&mut innovatorved, &mut innovatorved_seen),
        };
        if seen.contains(&case.pattern) {
            continue;
        }
        if !seen.is_empty() && seen.len() >= sel.pattern_limit {
            continue;
        }
        seen.insert(case.pattern.clone());
        bucket.push(case.pattern);
    }
    if suricata.is_empty()
        && spamassassin.is_empty()
        && regexlib.is_empty()
        && s2_elab.is_empty()
        && innovatorved.is_empty()
    {
        panic!(
            "no patterns loaded from {DATA_ROOT} (sources={:?}, limit={}, match_intent={:?}) -- \
             run dataset_prepare first",
            sel.sources, fmt_limit(sel.pattern_limit), sel.match_intent
        );
    }
    (suricata, spamassassin, regexlib, s2_elab, innovatorved)
}

// -------------------------------
// Per-pattern measurement
// -------------------------------

/// Iterative, not recursive: mirrors `node_count` in
/// `src/data/prepare.rs` (not `pub`, so not reusable from here) rather
/// than the naive recursive version in `examples/common/thesis_figures.rs`
/// (not reachable from `benches/` anyway) -- a long, unbalanced `Seq`
/// chain would otherwise recurse once per node.
fn node_count(r: &Regex) -> usize {
    let mut stack = vec![r];
    let mut count = 0usize;
    while let Some(node) = stack.pop() {
        count += 1;
        match node {
            Regex::Phi | Regex::Eps | Regex::Lit(_) => {}
            Regex::Alt(a, b) | Regex::Seq(a, b) => {
                stack.push(a);
                stack.push(b);
            }
            Regex::Star(a) => stack.push(a),
        }
    }
    count
}

struct Measurement {
    nodes: usize,
    micros: u64,
    total_bytes: usize,
    peak_bytes: usize,
    rendered: Option<String>,
}

enum MeasureOutcome {
    Done(Measurement),
    Rejected(String),
    Crashed,
}

/// Times and memory-measures one pattern's `core_regex` call. Counters
/// are read immediately after the call returns, before node-counting
/// or rendering, so neither adds to the reported cost -- same ordering
/// `examples/bench_worker.rs` uses to exclude translation from the
/// parser-call measurement, just the other call being isolated here.
fn measure_one(pattern: &str, render_max_nodes: usize) -> MeasureOutcome {
    let result = std::panic::catch_unwind(|| {
        let baseline = CURRENT.load(Ordering::SeqCst);
        PEAK.store(baseline, Ordering::SeqCst);
        TOTAL.store(0, Ordering::SeqCst);

        let start = Instant::now();
        let outcome = core_regex(pattern);
        let micros = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX);

        let total_bytes = TOTAL.load(Ordering::SeqCst);
        let peak_bytes = PEAK.load(Ordering::SeqCst).saturating_sub(baseline);
        (outcome, micros, total_bytes, peak_bytes)
    });

    let (outcome, micros, total_bytes, peak_bytes) = match result {
        Ok(v) => v,
        Err(_) => return MeasureOutcome::Crashed,
    };

    match outcome {
        Err(e) => MeasureOutcome::Rejected(e),
        Ok(r) => {
            let nodes = node_count(&r);
            // `{r:?}` (derived `Debug`), not a flattened/pretty reading:
            // the point of this column is to show the translated tree's
            // actual constructors (`Seq`, `Alt`, `Star`, `Lit`, `Eps`,
            // `Phi`), the same way `demo_parse.rs` already prints a
            // `ParseTree` via `{:?}` elsewhere in this codebase.
            let rendered = if nodes <= render_max_nodes { Some(format!("{r:?}")) } else { None };
            MeasureOutcome::Done(Measurement { nodes, micros, total_bytes, peak_bytes, rendered })
        }
    }
}

// -------------------------------
// Reporting
// -------------------------------

struct SourcePatternStat {
    pattern: String,
    nodes: usize,
    micros: u64,
    total_bytes: usize,
    peak_bytes: usize,
    rendered: Option<String>,
}

struct SummaryRow {
    source: &'static str,
    patterns: usize,
    translated: usize,
    rejected: usize,
    crashed: usize,
    mean_ms: Option<f64>,
    median_ms: Option<f64>,
    p99_ms: Option<f64>,
    max_ms: Option<f64>,
    mean_total_bytes: Option<f64>,
    median_total_bytes: Option<f64>,
    p99_total_bytes: Option<f64>,
    max_total_bytes: Option<usize>,
    mean_peak_bytes: Option<f64>,
    median_peak_bytes: Option<f64>,
    p99_peak_bytes: Option<f64>,
    max_peak_bytes: Option<usize>,
    mean_nodes: Option<f64>,
    median_nodes: Option<f64>,
    p99_nodes: Option<f64>,
    max_nodes: Option<usize>,
}

fn us_to_ms(us: u64) -> f64 {
    us as f64 / 1_000.0
}

fn stats_f64(sorted: &[f64]) -> (f64, f64, f64, f64) {
    let n = sorted.len();
    let mean = sorted.iter().sum::<f64>() / n as f64;
    (mean, sorted[n / 2], sorted[(n * 99) / 100], sorted[n - 1])
}

fn fmt_opt(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.3}")).unwrap_or_default()
}

/// Runs every distinct pattern in `patterns` through `measure_one`,
/// printing and returning both the per-source summary and the
/// per-pattern rows the catalogue CSV needs.
fn report(
    label: &'static str,
    patterns: &[String],
    render_max_nodes: usize,
) -> (Option<SummaryRow>, Vec<SourcePatternStat>) {
    if patterns.is_empty() {
        eprintln!("skip {label}: no patterns");
        return (None, Vec::new());
    }

    let mut ms: Vec<f64> = Vec::with_capacity(patterns.len());
    let mut total_bytes: Vec<f64> = Vec::with_capacity(patterns.len());
    let mut peak_bytes: Vec<f64> = Vec::with_capacity(patterns.len());
    let mut nodes_f: Vec<f64> = Vec::with_capacity(patterns.len());
    let mut per_pattern = Vec::with_capacity(patterns.len());
    let mut rejected = 0usize;
    let mut crashed = 0usize;

    for pattern in patterns {
        match measure_one(pattern, render_max_nodes) {
            MeasureOutcome::Done(m) => {
                ms.push(us_to_ms(m.micros));
                total_bytes.push(m.total_bytes as f64);
                peak_bytes.push(m.peak_bytes as f64);
                nodes_f.push(m.nodes as f64);
                per_pattern.push(SourcePatternStat {
                    pattern: pattern.clone(),
                    nodes: m.nodes,
                    micros: m.micros,
                    total_bytes: m.total_bytes,
                    peak_bytes: m.peak_bytes,
                    rendered: m.rendered,
                });
            }
            MeasureOutcome::Rejected(e) => {
                rejected += 1;
                eprintln!("  rejected: {label} pat={pattern:?} -- {e}");
            }
            MeasureOutcome::Crashed => {
                crashed += 1;
                eprintln!("  crash: {label} pat={pattern:?} -- core_regex panicked");
            }
        }
    }

    let summary = if ms.is_empty() {
        None
    } else {
        let mut ms_sorted = ms.clone();
        ms_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut total_sorted = total_bytes.clone();
        total_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut peak_sorted = peak_bytes.clone();
        peak_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut nodes_sorted = nodes_f.clone();
        nodes_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let (ms_mean, ms_median, ms_p99, ms_max) = stats_f64(&ms_sorted);
        let (tb_mean, tb_median, tb_p99, tb_max) = stats_f64(&total_sorted);
        let (pb_mean, pb_median, pb_p99, pb_max) = stats_f64(&peak_sorted);
        let (nd_mean, nd_median, nd_p99, nd_max) = stats_f64(&nodes_sorted);

        println!("\n{label} ({} patterns, {} translated, {} rejected, {} crashed)", patterns.len(), ms.len(), rejected, crashed);
        println!(
            "  time (ms)   mean {ms_mean:.3}  median {ms_median:.3}  p99 {ms_p99:.3}  max {ms_max:.3}"
        );
        println!(
            "  total bytes mean {tb_mean:.0}  median {tb_median:.0}  p99 {tb_p99:.0}  max {tb_max:.0}"
        );
        println!(
            "  peak bytes  mean {pb_mean:.0}  median {pb_median:.0}  p99 {pb_p99:.0}  max {pb_max:.0}"
        );
        println!(
            "  nodes       mean {nd_mean:.1}  median {nd_median:.1}  p99 {nd_p99:.1}  max {nd_max:.0}"
        );

        Some(SummaryRow {
            source: label,
            patterns: patterns.len(),
            translated: ms.len(),
            rejected,
            crashed,
            mean_ms: Some(ms_mean),
            median_ms: Some(ms_median),
            p99_ms: Some(ms_p99),
            max_ms: Some(ms_max),
            mean_total_bytes: Some(tb_mean),
            median_total_bytes: Some(tb_median),
            p99_total_bytes: Some(tb_p99),
            max_total_bytes: Some(tb_max as usize),
            mean_peak_bytes: Some(pb_mean),
            median_peak_bytes: Some(pb_median),
            p99_peak_bytes: Some(pb_p99),
            max_peak_bytes: Some(pb_max as usize),
            mean_nodes: Some(nd_mean),
            median_nodes: Some(nd_median),
            p99_nodes: Some(nd_p99),
            max_nodes: Some(nd_max as usize),
        })
    };

    (summary, per_pattern)
}

// -------------------------------
// CSV / text output
// -------------------------------

/// Appends exactly one row for one source's just-finished `report()`
/// call. Writes the header only when `path` doesn't exist yet -- same
/// convention `dataset_prepare.rs` uses for its own CSV. Called once
/// per source as soon as that source finishes, not accumulated and
/// rewritten the way `write_patterns_csv` still is below: an append is
/// already durable the moment it returns, so there's nothing later to
/// lose on a crash, not even the within-this-run safety net the
/// rewrite approach needed.
fn append_summary_row(
    path: &str,
    run_label: &str,
    pattern_limit: &str,
    r: &SummaryRow,
) -> std::io::Result<()> {
    let write_header = !Path::new(path).exists();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    if write_header {
        writeln!(
            f,
            "run_label,pattern_limit,source,patterns,translated,rejected,crashed,\
             mean_ms,median_ms,p99_ms,max_ms,\
             mean_total_bytes,median_total_bytes,p99_total_bytes,max_total_bytes,\
             mean_peak_bytes,median_peak_bytes,p99_peak_bytes,max_peak_bytes,\
             mean_nodes,median_nodes,p99_nodes,max_nodes"
        )?;
    }
    writeln!(
        f,
        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        csv_quote(run_label),
        pattern_limit,
        r.source,
        r.patterns,
        r.translated,
        r.rejected,
        r.crashed,
        fmt_opt(r.mean_ms),
        fmt_opt(r.median_ms),
        fmt_opt(r.p99_ms),
        fmt_opt(r.max_ms),
        fmt_opt(r.mean_total_bytes),
        fmt_opt(r.median_total_bytes),
        fmt_opt(r.p99_total_bytes),
        r.max_total_bytes.map(|v| v.to_string()).unwrap_or_default(),
        fmt_opt(r.mean_peak_bytes),
        fmt_opt(r.median_peak_bytes),
        fmt_opt(r.p99_peak_bytes),
        r.max_peak_bytes.map(|v| v.to_string()).unwrap_or_default(),
        fmt_opt(r.mean_nodes),
        fmt_opt(r.median_nodes),
        fmt_opt(r.p99_nodes),
        r.max_nodes.map(|v| v.to_string()).unwrap_or_default(),
    )?;
    Ok(())
}

/// CSV-quotes a field: wraps in `"..."`, doubling any embedded `"`.
/// Needed for both `pattern` (real patterns routinely contain commas,
/// e.g. `{2,5}`) and `rendered` (`Debug`'s own constructor-call syntax
/// is built out of commas, e.g. `Seq(Lit('a'), Lit('b'))` -- every
/// `Seq`/`Alt` node adds at least one, not just a pattern that happens
/// to match a literal `,`).
fn csv_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn write_patterns_csv(
    path: &str,
    rows: &[(&'static str, &SourcePatternStat)],
) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "source,pattern,nodes,micros,total_bytes,peak_bytes,translated")?;
    for (source, r) in rows {
        let translated = r
            .rendered
            .as_deref()
            .map(csv_quote)
            .unwrap_or_else(|| format!("<{} nodes, not rendered>", r.nodes));
        writeln!(
            f,
            "{},{},{},{},{},{},{}",
            source,
            csv_quote(&r.pattern),
            r.nodes,
            r.micros,
            r.total_bytes,
            r.peak_bytes,
            translated,
        )?;
    }
    Ok(())
}

/// Smallest `count` patterns (by node count) for one source, for the
/// human-readable examples file. Ties break on pattern text so the
/// choice is deterministic across reruns.
fn smallest_examples(per_pattern: &[SourcePatternStat], count: usize) -> Vec<&SourcePatternStat> {
    let mut rendered_only: Vec<&SourcePatternStat> =
        per_pattern.iter().filter(|p| p.rendered.is_some()).collect();
    rendered_only.sort_by(|a, b| a.nodes.cmp(&b.nodes).then_with(|| a.pattern.cmp(&b.pattern)));
    rendered_only.into_iter().take(count).collect()
}

fn write_examples_txt(
    path: &str,
    by_source: &[(&'static str, Vec<SourcePatternStat>)],
    count: usize,
) -> std::io::Result<()> {
    let mut buf = String::new();
    writeln!(buf, "Smallest {count} translated patterns per source, by node count.").unwrap();
    writeln!(buf, "Produced by `cargo bench --bench bench_translate`.").unwrap();
    writeln!(buf, "Not a measurement -- a starting point for hand-picking thesis examples;").unwrap();
    writeln!(buf, "see the full catalogue in reports/benches/bench_translate*_patterns.csv").unwrap();
    writeln!(buf, "for anything more narratively specific.").unwrap();

    for (source, per_pattern) in by_source {
        let examples = smallest_examples(per_pattern, count);
        if examples.is_empty() {
            continue;
        }
        writeln!(buf).unwrap();
        writeln!(buf, "{source}").unwrap();
        writeln!(buf, "{}", "-".repeat(source.len())).unwrap();
        for ex in examples {
            writeln!(buf).unwrap();
            writeln!(buf, "  pattern:     {}", ex.pattern).unwrap();
            writeln!(buf, "  nodes:       {}", ex.nodes).unwrap();
            writeln!(buf, "  time:        {:.3} ms", us_to_ms(ex.micros)).unwrap();
            writeln!(buf, "  peak bytes:  {}", ex.peak_bytes).unwrap();
            writeln!(buf, "  translated:  {}", ex.rendered.as_deref().unwrap_or("<not rendered>")).unwrap();
        }
    }

    std::fs::write(path, &buf)
}

fn main() {
    let sel = bench_selection();
    let render_cap = render_max_nodes();
    let example_count = examples_per_source();
    let (suricata, spamassassin, regexlib, s2_elab, innovatorved) = load_patterns(&sel);
    eprintln!(
        "bench_translate: {} suricata, {} spamassassin, {} regexlib, {} s2-elab, {} innovatorved \
         distinct patterns (limit {} each, sources={:?}, match_intent={:?}, render_cap={render_cap} nodes)",
        suricata.len(),
        spamassassin.len(),
        regexlib.len(),
        s2_elab.len(),
        innovatorved.len(),
        fmt_limit(sel.pattern_limit),
        sel.sources,
        sel.match_intent,
    );

    if let Err(e) = std::fs::create_dir_all(REPORTS_DIR) {
        eprintln!("could not create {REPORTS_DIR}: {e}");
        return;
    }
    if let Err(e) = std::fs::create_dir_all(DEMOS_DIR) {
        eprintln!("could not create {DEMOS_DIR}: {e}");
        return;
    }
    let label = run_label_suffix();
    let summary_path = format!("{REPORTS_DIR}/bench_translate.csv");
    let patterns_path = format!("{REPORTS_DIR}/bench_translate{label}_patterns.csv");
    let examples_path = format!("{DEMOS_DIR}/translation_examples.txt");
    let run_label = std::env::var("BENCH_RUN_LABEL").unwrap_or_default();
    let pattern_limit_str = fmt_limit(sel.pattern_limit);

    let mut all_patterns: Vec<(&'static str, SourcePatternStat)> = Vec::new();
    let mut by_source: Vec<(&'static str, Vec<SourcePatternStat>)> = Vec::new();

    for (label, patterns) in [
        ("dataset_suricata", &suricata),
        ("dataset_spamassassin", &spamassassin),
        ("dataset_regexlib", &regexlib),
        ("dataset_s2_elab", &s2_elab),
        ("dataset_innovatorved", &innovatorved),
    ] {
        let (summary, per_pattern) = report(label, patterns, render_cap);
        if let Some(s) = &summary {
            if let Err(e) = append_summary_row(&summary_path, &run_label, &pattern_limit_str, s) {
                eprintln!("could not append to {summary_path}: {e}");
            }
        }
        for p in &per_pattern {
            all_patterns.push((
                label,
                SourcePatternStat {
                    pattern: p.pattern.clone(),
                    nodes: p.nodes,
                    micros: p.micros,
                    total_bytes: p.total_bytes,
                    peak_bytes: p.peak_bytes,
                    rendered: p.rendered.clone(),
                },
            ));
        }
        by_source.push((label, per_pattern));

        // Re-written after every source, not just once at the end: see
        // the identical comment in bench_corpus_time.rs's main(). The
        // overview CSV doesn't need this -- `append_summary_row` above
        // already wrote (and flushed) that source's row the moment it
        // finished, durable before this point regardless.
        let refs: Vec<(&'static str, &SourcePatternStat)> =
            all_patterns.iter().map(|(s, p)| (*s, p)).collect();
        if let Err(e) = write_patterns_csv(&patterns_path, &refs) {
            eprintln!("could not write {patterns_path}: {e}");
        }
        if example_count > 0 {
            if let Err(e) = write_examples_txt(&examples_path, &by_source, example_count) {
                eprintln!("could not write {examples_path}: {e}");
            }
        }
    }
    eprintln!("appended to {summary_path}");
    eprintln!("wrote {patterns_path}");
    if example_count > 0 {
        eprintln!("wrote {examples_path}");
    }
}
