//! examples/bench_translate_depth_worker.rs
//!
//! Worker for `benches/bench_translate_depth.rs`: runs ONE (mechanism, n)
//! job to completion -- or to a crash -- then exits.
//!
//! Grounds `MAX_BOUND` (`src/frontend/translate.rs`) and `MAX_CORE_DEPTH`
//! (`src/data/prepare.rs`) in a real stack-overflow crash point, measured
//! on the same kind of stack `core_regex_tagged` actually runs on:
//! `examples/dataset_prepare.rs` never spawns a custom-stack thread, so it
//! runs translation on the OS's plain default main-thread stack, same as
//! this worker (deliberately, like `bench_stack_depth_worker.rs`: a
//! custom large stack here would hide the exact risk being measured).
//!
//! Two mechanisms, selected by argv[1]:
//!
//!   - `bound_unroll`: reproduces `desugar_bound`'s private `optional_extra`
//!     (translate.rs:106-112) exactly, since `MAX_BOUND` makes the real,
//!     public `translate()` refuse any bound large enough to find a real
//!     crash point -- the real function cannot be probed past its own
//!     cap, so the identical recursive shape is reproduced here instead.
//!     One recursive call per unit of the open part of the bound
//!     (`n - m` for `r{m,n}`), each frame holding one `Regex::alt`/`Seq`
//!     wrapper and a cloned copy of the body -- the same shape that would
//!     run inside `translate()` itself if `MAX_BOUND` were not there.
//!   - `syntactic_nesting`: calls the real, public, uncapped pipeline
//!     (`parse_ext_pattern` then `translate`) on a pattern with `n`
//!     nested non-capturing groups, `(?:(?:...(?:a)...))`. Exercises the
//!     parser's recursive descent and `translate`'s `Group`/`GroupNonMarking`
//!     arm exactly as a real deeply-nested pattern would.
//!
//! argv: <prog> <bound_unroll|syntactic_nesting>
//! stdin: one line of JSON {"n": <u32>}
//!
//! Exit codes:
//!   0 = ran to completion (the job's own success/failure does not
//!       matter here, only whether the call returned at all)
//!   2 = stdin/job JSON unreadable, or bad argv
//!   a signal (no exit code; `status.code()` is `None`) = crashed,
//!       almost always a stack overflow for these two mechanisms.

use std::io::Read;

use regex_engine::frontend::{parse_ext_pattern, translate};
use regex_engine::types::Regex;

#[derive(serde::Deserialize)]
struct Job {
    n: u32,
}

/// Faithful reproduction of `translate.rs`'s private `optional_extra`
/// (see module doc). Zero to `k` further optional copies of `r`.
fn optional_extra_probe(r: &Regex, k: u32) -> Regex {
    if k == 0 {
        Regex::Eps
    } else {
        Regex::alt(Regex::Eps, Regex::seq(r.clone(), optional_extra_probe(r, k - 1)))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: bench_translate_depth_worker <bound_unroll|syntactic_nesting>  (job on stdin)");
        std::process::exit(2);
    }
    let mechanism = args[1].as_str();
    if mechanism != "bound_unroll" && mechanism != "syntactic_nesting" {
        eprintln!("mechanism must be bound_unroll or syntactic_nesting, got {mechanism:?}");
        std::process::exit(2);
    }

    let mut stdin_bytes = Vec::new();
    if let Err(e) = std::io::stdin().read_to_end(&mut stdin_bytes) {
        eprintln!("worker: reading stdin: {e}");
        std::process::exit(2);
    }
    let job: Job = match serde_json::from_slice(&stdin_bytes) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("worker: parsing job JSON: {e}");
            std::process::exit(2);
        }
    };

    if mechanism == "bound_unroll" {
        let body = Regex::lit('a');
        let r = optional_extra_probe(&body, job.n);
        // Force the structure to actually be built and held (not optimized
        // away), the same way `desugar_bound` builds and returns it.
        std::hint::black_box(&r);
    } else {
        let pattern = format!("{}a{}", "(?:".repeat(job.n as usize), ")".repeat(job.n as usize));
        let ep = match parse_ext_pattern(&pattern) {
            Ok(ep) => ep,
            Err(e) => {
                eprintln!("worker: parse_ext_pattern failed: {e}");
                std::process::exit(2);
            }
        };
        let r = match translate(&ep) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("worker: translate failed: {e}");
                std::process::exit(2);
            }
        };
        std::hint::black_box(&r);
    }
    // Reaching here means the call returned (and the result was dropped)
    // without crashing. Nothing on stdout is needed -- the parent only
    // cares about the exit status.
}
