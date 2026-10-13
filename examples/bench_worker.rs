//! examples/bench_worker.rs
//!
//! cargo build --release --example bench_worker
//!
//! Shared worker for `bench_corpus_time` and `bench_memory`.
//! Runs one (parser, pattern, input) job and exits.
//! Reports elapsed time and allocation stats for that call.
//!
//! The parent enforces the per-entry timeout by killing this process.
//! A killed process reclaims its memory; a thread cannot be killed.
//!
//! argv: <prog> <parser_idx 0..=4>
//!   Index order: deriv_std_rec, deriv_std_loop, deriv_bc, pderiv_bc,
//!   pderiv_std -- matches both callers' `PARSERS` ordering.
//! stdin: one line of JSON `{"pattern": "...", "input": "..."}`.
//!   Pattern and input travel over stdin, not argv: some real
//!   candidates contain a raw NUL byte, which `Command::arg` cannot
//!   carry.
//! stdout: on clean exit, one line of JSON
//!   `{"micros": N, "total_bytes": N, "peak_bytes": N}`.
//!   `micros` covers only the parser call.
//!   It excludes job I/O and `core_regex` translation.
//! exit codes:
//!   0 = ran to completion (stdout has the result line).
//!   2 = stdin job unreadable, or the pattern did not translate.
//!   3 = the parser call panicked.
//!   a signal means it crashed, e.g. a stack overflow.
//!   The caller distinguishes all of these from a timeout, which it
//!   detects by killing the process before it exits.

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use regex_engine::data::prepare::core_regex;
use regex_engine::parsers::{
    parse_deriv_bc, parse_deriv_std_loop, parse_deriv_std_rec, parse_pderiv_bc, parse_pderiv_std,
    ParseTree,
};
use regex_engine::types::Regex;

// Safe here because this process measures exactly one job then exits.
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

#[derive(serde::Deserialize)]
struct Job {
    pattern: String,
    input: String,
}

#[derive(serde::Serialize)]
struct JobResult {
    micros: u64,
    total_bytes: usize,
    peak_bytes: usize,
}

type ParserFn = fn(&str, &Regex) -> Option<ParseTree>;
// Both callers' `PARSERS` must match this order position for position.
const PARSERS: [ParserFn; 5] =
    [parse_deriv_std_rec, parse_deriv_std_loop, parse_deriv_bc, parse_pderiv_bc, parse_pderiv_std];

// Deep recursion on deriv_std_rec needs more than the default stack.
const WORKER_STACK_BYTES: usize = 256 * 1024 * 1024;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: bench_worker <parser_idx 0..=4>  (job on stdin)");
        std::process::exit(2);
    }
    let parser_idx: usize = args[1].parse().unwrap_or_else(|_| {
        eprintln!("parser_idx must be 0..=4, got {:?}", args[1]);
        std::process::exit(2);
    });
    let Some(&parser) = PARSERS.get(parser_idx) else {
        eprintln!("parser_idx must be 0..=4, got {parser_idx}");
        std::process::exit(2);
    };

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

    // Baseline after parsing and translation, so neither counts as cost.
    let baseline = CURRENT.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    TOTAL.store(0, Ordering::SeqCst);

    let input = job.input;
    let handle = std::thread::Builder::new()
        .stack_size(WORKER_STACK_BYTES)
        .spawn(move || {
            let start = Instant::now();
            let _ = parser(&input, &regex);
            // u64 clamp in principle, no-op in practice.
            u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX)
        })
        .expect("spawning the worker's parser-call thread");

    let micros = match handle.join() {
        Ok(micros) => micros,
        Err(_) => {
            eprintln!("worker: parser call panicked");
            std::process::exit(3);
        }
    };

    let total_bytes = TOTAL.load(Ordering::SeqCst);
    let peak_bytes = PEAK.load(Ordering::SeqCst).saturating_sub(baseline);
    let result = JobResult { micros, total_bytes, peak_bytes };
    println!("{}", serde_json::to_string(&result).expect("serializing the result"));
    // Explicit flush: println! is buffered.
    std::io::stdout().flush().expect("flushing worker stdout");
}