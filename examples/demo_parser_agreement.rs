//! examples/demo_parser_agreement.rs
//!
//! cargo run --release --example demo_parser_agreement
//!
//! Cross-parser agreement over the prepared dataset.
//! Reads `data/processed/`, written by `dataset_prepare`.
//! For each (pattern, input) entry, all five parsers run.
//! Three checks:
//!   - POSIX parsers agree among themselves;
//!   - Greedy parsers agree among themselves;
//!   - all five agree on membership.
//!
//! Each parser call runs in its own subprocess, not a thread.
//! An earlier version used threads with a channel-based timeout.
//! `rx.recv_timeout` gives up waiting, but Rust cannot cancel the thread.
//! A hung call keeps running and keeps allocating in the background.
//! Enough of those piling up is what OOM-killed full differential sweeps.
//! A process can be killed outright (`Child::kill()`), reclaiming memory.
//! Cost: up to five short-lived processes per entry, not five threads.
//! Slower in wall-clock terms when parsers finish quickly.
//! Bounded, correct memory over raw throughput is the point.
//! `BENCH_PATTERN_LIMIT` caps a sweep's size.
//!
//! Build the worker first:
//!   cargo build --release --example demo_parser_agreement_worker
//!
//! Also saves its output to reports/demos/parser_agreement.txt,
//! replacing any previous run. This is the one demo in that family
//! whose exit code matters (1 on a real bug) and whose "last known
//! clean" state is worth a persisted record rather than only ever
//! existing in scrollback.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use std::fmt::Write as _;

use regex_engine::data::load_prepared_cases;
use regex_engine::data::prepare::core_regex;
use regex_engine::data::types::{PreparedCase, SourceKind};
use regex_engine::types::ParseTree;

const DATA_ROOT: &str = "data/processed";
const DEFAULT_TIMEOUT_MS: u64 = 2000;
/// Cap on printed instances per bug/outcome kind; beyond this, tally only.
const MAX_PRINTED_PER_BUG_KIND: usize = 20;
const WORKER_NAME: &str = "demo_parser_agreement_worker";
const PARSER_NAMES: [&str; 5] = ["rec", "loop", "bc", "pstd", "pbc"];

const SOURCES: [SourceKind; 5] = [
    SourceKind::Suricata,
    SourceKind::SpamAssassin,
    SourceKind::RegexLib,
    SourceKind::S2ELab,
    SourceKind::Innovatorved,
];

fn source_idx(s: SourceKind) -> usize {
    SOURCES.iter().position(|&x| x == s).expect("exhaustive")
}

struct Selection {
    sources: Vec<SourceKind>,
    pattern_limit: Option<usize>,
    timeout: Duration,
}

fn selection() -> Selection {
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
    let pattern_limit = std::env::var("BENCH_PATTERN_LIMIT").ok().map(|s| {
        s.parse().unwrap_or_else(|_| panic!("BENCH_PATTERN_LIMIT must be a positive integer, got {s:?}"))
    });
    let timeout_ms = std::env::var("AGREEMENT_TIMEOUT_MS")
        .ok()
        .map(|s| {
            s.parse().unwrap_or_else(|_| panic!("AGREEMENT_TIMEOUT_MS must be a positive integer, got {s:?}"))
        })
        .unwrap_or(DEFAULT_TIMEOUT_MS);
    Selection { sources, pattern_limit, timeout: Duration::from_millis(timeout_ms) }
}

/// Deserializes a worker's one-line `Option<ParseTree>` output.
///
/// Plain `serde_json::from_str` enforces a fixed logical recursion limit.
/// That default is well short of what a large corpus match nests to.
/// Recursing natively to deserialize a deep tree risks a stack overflow.
/// That is the same risk as building a deep tree in the worker.
/// `serde_stacker` grows the stack as needed instead.
fn deserialize_tree(json: &str) -> serde_json::Result<Option<ParseTree>> {
    let mut de = serde_json::Deserializer::from_str(json);
    de.disable_recursion_limit();
    let de = serde_stacker::Deserializer::new(&mut de);
    serde::Deserialize::deserialize(de)
}

/// Path of the worker binary next to this executable.
fn worker_path() -> PathBuf {
    let exe = std::env::current_exe().expect("reading this process's own exe path");
    let dir = exe.parent().expect("exe has a parent directory");
    let path = dir.join(WORKER_NAME);
    if !path.exists() {
        panic!(
            "worker not found at {path:?} -- build it first:\n  \
             cargo build --release --example {WORKER_NAME}"
        );
    }
    path
}

/// Prints `body` and also saves it to `reports/demos/parser_agreement.txt`,
/// replacing any previous run -- the same "latest run, not a log"
/// convention the corpus benches use for their CSVs.
fn save_report(body: &str) {
    print!("{body}");
    let dir = "reports/demos";
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("could not create {dir}: {e}");
        return;
    }
    let path = format!("{dir}/parser_agreement.txt");
    if let Err(e) = std::fs::write(&path, body) {
        eprintln!("could not write {path}: {e}");
    } else {
        eprintln!("wrote {path}");
    }
}

/// One parser's outcome for one entry.
enum Outcome {
    Done(Option<ParseTree>),
    /// Worker did not exit within the timeout; killed.
    TimedOut,
    /// Worker exited (or was killed by a signal) without a usable result.
    Crashed(String),
}

/// Spawns the worker for one (parser, pattern, input) job and waits.
/// On timeout, kills the child.
/// Killing a process reclaims its whole address space.
/// Giving up on waiting for a thread does not.
fn run_parser_isolated(
    worker: &PathBuf,
    timeout: Duration,
    parser_idx: usize,
    pattern: &str,
    input: &str,
) -> Outcome {
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

    // Pattern/input travel over stdin as one line of JSON, not argv.
    // `Command::arg` cannot carry a string with an embedded NUL byte at all.
    // OS process arguments are C strings; some Suricata candidates contain a NUL.
    // Written and stdin dropped before the reader thread starts.
    // So this write cannot block on a pipe nothing is draining yet.
    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        let job = serde_json::json!({ "pattern": pattern, "input": input }).to_string();
        if let Err(e) = stdin.write_all(job.as_bytes()) {
            panic!("failed to write job to {worker:?}'s stdin: {e}");
        }
    }

    // Drain stdout/stderr on a background thread while polling for exit.
    // This thread's lifetime is bounded by the child's.
    // A pipe read returns as soon as the child closes it.
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

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process has exited, so the pipes are closed.
                // The reader thread is finishing; this recv does not block on new work.
                let (out, err) = rx.recv().unwrap_or_default();
                if !status.success() {
                    let reason = match status.code() {
                        Some(code) => format!("exit code {code}"),
                        None => "killed by signal (likely stack overflow)".to_string(),
                    };
                    return Outcome::Crashed(format!("{reason}; stderr={}", err.trim()));
                }
                return match deserialize_tree(out.trim()) {
                    Ok(tree) => Outcome::Done(tree),
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

/// Loads `sel`'s cases; `pattern_limit` caps distinct patterns per source.
/// Unset loads everything.
fn select_cases(sel: &Selection) -> Vec<PreparedCase> {
    let cases = load_prepared_cases(std::path::Path::new(DATA_ROOT), &sel.sources)
        .unwrap_or_else(|e| {
            panic!(
                "couldn't read prepared cases under {} ({e}) -- run `cargo run --release --example \
                 dataset_prepare -- <suricata|spamassassin|regexlib|s2-elab|innovatorved> <file>...` first",
                DATA_ROOT
            )
        });
    let Some(limit) = sel.pattern_limit else {
        return cases;
    };
    let mut admitted: [std::collections::HashSet<String>; 5] = Default::default();
    cases
        .into_iter()
        .filter(|case| {
            let patterns = &mut admitted[source_idx(case.source)];
            let already_admitted = patterns.contains(&case.pattern);
            if !already_admitted && patterns.len() >= limit {
                return false;
            }
            patterns.insert(case.pattern.clone());
            true
        })
        .collect()
}

#[derive(Default)]
struct Tally {
    entries: usize,
    /// At least one parser timed out -- not a bug.
    timeouts: usize,
    /// At least one worker crashed -- not a membership/shape bug.
    /// But not dismissed the way a timeout is either.
    crashes: usize,
    membership_bugs: usize,
    posix_shape_bugs: usize,
    greedy_shape_bugs: usize,
    /// Entries where both families were internally consistent and matched.
    /// So the POSIX-vs-Greedy shape comparison was meaningful.
    cross_compared: usize,
    cross_same_shape: usize,
    cross_different_shape: usize,
}

/// Names of whichever parsers timed out, in call order; `None` if none did.
fn timed_out_names(outcomes: &[Outcome; 5]) -> Option<Vec<&'static str>> {
    let names: Vec<&'static str> = outcomes
        .iter()
        .zip(PARSER_NAMES)
        .filter(|(o, _)| matches!(o, Outcome::TimedOut))
        .map(|(_, name)| name)
        .collect();
    if names.is_empty() { None } else { Some(names) }
}

/// Names of whichever parsers crashed, with reason; empty if none did.
fn crashed_names(outcomes: &[Outcome; 5]) -> Vec<(&'static str, &str)> {
    outcomes
        .iter()
        .zip(PARSER_NAMES)
        .filter_map(|(o, name)| match o {
            Outcome::Crashed(reason) => Some((name, reason.as_str())),
            _ => None,
        })
        .collect()
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "..."
    }
}

fn main() {
    let sel = selection();
    let worker = worker_path();
    let cases = select_cases(&sel);
    if cases.is_empty() {
        panic!(
            "no prepared cases loaded from {DATA_ROOT} for sources={:?} -- run dataset_prepare first",
            sel.sources
        );
    }

    let mut buf = String::new();
    let mut tallies: [Tally; 5] = Default::default();
    let mut printed_membership = 0usize;
    let mut printed_posix = 0usize;
    let mut printed_greedy = 0usize;
    let mut printed_timeout = 0usize;
    let mut printed_crash = 0usize;

    for case in &cases {
        // Translation itself is not the source of the memory-pileup problem.
        // Only running a parser is, so this call stays in-process.
        // `core_regex`, not `parse_pcre_rule`: `case.pattern` was accepted via `core_regex_tagged`.
        if let Err(e) = core_regex(&case.pattern) {
            eprintln!("skipping prepared case (should have been pre-verified): {} -- {}", case.pattern, e);
            continue;
        }

        let outcomes = std::array::from_fn(|i| {
            run_parser_isolated(&worker, sel.timeout, i, &case.pattern, &case.input)
        });

        let tally = &mut tallies[source_idx(case.source)];
        tally.entries += 1;

        if let Some(names) = timed_out_names(&outcomes) {
            tally.timeouts += 1;
            if printed_timeout < MAX_PRINTED_PER_BUG_KIND {
                writeln!(
                    buf,
                    "TIMEOUT >{}ms  {:?}  pat={:?}  in={:?}  [{}]",
                    sel.timeout.as_millis(), case.source, truncate(&case.pattern, 60),
                    truncate(&case.input, 40), names.join(",")
                ).unwrap();
                printed_timeout += 1;
            }
            continue;
        }

        let crashed = crashed_names(&outcomes);
        if !crashed.is_empty() {
            tally.crashes += 1;
            if printed_crash < MAX_PRINTED_PER_BUG_KIND {
                let detail: Vec<String> = crashed.iter().map(|(n, r)| format!("{n}: {r}")).collect();
                writeln!(
                    buf,
                    "CRASH  {:?}  pat={:?}  in={:?}  {}",
                    case.source, truncate(&case.pattern, 60), truncate(&case.input, 40),
                    detail.join(" | ")
                ).unwrap();
                printed_crash += 1;
            }
            continue;
        }

        let [rec, loop_, bc, pstd, pbc] = outcomes.map(|o| match o {
            Outcome::Done(r) => r,
            Outcome::TimedOut | Outcome::Crashed(_) => {
                unreachable!("checked by timed_out_names/crashed_names above")
            }
        });

        let all_matched = [rec.is_some(), loop_.is_some(), bc.is_some(), pstd.is_some(), pbc.is_some()];
        let membership_ok = all_matched.iter().all(|&m| m == case.verified_match);
        if !membership_ok {
            tally.membership_bugs += 1;
            if printed_membership < MAX_PRINTED_PER_BUG_KIND {
                writeln!(
                    buf,
                    "MEMBERSHIP BUG  {:?}  pat={:?}  in={:?}  expected={}  got=[rec={} loop={} bc={} pstd={} pbc={}]",
                    case.source, truncate(&case.pattern, 60), truncate(&case.input, 40),
                    case.verified_match, all_matched[0], all_matched[1], all_matched[2],
                    all_matched[3], all_matched[4]
                ).unwrap();
                printed_membership += 1;
            }
            continue;
        }

        if !case.verified_match {
            // Verified non-match: every parser returned None; no trees.
            continue;
        }

        let posix_shape_ok = rec == loop_ && loop_ == bc;
        if !posix_shape_ok {
            tally.posix_shape_bugs += 1;
            if printed_posix < MAX_PRINTED_PER_BUG_KIND {
                writeln!(
                    buf,
                    "POSIX SHAPE BUG  {:?}  pat={:?}  in={:?}  rec={:?} loop={:?} bc={:?}",
                    case.source, truncate(&case.pattern, 60), truncate(&case.input, 40),
                    rec, loop_, bc
                ).unwrap();
                printed_posix += 1;
            }
        }

        let greedy_shape_ok = pstd == pbc;
        if !greedy_shape_ok {
            tally.greedy_shape_bugs += 1;
            if printed_greedy < MAX_PRINTED_PER_BUG_KIND {
                writeln!(
                    buf,
                    "GREEDY SHAPE BUG  {:?}  pat={:?}  in={:?}  pstd={:?} pbc={:?}",
                    case.source, truncate(&case.pattern, 60), truncate(&case.input, 40),
                    pstd, pbc
                ).unwrap();
                printed_greedy += 1;
            }
        }

        if posix_shape_ok && greedy_shape_ok {
            tally.cross_compared += 1;
            let posix_tree: &Option<ParseTree> = &bc;
            let greedy_tree: &Option<ParseTree> = &pbc;
            if posix_tree == greedy_tree {
                tally.cross_same_shape += 1;
            } else {
                tally.cross_different_shape += 1;
            }
        }
    }

    writeln!(buf).unwrap();
    writeln!(
        buf,
        "{:<13} {:>8} {:>9} {:>8} {:>10} {:>11} {:>12} {:>9} {:>8} {:>7}",
        "Source", "Entries", "Timeouts", "Crashes", "Mem.bugs", "POSIX-bugs", "Greedy-bugs",
        "Compared", "Same", "Diff%"
    ).unwrap();
    writeln!(buf, "{}", "-".repeat(105)).unwrap();

    let mut total = Tally::default();
    for &source in &SOURCES {
        let t = &tallies[source_idx(source)];
        if t.entries == 0 {
            continue;
        }
        let diff_pct = if t.cross_compared > 0 {
            100.0 * t.cross_different_shape as f64 / t.cross_compared as f64
        } else {
            0.0
        };
        writeln!(
            buf,
            "{:<13} {:>8} {:>9} {:>8} {:>10} {:>11} {:>12} {:>9} {:>8} {:>6.1}",
            format!("{:?}", source), t.entries, t.timeouts, t.crashes,
            t.membership_bugs, t.posix_shape_bugs, t.greedy_shape_bugs, t.cross_compared,
            t.cross_same_shape, diff_pct
        ).unwrap();
        total.entries += t.entries;
        total.timeouts += t.timeouts;
        total.crashes += t.crashes;
        total.membership_bugs += t.membership_bugs;
        total.posix_shape_bugs += t.posix_shape_bugs;
        total.greedy_shape_bugs += t.greedy_shape_bugs;
        total.cross_compared += t.cross_compared;
        total.cross_same_shape += t.cross_same_shape;
        total.cross_different_shape += t.cross_different_shape;
    }
    writeln!(buf, "{}", "-".repeat(105)).unwrap();
    let total_diff_pct = if total.cross_compared > 0 {
        100.0 * total.cross_different_shape as f64 / total.cross_compared as f64
    } else {
        0.0
    };
    writeln!(
        buf,
        "{:<13} {:>8} {:>9} {:>8} {:>10} {:>11} {:>12} {:>9} {:>8} {:>6.1}",
        "Total", total.entries, total.timeouts, total.crashes, total.membership_bugs,
        total.posix_shape_bugs, total.greedy_shape_bugs, total.cross_compared,
        total.cross_same_shape, total_diff_pct
    ).unwrap();
    writeln!(buf).unwrap();
    writeln!(buf, "Compared = matching entries where both families were internally consistent.").unwrap();
    writeln!(buf, "Same / Diff = their POSIX-vs-Greedy shape agreement.").unwrap();
    writeln!(buf, "Diff is not a bug: the two policies may pick different trees for an ambiguous match.").unwrap();
    writeln!(buf, "Timeouts are not bugs: deriv_std_rec/deriv_std_loop run unsimplified, and a large or").unwrap();
    writeln!(buf, "  ambiguous pattern can be infeasible in the configured budget.").unwrap();
    writeln!(buf, "Crashes are counted, not dismissed (usually a stack overflow on a deep expression),").unwrap();
    writeln!(buf, "  but are not membership/shape disagreements -- nothing was produced to compare.").unwrap();

    let bugs = total.membership_bugs + total.posix_shape_bugs + total.greedy_shape_bugs;
    if bugs > 0 {
        writeln!(buf).unwrap();
        writeln!(
            buf,
            "{} BUG(S) across {} entries (printed details capped at {} per kind).",
            bugs, total.entries, MAX_PRINTED_PER_BUG_KIND
        ).unwrap();
    } else {
        writeln!(buf).unwrap();
        writeln!(
            buf,
            "No bugs across {} entries ({} timed out, {} crashed).",
            total.entries, total.timeouts, total.crashes
        ).unwrap();
    }

    save_report(&buf);

    if bugs > 0 {
        std::process::exit(1);
    }
}
