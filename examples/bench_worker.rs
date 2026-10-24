//! examples/bench_worker.rs
//!
//! cargo build --release --example bench_worker
//!
//! Shared worker for `bench_corpus_time`, `bench_corpus_memory`,
//! `bench_pathological_time`, and `bench_pathological_memory`.
//! Runs one (parser, pattern, input) job and exits.
//! Reports elapsed time and allocation stats for that call.
//!
//! The parent enforces the per-entry timeout by killing this process.
//! A killed process reclaims its memory; a thread cannot be killed.
//! Killing is immediate and blocking, so entries no longer overlap:
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
use regex_engine::parsers::{ALL, BcTiming, parse_bitcoded_recursive_timed, parse_pderiv_bc_timed};

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
    // Breakdown of `micros` for the bit-coded parsers (deriv_bc,
    // pderiv_bc) only; for the other three, encode_micros and
    // decode_micros are 0 and derive_micros equals the full `micros`.
    encode_micros: u64,
    derive_micros: u64,
    decode_micros: u64,
}

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
    if ALL.get(parser_idx).is_none() {
        eprintln!("parser_idx must be 0..=4, got {parser_idx}");
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

    // Baseline after parsing and translation, so neither counts as cost.
    let baseline = CURRENT.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    TOTAL.store(0, Ordering::SeqCst);

    let input = job.input;
    let handle = std::thread::Builder::new()
        .stack_size(WORKER_STACK_BYTES)
        .spawn(move || {
            let start = Instant::now();
            let timing = match parser_idx {
                2 => parse_bitcoded_recursive_timed(&input, &regex).1,
                3 => parse_pderiv_bc_timed(&input, &regex).1,
                _ => {
                    let parser = ALL[parser_idx].parser();
                    let _ = parser(&input, &regex);
                    BcTiming { encode_us: 0, derive_us: 0, decode_us: 0 }
                }
            };
            // u64 clamp in principle, no-op in practice.
            let total = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX);
            match parser_idx {
                2 | 3 => (total, timing),
                _ => (total, BcTiming { encode_us: 0, derive_us: total, decode_us: 0 }),
            }
        })
        .expect("spawning the worker's parser-call thread");

    let (micros, timing) = match handle.join() {
        Ok(result) => result,
        Err(_) => {
            eprintln!("worker: parser call panicked");
            std::process::exit(3);
        }
    };

    let total_bytes = TOTAL.load(Ordering::SeqCst);
    let peak_bytes = PEAK.load(Ordering::SeqCst).saturating_sub(baseline);
    let result = JobResult {
        micros,
        total_bytes,
        peak_bytes,
        encode_micros: timing.encode_us,
        derive_micros: timing.derive_us,
        decode_micros: timing.decode_us,
    };
    println!("{}", serde_json::to_string(&result).expect("serializing the result"));
    // Explicit flush: println! is buffered.
    std::io::stdout().flush().expect("flushing worker stdout");
}