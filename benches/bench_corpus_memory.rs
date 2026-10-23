//! benches/bench_corpus_memory.rs
//!
//! Heap allocation of the five parsers against real corpus patterns.
//! S2ELab only -- the only source with real prepared data today (see
//! `docs/DATASET.md`); no source selection.
//! "Heap", specifically: the counting allocator hooks `GlobalAlloc`,
//! which sees heap traffic only. Each worker's 256 MB call-thread
//! stack (see `bench_worker.rs`) is invisible to it, so a pattern
//! whose cost were dominated by recursion depth rather than
//! allocation would under-report here. The patterns that motivated
//! process isolation reached multiple GB; a fixed 256 MB stack cannot
//! account for that on its own (it would overflow and crash well
//! before contributing gigabytes), so the heap figures below are the
//! right number for those cases specifically -- but they are heap
//! numbers, not a general memory bound.
//!
//! Each entry runs in its own worker *process* (`bench_worker`, shared
//! with `bench_corpus_time`), bounded by `BENCH_ENTRY_TIMEOUT_MS`
//! (default 2000). See `bench_corpus_time.rs`'s module doc and
//! `examples/bench_worker.rs` for the full history of why: a thread
//! -based timeout only abandons a hung call rather than stopping it,
//! which first caused unbounded memory growth, and then -- even after
//! bounding how many abandoned threads could run concurrently -- still
//! reached a kernel OOM kill, because a single abandoned call's own
//! growth was enough on its own. A killed *process* actually reclaims
//! its memory; a thread cannot be killed in safe/stable Rust at all.
//!
//! This also removes a measurement problem the old thread-based
//! version had and could only document, not fix: its counting
//! allocator's byte counters were one global `static` per process, so
//! a still-running abandoned thread's continuing allocations could
//! inflate the *next* entry's reported total/peak, because
//! `GlobalAlloc` has no notion of which thread is calling it. Each
//! entry's counting allocator now lives inside that entry's own
//! worker process (see `bench_worker.rs`), which measures exactly one
//! job and exits -- there is no next entry for it to contaminate, and
//! a killed worker's counters simply cease to exist along with it.
//!
//! A `crashed` entry (the worker exited non-zero or died by signal,
//! e.g. a stack overflow) is counted separately from `timed_out`:
//! unlike the old thread-based version, that failure now only takes
//! down one worker process instead of this whole run.
//!
//! `max_peak` is a lower bound on the corpus's true worst case, not
//! the true worst case itself: a `timed_out` entry's worker is killed
//! before it ever writes a result line, so it contributes nothing to
//! `max_peak` -- not a value capped at the timeout, a full exclusion.
//! The entries most likely to time out are exactly the ones process
//! isolation exists to contain, so they are also the ones most likely
//! to have had the largest true peak. `measured` is how many entries
//! `avg_bytes`/`max_peak` are actually computed over.
//!
//! Writes a CSV summary to `reports/benches/bench_corpus_memory.csv`,
//! replacing any existing file.
//!
//! Build the worker first, then run:
//!   cargo build --release --example bench_worker
//!   cargo bench --bench bench_corpus_memory

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use regex_engine::data::load_prepared_cases;
use regex_engine::data::prepare::core_regex;
use regex_engine::data::types::{MatchIntent, SourceKind};
use regex_engine::parsers::ALL;

const DATA_ROOT: &str = "data/processed";
/// Default when `BENCH_PATTERN_LIMIT` is unset: no cap. See the same
/// constant in `bench_corpus_time.rs` for why `usize::MAX` means that.
const PATTERN_LIMIT_PER_SOURCE: usize = usize::MAX;
/// Default when `BENCH_ENTRY_TIMEOUT_MS` is unset. See `bench_corpus_time.rs`.
const DEFAULT_ENTRY_TIMEOUT_MS: u64 = 2000;
const REPORTS_DIR: &str = "reports/benches";
const WORKER_NAME: &str = "bench_worker";

/// `pattern_limit` for display: `usize::MAX` reads as "unlimited".
fn fmt_limit(n: usize) -> String {
    if n == usize::MAX { "unlimited".to_string() } else { n.to_string() }
}

struct BenchSelection {
    pattern_limit: usize,
    match_intent: Option<MatchIntent>,
}

fn bench_selection() -> BenchSelection {
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
    BenchSelection { pattern_limit, match_intent }
}

/// Per-entry cap. See the module doc.
fn entry_timeout() -> Duration {
    let ms: u64 = std::env::var("BENCH_ENTRY_TIMEOUT_MS")
        .ok()
        .map(|s| {
            s.parse().unwrap_or_else(|_| panic!("BENCH_ENTRY_TIMEOUT_MS must be a positive integer, got {s:?}"))
        })
        .unwrap_or(DEFAULT_ENTRY_TIMEOUT_MS);
    Duration::from_millis(ms)
}

/// Optional filename suffix (e.g. `_M3`). See the identical function
/// in `bench_corpus_time.rs` for why.
fn run_label_suffix() -> String {
    std::env::var("BENCH_RUN_LABEL").ok().map(|l| format!("_{l}")).unwrap_or_default()
}

struct Entry {
    pattern: String,
    input: String,
}

/// Capped at `pattern_limit` distinct patterns, optionally filtered to
/// one `MatchIntent` tier first.
fn load_corpus(sel: &BenchSelection) -> Vec<Entry> {
    let cases = load_prepared_cases(std::path::Path::new(DATA_ROOT), &[SourceKind::S2ELab])
        .unwrap_or_else(|e| panic!("cannot read {DATA_ROOT}: {e} -- run dataset_prepare first"));

    let mut entries = Vec::new();
    let mut patterns = std::collections::HashSet::new();

    for case in cases {
        if let Some(wanted) = sel.match_intent {
            if case.match_intent != wanted {
                continue;
            }
        }
        let already_admitted = patterns.contains(&case.pattern);
        if !already_admitted && patterns.len() >= sel.pattern_limit {
            continue;
        }
        // `core_regex`, not `parse_pcre_rule`.
        // An `ExplicitFullMatch` pattern only translates through
        // `core_regex`'s `^...$` stripping. Checked here, upfront, so a
        // translation failure is caught once per distinct pattern
        // rather than once per worker call; the `Regex` itself is not
        // kept -- `bench_worker` rebuilds it from `case.pattern` in its
        // own process.
        if let Err(e) = core_regex(&case.pattern) {
            eprintln!("skipping pre-verified case: {} -- {e}", case.pattern);
            continue;
        }
        patterns.insert(case.pattern.clone());
        entries.push(Entry { pattern: case.pattern, input: case.input });
    }
    if entries.is_empty() {
        panic!(
            "no entries loaded from {DATA_ROOT} (limit={}, match_intent={:?}) -- run dataset_prepare first",
            fmt_limit(sel.pattern_limit), sel.match_intent
        );
    }
    entries
}

// -------------------------------
// Isolated call
// -------------------------------

/// Path of the shared worker binary. See the identical function in
/// `bench_corpus_time.rs` for why this exact relative path is correct.
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
    #[allow(dead_code)] // bench_corpus_time reads this field from the same wire format
    micros: u64,
    total_bytes: usize,
    peak_bytes: usize,
}

enum Outcome {
    Done { total_bytes: usize, peak_bytes: usize },
    TimedOut,
    Crashed(String),
}

/// Runs one (parser, pattern, input) job in its own process, bounded
/// by `cap`, and reads back its allocation totals. On timeout, kills
/// the child, reclaiming its memory -- see the module doc.
///
/// Mirrors `bench_corpus_time.rs`'s `run_entry_isolated` almost
/// exactly, differing only in which `JobResult` fields it keeps;
/// duplicated rather than shared, consistent with this codebase's
/// existing convention of not sharing code between bench binaries.
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
                    let _ = child.wait(); // reap; do not leave a zombie
                    return Outcome::TimedOut;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => panic!("error polling worker process: {e}"),
        }
    }
}

// -------------------------------
// Reporting
// -------------------------------

struct SummaryRow {
    source: &'static str,
    parser: &'static str,
    entries: usize,
    measured: usize,
    timed_out: usize,
    crashed: usize,
    total_bytes: usize,
    avg_bytes: Option<usize>,
    max_peak: usize,
}

fn report(label: &'static str, corpus: &[Entry], worker: &Path, cap: Duration) -> Vec<SummaryRow> {
    if corpus.is_empty() {
        eprintln!("skip {label}: no entries");
        return Vec::new();
    }
    println!("\n{label} ({} entries)", corpus.len());
    println!(
        "{:<16} {:>9} {:>9} {:>7} {:>14} {:>14} {:>14}",
        "parser", "measured", "timed_out", "crashed", "total bytes", "avg/entry", "max peak"
    );
    println!("{:-<16} {:->9} {:->9} {:->7} {:->14} {:->14} {:->14}", "", "", "", "", "", "", "");

    let mut rows = Vec::new();
    for (parser_idx, parser_type) in ALL.iter().enumerate() {
        let mut total_sum = 0usize;
        let mut peak_max = 0usize;
        let mut measured = 0usize;
        let mut timed_out = 0usize;
        let mut crashed = 0usize;
        for entry in corpus {
            match run_entry_isolated(worker, cap, parser_idx, &entry.pattern, &entry.input) {
                Outcome::Done { total_bytes, peak_bytes } => {
                    total_sum += total_bytes;
                    peak_max = peak_max.max(peak_bytes);
                    measured += 1;
                }
                Outcome::TimedOut => timed_out += 1,
                Outcome::Crashed(reason) => {
                    crashed += 1;
                    eprintln!(
                        "  crash: {label}/{} pat={:?} in={:?} -- {reason}",
                        parser_type.name(),
                        entry.pattern,
                        entry.input,
                    );
                }
            }
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
            source: label,
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
    rows
}

// -------------------------------
// CSV output
// -------------------------------

fn write_summary_csv(path: &str, rows: &[SummaryRow]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "source,parser,entries,measured,timed_out,crashed,total_bytes,avg_bytes,max_peak_bytes")?;
    for r in rows {
        writeln!(
            f,
            "{},{},{},{},{},{},{},{},{}",
            r.source,
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
    let sel = bench_selection();
    let cap = entry_timeout();
    let worker = worker_path();
    let corpus = load_corpus(&sel);
    eprintln!(
        "bench_corpus_memory: {} s2-elab entries (limit {}, match_intent={:?}, entry_timeout={:?})",
        corpus.len(),
        fmt_limit(sel.pattern_limit),
        sel.match_intent,
        cap,
    );

    if let Err(e) = std::fs::create_dir_all(REPORTS_DIR) {
        eprintln!("could not create {REPORTS_DIR}: {e}");
        return;
    }
    let path = format!("{REPORTS_DIR}/bench_corpus_memory{}.csv", run_label_suffix());

    let rows = report("dataset_s2_elab", &corpus, &worker, cap);
    if let Err(e) = write_summary_csv(&path, &rows) {
        eprintln!("could not write {path}: {e}");
    }
    eprintln!("wrote {path}");
}
