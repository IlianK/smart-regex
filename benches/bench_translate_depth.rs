//! benches/bench_translate_depth.rs
//!
//! Finds the real stack-overflow crash point for the two recursive
//! mechanisms behind `translate.rs`'s `MAX_BOUND` and `prepare.rs`'s
//! `MAX_CORE_DEPTH`, so those caps can be reported as a measured safety
//! margin rather than an unexplained round number (see
//! `examples/bench_translate_depth_worker.rs` for exactly what each
//! mechanism reproduces and why).
//!
//! `core_regex_tagged` (the only caller of `translate` outside tests)
//! runs on whatever stack its caller gives it; `examples/dataset_prepare.rs`
//! never spawns a custom-stack thread, so in practice that is the OS's
//! plain default main-thread stack -- `ulimit -s` on this machine, not
//! the 256 MiB custom worker stack the rest of this chapter's benches use
//! for everything else. Each probe below runs under an explicit
//! `ulimit -s 8192` (8 MiB, a stock Linux default) via a `sh -c` wrapper,
//! so the result holds regardless of the parent shell's own limit and
//! stays comparable across machines.
//!
//! Same process-isolation and doubling-sweep shape as
//! `bench_stack_depth.rs`: a crashed PROCESS is cleanly detected (killed
//! by a signal); a crashed THREAD is not. N starts at `DEPTH_N_START` and
//! doubles until the worker is killed by a signal (`outcome = crashed`),
//! a probe exceeds the per-probe time cap (`outcome = timed_out`), or N
//! exceeds `DEPTH_N_CAP` without either (`outcome = reached_sweep_cap`).
//!
//! Configuration (env vars):
//!   - `DEPTH_TIMEOUT_MS` (per-probe cap, default 10000)
//!   - `DEPTH_N_START` (default 100)
//!   - `DEPTH_N_CAP` (default 2,000,000)
//!   - `DEPTH_STACK_KIB` (default 8192, matching a stock Linux default)
//!
//! Writes `reports/benches/bench_translate_depth.csv`, replacing any
//! existing file.
//!
//! Build the worker first, then run:
//!   cargo build --release --example bench_translate_depth_worker
//!   cargo bench --bench bench_translate_depth

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const DEFAULT_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_N_START: u32 = 100;
const DEFAULT_N_CAP: u32 = 2_000_000;
const DEFAULT_STACK_KIB: u32 = 8192;
const REPORTS_DIR: &str = "reports/benches";
const WORKER_NAME: &str = "bench_translate_depth_worker";
const MECHANISMS: [&str; 2] = ["bound_unroll", "syntactic_nesting"];

fn timeout_cap() -> Duration {
    let ms: u64 = std::env::var("DEPTH_TIMEOUT_MS")
        .ok()
        .map(|s| s.parse().unwrap_or_else(|_| panic!("DEPTH_TIMEOUT_MS must be a positive integer, got {s:?}")))
        .unwrap_or(DEFAULT_TIMEOUT_MS);
    Duration::from_millis(ms)
}

fn n_start() -> u32 {
    std::env::var("DEPTH_N_START")
        .ok()
        .map(|s| s.parse().unwrap_or_else(|_| panic!("DEPTH_N_START must be a positive integer, got {s:?}")))
        .unwrap_or(DEFAULT_N_START)
}

fn n_cap() -> u32 {
    std::env::var("DEPTH_N_CAP")
        .ok()
        .map(|s| s.parse().unwrap_or_else(|_| panic!("DEPTH_N_CAP must be a positive integer, got {s:?}")))
        .unwrap_or(DEFAULT_N_CAP)
}

fn stack_kib() -> u32 {
    std::env::var("DEPTH_STACK_KIB")
        .ok()
        .map(|s| s.parse().unwrap_or_else(|_| panic!("DEPTH_STACK_KIB must be a positive integer, got {s:?}")))
        .unwrap_or(DEFAULT_STACK_KIB)
}

// -------------------------------
// Isolated probe -- same shape as bench_stack_depth.rs's `probe`,
// duplicated rather than shared per this codebase's convention (benches
// do not share a `common` module with each other; see that file's own
// module doc). The one difference: the worker runs under an explicit
// `ulimit -s`, via a `sh -c` wrapper, instead of inheriting whatever
// stack the bench's own parent shell happens to have.
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

fn probe(worker: &Path, cap: Duration, stack_kib: u32, mechanism: &str, n: u32) -> Outcome {
    let shell_cmd = format!("ulimit -s {stack_kib} && exec {:?} {mechanism}", worker);
    let mut child: Child = match Command::new("sh")
        .arg("-c")
        .arg(&shell_cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => panic!("failed to spawn {worker:?} via sh -c: {e}"),
    };

    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        let job = serde_json::json!({ "n": n }).to_string();
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
    mechanism: &'static str,
    max_n_survived: u32,
    outcome: &'static str,
    next_n: u32,
    detail: String,
    elapsed_s: f64,
}

fn sweep(worker: &Path, cap: Duration, stack_kib: u32, mechanism: &'static str, start: u32, cap_n: u32) -> SweepResult {
    let mut n = start.max(1);
    let mut last_good = 0u32;
    let start_time = Instant::now();

    loop {
        if n > cap_n {
            return SweepResult {
                mechanism,
                max_n_survived: last_good,
                outcome: "reached_sweep_cap",
                next_n: n,
                detail: format!("survived every N up to the sweep cap ({cap_n})"),
                elapsed_s: start_time.elapsed().as_secs_f64(),
            };
        }
        print!("  {mechanism:<18} n={n:<9} ");
        std::io::stdout().flush().unwrap();
        let t0 = Instant::now();
        match probe(worker, cap, stack_kib, mechanism, n) {
            Outcome::Survived => {
                println!("ok ({:.3}s)", t0.elapsed().as_secs_f64());
                last_good = n;
                n = n.saturating_mul(2);
            }
            Outcome::Crashed(reason) => {
                println!("CRASHED");
                return SweepResult {
                    mechanism,
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
                    mechanism,
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
    writeln!(f, "mechanism,max_n_survived,outcome,next_n,elapsed_s,detail")?;
    for r in rows {
        writeln!(
            f,
            "{},{},{},{},{:.3},{}",
            r.mechanism,
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
    let cap = timeout_cap();
    let start = n_start();
    let cap_n = n_cap();
    let stack_kib = stack_kib();
    let worker = worker_path();

    eprintln!(
        "bench_translate_depth: 2 mechanism(s) (bound_unroll, syntactic_nesting), \
         {stack_kib} KiB worker stack, per-probe cap={cap:?}, sweep N from {start} doubling up to {cap_n}",
    );

    let mut results = Vec::new();
    for &mechanism in &MECHANISMS {
        println!("\n{mechanism}");
        results.push(sweep(&worker, cap, stack_kib, mechanism, start, cap_n));
        let r = results.last().unwrap();
        println!("  summary: survived up to N={} ({})", r.max_n_survived, r.outcome);
    }

    if let Err(e) = std::fs::create_dir_all(REPORTS_DIR) {
        eprintln!("could not create {REPORTS_DIR}: {e}");
        return;
    }
    let out_path = format!("{REPORTS_DIR}/bench_translate_depth.csv");
    if let Err(e) = write_csv(&out_path, &results) {
        eprintln!("could not write {out_path}: {e}");
    } else {
        eprintln!("\nwrote {out_path}");
    }
}
