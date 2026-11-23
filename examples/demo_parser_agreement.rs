//! examples/demo_parser_agreement.rs
//!
//! Two-parser variant: `deriv_bc` (POSIX representative) and
//! `pderiv_bc` (non-POSIX representative), on the prepared corpus.
//! Reads `data/processed/`, written by `dataset_prepare`.
//!
//! Checks, per entry:
//!   - membership: both parsers agree with `expected_match`.
//!   - cross-family shape: when both match, do they return the same tree?
//!     Same shape is allowed but not required across families, so a
//!     disagreement is reported as "ambiguous", not a bug.
//!
//! Each parser call runs in its own subprocess, not a thread. See
//! `demo_parser_agreement_pathological.rs`'s module doc for the same
//! rationale (Rust cannot cancel a hung thread; a process can be killed).
//!
//! Build the worker first:
//!   cargo build --release --example demo_parser_agreement_worker
//!
//! Selection env vars (unchanged from the five-parser version):
//!   BENCH_SOURCE (comma-separated; default: every source)
//!   BENCH_PATTERN_LIMIT (cap distinct patterns per source)
//!   BENCH_MATCH_INTENT (direct/explicit/both; default explicit)
//!   AGREEMENT_TIMEOUT_MS (per-call timeout)
//!
//! Saves three files, replacing any previous run:
//!   - reports/demos/parser_agreement.csv -- one row per source plus a
//!     total.
//!   - reports/demos/parser_agreement_samples.csv -- every flagged entry.
//!   - reports/demos/parser_agreement_by_pattern.csv -- the same
//!     cross-family shape comparison, broken out per (source, pattern),
//!     uncapped and sorted so patterns with any disagreement come first.
//! This is the one demo in that family whose exit code matters (1 on a
//! real membership bug).

use std::collections::HashMap;
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
use regex_engine::data::types::{MatchIntent, PreparedCase, SourceKind};
use regex_engine::types::ParseTree;

const DATA_ROOT: &str = "data/processed";
const DEFAULT_TIMEOUT_MS: u64 = 2000;
/// Cap on printed instances per bug/outcome kind; beyond this, tally only.
const MAX_PRINTED_PER_BUG_KIND: usize = 20;
const WORKER_NAME: &str = "demo_parser_agreement_worker";

/// Worker parser indices: 2 = `deriv_bc`, 4 = `pderiv_bc`.
const IDX_DERIV_BC: usize = 2;
const IDX_PDERIV_BC: usize = 4;

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
    match_intent: Option<MatchIntent>,
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
        // Agreement is a correctness check, not a performance sweep, so
        // default to every source: a membership bug hiding anywhere is
        // exactly as real a bug.
        .unwrap_or_else(|| SOURCES.to_vec());
    let pattern_limit = std::env::var("BENCH_PATTERN_LIMIT").ok().map(|s| {
        s.parse().unwrap_or_else(|_| panic!("BENCH_PATTERN_LIMIT must be a positive integer, got {s:?}"))
    });
    // Same default as bench_corpus.rs: the 463 anchor-stripped patterns,
    // because a prepared case's claimed label is only unambiguous for
    // this engine's full-string matching under ExplicitFullMatch.
    let match_intent = match std::env::var("BENCH_MATCH_INTENT").ok() {
        None => Some(MatchIntent::ExplicitFullMatch),
        Some(s) => match s.trim().to_lowercase().as_str() {
            "both" => None,
            "direct" => Some(MatchIntent::UnanchoredAmbiguous),
            "explicit" => Some(MatchIntent::ExplicitFullMatch),
            other => panic!("BENCH_MATCH_INTENT must be direct, explicit, or both, got {other:?}"),
        },
    };
    let timeout_ms = std::env::var("AGREEMENT_TIMEOUT_MS")
        .ok()
        .map(|s| {
            s.parse().unwrap_or_else(|_| panic!("AGREEMENT_TIMEOUT_MS must be a positive integer, got {s:?}"))
        })
        .unwrap_or(DEFAULT_TIMEOUT_MS);
    Selection { sources, pattern_limit, match_intent, timeout: Duration::from_millis(timeout_ms) }
}

fn deserialize_tree(json: &str) -> serde_json::Result<Option<ParseTree>> {
    let mut de = serde_json::Deserializer::from_str(json);
    de.disable_recursion_limit();
    let de = serde_stacker::Deserializer::new(&mut de);
    serde::Deserialize::deserialize(de)
}

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

enum Outcome {
    Done(Option<ParseTree>),
    TimedOut,
    Crashed(String),
}

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
    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        let job = serde_json::json!({
            "pattern": pattern,
            "input": input,
            "mem_cap_bytes": mem_cap_bytes()
        })
        .to_string();
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
    let deadline = Instant::now() + timeout;
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
                    let _ = child.wait();
                    return Outcome::TimedOut;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => panic!("error polling worker process: {e}"),
        }
    }
}

fn select_cases(sel: &Selection) -> Vec<PreparedCase> {
    let cases = load_prepared_cases(std::path::Path::new(DATA_ROOT), &sel.sources)
        .unwrap_or_else(|e| {
            panic!(
                "couldn't read prepared cases under {} ({e}) -- run `cargo run --release --example \
                 dataset_prepare -- <suricata|spamassassin|regexlib|s2-elab|innovatorved> <file>...` first",
                DATA_ROOT
            )
        });
    let cases: Vec<PreparedCase> = match sel.match_intent {
        None => cases,
        Some(wanted) => cases.into_iter().filter(|case| case.match_intent == wanted).collect(),
    };
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
    matches: usize,
    non_matches: usize,
    /// At least one parser timed out or crashed.
    excluded: usize,
    /// `deriv_bc` timed out or crashed, when `pderiv_bc` completed.
    deriv_bc_excluded: usize,
    /// `pderiv_bc` timed out or crashed, when `deriv_bc` completed.
    pderiv_bc_excluded: usize,
    /// Both completed, and at least one disagrees with `expected_match`.
    membership_bugs: usize,
    /// Both matched and returned the same tree.
    cross_same_shape: usize,
    /// Both matched and returned different trees (allowed across families).
    cross_different_shape: usize,
}

impl Tally {
    fn completed(&self) -> usize {
        self.entries - self.excluded
    }
    fn match_agree_pct(&self) -> f64 {
        let n = self.completed();
        if n == 0 { 0.0 } else { 100.0 * (n - self.membership_bugs) as f64 / n as f64 }
    }
    fn shape_same_pct(&self) -> f64 {
        let n = self.cross_same_shape + self.cross_different_shape;
        if n == 0 { 0.0 } else { 100.0 * self.cross_same_shape as f64 / n as f64 }
    }
    fn shape_diff_pct(&self) -> f64 {
        let n = self.cross_same_shape + self.cross_different_shape;
        if n == 0 { 0.0 } else { 100.0 * self.cross_different_shape as f64 / n as f64 }
    }
    fn accumulate(&mut self, t: &Tally) {
        self.entries += t.entries;
        self.matches += t.matches;
        self.non_matches += t.non_matches;
        self.excluded += t.excluded;
        self.deriv_bc_excluded += t.deriv_bc_excluded;
        self.pderiv_bc_excluded += t.pderiv_bc_excluded;
        self.membership_bugs += t.membership_bugs;
        self.cross_same_shape += t.cross_same_shape;
        self.cross_different_shape += t.cross_different_shape;
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "..."
    }
}

#[derive(Default, Clone, Copy)]
struct PatternAgreement {
    compared: usize,
    same_shape: usize,
    different_shape: usize,
}

fn write_pattern_agreement_csv(
    path: &str,
    by_pattern: &HashMap<(SourceKind, String), PatternAgreement>,
) -> std::io::Result<()> {
    let mut rows: Vec<(&SourceKind, &String, &PatternAgreement)> =
        by_pattern.iter().map(|((s, p), a)| (s, p, a)).collect();
    rows.sort_by(|a, b| {
        b.2.different_shape
            .cmp(&a.2.different_shape)
            .then(b.2.compared.cmp(&a.2.compared))
            .then(format!("{:?}", a.0).cmp(&format!("{:?}", b.0)))
            .then(a.1.cmp(b.1))
    });

    let mut f = std::fs::File::create(path)?;
    writeln!(f, "source,pattern,n_inputs_compared,n_agree,n_disagree,disagree_pct")?;
    for (source, pattern, a) in rows {
        let pct = if a.compared == 0 { 0.0 } else { 100.0 * a.different_shape as f64 / a.compared as f64 };
        writeln!(
            f,
            "{:?},{},{},{},{},{:.1}",
            source, csv_quote(pattern), a.compared, a.same_shape, a.different_shape, pct
        )?;
    }
    Ok(())
}

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

    eprintln!(
        "demo_parser_agreement (deriv_bc vs pderiv_bc): {} cases across {} source(s) (timeout={:?})",
        cases.len(),
        sel.sources.len(),
        sel.timeout
    );

    let mut tallies: [Tally; 5] = Default::default();
    let mut samples: Vec<Sample> = Vec::new();
    let mut by_pattern: HashMap<(SourceKind, String), PatternAgreement> = HashMap::new();
    let mut printed_membership = 0usize;
    let mut printed_timeout = 0usize;
    let mut printed_crash = 0usize;
    let mut printed_ambiguous = 0usize;

    for case in &cases {
        if let Err(e) = core_regex(&case.pattern) {
            eprintln!("skipping prepared case (should have been pre-verified): {} -- {}", case.pattern, e);
            continue;
        }

        let o_bc = run_parser_isolated(&worker, sel.timeout, IDX_DERIV_BC, &case.pattern, &case.input);
        let o_pbc = run_parser_isolated(&worker, sel.timeout, IDX_PDERIV_BC, &case.pattern, &case.input);

        let tally = &mut tallies[source_idx(case.source)];
        tally.entries += 1;
        if case.expected_match {
            tally.matches += 1;
        } else {
            tally.non_matches += 1;
        }

        let bc_done = matches!(o_bc, Outcome::Done(_));
        let pbc_done = matches!(o_pbc, Outcome::Done(_));
        let bc_excluded = !bc_done;
        let pbc_excluded = !pbc_done;
        if bc_excluded || pbc_excluded {
            tally.excluded += 1;
            if bc_excluded && pbc_done {
                tally.deriv_bc_excluded += 1;
            }
            if pbc_excluded && bc_done {
                tally.pderiv_bc_excluded += 1;
            }
            for (name, outcome) in [("deriv_bc", &o_bc), ("pderiv_bc", &o_pbc)] {
                match outcome {
                    Outcome::TimedOut => {
                        if printed_timeout < MAX_PRINTED_PER_BUG_KIND {
                            println!(
                                "TIMEOUT >{}ms  {:?}  {}  pat={:?}  in={:?}",
                                sel.timeout.as_millis(), case.source, name,
                                truncate(&case.pattern, 60), truncate(&case.input, 40)
                            );
                            push_sample(
                                &mut samples, "timeout", case.source, &case.pattern, &case.input,
                                format!("{name} timed out after {}ms", sel.timeout.as_millis()),
                            );
                            printed_timeout += 1;
                        }
                    }
                    Outcome::Crashed(reason) => {
                        if printed_crash < MAX_PRINTED_PER_BUG_KIND {
                            println!(
                                "CRASH  {:?}  {}  pat={:?}  in={:?}  {}",
                                case.source, name, truncate(&case.pattern, 60),
                                truncate(&case.input, 40), reason
                            );
                            push_sample(
                                &mut samples, "crash", case.source, &case.pattern, &case.input,
                                format!("{name}: {reason}"),
                            );
                            printed_crash += 1;
                        }
                    }
                    Outcome::Done(_) => {}
                }
            }
            continue;
        }

        let bc = match o_bc { Outcome::Done(t) => t, _ => unreachable!() };
        let pbc = match o_pbc { Outcome::Done(t) => t, _ => unreachable!() };

        let bc_match = bc.is_some();
        let pbc_match = pbc.is_some();
        let membership_ok = bc_match == case.expected_match && pbc_match == case.expected_match;
        if !membership_ok {
            tally.membership_bugs += 1;
            if printed_membership < MAX_PRINTED_PER_BUG_KIND {
                println!(
                    "MEMBERSHIP BUG  {:?}  pat={:?}  in={:?}  expected={}  got=[deriv_bc={} pderiv_bc={}]",
                    case.source, truncate(&case.pattern, 60), truncate(&case.input, 40),
                    case.expected_match, bc_match, pbc_match
                );
                push_sample(
                    &mut samples, "membership_bug", case.source, &case.pattern, &case.input,
                    format!("expected={} got=[deriv_bc={} pderiv_bc={}]", case.expected_match, bc_match, pbc_match),
                );
                printed_membership += 1;
            }
            continue;
        }

        if !case.expected_match {
            continue;
        }

        // Cross-family shape comparison. Same tree is allowed but not
        // required across families, so a disagreement is reported as
        // "ambiguous", not a bug.
        let pattern_entry = by_pattern.entry((case.source, case.pattern.clone())).or_default();
        pattern_entry.compared += 1;
        if bc == pbc {
            tally.cross_same_shape += 1;
            pattern_entry.same_shape += 1;
        } else {
            tally.cross_different_shape += 1;
            pattern_entry.different_shape += 1;
            if printed_ambiguous < MAX_PRINTED_PER_BUG_KIND {
                println!(
                    "AMBIGUOUS SHAPE  {:?}  pat={:?}  in={:?}\n  deriv_bc  ={:?}\n  pderiv_bc ={:?}",
                    case.source, truncate(&case.pattern, 60), truncate(&case.input, 40), bc, pbc
                );
                push_sample(
                    &mut samples, "ambiguous_shape", case.source, &case.pattern, &case.input,
                    format!("deriv_bc={:?} | pderiv_bc={:?}", bc, pbc),
                );
                printed_ambiguous += 1;
            }
        }
    }

    println!();
    println!(
        "{:<13} {:>8} {:>8} {:>8} {:>9} {:>10} {:>10} {:>10} {:>9} {:>9}",
        "Source", "Entries", "Matches", "NonMatch", "Excluded", "MatchAgr%", "ShapeSame", "ShapeDiff", "Same%", "Diff%"
    );
    println!("{}", "-".repeat(115));

    let mut summary_csv = String::new();
    writeln!(
        summary_csv,
        "source,entries,matches,non_matches,excluded,deriv_bc_excluded,pderiv_bc_excluded,\
         match_agree_pct,shape_same,shape_diff,shape_same_pct,shape_diff_pct"
    )
    .unwrap();

    let mut total = Tally::default();
    for &source in &SOURCES {
        let t = &tallies[source_idx(source)];
        if t.entries == 0 {
            continue;
        }
        println!(
            "{:<13} {:>8} {:>8} {:>8} {:>9} {:>9.1}% {:>10} {:>10} {:>8.1}% {:>8.1}%",
            format!("{:?}", source), t.entries, t.matches, t.non_matches, t.excluded,
            t.match_agree_pct(), t.cross_same_shape, t.cross_different_shape,
            t.shape_same_pct(), t.shape_diff_pct()
        );
        writeln!(
            summary_csv,
            "{:?},{},{},{},{},{},{},{:.1},{},{},{:.1},{:.1}",
            source, t.entries, t.matches, t.non_matches, t.excluded,
            t.deriv_bc_excluded, t.pderiv_bc_excluded, t.match_agree_pct(),
            t.cross_same_shape, t.cross_different_shape, t.shape_same_pct(), t.shape_diff_pct()
        )
        .unwrap();
        total.accumulate(t);
    }
    println!("{}", "-".repeat(115));
    println!(
        "{:<13} {:>8} {:>8} {:>8} {:>9} {:>9.1}% {:>10} {:>10} {:>8.1}% {:>8.1}%",
        "Total", total.entries, total.matches, total.non_matches, total.excluded,
        total.match_agree_pct(), total.cross_same_shape, total.cross_different_shape,
        total.shape_same_pct(), total.shape_diff_pct()
    );
    writeln!(
        summary_csv,
        "Total,{},{},{},{},{},{},{:.1},{},{},{:.1},{:.1}",
        total.entries, total.matches, total.non_matches, total.excluded,
        total.deriv_bc_excluded, total.pderiv_bc_excluded, total.match_agree_pct(),
        total.cross_same_shape, total.cross_different_shape, total.shape_same_pct(), total.shape_diff_pct()
    )
    .unwrap();

    println!();
    println!("Two cross-family representatives only:");
    println!("  deriv_bc  = POSIX representative (worker index {IDX_DERIV_BC})");
    println!("  pderiv_bc = non-POSIX representative (worker index {IDX_PDERIV_BC})");
    println!("Within-family agreement is not checked here -- each family has one");
    println!("representative. MatchAgr% is a real bug if it is ever below 100%.");
    println!("Shape disagreements are not bugs: the two policies may legitimately");
    println!("pick different trees on an ambiguous pattern.");

    let bugs = total.membership_bugs;
    println!();
    if bugs > 0 {
        println!("{} membership bug(s) across {} entries (printed details capped at {}).", bugs, total.entries, MAX_PRINTED_PER_BUG_KIND);
    } else {
        println!(
            "No membership bugs across {} entries ({} excluded: {} deriv_bc-only failures, {} pderiv_bc-only failures).",
            total.entries, total.excluded, total.deriv_bc_excluded, total.pderiv_bc_excluded
        );
    }

    save("parser_agreement", "csv", &summary_csv);
    let samples_path = "reports/demos/parser_agreement_samples.csv";
    if let Err(e) = write_samples_csv(samples_path, &samples) {
        eprintln!("could not write {samples_path}: {e}");
    } else {
        eprintln!("wrote {samples_path}");
    }

    let by_pattern_path = "reports/demos/parser_agreement_by_pattern.csv";
    if let Err(e) = write_pattern_agreement_csv(by_pattern_path, &by_pattern) {
        eprintln!("could not write {by_pattern_path}: {e}");
    } else {
        eprintln!(
            "wrote {by_pattern_path} ({} distinct (source, pattern) pairs compared)",
            by_pattern.len()
        );
    }

    if bugs > 0 {
        std::process::exit(1);
    }
}