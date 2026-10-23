//! examples/bench_stack_depth_worker.rs
//!
//! Worker for `benches/bench_stack_depth.rs`: runs ONE (parser, pattern,
//! input) job to completion -- or to a crash -- then exits.
//!
//! Deliberately does NOT run the call on a custom large-stack thread the
//! way `bench_worker.rs` does. That file's 256 MiB thread exists so a
//! *correct* deep computation still finishes; here the opposite is
//! wanted: the native stack this process's main thread gets from the OS
//! is exactly what `deriv_std_rec` (native per-character recursion) is
//! being compared against `deriv_std_loop` (an explicit `Vec`, no such
//! recursion) on. Giving both a huge custom stack would hide the one
//! difference this bench exists to measure.
//!
//! argv: <prog> <parser_idx 0|1>   (0 = deriv_std_rec, 1 = deriv_std_loop)
//! stdin: one line of JSON {"pattern": ..., "input": ...}
//!
//! Exit codes:
//!   0 = ran to completion (match or no match -- which one does not
//!       matter here, only whether the call returned at all)
//!   2 = stdin/job JSON unreadable, parser_idx out of range, or the
//!       pattern did not translate
//!   a signal (no exit code; `status.code()` is `None`) = crashed,
//!       almost always a stack overflow for these two parsers.

use std::io::Read;

use regex_engine::data::prepare::core_regex;
use regex_engine::parsers::{parse_deriv_std_loop, parse_deriv_std_rec};

#[derive(serde::Deserialize)]
struct Job {
    pattern: String,
    input: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: bench_stack_depth_worker <parser_idx 0|1>  (job on stdin)");
        std::process::exit(2);
    }
    let parser_idx: usize = args[1].parse().unwrap_or_else(|_| {
        eprintln!("parser_idx must be 0 or 1, got {:?}", args[1]);
        std::process::exit(2);
    });
    if parser_idx > 1 {
        eprintln!("parser_idx must be 0 (deriv_std_rec) or 1 (deriv_std_loop), got {parser_idx}");
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

    let regex = match core_regex(&job.pattern) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("worker: core_regex({:?}) failed: {e}", job.pattern);
            std::process::exit(2);
        }
    };

    let _ = if parser_idx == 0 {
        parse_deriv_std_rec(&job.input, &regex)
    } else {
        parse_deriv_std_loop(&job.input, &regex)
    };
    // Reaching here means the call returned without crashing. Nothing on
    // stdout is needed -- the parent only cares about the exit status.
}
