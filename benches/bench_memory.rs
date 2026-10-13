//! benches/bench_memory.rs
//!
//! Memory consumption of the five parsers against real corpus patterns.
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
//! Writes a CSV summary to `reports/benches/bench_memory.csv`,
//! replacing any existing file.
//!
//! Build the worker first, then run:
//!   cargo build --release --example bench_worker
//!   cargo bench --bench bench_memory

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use regex_engine::data::load_prepared_cases;
use regex_engine::data::prepare::core_regex;
use regex_engine::data::types::{MatchIntent, SourceKind};
use regex_engine::parsers::ParserType;

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

struct Entry {
    pattern: String,
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
        bucket.push(Entry { pattern: case.pattern, input: case.input });
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
// Isolated call
// -------------------------------

/// Index order fixed by `bench_worker.rs`'s own `PARSERS` array; this
/// list's position is what `run_entry_isolated` sends as `parser_idx`.
const PARSERS: &[ParserType] = &[
    ParserType::DerivStdRec,
    ParserType::DerivStdLoop,
    ParserType::DerivBc,
    ParserType::PDerivBc,
    ParserType::PDerivStd,
];

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
                    let _ = child.kill();
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
        "{:<16} {:>9} {:>7} {:>14} {:>14} {:>14}",
        "parser", "timed_out", "crashed", "total bytes", "avg/entry", "max peak"
    );
    println!("{:-<16} {:->9} {:->7} {:->14} {:->14} {:->14}", "", "", "", "", "", "");

    let mut rows = Vec::new();
    for (parser_idx, parser_type) in PARSERS.iter().enumerate() {
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
            "{:<16} {:>9} {:>7} {:>14} {:>14} {:>14}",
            parser_type.name(),
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
    writeln!(f, "source,parser,entries,timed_out,crashed,total_bytes,avg_bytes,max_peak_bytes")?;
    for r in rows {
        writeln!(
            f,
            "{},{},{},{},{},{},{},{}",
            r.source,
            r.parser,
            r.entries,
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
    let (suricata, spamassassin, regexlib, s2_elab, innovatorved) = load_corpus(&sel);
    eprintln!(
        "bench_memory: {} suricata, {} spamassassin, {} regexlib, {} s2-elab, {} innovatorved \
         (limit {} each, sources={:?}, match_intent={:?}, entry_timeout={:?})",
        suricata.len(),
        spamassassin.len(),
        regexlib.len(),
        s2_elab.len(),
        innovatorved.len(),
        fmt_limit(sel.pattern_limit),
        sel.sources,
        sel.match_intent,
        cap,
    );

    if let Err(e) = std::fs::create_dir_all(REPORTS_DIR) {
        eprintln!("could not create {REPORTS_DIR}: {e}");
        return;
    }
    let path = format!("{REPORTS_DIR}/bench_memory.csv");

    // Written again after every source, not just once at the end: see
    // the identical comment in bench_corpus_time.rs's main().
    let mut all_rows = Vec::new();
    for (label, corpus) in [
        ("dataset_suricata", &suricata),
        ("dataset_spamassassin", &spamassassin),
        ("dataset_regexlib", &regexlib),
        ("dataset_s2_elab", &s2_elab),
        ("dataset_innovatorved", &innovatorved),
    ] {
        all_rows.extend(report(label, corpus, &worker, cap));
        if let Err(e) = write_summary_csv(&path, &all_rows) {
            eprintln!("could not write {path}: {e}");
        }
    }
    eprintln!("wrote {path}");
}
