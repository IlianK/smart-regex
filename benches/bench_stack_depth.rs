//! benches/bench_stack_depth.rs
//!
//! Replaces `examples/demo_crash.rs` (removed). Compares the one pair in
//! this engine where the same algorithm has two implementations whose
//! *stack* behaviour differs: `deriv_std_rec` (native recursion, one
//! stack frame per input character -- see `parse_recursive_helper` in
//! `src/parsers/deriv_std/parse.rs`) vs. `deriv_std_loop` (an explicit
//! `Vec`, no such recursion). Every other parser is out of scope here.
//!
//! Inputs come from the hand-curated stress-pattern dataset
//! (`data/pathological/pathological_patterns.json`, `src/data/pathological.rs`),
//! not a synthetic `a*` sample, scoped to two rows, one from each of the
//! categories this dataset itself names for the two mechanisms a long
//! input can exhaust: category A (nested quantifiers) and category H
//! (many adjacent stars). Both P01 and P30 below were checked by hand
//! before being picked, not guessed:
//!
//!   - P01 `(a+)+` (category A): the simplest, most-cited member of the
//!     nested-quantifier family. Measured directly (`parse_deriv_std_loop`,
//!     release build): "a"*20 takes ~1.1s, "a"*25 exceeds a 2s budget --
//!     even with the OS stack cut to 512 KiB, "a"*20 still *completes*,
//!     so the wall here is derivative-SIZE blowup (this pattern's
//!     unsimplified residual roughly doubles in node count per
//!     character), not stack DEPTH: the tree stays shallow even as it
//!     gets enormous. That makes P01 a weak stack probe on its own, but
//!     it is category A's own canonical example and is included for
//!     that reason -- and because confirming *which* wall a pathological
//!     pattern hits first is itself a real finding, not a wasted row.
//!   - P30 `(\w+)` x10 (category H): ten adjacent unbounded groups: the
//!     dataset's own `notes` field says "N must be >= 10". Measured the
//!     same way: "a"*25 ~0.3s, "a"*50 ~3.1s, "a"*100 exceeds a 5s
//!     budget -- milder than P01 (polynomial vs. P01's near-exponential
//!     growth, matching this category's documented `stress_axis`), but
//!     still a derivative-SIZE wall reached at double-digit N, long
//!     before `deriv_std_rec`'s native recursion (which survived well
//!     past 10,000 characters of plain `a*` in the old demo) would ever
//!     be the bottleneck.
//!
//! Consequence, confirmed by the two findings above: for BOTH rows, the
//! wall either parser hits first is almost certainly the shared
//! derivative-size blowup inside `deriv()` (called by `deriv_std_rec`
//! AND `deriv_std_loop` alike), not the one thing that is actually
//! different between them (`deriv_std_rec`'s extra per-character stack
//! frame). So this bench does not assume a crash will even happen --
//! each sweep below ends on EITHER a real crash (a killed/signalled
//! worker: almost certainly a genuine stack overflow) OR a timeout (the
//! per-probe time cap, almost certainly derivative-size blowup, not a
//! stack answer at all), and the two are reported as distinct
//! `outcome`s rather than conflated into one "failed" bucket. "Both
//! parsers hit the same timeout wall at the same N" is itself the
//! correct, honest result for a row where that is what actually
//! happens -- printed, not smoothed over (the project's own standing
//! rule; see `demo_growth_eval.rs`'s module doc).
//!
//! Each probe is its own `bench_stack_depth_worker` subprocess, the same
//! process-isolation reasoning `bench_worker.rs`/`bench_corpus_time.rs`
//! already use: a crashed PROCESS is cleanly detected (killed by a
//! signal) and its memory reclaimed; a crashed THREAD is not. Unlike
//! `bench_worker.rs`, the worker here deliberately does NOT run on a
//! custom large-stack thread -- see that file's own module doc for why.
//!
//! Sweep: for each (row, parser), N starts at `STACKDEPTH_N_START` and
//! doubles ("sweep N upward" -- not a binary search like the old demo,
//! since a binary search needs an upper bound in hand first, which is
//! exactly what is not known up front here) until the worker is killed
//! by a signal (`outcome = crashed`), a probe exceeds the per-probe time
//! cap (`outcome = timed_out`), or N exceeds `STACKDEPTH_N_CAP` without
//! either (`outcome = reached_sweep_cap`). The last N that completed
//! cleanly is `max_n_survived`.
//!
//! Configuration (env vars, same `SCREAMING_CASE` convention as the
//! other benches in this file's family):
//!   - `STACKDEPTH_JSON_PATH` (default `data/pathological/pathological_patterns.json`)
//!   - `STACKDEPTH_ROW_IDS` (comma-separated row ids, default `P01,P30`)
//!   - `STACKDEPTH_TIMEOUT_MS` (per-probe cap, default 2000)
//!   - `STACKDEPTH_N_START` (default 1)
//!   - `STACKDEPTH_N_CAP` (default 2,000,000)
//!
//! Writes `reports/benches/bench_stack_depth.csv`, replacing any
//! existing file -- the same "latest run, not a log" convention as
//! `bench_pathological_time.rs`.
//!
//! Build the worker first, then run:
//!   cargo build --release --example bench_stack_depth_worker
//!   cargo bench --bench bench_stack_depth

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use regex_engine::data::pathological::{self, PathologicalRow};

const DEFAULT_JSON_PATH: &str = "data/pathological/pathological_patterns.json";
const DEFAULT_ROW_IDS: &str = "P01,P30";
const DEFAULT_TIMEOUT_MS: u64 = 2000;
const DEFAULT_N_START: u32 = 1;
const DEFAULT_N_CAP: u32 = 2_000_000;
const REPORTS_DIR: &str = "reports/benches";
const WORKER_NAME: &str = "bench_stack_depth_worker";
/// `ParserType::name()` for parser_idx 0 and 1 -- the only two in scope.
const PARSER_NAMES: [&str; 2] = ["deriv_std_rec", "deriv_std_loop"];

fn json_path() -> PathBuf {
    std::env::var("STACKDEPTH_JSON_PATH").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(DEFAULT_JSON_PATH))
}

fn row_ids() -> Vec<String> {
    std::env::var("STACKDEPTH_ROW_IDS")
        .unwrap_or_else(|_| DEFAULT_ROW_IDS.to_string())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn timeout_cap() -> Duration {
    let ms: u64 = std::env::var("STACKDEPTH_TIMEOUT_MS")
        .ok()
        .map(|s| s.parse().unwrap_or_else(|_| panic!("STACKDEPTH_TIMEOUT_MS must be a positive integer, got {s:?}")))
        .unwrap_or(DEFAULT_TIMEOUT_MS);
    Duration::from_millis(ms)
}

fn n_start() -> u32 {
    std::env::var("STACKDEPTH_N_START")
        .ok()
        .map(|s| s.parse().unwrap_or_else(|_| panic!("STACKDEPTH_N_START must be a positive integer, got {s:?}")))
        .unwrap_or(DEFAULT_N_START)
}

fn n_cap() -> u32 {
    std::env::var("STACKDEPTH_N_CAP")
        .ok()
        .map(|s| s.parse().unwrap_or_else(|_| panic!("STACKDEPTH_N_CAP must be a positive integer, got {s:?}")))
        .unwrap_or(DEFAULT_N_CAP)
}

// -------------------------------
// Isolated probe -- same shape as bench_pathological_time.rs's
// run_entry_isolated, duplicated rather than shared per this codebase's
// convention (see that file's own module doc).
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

enum Outcome {
    Survived,
    Crashed(String),
    TimedOut,
}

fn probe(worker: &Path, cap: Duration, parser_idx: usize, pattern: &str, input: &str) -> Outcome {
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
        let _ = tx.send(err);
    });

    let deadline = Instant::now() + cap;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let err = rx.recv().unwrap_or_default();
                if status.success() {
                    return Outcome::Survived;
                }
                let reason = match status.code() {
                    Some(code) => format!("exit code {code} (not a crash -- see stderr); stderr={}", err.trim()),
                    None => format!("killed by signal (stack overflow); stderr={}", err.trim()),
                };
                return Outcome::Crashed(reason);
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
// Sweep
// -------------------------------

struct SweepResult {
    row_id: String,
    category: String,
    parser: &'static str,
    max_n_survived: u32,
    outcome: &'static str,
    next_n: u32,
    detail: String,
    elapsed_s: f64,
}

fn sweep(
    worker: &Path,
    cap: Duration,
    parser_idx: usize,
    row: &PathologicalRow,
    start: u32,
    cap_n: u32,
) -> SweepResult {
    let p = pathological::DEFAULT_P;
    let mut n = start.max(1);
    let mut last_good = 0u32;
    let start_time = Instant::now();

    loop {
        if n > cap_n {
            return SweepResult {
                row_id: row.id.clone(),
                category: row.category.clone(),
                parser: PARSER_NAMES[parser_idx],
                max_n_survived: last_good,
                outcome: "reached_sweep_cap",
                next_n: n,
                detail: format!("survived every N up to the sweep cap ({cap_n})"),
                elapsed_s: start_time.elapsed().as_secs_f64(),
            };
        }
        let (pattern, m, _) = pathological::expand(row, n, p);
        let input = m.unwrap_or_else(|| panic!("row {} has no match_input to sweep", row.id));
        print!("  {} / {:<14} n={:<9} ", row.id, PARSER_NAMES[parser_idx], n);
        std::io::stdout().flush().unwrap();
        let t0 = Instant::now();
        match probe(worker, cap, parser_idx, &pattern, &input) {
            Outcome::Survived => {
                println!("ok ({:.3}s)", t0.elapsed().as_secs_f64());
                last_good = n;
                n = n.saturating_mul(2);
            }
            Outcome::Crashed(reason) => {
                println!("CRASHED");
                return SweepResult {
                    row_id: row.id.clone(),
                    category: row.category.clone(),
                    parser: PARSER_NAMES[parser_idx],
                    max_n_survived: last_good,
                    outcome: "crashed",
                    next_n: n,
                    detail: reason,
                    elapsed_s: start_time.elapsed().as_secs_f64(),
                };
            }
            Outcome::TimedOut => {
                println!("TIMED OUT (>{:.1}s)", cap.as_secs_f64());
                return SweepResult {
                    row_id: row.id.clone(),
                    category: row.category.clone(),
                    parser: PARSER_NAMES[parser_idx],
                    max_n_survived: last_good,
                    outcome: "timed_out",
                    next_n: n,
                    detail: format!("exceeded the {:.1}s per-probe cap, not a crash", cap.as_secs_f64()),
                    elapsed_s: start_time.elapsed().as_secs_f64(),
                };
            }
        }
    }
}

fn write_csv(path: &str, rows: &[SweepResult]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "row_id,category,parser,max_n_survived,outcome,next_n,elapsed_s,detail")?;
    for r in rows {
        writeln!(
            f,
            "{},{},{},{},{},{},{:.3},{}",
            r.row_id,
            r.category,
            r.parser,
            r.max_n_survived,
            r.outcome,
            r.next_n,
            r.elapsed_s,
            common::csv_quote(&r.detail),
        )?;
    }
    Ok(())
}

// A tiny local stand-in for `examples/common::csv_quote`: benches do not
// share the `examples/common/` module (different compilation unit), and
// this is the only CSV field here that can contain a comma (the crash
// reason's `stderr=...`), so a one-off helper is clearer than a
// cross-directory `#[path]` include for a single function.
mod common {
    pub fn csv_quote(s: &str) -> String {
        format!("\"{}\"", s.replace('"', "\"\""))
    }
}

fn main() {
    let path = json_path();
    let ids = row_ids();
    let cap = timeout_cap();
    let start = n_start();
    let cap_n = n_cap();
    let worker = worker_path();

    let doc = pathological::load(&path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));
    let all_rows: Vec<&PathologicalRow> = doc
        .pathological_patterns
        .iter()
        .chain(doc.baseline_and_edge_patterns.iter())
        .chain(doc.generated_patterns.iter())
        .collect();

    let rows: Vec<&PathologicalRow> = ids
        .iter()
        .map(|id| {
            *all_rows
                .iter()
                .find(|r| &r.id == id)
                .unwrap_or_else(|| panic!("row id {id:?} not found in {path:?}"))
        })
        .collect();

    eprintln!(
        "bench_stack_depth: {} row(s) x 2 parsers (deriv_std_rec, deriv_std_loop), \
         per-probe cap={cap:?}, sweep N from {start} doubling up to {cap_n}",
        rows.len(),
    );

    let mut results = Vec::new();
    for row in &rows {
        println!("\n{} ({}) -- {}", row.id, row.category, row.regex.as_deref().unwrap_or("<templated>"));
        for parser_idx in 0..=1 {
            results.push(sweep(&worker, cap, parser_idx, row, start, cap_n));
        }

        let rec = &results[results.len() - 2];
        let loop_ = &results[results.len() - 1];
        println!(
            "  summary: deriv_std_rec survived up to N={} ({}); deriv_std_loop survived up to N={} ({})",
            rec.max_n_survived, rec.outcome, loop_.max_n_survived, loop_.outcome
        );
        if rec.max_n_survived > 0 && loop_.max_n_survived > 0 {
            if loop_.max_n_survived > rec.max_n_survived {
                println!(
                    "  deriv_std_loop survived {:.2}x further than deriv_std_rec on {}",
                    loop_.max_n_survived as f64 / rec.max_n_survived as f64,
                    row.id
                );
            } else if rec.max_n_survived > loop_.max_n_survived {
                println!(
                    "  deriv_std_rec survived {:.2}x further than deriv_std_loop on {} (unexpected)",
                    rec.max_n_survived as f64 / loop_.max_n_survived as f64,
                    row.id
                );
            } else {
                println!(
                    "  both survived to the same N on {} -- the wall they hit is shared \
                     (derivative-size blowup), not the stack-frame difference this bench targets",
                    row.id
                );
            }
        }
    }

    if let Err(e) = std::fs::create_dir_all(REPORTS_DIR) {
        eprintln!("could not create {REPORTS_DIR}: {e}");
        return;
    }
    let out_path = format!("{REPORTS_DIR}/bench_stack_depth.csv");
    if let Err(e) = write_csv(&out_path, &results) {
        eprintln!("could not write {out_path}: {e}");
    } else {
        eprintln!("\nwrote {out_path}");
    }
}
