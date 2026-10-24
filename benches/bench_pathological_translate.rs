//! benches/bench_pathological_translate.rs
//!
//! Cost of frontend translation (`core_regex`: PCRE string -> `Regex`)
//! over the hand-curated stress-pattern dataset
//! (`data/pathological/pathological_patterns.json`), grouped by
//! category (A-L) -- the translation-only counterpart to
//! `bench_pathological_time.rs`/`bench_pathological_memory.rs`, the
//! same relationship `bench_corpus_translate.rs` has to
//! `bench_corpus_time.rs`/`bench_corpus_memory.rs`.
//!
//! Why this exists: categories **C**, **D**, **J**, and **K** are
//! explicitly built around compile/translate blow-up --
//! `{m,n}`-bound unrolling (`repeat_exact`/`optional_extra` in
//! `src/frontend/translate.rs`) for C/D, large alternation/group trees
//! for J/K -- as their `stress_axis`. `bench_pathological_time.rs` and
//! `bench_pathological_memory.rs` both measure the parser call on an
//! already-translated `Regex` (same baseline-after-translate design as
//! `bench_corpus_time.rs`/`bench_corpus_memory.rs`), so translation
//! cost itself -- the actual axis those four categories exist to
//! stress -- was not measured anywhere. This file closes that gap.
//!
//! No worker-process isolation, for the same reason
//! `bench_corpus_translate.rs` has none: `core_regex` enforces
//! `MAX_CORE_DEPTH`/`MAX_CORE_NODES` before the expensive part of
//! translation runs, so there is no unbounded call here for process
//! isolation to contain -- unlike the parsers, which `bench_worker`
//! isolates precisely because they lack such a cap. A per-pattern
//! `catch_unwind` stands in for crash containment, same as
//! `bench_corpus_translate.rs`.
//!
//! Row selection, sweep, and loading reuse
//! `regex_engine::data::pathological::{load, expand}` and this file's
//! own copies of `bench_pathological_time.rs`'s section/category
//! selection helpers (duplicated, not shared, per this codebase's
//! existing convention of not sharing code between bench binaries).
//! One difference from the other two `bench_pathological_*` targets:
//! this file sweeps `PATHO_P` only, not `PATHO_N`. A pattern's
//! translated form depends only on the pattern text -- `pattern_template`
//! resolved through a row's `generator` at a given `p` (pattern size);
//! `n` (input length) only ever parameterizes `match_input`/
//! `nomatch_input`, which translation never touches. So `PATHO_N` is
//! read nowhere in this file and has no effect here -- not an
//! oversight, the same reasoning `bench_corpus_translate.rs` uses for
//! not reading `BENCH_ENTRY_TIMEOUT_MS`/`BENCH_SPLIT_OUTCOME` (nothing
//! here for that variable to act on). For a row with no `generator`,
//! `p` doesn't move its pattern either, so it is measured exactly
//! once; a `generated_patterns` row with a `generator` is measured
//! once per distinct swept `p` -- the category-J/K blow-up curve this
//! bench exists to show.
//!
//! Writes `reports/benches/bench_pathological_translate.csv` -- one
//! row per category: mean/median/p99/max of time (ms), allocation
//! (total and peak bytes), and translated size (nodes), over every
//! row/`p` combination that translated successfully (`translated`;
//! `rejected` -- `core_regex` itself returned `Err`, e.g. hit its
//! depth/node cap -- and `crashed` are excluded, same reasoning as
//! `bench_corpus_translate.rs`'s own summary row). Always this exact
//! filename, ignoring `BENCH_RUN_LABEL` -- **appended** to, never
//! overwritten, the same convention `bench_corpus_translate.rs` uses
//! for its own aggregate CSV, so a run scoped to one category and a
//! run covering every category both add their row(s) to the same
//! growing file. `run_label` and `pattern_limit` columns record each
//! row's own invocation, since the filename no longer can.
//!
//! Also writes `reports/benches/bench_pathological_translate{label}_patterns.csv`
//! (one row per distinct row/`p` entry that translated successfully:
//! id, category, p, pattern, nodes, micros, bytes, and the `Regex` via
//! its `Debug`, when nodes <= `BENCH_TRANSLATE_RENDER_MAX_NODES`,
//! default 150) -- a catalogue for hand-picking thesis examples, same
//! role and naming (label baked into the filename, not a column) as
//! `bench_corpus_translate.rs`'s own `*_patterns.csv`. Overwritten each
//! run, not appended.
//!
//! Reads `PATHO_JSON_PATH`, `PATHO_SECTION`, `PATHO_CATEGORY`,
//! `PATHO_P`, `BENCH_PATTERN_LIMIT` (caps selected *rows*, before the
//! `p` sweep, same as the other two `bench_pathological_*` targets),
//! `BENCH_RUN_LABEL`, and `BENCH_TRANSLATE_RENDER_MAX_NODES`. Does not
//! read `PATHO_N` (see above) or `BENCH_ENTRY_TIMEOUT_MS` (no process
//! isolation to bound).
//!
//! cargo bench --bench bench_pathological_translate

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::{BTreeMap, HashSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use regex_engine::data::pathological::{self, PathologicalDoc, PathologicalRow};
use regex_engine::data::prepare::core_regex;
use regex_engine::types::Regex;

const DEFAULT_JSON_PATH: &str = "data/pathological/pathological_patterns.json";
const PATTERN_LIMIT: usize = usize::MAX;
const REPORTS_DIR: &str = "reports/benches";
/// Same variable and default as `bench_corpus_translate.rs`'s own
/// render cap -- one convention across the whole translate-bench family.
const DEFAULT_RENDER_MAX_NODES: usize = 150;

fn fmt_limit(n: usize) -> String {
    if n == usize::MAX { "unlimited".to_string() } else { n.to_string() }
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

// -------------------------------
// Counting allocator -- mirrors bench_corpus_translate.rs's own: one
// process, many patterns, reset to a fresh baseline per pattern in
// measure_one below.
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
// Selection -- mirrors bench_pathological_time.rs's own, minus PATHO_N
// (see module doc for why this file never reads it).
// -------------------------------

#[derive(Clone, Copy)]
enum Section {
    Pathological,
    Baseline,
    Generated,
    All,
}

fn parse_section() -> Section {
    match std::env::var("PATHO_SECTION").ok().as_deref() {
        None | Some("all") => Section::All,
        Some("pathological") => Section::Pathological,
        Some("baseline") => Section::Baseline,
        Some("generated") => Section::Generated,
        Some(other) => panic!("PATHO_SECTION must be pathological, baseline, generated, or all, got {other:?}"),
    }
}

fn parse_categories() -> Option<HashSet<String>> {
    std::env::var("PATHO_CATEGORY").ok().map(|s| s.split(',').map(|c| c.trim().to_uppercase()).collect())
}

fn parse_u32_list(var: &str) -> Option<Vec<u32>> {
    std::env::var(var).ok().map(|s| {
        s.split(',')
            .map(|part| part.trim().parse().unwrap_or_else(|_| panic!("{var} entries must be positive integers, got {part:?}")))
            .collect()
    })
}

fn pattern_limit() -> usize {
    std::env::var("BENCH_PATTERN_LIMIT")
        .ok()
        .map(|s| s.parse().unwrap_or_else(|_| panic!("BENCH_PATTERN_LIMIT must be a positive integer, got {s:?}")))
        .unwrap_or(PATTERN_LIMIT)
}

/// Optional filename suffix, same role as in `bench_pathological_time.rs`.
/// Only affects nothing here directly -- the aggregate CSV ignores it,
/// same as `bench_corpus_translate.rs`'s own aggregate CSV -- kept for
/// parity with the other `PATHO_*`-reading targets' env surface.
fn run_label_suffix() -> String {
    std::env::var("BENCH_RUN_LABEL").unwrap_or_default()
}

fn json_path() -> PathBuf {
    std::env::var("PATHO_JSON_PATH").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(DEFAULT_JSON_PATH))
}

fn select_rows<'a>(
    doc: &'a PathologicalDoc,
    section: Section,
    categories: &Option<HashSet<String>>,
) -> Vec<&'a PathologicalRow> {
    let include = |r: &&PathologicalRow| categories.as_ref().map_or(true, |cats| cats.contains(&r.category));
    let mut rows: Vec<&PathologicalRow> = Vec::new();
    match section {
        Section::Pathological => rows.extend(doc.pathological_patterns.iter().filter(include)),
        Section::Baseline => rows.extend(doc.baseline_and_edge_patterns.iter().filter(include)),
        Section::Generated => rows.extend(doc.generated_patterns.iter().filter(include)),
        Section::All => {
            rows.extend(doc.pathological_patterns.iter().filter(include));
            rows.extend(doc.baseline_and_edge_patterns.iter().filter(include));
            rows.extend(doc.generated_patterns.iter().filter(include));
        }
    }
    rows
}

struct PatternEntry {
    row_id: String,
    category: String,
    p: u32,
    pattern: String,
}

/// One entry per row per swept `p`. `p` only moves the resulting
/// pattern for a `generated_patterns` row with a `generator` --
/// `pathological::expand`'s own `n` parameter is irrelevant to the
/// pattern it returns, so it is called with a fixed, unused value
/// (`0`) rather than threading a real `n` sweep through this file.
fn build_entries(doc: &PathologicalDoc, rows: &[&PathologicalRow], p_override: &Option<Vec<u32>>) -> Vec<PatternEntry> {
    const UNUSED_N: u32 = 0;
    let mut entries = Vec::new();
    for row in rows {
        let cat = doc.categories.get(&row.category);
        let needs_p = row.generator.is_some();
        let p_list = if needs_p {
            p_override
                .clone()
                .or_else(|| row.suggested_p.clone())
                .or_else(|| cat.and_then(|c| c.suggested_p.clone()))
                .unwrap_or_else(|| vec![pathological::DEFAULT_P])
        } else {
            vec![pathological::DEFAULT_P]
        };
        for &p in &p_list {
            let (pattern, _match_input, _nomatch_input) = pathological::expand(row, UNUSED_N, p);
            entries.push(PatternEntry {
                row_id: row.id.clone(),
                category: row.category.clone(),
                p,
                pattern,
            });
        }
    }
    entries
}

fn group_by_category(entries: Vec<PatternEntry>) -> BTreeMap<String, Vec<PatternEntry>> {
    let mut map: BTreeMap<String, Vec<PatternEntry>> = BTreeMap::new();
    for e in entries {
        map.entry(e.category.clone()).or_default().push(e);
    }
    map
}

// -------------------------------
// Per-pattern measurement -- mirrors bench_corpus_translate.rs's
// measure_one/node_count exactly.
// -------------------------------

/// Iterative, not recursive -- see the identical function in
/// `bench_corpus_translate.rs` for why.
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

/// One successfully-translated entry, kept for the per-pattern
/// catalogue -- the pathological-dataset counterpart to
/// `bench_corpus_translate.rs`'s `SourcePatternStat`.
struct PatternStat {
    row_id: String,
    category: String,
    p: u32,
    pattern: String,
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

fn measure_one(pattern: &str, render_cap: usize) -> MeasureOutcome {
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
            let rendered = if nodes <= render_cap { Some(format!("{r:?}")) } else { None };
            MeasureOutcome::Done(Measurement { nodes, micros, total_bytes, peak_bytes, rendered })
        }
    }
}

// -------------------------------
// Reporting -- grouped by category, one row per category (no parser
// dimension: translation doesn't depend on which parser will run
// later, same reason bench_corpus_translate.rs's summary has no
// per-parser breakdown either).
// -------------------------------

struct SummaryRow {
    category: String,
    category_name: String,
    rows: usize,
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

fn report(
    category: &str,
    category_name: &str,
    entries: &[PatternEntry],
    render_cap: usize,
) -> (Option<SummaryRow>, Vec<PatternStat>) {
    if entries.is_empty() {
        return (None, Vec::new());
    }

    let mut ms: Vec<f64> = Vec::with_capacity(entries.len());
    let mut total_bytes: Vec<f64> = Vec::with_capacity(entries.len());
    let mut peak_bytes: Vec<f64> = Vec::with_capacity(entries.len());
    let mut nodes_f: Vec<f64> = Vec::with_capacity(entries.len());
    let mut rejected = 0usize;
    let mut crashed = 0usize;
    let mut per_pattern: Vec<PatternStat> = Vec::with_capacity(entries.len());

    for entry in entries {
        match measure_one(&entry.pattern, render_cap) {
            MeasureOutcome::Done(m) => {
                ms.push(us_to_ms(m.micros));
                total_bytes.push(m.total_bytes as f64);
                peak_bytes.push(m.peak_bytes as f64);
                nodes_f.push(m.nodes as f64);
                per_pattern.push(PatternStat {
                    row_id: entry.row_id.clone(),
                    category: category.to_string(),
                    p: entry.p,
                    pattern: entry.pattern.clone(),
                    nodes: m.nodes,
                    micros: m.micros,
                    total_bytes: m.total_bytes,
                    peak_bytes: m.peak_bytes,
                    rendered: m.rendered,
                });
            }
            MeasureOutcome::Rejected(e) => {
                rejected += 1;
                eprintln!(
                    "  rejected: {category}/{} row={} p={} pat={:?} -- {e}",
                    category_name, entry.row_id, entry.p, entry.pattern,
                );
            }
            MeasureOutcome::Crashed => {
                crashed += 1;
                eprintln!(
                    "  crash: {category}/{} row={} p={} pat={:?} -- core_regex panicked",
                    category_name, entry.row_id, entry.p, entry.pattern,
                );
            }
        }
    }

    if ms.is_empty() {
        println!("\n{category} ({category_name}, {} rows, 0 translated, {rejected} rejected, {crashed} crashed)", entries.len());
        return (Some(SummaryRow {
            category: category.to_string(),
            category_name: category_name.to_string(),
            rows: entries.len(),
            translated: 0,
            rejected,
            crashed,
            mean_ms: None, median_ms: None, p99_ms: None, max_ms: None,
            mean_total_bytes: None, median_total_bytes: None, p99_total_bytes: None, max_total_bytes: None,
            mean_peak_bytes: None, median_peak_bytes: None, p99_peak_bytes: None, max_peak_bytes: None,
            mean_nodes: None, median_nodes: None, p99_nodes: None, max_nodes: None,
        }), per_pattern);
    }

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

    println!("\n{category} ({category_name}, {} rows, {} translated, {rejected} rejected, {crashed} crashed)", entries.len(), ms.len());
    println!("  time (ms)   mean {ms_mean:.3}  median {ms_median:.3}  p99 {ms_p99:.3}  max {ms_max:.3}");
    println!("  total bytes mean {tb_mean:.0}  median {tb_median:.0}  p99 {tb_p99:.0}  max {tb_max:.0}");
    println!("  peak bytes  mean {pb_mean:.0}  median {pb_median:.0}  p99 {pb_p99:.0}  max {pb_max:.0}");
    println!("  nodes       mean {nd_mean:.1}  median {nd_median:.1}  p99 {nd_p99:.1}  max {nd_max:.0}");

    (Some(SummaryRow {
        category: category.to_string(),
        category_name: category_name.to_string(),
        rows: entries.len(),
        translated: ms.len(),
        rejected,
        crashed,
        mean_ms: Some(ms_mean), median_ms: Some(ms_median), p99_ms: Some(ms_p99), max_ms: Some(ms_max),
        mean_total_bytes: Some(tb_mean), median_total_bytes: Some(tb_median), p99_total_bytes: Some(tb_p99), max_total_bytes: Some(tb_max as usize),
        mean_peak_bytes: Some(pb_mean), median_peak_bytes: Some(pb_median), p99_peak_bytes: Some(pb_p99), max_peak_bytes: Some(pb_max as usize),
        mean_nodes: Some(nd_mean), median_nodes: Some(nd_median), p99_nodes: Some(nd_p99), max_nodes: Some(nd_max as usize),
    }), per_pattern)
}

// -------------------------------
// CSV output -- appended, mirrors bench_corpus_translate.rs's
// append_summary_row exactly in spirit (one row per selection unit,
// written the moment it finishes).
// -------------------------------

fn csv_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Per-pattern catalogue, mirrors `bench_corpus_translate.rs`'s
/// `write_patterns_csv` exactly in spirit: a starting point for
/// hand-picking thesis examples, not a measurement in itself.
fn write_patterns_csv(path: &str, rows: &[PatternStat]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "id,category,p,pattern,nodes,micros,total_bytes,peak_bytes,translated")?;
    for r in rows {
        let translated = r
            .rendered
            .as_deref()
            .map(csv_quote)
            .unwrap_or_else(|| format!("<{} nodes, not rendered>", r.nodes));
        writeln!(
            f,
            "{},{},{},{},{},{},{},{},{}",
            r.row_id,
            r.category,
            r.p,
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

fn append_summary_row(path: &str, run_label: &str, pattern_limit: &str, r: &SummaryRow) -> std::io::Result<()> {
    let write_header = !Path::new(path).exists();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    if write_header {
        writeln!(
            f,
            "run_label,pattern_limit,category,category_name,rows,translated,rejected,crashed,\
             mean_ms,median_ms,p99_ms,max_ms,\
             mean_total_bytes,median_total_bytes,p99_total_bytes,max_total_bytes,\
             mean_peak_bytes,median_peak_bytes,p99_peak_bytes,max_peak_bytes,\
             mean_nodes,median_nodes,p99_nodes,max_nodes"
        )?;
    }
    writeln!(
        f,
        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        csv_quote(run_label),
        pattern_limit,
        r.category,
        csv_quote(&r.category_name),
        r.rows,
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

fn main() {
    let section = parse_section();
    let categories = parse_categories();
    let p_override = parse_u32_list("PATHO_P");
    let limit = pattern_limit();
    let path = json_path();
    let render_cap = render_max_nodes();

    let doc = pathological::load(&path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));
    let mut rows = select_rows(&doc, section, &categories);
    rows.truncate(limit);
    if rows.is_empty() {
        panic!("no rows selected from {path:?} (section/category filters too narrow?)");
    }
    let entries = build_entries(&doc, &rows, &p_override);
    eprintln!(
        "bench_pathological_translate: {} rows -> {} pattern entries (limit {})",
        rows.len(),
        entries.len(),
        fmt_limit(limit),
    );

    if let Err(e) = std::fs::create_dir_all(REPORTS_DIR) {
        eprintln!("could not create {REPORTS_DIR}: {e}");
        return;
    }
    let summary_path = format!("{REPORTS_DIR}/bench_pathological_translate.csv");
    let run_label = run_label_suffix();
    let label_suffix = if run_label.is_empty() { String::new() } else { format!("_{run_label}") };
    let patterns_path = format!("{REPORTS_DIR}/bench_pathological_translate{label_suffix}_patterns.csv");
    let pattern_limit_str = fmt_limit(limit);

    let mut all_patterns: Vec<PatternStat> = Vec::new();

    for (category, cat_entries) in group_by_category(entries) {
        let category_name = doc.categories.get(&category).map(|c| c.name.as_str()).unwrap_or("?");
        let (summary, per_pattern) = report(&category, category_name, &cat_entries, render_cap);
        if let Some(summary) = summary {
            if let Err(e) = append_summary_row(&summary_path, &run_label, &pattern_limit_str, &summary) {
                eprintln!("could not append to {summary_path}: {e}");
            }
        }
        all_patterns.extend(per_pattern);

        // Re-written after every category, not just once at the end --
        // same reasoning as bench_corpus_translate.rs's main().
        if let Err(e) = write_patterns_csv(&patterns_path, &all_patterns) {
            eprintln!("could not write {patterns_path}: {e}");
        }
    }
    eprintln!("appended to {summary_path}");
    eprintln!("wrote {patterns_path}");
}
