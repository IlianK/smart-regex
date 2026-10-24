//! benches/bench_pathological_memory.rs
//!
//! Heap allocation per parser over the hand-curated stress-pattern
//! dataset (`data/pathological/pathological_patterns.json`) -- the
//! memory counterpart to `bench_pathological_time.rs`, which documents
//! the sweep/expansion/grouping scheme and the `PATHO_*` variables
//! shared with this file. See `bench_corpus_memory.rs` for why "heap"
//! specifically, not memory in general.
//!
//! Same process-isolation architecture as `bench_corpus_memory.rs`,
//! deliberately duplicated rather than shared (this codebase's existing
//! convention): every entry runs in its own `bench_worker` process,
//! bounded by `BENCH_ENTRY_TIMEOUT_MS`, killed on timeout, with the
//! counting `#[global_allocator]` living in that worker, not here.
//!
//! Writes `reports/benches/bench_pathological_memory.csv` (one row per
//! category x parser) and `reports/benches/bench_pathological_memory_entries.csv`
//! (one row per individual (row, n, p, parser, match/non-match side)
//! measurement, including the actual pattern and input text used,
//! truncated past 200 characters -- same role and convention as
//! `bench_pathological_time.rs`'s own entries file). Both replace any
//! existing file.
//!
//! Build the worker first, then run:
//!   cargo build --release --example bench_worker
//!   cargo bench --bench bench_pathological_memory

use std::collections::{BTreeMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use regex_engine::data::pathological::{self, PathologicalDoc, PathologicalRow};
use regex_engine::parsers::ALL;

const DEFAULT_JSON_PATH: &str = "data/pathological/pathological_patterns.json";
const PATTERN_LIMIT: usize = usize::MAX;
const DEFAULT_ENTRY_TIMEOUT_MS: u64 = 2000;
const REPORTS_DIR: &str = "reports/benches";
const WORKER_NAME: &str = "bench_worker";

fn fmt_limit(n: usize) -> String {
    if n == usize::MAX { "unlimited".to_string() } else { n.to_string() }
}

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

fn parse_row_ids() -> Option<HashSet<String>> {
    std::env::var("PATHO_ROW_ID").ok().map(|s| s.split(',').map(|id| id.trim().to_string()).collect())
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

/// Per-entry cap. See `bench_corpus_memory.rs`'s module doc.
fn entry_timeout() -> Duration {
    let ms: u64 = std::env::var("BENCH_ENTRY_TIMEOUT_MS")
        .ok()
        .map(|s| s.parse().unwrap_or_else(|_| panic!("BENCH_ENTRY_TIMEOUT_MS must be a positive integer, got {s:?}")))
        .unwrap_or(DEFAULT_ENTRY_TIMEOUT_MS);
    Duration::from_millis(ms)
}

/// Optional filename suffix, same role as in `bench_corpus_memory.rs`.
fn run_label_suffix() -> String {
    std::env::var("BENCH_RUN_LABEL").ok().map(|l| format!("_{l}")).unwrap_or_default()
}

fn json_path() -> PathBuf {
    std::env::var("PATHO_JSON_PATH").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(DEFAULT_JSON_PATH))
}

struct Entry {
    row_id: String,
    category: String,
    n: u32,
    p: u32,
    side: &'static str,
    pattern: String,
    input: String,
}

fn select_rows<'a>(
    doc: &'a PathologicalDoc,
    section: Section,
    categories: &Option<HashSet<String>>,
    row_ids: &Option<HashSet<String>>,
) -> Vec<&'a PathologicalRow> {
    let include = |r: &&PathologicalRow| {
        categories.as_ref().map_or(true, |cats| cats.contains(&r.category))
            && row_ids.as_ref().map_or(true, |ids| ids.contains(&r.id))
    };
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

fn build_entries(
    doc: &PathologicalDoc,
    rows: &[&PathologicalRow],
    n_override: &Option<Vec<u32>>,
    p_override: &Option<Vec<u32>>,
) -> Vec<Entry> {
    let mut entries = Vec::new();
    for row in rows {
        let cat = doc.categories.get(&row.category);
        let n_list = n_override
            .clone()
            .or_else(|| row.suggested_n.clone())
            .or_else(|| cat.and_then(|c| c.suggested_n.clone()))
            .unwrap_or_else(|| vec![pathological::DEFAULT_N]);
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
            for &n in &n_list {
                let (pattern, m, nm) = pathological::expand(row, n, p);
                if let Some(input) = m {
                    entries.push(Entry {
                        row_id: row.id.clone(),
                        category: row.category.clone(),
                        n,
                        p,
                        side: "match",
                        pattern: pattern.clone(),
                        input,
                    });
                }
                if let Some(input) = nm {
                    entries.push(Entry {
                        row_id: row.id.clone(),
                        category: row.category.clone(),
                        n,
                        p,
                        side: "nomatch",
                        pattern: pattern.clone(),
                        input,
                    });
                }
            }
        }
    }
    entries
}

fn group_by_category(entries: Vec<Entry>) -> BTreeMap<String, Vec<Entry>> {
    let mut map: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
    for e in entries {
        map.entry(e.category.clone()).or_default().push(e);
    }
    map
}

// -------------------------------
// Isolated call -- mirrors bench_corpus_memory.rs's run_entry_isolated
// exactly; duplicated, not shared, per this codebase's convention.
// -------------------------------

fn worker_path() -> PathBuf {
    let exe = std::env::current_exe().expect("reading this process's own exe path");
    let deps_dir = exe.parent().expect("exe has a parent directory");
    let profile_dir = deps_dir.parent().expect("deps/ has a parent directory");
    let path = profile_dir.join("examples").join(WORKER_NAME);
    if !path.exists() {
        panic!(
            "worker not found at {path:?} -- build it first:\n  \
             cargo build --release --example {WORKER_NAME}"
        );
    }
    path
}

#[derive(serde::Deserialize)]
struct JobResult {
    #[allow(dead_code)]
    micros: u64,
    total_bytes: usize,
    peak_bytes: usize,
}

enum Outcome {
    Done { total_bytes: usize, peak_bytes: usize },
    TimedOut,
    Crashed(String),
}

fn run_entry_isolated(worker: &Path, cap: Duration, parser_idx: usize, pattern: &str, input: &str) -> Outcome {
    let mut child: Child = match Command::new(worker)
        .arg(parser_idx.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => panic!("failed to spawn {worker:?}: {e}"),
    };

    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        let job = serde_json::json!({ "pattern": pattern, "input": input }).to_string();
        if let Err(e) = stdin.write_all(job.as_bytes()) {
            panic!("failed to write job to {worker:?}'s stdin: {e}");
        }
    }

    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out);
        let mut err = String::new();
        let _ = stderr.read_to_string(&mut err);
        let _ = tx.send((out, err));
    });

    let deadline = Instant::now() + cap;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let (out, err) = rx.recv().unwrap_or_default();
                if !status.success() {
                    let reason = match status.code() {
                        Some(code) => format!("exit code {code}"),
                        None => "killed by signal (likely stack overflow)".to_string(),
                    };
                    return Outcome::Crashed(format!("{reason}; stderr={}", err.trim()));
                }
                return match serde_json::from_str::<JobResult>(out.trim()) {
                    Ok(r) => Outcome::Done { total_bytes: r.total_bytes, peak_bytes: r.peak_bytes },
                    Err(e) => Outcome::Crashed(format!(
                        "worker exited 0 but its output did not parse: {e}; stdout={:?} stderr={:?}",
                        out.trim(),
                        err.trim()
                    )),
                };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    if let Err(e) = child.kill() {
                        eprintln!("warning: failed to kill timed-out worker: {e}");
                    }
                    let _ = child.wait();
                    return Outcome::TimedOut;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => panic!("error polling worker process: {e}"),
        }
    }
}

// -------------------------------
// Reporting -- grouped by category instead of SourceKind
// -------------------------------

struct SummaryRow {
    category: String,
    category_name: String,
    parser: &'static str,
    entries: usize,
    measured: usize,
    timed_out: usize,
    crashed: usize,
    total_bytes: usize,
    avg_bytes: Option<usize>,
    max_peak: usize,
}

/// One (row, n, p, parser, side) measurement -- the entries file's raw
/// material, before it gets collapsed into one category-wide summary.
/// Mirrors `bench_pathological_time.rs`'s own `EntryResult`.
struct EntryResult {
    row_id: String,
    category: String,
    parser: &'static str,
    n: u32,
    p: u32,
    side: &'static str,
    pattern: String,
    input: String,
    outcome: &'static str,
    total_bytes: Option<usize>,
    peak_bytes: Option<usize>,
}

fn csv_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// See `bench_pathological_time.rs`'s own `truncate` for why: at the
/// Fig.-7 end of a sweep, `input` can run into the millions of
/// characters, and a CSV row that long defeats the point of a quick
/// per-pattern lookup table.
fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { s.chars().take(n).collect::<String>() + "..." }
}

fn report(
    category: &str,
    category_name: &str,
    corpus: &[Entry],
    worker: &Path,
    cap: Duration,
) -> (Vec<SummaryRow>, Vec<EntryResult>) {
    if corpus.is_empty() {
        return (Vec::new(), Vec::new());
    }
    println!("\n{category} ({category_name}, {} entries)", corpus.len());
    println!(
        "{:<16} {:>9} {:>9} {:>7} {:>14} {:>14} {:>14}",
        "parser", "measured", "timed_out", "crashed", "total bytes", "avg/entry", "max peak"
    );
    println!("{:-<16} {:->9} {:->9} {:->7} {:->14} {:->14} {:->14}", "", "", "", "", "", "", "");

    let mut rows = Vec::new();
    let mut entry_rows = Vec::new();
    for (parser_idx, parser_type) in ALL.iter().enumerate() {
        let mut total_sum = 0usize;
        let mut peak_max = 0usize;
        let mut measured = 0usize;
        let mut timed_out = 0usize;
        let mut crashed = 0usize;
        for entry in corpus {
            let (outcome, total_bytes, peak_bytes) =
                match run_entry_isolated(worker, cap, parser_idx, &entry.pattern, &entry.input) {
                    Outcome::Done { total_bytes, peak_bytes } => {
                        total_sum += total_bytes;
                        peak_max = peak_max.max(peak_bytes);
                        measured += 1;
                        ("measured", Some(total_bytes), Some(peak_bytes))
                    }
                    Outcome::TimedOut => {
                        timed_out += 1;
                        ("timed_out", None, None)
                    }
                    Outcome::Crashed(reason) => {
                        crashed += 1;
                        eprintln!(
                            "  crash: {category}/{} row={} n={} p={} pat={:?} in={:?} -- {reason}",
                            parser_type.name(),
                            entry.row_id,
                            entry.n,
                            entry.p,
                            entry.pattern,
                            entry.input,
                        );
                        ("crashed", None, None)
                    }
                };
            entry_rows.push(EntryResult {
                row_id: entry.row_id.clone(),
                category: entry.category.clone(),
                parser: parser_type.name(),
                n: entry.n,
                p: entry.p,
                side: entry.side,
                pattern: entry.pattern.clone(),
                input: entry.input.clone(),
                outcome,
                total_bytes,
                peak_bytes,
            });
        }
        let avg = if measured > 0 { Some(total_sum / measured) } else { None };
        println!(
            "{:<16} {:>9} {:>9} {:>7} {:>14} {:>14} {:>14}",
            parser_type.name(),
            measured,
            timed_out,
            crashed,
            total_sum,
            avg.map(|v| v.to_string()).unwrap_or_else(|| "n/a".to_string()),
            peak_max,
        );
        rows.push(SummaryRow {
            category: category.to_string(),
            category_name: category_name.to_string(),
            parser: parser_type.name(),
            entries: corpus.len(),
            measured,
            timed_out,
            crashed,
            total_bytes: total_sum,
            avg_bytes: avg,
            max_peak: peak_max,
        });
    }
    (rows, entry_rows)
}

fn write_entries_csv(path: &str, rows: &[EntryResult]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "row_id,category,parser,n,p,side,pattern,input,outcome,total_bytes,peak_bytes")?;
    for r in rows {
        writeln!(
            f,
            "{},{},{},{},{},{},{},{},{},{},{}",
            r.row_id,
            r.category,
            r.parser,
            r.n,
            r.p,
            r.side,
            csv_quote(&truncate(&r.pattern, 200)),
            csv_quote(&truncate(&r.input, 200)),
            r.outcome,
            r.total_bytes.map(|v| v.to_string()).unwrap_or_default(),
            r.peak_bytes.map(|v| v.to_string()).unwrap_or_default(),
        )?;
    }
    Ok(())
}

fn write_summary_csv(path: &str, rows: &[SummaryRow]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "category,category_name,parser,entries,measured,timed_out,crashed,total_bytes,avg_bytes,max_peak_bytes")?;
    for r in rows {
        writeln!(
            f,
            "{},{},{},{},{},{},{},{},{},{}",
            r.category,
            r.category_name,
            r.parser,
            r.entries,
            r.measured,
            r.timed_out,
            r.crashed,
            r.total_bytes,
            r.avg_bytes.map(|v| v.to_string()).unwrap_or_default(),
            r.max_peak,
        )?;
    }
    Ok(())
}

fn main() {
    let section = parse_section();
    let categories = parse_categories();
    let row_ids = parse_row_ids();
    let n_override = parse_u32_list("PATHO_N");
    let p_override = parse_u32_list("PATHO_P");
    let limit = pattern_limit();
    let cap = entry_timeout();
    let worker = worker_path();
    let path = json_path();

    let doc = pathological::load(&path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));
    let mut rows = select_rows(&doc, section, &categories, &row_ids);
    rows.truncate(limit);
    if rows.is_empty() {
        panic!("no rows selected from {path:?} (section/category filters too narrow?)");
    }
    let entries = build_entries(&doc, &rows, &n_override, &p_override);
    if entries.is_empty() {
        panic!("{} rows selected but every match_input/nomatch_input was null", rows.len());
    }
    eprintln!(
        "bench_pathological_memory: {} rows -> {} entries (limit {}, entry_timeout={:?})",
        rows.len(),
        entries.len(),
        fmt_limit(limit),
        cap,
    );

    if let Err(e) = std::fs::create_dir_all(REPORTS_DIR) {
        eprintln!("could not create {REPORTS_DIR}: {e}");
        return;
    }
    let label = run_label_suffix();
    let path_out = format!("{REPORTS_DIR}/bench_pathological_memory{label}.csv");
    let entries_path = format!("{REPORTS_DIR}/bench_pathological_memory{label}_entries.csv");

    let mut all_rows = Vec::new();
    let mut all_entries = Vec::new();
    for (category, corpus) in group_by_category(entries) {
        let category_name = doc.categories.get(&category).map(|c| c.name.as_str()).unwrap_or("?");
        let (summary, entry_rows) = report(&category, category_name, &corpus, &worker, cap);
        all_rows.extend(summary);
        all_entries.extend(entry_rows);
        if let Err(e) = write_summary_csv(&path_out, &all_rows) {
            eprintln!("could not write {path_out}: {e}");
        }
        if let Err(e) = write_entries_csv(&entries_path, &all_entries) {
            eprintln!("could not write {entries_path}: {e}");
        }
    }
    eprintln!("wrote {path_out}");
    eprintln!("wrote {entries_path}");
}
