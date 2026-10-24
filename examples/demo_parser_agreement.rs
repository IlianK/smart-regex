//! examples/demo_parser_agreement.rs
//!
//! cargo run --release --example demo_parser_agreement
//!
//! Cross-parser agreement over the prepared dataset.
//! Reads `data/processed/`, written by `dataset_prepare`.
//! For each (pattern, input) entry, all five parsers run.
//! Three checks:
//!   - POSIX parsers agree among themselves;
//!   - non-POSIX parsers agree among themselves;
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
//! Saves two files, replacing any previous run:
//!   - reports/demos/parser_agreement.csv -- the summary table (one row
//!     per source plus a total): entries/matches/non-matches/timeouts/
//!     crashes/agreement percentages.
//!   - reports/demos/parser_agreement_samples.csv -- every individual
//!     flagged entry (timeout, crash, membership bug, POSIX/non-POSIX
//!     shape bug, ambiguous shape), capped at `MAX_PRINTED_PER_BUG_KIND`
//!     per kind same as stdout. Kept separate from the summary table per
//!     this project's "a listing of individual examples gets its own
//!     file" convention.
//! This is the one demo in that family whose exit code matters (1 on a
//! real bug) and whose "last known clean" state is worth a persisted
//! record rather than only ever existing in scrollback.

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[path = "common/thesis_figures.rs"]
mod common;
use common::{csv_quote, save};

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
        // Unlike most benches/examples (whose shared `load_prepared_cases`
        // default is S2ELab alone), this one defaults to
        // every source: agreement is a correctness check, not a
        // performance sweep, and a membership or shape bug hiding in
        // RegexLib/S2ELab/Innovatorved is exactly as real a bug.
        .unwrap_or_else(|| SOURCES.to_vec());
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
/// Per-worker virtual-memory ceiling (`setrlimit(RLIMIT_AS, ..)`, Unix
/// only -- see `bench_worker.rs`'s module doc for the fuller
/// rationale: unset means unbounded, same as before this existed).
fn mem_cap_bytes() -> Option<u64> {
    std::env::var("BENCH_MEM_CAP_MB").ok().map(|s| {
        let mb: u64 = s.parse().unwrap_or_else(|_| panic!("BENCH_MEM_CAP_MB must be a positive integer, got {s:?}"));
        mb * 1024 * 1024
    })
}

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
        let job = serde_json::json!({ "pattern": pattern, "input": input, "mem_cap_bytes": mem_cap_bytes() }).to_string();
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
    /// How many `entries` are verified matches vs. non-matches, per the
    /// dataset's own labeling -- independent of what any parser does.
    /// `matches + non_matches == entries` always.
    matches: usize,
    non_matches: usize,
    /// At least one parser timed out -- not a bug.
    timeouts: usize,
    /// At least one worker crashed -- not a membership/shape bug.
    /// But not dismissed the way a timeout is either.
    crashes: usize,
    /// Checked on EVERY completed entry, match or non-match: did all
    /// five parsers agree with `expected_match`? This is the "agreement
    /// on match result" dimension -- it covers non-matches too, since
    /// agreeing that something does NOT match is as real a check as
    /// agreeing that it does.
    membership_bugs: usize,
    /// A verified match with correct membership and a POSIX-internal
    /// and/or non-POSIX-internal shape disagreement. Internal-family
    /// consistency (3-way POSIX, 2-way non-POSIX) is a given -- each is
    /// its own correctness property, pinned by its own test -- so these
    /// are tracked only for the detailed bug lines and the overall bug
    /// count below, not as their own summary-table column. The
    /// comparison worth a column is the next one: POSIX vs. non-POSIX.
    posix_shape_bugs: usize,
    non_posix_shape_bugs: usize,
    /// Entries where both families were internally consistent and
    /// matched, so a single POSIX tree and a single non-POSIX tree
    /// both exist to compare. Denominator for shape agreement/disagreement.
    cross_compared: usize,
    cross_same_shape: usize,
    cross_different_shape: usize,
}

impl Tally {
    /// Entries where every parser actually ran to completion -- excludes
    /// timeouts and crashes. Denominator for match-result agreement.
    fn completed(&self) -> usize {
        self.entries - self.timeouts - self.crashes
    }

    /// % of `completed()` where all five parsers agreed with
    /// `expected_match`. Covers match AND non-match entries.
    fn match_agree_pct(&self) -> f64 {
        let n = self.completed();
        if n == 0 { 0.0 } else { 100.0 * (n - self.membership_bugs) as f64 / n as f64 }
    }

    /// % of `cross_compared` where POSIX and non-POSIX picked the SAME tree.
    fn shape_agree_pct(&self) -> f64 {
        if self.cross_compared == 0 {
            0.0
        } else {
            100.0 * self.cross_same_shape as f64 / self.cross_compared as f64
        }
    }

    /// % of `cross_compared` where POSIX and non-POSIX picked DIFFERENT
    /// trees. Not a bug -- the two policies may legitimately disagree
    /// on an ambiguous pattern. `shape_agree_pct + shape_disagree_pct`
    /// sums to 100% (of `cross_compared`).
    fn shape_disagree_pct(&self) -> f64 {
        if self.cross_compared == 0 {
            0.0
        } else {
            100.0 * self.cross_different_shape as f64 / self.cross_compared as f64
        }
    }

    fn accumulate(&mut self, t: &Tally) {
        self.entries += t.entries;
        self.matches += t.matches;
        self.non_matches += t.non_matches;
        self.timeouts += t.timeouts;
        self.crashes += t.crashes;
        self.membership_bugs += t.membership_bugs;
        self.posix_shape_bugs += t.posix_shape_bugs;
        self.non_posix_shape_bugs += t.non_posix_shape_bugs;
        self.cross_compared += t.cross_compared;
        self.cross_same_shape += t.cross_same_shape;
        self.cross_different_shape += t.cross_different_shape;
    }
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

/// One flagged entry, going into the samples file (kept separate from
/// the summary table -- see this file's own module doc).
struct Sample {
    kind: &'static str,
    source: String,
    pattern: String,
    input: String,
    detail: String,
}

fn push_sample(
    samples: &mut Vec<Sample>,
    kind: &'static str,
    source: SourceKind,
    pattern: &str,
    input: &str,
    detail: String,
) {
    samples.push(Sample {
        kind,
        source: format!("{source:?}"),
        pattern: pattern.to_string(),
        input: input.to_string(),
        detail,
    });
}

fn write_samples_csv(path: &str, samples: &[Sample]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "kind,source,pattern,input,detail")?;
    for s in samples {
        writeln!(
            f,
            "{},{},{},{},{}",
            s.kind,
            s.source,
            csv_quote(&s.pattern),
            csv_quote(&s.input),
            csv_quote(&s.detail),
        )?;
    }
    Ok(())
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

    let mut tallies: [Tally; 5] = Default::default();
    let mut samples: Vec<Sample> = Vec::new();
    let mut printed_membership = 0usize;
    let mut printed_posix = 0usize;
    let mut printed_non_posix = 0usize;
    let mut printed_timeout = 0usize;
    let mut printed_crash = 0usize;
    let mut printed_ambiguous = 0usize;

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
        if case.expected_match {
            tally.matches += 1;
        } else {
            tally.non_matches += 1;
        }

        if let Some(names) = timed_out_names(&outcomes) {
            tally.timeouts += 1;
            if printed_timeout < MAX_PRINTED_PER_BUG_KIND {
                println!(
                    "TIMEOUT >{}ms  {:?}  pat={:?}  in={:?}  [{}]",
                    sel.timeout.as_millis(), case.source, truncate(&case.pattern, 60),
                    truncate(&case.input, 40), names.join(",")
                );
                push_sample(
                    &mut samples, "timeout", case.source, &case.pattern, &case.input,
                    format!("timed out after {}ms: [{}]", sel.timeout.as_millis(), names.join(",")),
                );
                printed_timeout += 1;
            }
            continue;
        }

        let crashed = crashed_names(&outcomes);
        if !crashed.is_empty() {
            tally.crashes += 1;
            if printed_crash < MAX_PRINTED_PER_BUG_KIND {
                let detail: Vec<String> = crashed.iter().map(|(n, r)| format!("{n}: {r}")).collect();
                println!(
                    "CRASH  {:?}  pat={:?}  in={:?}  {}",
                    case.source, truncate(&case.pattern, 60), truncate(&case.input, 40),
                    detail.join(" | ")
                );
                push_sample(&mut samples, "crash", case.source, &case.pattern, &case.input, detail.join(" | "));
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
        let membership_ok = all_matched.iter().all(|&m| m == case.expected_match);
        if !membership_ok {
            tally.membership_bugs += 1;
            if printed_membership < MAX_PRINTED_PER_BUG_KIND {
                println!(
                    "MEMBERSHIP BUG  {:?}  pat={:?}  in={:?}  expected={}  got=[rec={} loop={} bc={} pstd={} pbc={}]",
                    case.source, truncate(&case.pattern, 60), truncate(&case.input, 40),
                    case.expected_match, all_matched[0], all_matched[1], all_matched[2],
                    all_matched[3], all_matched[4]
                );
                push_sample(
                    &mut samples, "membership_bug", case.source, &case.pattern, &case.input,
                    format!(
                        "expected={} got=[rec={} loop={} bc={} pstd={} pbc={}]",
                        case.expected_match, all_matched[0], all_matched[1], all_matched[2],
                        all_matched[3], all_matched[4]
                    ),
                );
                printed_membership += 1;
            }
            continue;
        }

        if !case.expected_match {
            // Expected non-match: every parser returned None; no trees.
            continue;
        }

        let posix_shape_ok = rec == loop_ && loop_ == bc;
        if !posix_shape_ok {
            tally.posix_shape_bugs += 1;
            if printed_posix < MAX_PRINTED_PER_BUG_KIND {
                println!(
                    "POSIX SHAPE BUG  {:?}  pat={:?}  in={:?}  rec={:?} loop={:?} bc={:?}",
                    case.source, truncate(&case.pattern, 60), truncate(&case.input, 40),
                    rec, loop_, bc
                );
                push_sample(
                    &mut samples, "posix_shape_bug", case.source, &case.pattern, &case.input,
                    format!("rec={:?} loop={:?} bc={:?}", rec, loop_, bc),
                );
                printed_posix += 1;
            }
        }

        let non_posix_shape_ok = pstd == pbc;
        if !non_posix_shape_ok {
            tally.non_posix_shape_bugs += 1;
            if printed_non_posix < MAX_PRINTED_PER_BUG_KIND {
                println!(
                    "NON-POSIX SHAPE BUG  {:?}  pat={:?}  in={:?}  pstd={:?} pbc={:?}",
                    case.source, truncate(&case.pattern, 60), truncate(&case.input, 40),
                    pstd, pbc
                );
                push_sample(
                    &mut samples, "non_posix_shape_bug", case.source, &case.pattern, &case.input,
                    format!("pstd={:?} pbc={:?}", pstd, pbc),
                );
                printed_non_posix += 1;
            }
        }

        if posix_shape_ok && non_posix_shape_ok {
            tally.cross_compared += 1;
            let posix_tree: &Option<ParseTree> = &bc;
            let non_posix_tree: &Option<ParseTree> = &pbc;
            if posix_tree == non_posix_tree {
                tally.cross_same_shape += 1;
            } else {
                tally.cross_different_shape += 1;
                // Not a bug -- POSIX and non-POSIX may legitimately pick
                // different trees on an ambiguous pattern. One
                // representative per family is enough: every POSIX
                // parser already agrees with `bc` (posix_shape_ok,
                // checked above), and every non-POSIX parser with `pbc`
                // (non_posix_shape_ok), so printing all five would just
                // repeat each tree two or three times over.
                if printed_ambiguous < MAX_PRINTED_PER_BUG_KIND {
                    println!(
                        "AMBIGUOUS SHAPE  {:?}  pat={:?}  in={:?}\n  posix (bc)     ={:?}\n  non-posix (pbc)={:?}",
                        case.source, truncate(&case.pattern, 60), truncate(&case.input, 40),
                        posix_tree, non_posix_tree
                    );
                    push_sample(
                        &mut samples, "ambiguous_shape", case.source, &case.pattern, &case.input,
                        format!("posix (bc)={:?} | non-posix (pbc)={:?}", posix_tree, non_posix_tree),
                    );
                    printed_ambiguous += 1;
                }
            }
        }
    }

    println!();
    println!(
        "{:<13} {:>8} {:>8} {:>8} {:>9} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "Source", "Entries", "Matches", "NonMatch", "Timeouts", "Crashes",
        "MatchAgr%", "ShapeAgr%", "ShapeDis%", "ShapeNAgr", "ShapeNDis"
    );
    println!("{}", "-".repeat(118));

    let mut summary_csv = String::new();
    writeln!(
        summary_csv,
        "source,entries,matches,non_matches,timeouts,crashes,match_agree_pct,shape_agree_pct,\
         shape_disagree_pct,shape_n_agree,shape_n_disagree"
    )
    .unwrap();

    let mut total = Tally::default();
    for &source in &SOURCES {
        let t = &tallies[source_idx(source)];
        if t.entries == 0 {
            continue;
        }
        println!(
            "{:<13} {:>8} {:>8} {:>8} {:>9} {:>8} {:>9.1}% {:>9.1}% {:>9.1}% {:>10} {:>10}",
            format!("{:?}", source), t.entries, t.matches, t.non_matches, t.timeouts, t.crashes,
            t.match_agree_pct(), t.shape_agree_pct(), t.shape_disagree_pct(),
            t.cross_same_shape, t.cross_different_shape
        );
        writeln!(
            summary_csv,
            "{:?},{},{},{},{},{},{:.1},{:.1},{:.1},{},{}",
            source, t.entries, t.matches, t.non_matches, t.timeouts, t.crashes,
            t.match_agree_pct(), t.shape_agree_pct(), t.shape_disagree_pct(),
            t.cross_same_shape, t.cross_different_shape
        )
        .unwrap();
        total.accumulate(t);
    }
    println!("{}", "-".repeat(118));
    println!(
        "{:<13} {:>8} {:>8} {:>8} {:>9} {:>8} {:>9.1}% {:>9.1}% {:>9.1}% {:>10} {:>10}",
        "Total", total.entries, total.matches, total.non_matches, total.timeouts, total.crashes,
        total.match_agree_pct(), total.shape_agree_pct(), total.shape_disagree_pct(),
        total.cross_same_shape, total.cross_different_shape
    );
    writeln!(
        summary_csv,
        "Total,{},{},{},{},{},{:.1},{:.1},{:.1},{},{}",
        total.entries, total.matches, total.non_matches, total.timeouts, total.crashes,
        total.match_agree_pct(), total.shape_agree_pct(), total.shape_disagree_pct(),
        total.cross_same_shape, total.cross_different_shape
    )
    .unwrap();

    println!();
    println!("Entries = Matches + NonMatch, per the dataset's own labeling, regardless of");
    println!("  what any parser actually did with them.");
    println!("MatchAgr%  = of entries where every parser ran to completion (Entries minus");
    println!("  Timeouts minus Crashes), the % where all five parsers agreed with the");
    println!("  verified expected result. Should be 100% for every parser on every source --");
    println!("  this is a real bug, not ambiguity, if it is ever not.");
    println!("ShapeAgr% / ShapeDis% = of ShapeNAgr + ShapeNDis (verified matches where both");
    println!("  the 3-way POSIX and 2-way non-POSIX families agreed with themselves, so one");
    println!("  POSIX tree and one non-POSIX tree both exist), the % where POSIX and non-POSIX");
    println!("  picked the SAME tree vs. a DIFFERENT one. Disagreement here is not a bug --");
    println!("  the two policies may legitimately pick different trees on an ambiguous");
    println!("  pattern. (Internal 3-way/2-way family agreement is checked separately, each");
    println!("  pinned by its own test; any failure there is printed above as its own bug");
    println!("  line and counted in the total bug count, not folded into this column.)");
    println!("Every ShapeDis% entry is printed above as \"AMBIGUOUS SHAPE\" (and saved to the");
    println!("  samples file as kind=ambiguous_shape), with the pattern, input, and one");
    println!("  representative tree per family (bc for POSIX, pbc for non-POSIX -- every parser");
    println!("  in a family already agrees with its own representative, so the other two/one");
    println!("  would just repeat it), capped at {MAX_PRINTED_PER_BUG_KIND} total like the bug lines.");
    println!("Timeouts are not bugs: deriv_std_rec/deriv_std_loop run unsimplified, and a large or");
    println!("  ambiguous pattern can be infeasible in the configured budget.");
    println!("Crashes are counted, not dismissed (usually a stack overflow on a deep expression),");
    println!("  but are not membership/shape disagreements -- nothing was produced to compare.");

    let bugs = total.membership_bugs + total.posix_shape_bugs + total.non_posix_shape_bugs;
    println!();
    if bugs > 0 {
        println!("{} BUG(S) across {} entries (printed details capped at {} per kind).", bugs, total.entries, MAX_PRINTED_PER_BUG_KIND);
    } else {
        println!("No bugs across {} entries ({} timed out, {} crashed).", total.entries, total.timeouts, total.crashes);
    }

    save("parser_agreement", "csv", &summary_csv);
    let samples_path = "reports/demos/parser_agreement_samples.csv";
    if let Err(e) = write_samples_csv(samples_path, &samples) {
        eprintln!("could not write {samples_path}: {e}");
    } else {
        eprintln!("wrote {samples_path}");
    }

    if bugs > 0 {
        std::process::exit(1);
    }
}
