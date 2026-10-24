//! benches/bench_pathological_time.rs
//!
//! Wall-clock cost per parser over the hand-curated stress-pattern
//! dataset (`data/pathological/pathological_patterns.json`), not the
//! real corpus -- see `bench_corpus_time.rs` for that one. Every row's
//! `match_input`/`nomatch_input` is expanded at each swept `n` (input
//! length) and, for a `generated_patterns` row with a `generator`,
//! each swept `p` (pattern size) too; each expansion becomes up to two
//! entries (match + non-match), grouped by category (A-L) for
//! reporting, the same role `source` plays in `bench_corpus_time.rs`.
//!
//! Same process-isolation architecture as `bench_corpus_time.rs`,
//! deliberately duplicated rather than shared (this codebase's existing
//! convention -- see that file's own module doc): every entry runs in
//! its own `bench_worker` process, bounded by `BENCH_ENTRY_TIMEOUT_MS`,
//! killed on timeout. No pre-filtering by `core_regex` before spawning
//! a worker here, unlike the real-corpus benches: a pathological
//! pattern's translation itself failing (hitting `core_regex`'s own
//! depth/node cap) is a real outcome worth seeing, not a bug to filter
//! out before measuring -- it surfaces as `crashed`, same as any other
//! non-zero worker exit.
//!
//! Configuration is this file's own `PATHO_*` variables, kept distinct
//! from `BENCH_SOURCE`-style names since nothing here selects a
//! `SourceKind`:
//!   - `PATHO_JSON_PATH` (default `data/pathological/pathological_patterns.json`)
//!   - `PATHO_SECTION` (`pathological`/`baseline`/`generated`/`all`, default `all`)
//!   - `PATHO_CATEGORY` (comma-separated `A`-`L`, default every category present)
//!   - `PATHO_ROW_ID` (comma-separated row ids, e.g. `P01,E36`; default
//!     every row the section/category filters admit) -- narrow to one
//!     row to get a single pattern's own scaling curve instead of a
//!     whole category's.
//!   - `PATHO_N` / `PATHO_P` (comma-separated sweep overrides; default
//!     each row's own category `suggested_n`/`suggested_p`, falling
//!     back to `pathological::DEFAULT_N`/`DEFAULT_P` if absent)
//! `BENCH_PATTERN_LIMIT`, `BENCH_ENTRY_TIMEOUT_MS`, and `BENCH_RUN_LABEL`
//! keep their existing names and meaning -- here, `BENCH_PATTERN_LIMIT`
//! caps the number of *rows* admitted, after section/category/row-id
//! filtering and before the `n`/`p` sweep. No `BENCH_MATCH_INTENT`:
//! these patterns have no anchor/match-intent concept.
//!
//! Writes `reports/benches/bench_pathological_time.csv` (one row per
//! category x parser, mean/median/p99/max over every entry in that
//! category -- a category can mix several rows and several `n`, so
//! this number does not describe any one pattern's own growth curve)
//! and `reports/benches/bench_pathological_time_entries.csv` (one row
//! per individual (row, n, p, parser, match/non-match side) measurement
//! -- group this by `row_id` and `parser`, sort by `n`, and it is
//! exactly the (input size, time) series a Fig.-7-style plot needs for
//! one pattern). Both replace any existing file, same "latest run, not
//! a log" convention as `bench_corpus_time.rs`.
//!
//! Build the worker first, then run:
//!   cargo build --release --example bench_worker
//!   cargo bench --bench bench_pathological_time

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

/// Per-entry cap. See `bench_corpus_time.rs`'s module doc for what this
/// does and does not protect against.
fn entry_timeout() -> Duration {
    let ms: u64 = std::env::var("BENCH_ENTRY_TIMEOUT_MS")
        .ok()
        .map(|s| s.parse().unwrap_or_else(|_| panic!("BENCH_ENTRY_TIMEOUT_MS must be a positive integer, got {s:?}")))
        .unwrap_or(DEFAULT_ENTRY_TIMEOUT_MS);
    Duration::from_millis(ms)
}

/// Optional filename suffix, same role as in `bench_corpus_time.rs`.
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

/// Rows from the selected section(s), filtered by category and row id
/// if set.
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

/// Sweeps every row over its own `n`/`p` values (env override, else the
/// row's own `suggested_n`/`suggested_p`, else its category's, else a
/// fallback default), expanding each point into up to two entries.
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
// Isolated call -- mirrors bench_corpus_time.rs's run_entry_isolated
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
    micros: u64,
    #[allow(dead_code)]
    total_bytes: usize,
    #[allow(dead_code)]
    peak_bytes: usize,
    encode_micros: u64,
    derive_micros: u64,
    decode_micros: u64,
}

/// `total` is `micros` as before; `encode`/`derive`/`decode` are only
/// meaningful for deriv_bc/pderiv_bc (0/total/0 for the other three).
#[derive(Clone, Copy)]
struct Timing {
    total: Duration,
    encode: Duration,
    derive: Duration,
    decode: Duration,
}

enum Outcome {
    Done(Timing),
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
                    Ok(r) => Outcome::Done(Timing {
                        total: Duration::from_micros(r.micros),
                        encode: Duration::from_micros(r.encode_micros),
                        derive: Duration::from_micros(r.derive_micros),
                        decode: Duration::from_micros(r.decode_micros),
                    }),
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
    wall_s: f64,
    mean_ms: Option<f64>,
    median_ms: Option<f64>,
    p99_ms: Option<f64>,
    max_ms: Option<f64>,
    // Breakdown of mean_ms for deriv_bc/pderiv_bc (encode is 0 for the
    // other three parsers, decode is 0 for them too, derive is mean_ms).
    mean_encode_ms: Option<f64>,
    mean_derive_ms: Option<f64>,
    mean_decode_ms: Option<f64>,
}

/// One (row, n, p, parser, side) measurement -- the raw material for a
/// Fig.-7-style scaling plot, where the summary CSV only has the
/// category-wide aggregate.
struct EntryResult {
    row_id: String,
    category: String,
    parser: &'static str,
    n: u32,
    p: u32,
    side: &'static str,
    outcome: &'static str,
    micros: Option<u64>,
    encode_micros: Option<u64>,
    derive_micros: Option<u64>,
    decode_micros: Option<u64>,
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1_000.0
}

fn fmt_opt_ms(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{x:.3}"),
        None => "n/a".to_string(),
    }
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

    let mut rows = Vec::new();
    let mut entry_rows = Vec::new();
    println!("\n{category} ({category_name}, {} entries)", corpus.len());
    println!(
        "{:<16} {:>9} {:>9} {:>7} {:>12} {:>12} {:>12} {:>12} {:>12} {:>12} {:>12} {:>12}",
        "parser", "measured", "timed_out", "crashed", "wall (s)", "mean (ms)", "median (ms)", "p99 (ms)", "max (ms)",
        "encode (ms)", "derive (ms)", "decode (ms)"
    );
    println!("{:-<16} {:->9} {:->9} {:->7} {:->12} {:->12} {:->12} {:->12} {:->12} {:->12} {:->12} {:->12}", "", "", "", "", "", "", "", "", "", "", "", "");

    for (parser_idx, parser_type) in ALL.iter().enumerate() {
        let mut all: Vec<Timing> = Vec::with_capacity(corpus.len());
        let mut timed_out = 0usize;
        let mut crashed = 0usize;
        let start = Instant::now();
        for entry in corpus {
            let (outcome, micros, encode_micros, derive_micros, decode_micros) =
                match run_entry_isolated(worker, cap, parser_idx, &entry.pattern, &entry.input) {
                    Outcome::Done(t) => {
                        all.push(t);
                        (
                            "measured",
                            Some(t.total.as_micros() as u64),
                            Some(t.encode.as_micros() as u64),
                            Some(t.derive.as_micros() as u64),
                            Some(t.decode.as_micros() as u64),
                        )
                    }
                    Outcome::TimedOut => {
                        timed_out += 1;
                        ("timed_out", None, None, None, None)
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
                        ("crashed", None, None, None, None)
                    }
                };
            entry_rows.push(EntryResult {
                row_id: entry.row_id.clone(),
                category: entry.category.clone(),
                parser: parser_type.name(),
                n: entry.n,
                p: entry.p,
                side: entry.side,
                outcome,
                micros,
                encode_micros,
                derive_micros,
                decode_micros,
            });
        }
        let total = start.elapsed();

        let (mean, median, p99, max, mean_encode, mean_derive, mean_decode) = if all.is_empty() {
            (None, None, None, None, None, None, None)
        } else {
            let mut sorted: Vec<Duration> = all.iter().map(|t| t.total).collect();
            sorted.sort();
            let n = sorted.len();
            let mean_of = |pick: fn(&Timing) -> Duration| ms(all.iter().map(pick).sum::<Duration>() / n as u32);
            (
                Some(ms(sorted.iter().sum::<Duration>() / n as u32)),
                Some(ms(sorted[n / 2])),
                Some(ms(sorted[(n * 99) / 100])),
                Some(ms(*sorted.last().expect("non-empty"))),
                Some(mean_of(|t| t.encode)),
                Some(mean_of(|t| t.derive)),
                Some(mean_of(|t| t.decode)),
            )
        };

        println!(
            "{:<16} {:>9} {:>9} {:>7} {:>12.3} {:>12} {:>12} {:>12} {:>12} {:>12} {:>12} {:>12}",
            parser_type.name(),
            all.len(),
            timed_out,
            crashed,
            total.as_secs_f64(),
            fmt_opt_ms(mean),
            fmt_opt_ms(median),
            fmt_opt_ms(p99),
            fmt_opt_ms(max),
            fmt_opt_ms(mean_encode),
            fmt_opt_ms(mean_derive),
            fmt_opt_ms(mean_decode),
        );

        rows.push(SummaryRow {
            category: category.to_string(),
            category_name: category_name.to_string(),
            parser: parser_type.name(),
            entries: corpus.len(),
            measured: all.len(),
            timed_out,
            crashed,
            wall_s: total.as_secs_f64(),
            mean_ms: mean,
            median_ms: median,
            p99_ms: p99,
            max_ms: max,
            mean_encode_ms: mean_encode,
            mean_derive_ms: mean_derive,
            mean_decode_ms: mean_decode,
        });
    }
    (rows, entry_rows)
}

fn write_entries_csv(path: &str, rows: &[EntryResult]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "row_id,category,parser,n,p,side,outcome,micros,encode_micros,derive_micros,decode_micros")?;
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
            r.outcome,
            r.micros.map(|v| v.to_string()).unwrap_or_default(),
            r.encode_micros.map(|v| v.to_string()).unwrap_or_default(),
            r.derive_micros.map(|v| v.to_string()).unwrap_or_default(),
            r.decode_micros.map(|v| v.to_string()).unwrap_or_default(),
        )?;
    }
    Ok(())
}

fn write_summary_csv(path: &str, rows: &[SummaryRow]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "category,category_name,parser,entries,measured,timed_out,crashed,wall_s,mean_ms,median_ms,p99_ms,max_ms,mean_encode_ms,mean_derive_ms,mean_decode_ms")?;
    for r in rows {
        writeln!(
            f,
            "{},{},{},{},{},{},{},{:.6},{},{},{},{},{},{},{}",
            r.category,
            r.category_name,
            r.parser,
            r.entries,
            r.measured,
            r.timed_out,
            r.crashed,
            r.wall_s,
            r.mean_ms.map(|v| format!("{v:.6}")).unwrap_or_default(),
            r.median_ms.map(|v| format!("{v:.6}")).unwrap_or_default(),
            r.p99_ms.map(|v| format!("{v:.6}")).unwrap_or_default(),
            r.max_ms.map(|v| format!("{v:.6}")).unwrap_or_default(),
            r.mean_encode_ms.map(|v| format!("{v:.6}")).unwrap_or_default(),
            r.mean_derive_ms.map(|v| format!("{v:.6}")).unwrap_or_default(),
            r.mean_decode_ms.map(|v| format!("{v:.6}")).unwrap_or_default(),
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
        "bench_pathological_time: {} rows -> {} entries (limit {}, entry_timeout={:?})",
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
    let summary_path = format!("{REPORTS_DIR}/bench_pathological_time{label}.csv");
    let entries_path = format!("{REPORTS_DIR}/bench_pathological_time{label}_entries.csv");

    // Written again after every category, not just once at the end --
    // same crash-safety reasoning as bench_corpus_time.rs's main().
    let mut all_summary = Vec::new();
    let mut all_entries = Vec::new();
    for (category, corpus) in group_by_category(entries) {
        let category_name = doc.categories.get(&category).map(|c| c.name.as_str()).unwrap_or("?");
        let (summary, entry_rows) = report(&category, category_name, &corpus, &worker, cap);
        all_summary.extend(summary);
        all_entries.extend(entry_rows);
        if let Err(e) = write_summary_csv(&summary_path, &all_summary) {
            eprintln!("could not write {summary_path}: {e}");
        }
        if let Err(e) = write_entries_csv(&entries_path, &all_entries) {
            eprintln!("could not write {entries_path}: {e}");
        }
    }
    eprintln!("wrote {summary_path}");
    eprintln!("wrote {entries_path}");
}
