//! examples/demo_parser_agreement_worker.rs
//!
//! cargo build --release --example demo_parser_agreement_worker
//!
//! Worker for `demo_parser_agreement`.
//! Runs exactly one (parser, pattern, input) job and exits.
//! `demo_parser_agreement` spawns one per parser call.
//! It enforces the wall-clock timeout itself by killing the process.
//!
//! argv: <prog> <parser_idx 0..=4>
//! stdin: one line of JSON `{"pattern": "...", "input": "..."}`.
//!   Pattern/input travel over stdin, not argv.
//!   Some real Suricata `content:`-derived candidates contain raw bytes.
//!   That includes an embedded NUL, which `Command::arg` cannot carry.
//!   OS process arguments are C strings; a pipe carries without issue.
//!   JSON's own string escaping uses `\u0000`, not a literal NUL byte.
//!   That escaping is what makes writing and reading that byte safe.
//! stdout: on a clean exit, `Option<ParseTree>` as one line of JSON.
//! exit codes:
//!   0 = ran to completion (stdout has the result).
//!       Even if the parser returned `None`, i.e. no match.
//!   2 = the stdin job could not be read or parsed.
//!       Also used when the pattern did not translate.
//!   3 = the parser call panicked.
//!   a signal (no exit code, e.g. SIGSEGV/SIGABRT) means it crashed.
//!   `demo_parser_agreement` distinguishes all of these from a timeout.
//!   It detects a timeout itself by killing the process before it exits.

use std::io::{Read, Write};

use regex_engine::data::prepare::core_regex;
use regex_engine::parsers::{
    parse_deriv_bc, parse_deriv_std_loop, parse_deriv_std_rec, parse_pderiv_bc, parse_pderiv_std,
    ParseTree,
};
use regex_engine::types::Regex;

#[derive(serde::Deserialize)]
struct Job {
    pattern: String,
    input: String,
}

type ParserFn = fn(&str, &Regex) -> Option<ParseTree>;
const PARSERS: [ParserFn; 5] =
    [parse_deriv_std_rec, parse_deriv_std_loop, parse_deriv_bc, parse_pderiv_std, parse_pderiv_bc];

/// Large stack for the parser call.
/// `deriv_std_rec` recurses over the expression.
/// A deeply ambiguous (not yet pathological) pattern can exceed the
/// platform default stack well short of the cases this tool isolates.
/// Matches `with_deep_stack` in `tests/test_thesis_figures.rs`.
const WORKER_STACK_BYTES: usize = 256 * 1024 * 1024;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: demo_parser_agreement_worker <parser_idx 0..=4>  (job on stdin)");
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
    let pattern = job.pattern;
    let input = job.input;

    let regex = match core_regex(&pattern) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("worker: core_regex({pattern:?}) failed: {e}");
            std::process::exit(2);
        }
    };

    // Serialization recurses natively as much as building does.
    // Serde's derived `Serialize` walks the tree the same way.
    // So it runs on this same large-stack thread.
    // `.join()` would return to the default-stack caller.
    let handle = std::thread::Builder::new()
        .stack_size(WORKER_STACK_BYTES)
        .spawn(move || {
            let tree = parser(&input, &regex);
            serde_json::to_string(&tree).expect("serializing the result tree")
        })
        .expect("spawning the worker's parser-call thread");

    let line = match handle.join() {
        Ok(line) => line,
        Err(_) => {
            eprintln!("worker: parser call panicked");
            std::process::exit(3);
        }
    };

    println!("{line}");
    // `println!` writes through a buffered handle.
    // Flush explicitly so the parent sees the line even if a
    // non-standard exit would skip the usual shutdown flush.
    std::io::stdout().flush().expect("flushing worker stdout");
}