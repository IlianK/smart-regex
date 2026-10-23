//! benches/bench_corpus_time.rs
//!
//! Wall-clock cost per parser over the real corpus.
//! S2ELab only -- the only source with real prepared data today (see
//! `docs/DATASET.md`); no source selection. One pass, each entry timed
//! individually. No sampling, no per-pattern group.
//!
//! A plain reporting binary rather than a Criterion target.
//! Criterion's model assumes a cheap closure measured many times; a
//! corpus pass takes seconds to hours per parser, so its statistics
//! would report sample-count variance, not corpus-scale performance.
//!
//! Every entry runs in its own worker *process* (`bench_worker`,
//! shared with `bench_corpus_memory`), bounded by `BENCH_ENTRY_TIMEOUT_MS`
//! (default 2000). A call that exceeds the cap is counted as
//! `timed_out`, not folded into the duration stats as if it were a
//! real measurement: on real corpora (Innovatorved and RegexLib
//! especially), a small fraction of entries make `deriv_std_rec` /
//! `deriv_std_loop` run for seconds to minutes on a single call, and
//! averaging that into "max" would just report however long this run
//! happened to wait, not a property of the parser.
//!
//! This used to run each entry on its own *thread* instead, bounded
//! the same way. That design had a fatal gap: giving up on
//! `rx.recv_timeout` does not stop the thread -- Rust cannot cancel a
//! thread -- so a capped-out call kept running and kept allocating in
//! the background. Bounding how many such threads could be
//! concurrently outstanding (a `StragglerPool`) was tried next, and
//! disproved empirically: on a real Innovatorved entry, a *single*
//! abandoned call's own growth reached a kernel OOM kill (~14 GB)
//! regardless of whether 1 or 2 such calls were allowed to run
//! concurrently. The bound on concurrency was never the limiting
//! factor -- one pathological entry was enough on its own, and no
//! pool size fixes that. A process can be killed outright
//! (`Child::kill()`), which actually reclaims its memory; a thread
//! cannot be. See `examples/bench_worker.rs` for the worker side of
//! this and the exact protocol between it and this file.
//!
//! A `crashed` entry (the worker exited non-zero or died by signal,
//! e.g. a stack overflow) is counted separately from `timed_out`:
//! unlike the old thread-based version, that failure now only takes
//! down one worker process instead of this whole run.
//!
//! `wall (s)` and the distribution columns measure different things,
//! and do not satisfy `wall ~= entries x mean`: `wall` is this file's
//! own wall clock around the whole per-source loop, so it includes
//! one process spawn and one pipe round trip per entry; the
//! distribution columns come from each worker's self-reported
//! `micros`, which covers only the parser call itself (see
//! `bench_worker.rs`'s module doc for exactly what that excludes).
//! On sources whose patterns are individually cheap, spawn overhead
//! can dominate `wall` while barely showing up in `mean` at all.
//!
//! `median`/`p99`/`max` are computed over entries that completed --
//! `timed_out` and `crashed` entries contribute nothing to them, not
//! a value capped at the timeout. The entries most likely to be
//! excluded are the ones process isolation exists to contain, so
//! `max` is a lower bound on the corpus's true worst case, not the
//! true worst case itself. The `measured` column is how many entries
//! those statistics are actually computed over.
//!
//! Writes a CSV summary to `reports/benches/bench_corpus_time.csv`
//! (and `..._by_outcome.csv` when `BENCH_SPLIT_OUTCOME=1`), replacing
//! any existing file -- the same "latest run, not a log" convention
//! `dataset_prepare` uses for `prepared.jsonl`.
//!
//! Build the worker first, then run:
//!   cargo build --release --example bench_worker
//!   cargo bench --bench bench_corpus_time

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
/// Default when `BENCH_PATTERN_LIMIT` is unset: no cap.
const PATTERN_LIMIT_PER_SOURCE: usize = usize::MAX;
/// Default when `BENCH_ENTRY_TIMEOUT_MS` is unset. Generous: real
/// entries normally finish in microseconds to a few hundred
/// milliseconds; this exists only to bound the rare pathological one,
/// not to pre-empt slow-but-ordinary calls.
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

/// Per-entry cap. See the module doc for what this does and does not
/// protect against.
fn entry_timeout() -> Duration {
    let ms: u64 = std::env::var("BENCH_ENTRY_TIMEOUT_MS")
        .ok()
        .map(|s| {
            s.parse().unwrap_or_else(|_| panic!("BENCH_ENTRY_TIMEOUT_MS must be a positive integer, got {s:?}"))
        })
        .unwrap_or(DEFAULT_ENTRY_TIMEOUT_MS);
    Duration::from_millis(ms)
}

/// Optional filename suffix (e.g. `_T3`) so a sweep of differently
/// -configured runs lands in separate files instead of each one
/// overwriting the last. Unset keeps today's bare filename, so a
/// one-off run is unaffected.
fn run_label_suffix() -> String {
    std::env::var("BENCH_RUN_LABEL").ok().map(|l| format!("_{l}")).unwrap_or_default()
}

struct Entry {
    pattern: String,
    input: String,
    expected_match: bool,
}

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
        // `case.pattern` was accepted by `core_regex_tagged` to reach
        // `prepared.jsonl` at all; an `ExplicitFullMatch` pattern only
        // translates through `core_regex`'s `^...$` stripping. Checked
        // here, upfront, so a translation failure is caught once per
        // distinct pattern rather than once per worker call; the
        // `Regex` itself is not kept -- `bench_worker` rebuilds it from
        // `case.pattern` in its own process, the same way
        // `demo_parser_agreement_worker` already does.
        if let Err(e) = core_regex(&case.pattern) {
            eprintln!("skipping pre-verified case: {} -- {e}", case.pattern);
            continue;
        }
        patterns.insert(case.pattern.clone());
        entries.push(Entry {
            pattern: case.pattern,
            input: case.input,
            expected_match: case.expected_match,
        });
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

/// Path of the shared worker binary. Bench binaries land in
/// `target/<profile>/deps/<name>-<hash>`; examples land in
/// `target/<profile>/examples/<name>`, a sibling of `deps/`. So the
/// worker is two levels up from this process's own exe, then into
/// `examples/`.
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
    #[allow(dead_code)] // bench_corpus_memory reads this field from the same wire format
    total_bytes: usize,
    #[allow(dead_code)]
    peak_bytes: usize,
}

enum Outcome {
    Done(Duration),
    TimedOut,
    Crashed(String),
}

/// Runs one (parser, pattern, input) job in its own process
/// On timeout, kills the child
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
                    Ok(r) => Outcome::Done(Duration::from_micros(r.micros)),
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
    wall_s: f64,
    mean_ms: Option<f64>,
    median_ms: Option<f64>,
    p99_ms: Option<f64>,
    max_ms: Option<f64>,
}

struct OutcomeRow {
    source: &'static str,
    parser: &'static str,
    matches: usize,
    match_s: f64,
    non_matches: usize,
    non_match_s: f64,
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
    label: &'static str,
    corpus: &[Entry],
    worker: &Path,
    cap: Duration,
    split_outcome: bool,
) -> (Vec<SummaryRow>, Vec<OutcomeRow>) {
    if corpus.is_empty() {
        eprintln!("skip {label}: no entries");
        return (Vec::new(), Vec::new());
    }

    let mut summary_rows = Vec::new();
    let mut outcome_rows = Vec::new();

    println!("\n{label} ({} entries)", corpus.len());
    println!(
        "{:<16} {:>9} {:>9} {:>7} {:>12} {:>12} {:>12} {:>12} {:>12}",
        "parser", "measured", "timed_out", "crashed", "wall (s)", "mean (ms)", "median (ms)", "p99 (ms)", "max (ms)"
    );
    println!(
        "{:-<16} {:->9} {:->9} {:->7} {:->12} {:->12} {:->12} {:->12} {:->12}",
        "", "", "", "", "", "", "", "", ""
    );

    for (parser_idx, parser_type) in ALL.iter().enumerate() {
        let mut all: Vec<Duration> = Vec::with_capacity(corpus.len());
        let mut matches: Vec<Duration> = Vec::new();
        let mut non_matches: Vec<Duration> = Vec::new();
        let mut timed_out = 0usize;
        let mut crashed = 0usize;
        let start = Instant::now();
        for entry in corpus {
            match run_entry_isolated(worker, cap, parser_idx, &entry.pattern, &entry.input) {
                Outcome::Done(dt) => {
                    all.push(dt);
                    if entry.expected_match {
                        matches.push(dt);
                    } else {
                        non_matches.push(dt);
                    }
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
        let total = start.elapsed();

        let (mean, median, p99, max) = if all.is_empty() {
            (None, None, None, None)
        } else {
            let mut sorted = all.clone();
            sorted.sort();
            let n = sorted.len();
            (
                Some(ms(sorted.iter().sum::<Duration>() / n as u32)),
                Some(ms(sorted[n / 2])),
                Some(ms(sorted[(n * 99) / 100])),
                Some(ms(*sorted.last().expect("non-empty"))),
            )
        };

        println!(
            "{:<16} {:>9} {:>9} {:>7} {:>12.3} {:>12} {:>12} {:>12} {:>12}",
            parser_type.name(),
            all.len(),
            timed_out,
            crashed,
            total.as_secs_f64(),
            fmt_opt_ms(mean),
            fmt_opt_ms(median),
            fmt_opt_ms(p99),
            fmt_opt_ms(max),
        );

        summary_rows.push(SummaryRow {
            source: label,
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
        });

        if split_outcome {
            let mt = matches.iter().sum::<Duration>();
            let nt = non_matches.iter().sum::<Duration>();
            outcome_rows.push(OutcomeRow {
                source: label,
                parser: parser_type.name(),
                matches: matches.len(),
                match_s: mt.as_secs_f64(),
                non_matches: non_matches.len(),
                non_match_s: nt.as_secs_f64(),
            });
        }
    }

    if split_outcome && !outcome_rows.is_empty() {
        println!();
        println!("{label}, by outcome");
        println!(
            "{:<16} {:>12} {:>12} {:>12} {:>12}",
            "parser", "matches", "match (s)", "non-match", "non-match (s)"
        );
        println!(
            "{:-<16} {:->12} {:->12} {:->12} {:->12}",
            "", "", "", "", ""
        );
        for row in &outcome_rows {
            println!(
                "{:<16} {:>12} {:>12.3} {:>12} {:>12.3}",
                row.parser, row.matches, row.match_s, row.non_matches, row.non_match_s,
            );
        }
    }

    (summary_rows, outcome_rows)
}

// -------------------------------
// CSV output
// -------------------------------

fn write_summary_csv(path: &str, rows: &[SummaryRow]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "source,parser,entries,measured,timed_out,crashed,wall_s,mean_ms,median_ms,p99_ms,max_ms")?;
    for r in rows {
        writeln!(
            f,
            "{},{},{},{},{},{},{:.6},{},{},{},{}",
            r.source,
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
        )?;
    }
    Ok(())
}

fn write_outcome_csv(path: &str, rows: &[OutcomeRow]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "source,parser,matches,match_s,non_matches,non_match_s")?;
    for r in rows {
        writeln!(
            f,
            "{},{},{},{:.6},{},{:.6}",
            r.source, r.parser, r.matches, r.match_s, r.non_matches, r.non_match_s,
        )?;
    }
    Ok(())
}

fn main() {
    let sel = bench_selection();
    let cap = entry_timeout();
    let worker = worker_path();
    let split_outcome = std::env::var("BENCH_SPLIT_OUTCOME")
        .ok()
        .map(|s| match s.trim().to_lowercase().as_str() {
            "1" | "true" | "yes" => true,
            "0" | "false" | "no" => false,
            other => panic!("BENCH_SPLIT_OUTCOME must be 1 or 0, got {other:?}"),
        })
        .unwrap_or(false);
    let corpus = load_corpus(&sel);
    eprintln!(
        "bench_corpus_time: {} s2-elab entries (limit {}, match_intent={:?}, split_outcome={}, entry_timeout={:?})",
        corpus.len(),
        fmt_limit(sel.pattern_limit),
        sel.match_intent,
        split_outcome,
        cap,
    );

    if let Err(e) = std::fs::create_dir_all(REPORTS_DIR) {
        eprintln!("could not create {REPORTS_DIR}: {e}");
        return;
    }
    let label = run_label_suffix();
    let summary_path = format!("{REPORTS_DIR}/bench_corpus_time{label}.csv");
    let outcome_path = format!("{REPORTS_DIR}/bench_corpus_time{label}_by_outcome.csv");

    let (summary, outcome) = report("dataset_s2_elab", &corpus, &worker, cap, split_outcome);
    if let Err(e) = write_summary_csv(&summary_path, &summary) {
        eprintln!("could not write {summary_path}: {e}");
    }
    if split_outcome {
        if let Err(e) = write_outcome_csv(&outcome_path, &outcome) {
            eprintln!("could not write {outcome_path}: {e}");
        }
    }
    eprintln!("wrote {summary_path}");
    if split_outcome {
        eprintln!("wrote {outcome_path}");
    }
}
