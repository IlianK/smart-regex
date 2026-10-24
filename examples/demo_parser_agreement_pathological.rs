//! examples/demo_parser_agreement_pathological.rs
//!
//! cargo run --release --example demo_parser_agreement_pathological
//!
//! `demo_parser_agreement`'s counterpart for the hand-curated
//! stress-pattern dataset (`data/pathological/pathological_patterns.json`)
//! instead of the real S2ELab corpus. Same three checks, same process
//! isolation, same worker binary (`demo_parser_agreement_worker` is
//! already a generic `(parser_idx, pattern, input) -> Option<ParseTree>`
//! call with no S2ELab-specific dependency) -- only entry selection
//! differs: rows come from `pathological::{load, expand}`, grouped by
//! category (A-L) instead of `SourceKind`, with `expected_match` taken
//! from which side (`match_input` vs `nomatch_input`) produced the entry.
//!
//! Row selection reuses `bench_pathological_time`'s own env vars:
//!   - `PATHO_JSON_PATH` (default `data/pathological/pathological_patterns.json`)
//!   - `PATHO_SECTION` (`pathological`/`baseline`/`generated`/`all`, default `all`)
//!   - `PATHO_CATEGORY` (comma-separated `A`-`L`, default every category present)
//!   - `PATHO_ROW_ID` (comma-separated row ids, default every selected row)
//!   - `PATHO_N` / `PATHO_P` (comma-separated sweep overrides; default
//!     each row's own category `suggested_n`/`suggested_p`)
//! `BENCH_PATTERN_LIMIT` caps the number of *rows* admitted, before the
//! `n`/`p` sweep. `AGREEMENT_TIMEOUT_MS` (default 2000) is the per-call
//! timeout -- raise it for a large-`n` sweep, same as
//! `BENCH_ENTRY_TIMEOUT_MS` elsewhere.
//!
//! Build the worker first, then run:
//!   cargo build --release --example demo_parser_agreement_worker
//!   cargo run --release --example demo_parser_agreement_pathological
//!
//! Saves two files, replacing any previous run:
//!   - reports/demos/parser_agreement_pathological.csv -- one row per
//!     category plus a total, same columns as the S2ELab summary.
//!   - reports/demos/parser_agreement_pathological_samples.csv -- every
//!     flagged entry, same shape as the S2ELab samples file.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[path = "common/thesis_figures.rs"]
mod common;
use common::{csv_quote, save};

use regex_engine::data::pathological::{self, PathologicalDoc, PathologicalRow};
use regex_engine::data::prepare::core_regex;
use regex_engine::types::ParseTree;

const DEFAULT_JSON_PATH: &str = "data/pathological/pathological_patterns.json";
const DEFAULT_TIMEOUT_MS: u64 = 2000;
const MAX_PRINTED_PER_BUG_KIND: usize = 20;
const WORKER_NAME: &str = "demo_parser_agreement_worker";
const PARSER_NAMES: [&str; 5] = ["rec", "loop", "bc", "pstd", "pbc"];

// -------------------------------
// Row selection -- mirrors bench_pathological_time.rs's own, duplicated
// rather than shared per this codebase's convention (see that file's
// own module doc).
// -------------------------------

#[derive(Clone, Copy)]
enum Section {
    Pathological,
    Baseline,
    Generated,
    All,
}

fn parse_section() -> Section {
    match std::env::var("PATHO_SECTION").ok().as_deref() {
        None | Some("all") => Section::All,
        Some("pathological") => Section::Pathological,
        Some("baseline") => Section::Baseline,
        Some("generated") => Section::Generated,
        Some(other) => panic!("PATHO_SECTION must be pathological, baseline, generated, or all, got {other:?}"),
    }
}

fn parse_categories() -> Option<HashSet<String>> {
    std::env::var("PATHO_CATEGORY").ok().map(|s| s.split(',').map(|c| c.trim().to_uppercase()).collect())
}

fn parse_row_ids() -> Option<HashSet<String>> {
    std::env::var("PATHO_ROW_ID").ok().map(|s| s.split(',').map(|id| id.trim().to_string()).collect())
}

fn parse_u32_list(var: &str) -> Option<Vec<u32>> {
    std::env::var(var).ok().map(|s| {
        s.split(',')
            .map(|part| part.trim().parse().unwrap_or_else(|_| panic!("{var} entries must be positive integers, got {part:?}")))
            .collect()
    })
}

fn pattern_limit() -> Option<usize> {
    std::env::var("BENCH_PATTERN_LIMIT")
        .ok()
        .map(|s| s.parse().unwrap_or_else(|_| panic!("BENCH_PATTERN_LIMIT must be a positive integer, got {s:?}")))
}

fn timeout() -> Duration {
    let ms: u64 = std::env::var("AGREEMENT_TIMEOUT_MS")
        .ok()
        .map(|s| s.parse().unwrap_or_else(|_| panic!("AGREEMENT_TIMEOUT_MS must be a positive integer, got {s:?}")))
        .unwrap_or(DEFAULT_TIMEOUT_MS);
    Duration::from_millis(ms)
}

fn json_path() -> PathBuf {
    std::env::var("PATHO_JSON_PATH").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(DEFAULT_JSON_PATH))
}

struct Entry {
    category: String,
    pattern: String,
    input: String,
    expected_match: bool,
}

/// E51's own notes document that its match_input is deliberately a
/// non-match (a stack-depth stress input for bench_stack_depth, not a
/// correctness case), so it always fails this tool's membership check
/// by design -- not a bug, just not a row this check applies to.
const EXCLUDED_FROM_AGREEMENT: &[&str] = &["E51"];

fn select_rows<'a>(doc: &'a PathologicalDoc, section: Section, categories: &Option<HashSet<String>>, row_ids: &Option<HashSet<String>>) -> Vec<&'a PathologicalRow> {
    let include = |r: &&PathologicalRow| {
        !EXCLUDED_FROM_AGREEMENT.contains(&r.id.as_str())
            && categories.as_ref().map_or(true, |cats| cats.contains(&r.category))
            && row_ids.as_ref().map_or(true, |ids| ids.contains(&r.id))
    };
    let mut rows: Vec<&PathologicalRow> = Vec::new();
    match section {
        Section::Pathological => rows.extend(doc.pathological_patterns.iter().filter(include)),
        Section::Baseline => rows.extend(doc.baseline_and_edge_patterns.iter().filter(include)),
        Section::Generated => rows.extend(doc.generated_patterns.iter().filter(include)),
        Section::All => {
            rows.extend(doc.pathological_patterns.iter().filter(include));
            rows.extend(doc.baseline_and_edge_patterns.iter().filter(include));
            rows.extend(doc.generated_patterns.iter().filter(include));
        }
    }
    rows
}

fn build_entries(doc: &PathologicalDoc, rows: &[&PathologicalRow], n_override: &Option<Vec<u32>>, p_override: &Option<Vec<u32>>) -> Vec<Entry> {
    let mut entries = Vec::new();
    for row in rows {
        let cat = doc.categories.get(&row.category);
        let n_list = n_override.clone().or_else(|| row.suggested_n.clone()).or_else(|| cat.and_then(|c| c.suggested_n.clone())).unwrap_or_else(|| vec![pathological::DEFAULT_N]);
        let needs_p = row.generator.is_some();
        let p_list = if needs_p {
            p_override.clone().or_else(|| row.suggested_p.clone()).or_else(|| cat.and_then(|c| c.suggested_p.clone())).unwrap_or_else(|| vec![pathological::DEFAULT_P])
        } else {
            vec![pathological::DEFAULT_P]
        };
        for &p in &p_list {
            for &n in &n_list {
                let (pattern, m, nm) = pathological::expand(row, n, p);
                if let Some(input) = m {
                    entries.push(Entry { category: row.category.clone(), pattern: pattern.clone(), input, expected_match: true });
                }
                if let Some(input) = nm {
                    entries.push(Entry { category: row.category.clone(), pattern: pattern.clone(), input, expected_match: false });
                }
            }
        }
    }
    entries
}

// -------------------------------
// Isolated call -- identical to demo_parser_agreement.rs's own; reuses
// the same worker binary, duplicated here rather than shared.
// -------------------------------

fn deserialize_tree(json: &str) -> serde_json::Result<Option<ParseTree>> {
    let mut de = serde_json::Deserializer::from_str(json);
    de.disable_recursion_limit();
    let de = serde_stacker::Deserializer::new(&mut de);
    serde::Deserialize::deserialize(de)
}

/// Per-worker virtual-memory ceiling. See the identical function in
/// `demo_parser_agreement.rs` for why.
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
        panic!("worker not found at {path:?} -- build it first:\n  cargo build --release --example {WORKER_NAME}");
    }
    path
}

enum Outcome {
    Done(Option<ParseTree>),
    TimedOut,
    Crashed(String),
}

fn run_parser_isolated(worker: &PathBuf, timeout: Duration, parser_idx: usize, pattern: &str, input: &str) -> Outcome {
    let mut child: Child = match Command::new(worker).arg(parser_idx.to_string()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(c) => c,
        Err(e) => panic!("failed to spawn {worker:?}: {e}"),
    };
    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        let job = serde_json::json!({ "pattern": pattern, "input": input, "mem_cap_bytes": mem_cap_bytes() }).to_string();
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
                    Err(e) => Outcome::Crashed(format!("worker exited 0 but its output did not parse: {e}; stdout={:?} stderr={:?}", out.trim(), err.trim())),
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

#[derive(Default, Clone)]
struct Tally {
    entries: usize,
    matches: usize,
    non_matches: usize,
    timeouts: usize,
    crashes: usize,
    membership_bugs: usize,
    posix_shape_bugs: usize,
    non_posix_shape_bugs: usize,
    cross_compared: usize,
    cross_same_shape: usize,
    cross_different_shape: usize,
}

impl Tally {
    fn completed(&self) -> usize {
        self.entries - self.timeouts - self.crashes
    }
    fn match_agree_pct(&self) -> f64 {
        let n = self.completed();
        if n == 0 { 0.0 } else { 100.0 * (n - self.membership_bugs) as f64 / n as f64 }
    }
    fn shape_agree_pct(&self) -> f64 {
        if self.cross_compared == 0 { 0.0 } else { 100.0 * self.cross_same_shape as f64 / self.cross_compared as f64 }
    }
    fn shape_disagree_pct(&self) -> f64 {
        if self.cross_compared == 0 { 0.0 } else { 100.0 * self.cross_different_shape as f64 / self.cross_compared as f64 }
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

fn timed_out_names(outcomes: &[Outcome; 5]) -> Option<Vec<&'static str>> {
    let names: Vec<&'static str> = outcomes.iter().zip(PARSER_NAMES).filter(|(o, _)| matches!(o, Outcome::TimedOut)).map(|(_, name)| name).collect();
    if names.is_empty() { None } else { Some(names) }
}

fn crashed_names(outcomes: &[Outcome; 5]) -> Vec<(&'static str, &str)> {
    outcomes.iter().zip(PARSER_NAMES).filter_map(|(o, name)| match o { Outcome::Crashed(reason) => Some((name, reason.as_str())), _ => None }).collect()
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { s.chars().take(n).collect::<String>() + "..." }
}

struct Sample {
    kind: &'static str,
    category: String,
    pattern: String,
    input: String,
    detail: String,
}

fn push_sample(samples: &mut Vec<Sample>, kind: &'static str, category: &str, pattern: &str, input: &str, detail: String) {
    samples.push(Sample { kind, category: category.to_string(), pattern: pattern.to_string(), input: input.to_string(), detail });
}

fn write_samples_csv(path: &str, samples: &[Sample]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "kind,category,pattern,input,detail")?;
    for s in samples {
        writeln!(f, "{},{},{},{},{}", s.kind, s.category, csv_quote(&s.pattern), csv_quote(&s.input), csv_quote(&s.detail))?;
    }
    Ok(())
}

fn main() {
    let section = parse_section();
    let categories = parse_categories();
    let row_ids = parse_row_ids();
    let n_override = parse_u32_list("PATHO_N");
    let p_override = parse_u32_list("PATHO_P");
    let limit = pattern_limit();
    let cap = timeout();
    let worker = worker_path();
    let path = json_path();

    let doc = pathological::load(&path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));
    let mut rows = select_rows(&doc, section, &categories, &row_ids);
    if let Some(limit) = limit {
        rows.truncate(limit);
    }
    if rows.is_empty() {
        panic!("no rows selected from {path:?} (section/category/row-id filters too narrow?)");
    }
    let entries = build_entries(&doc, &rows, &n_override, &p_override);
    if entries.is_empty() {
        panic!("{} rows selected but every match_input/nomatch_input was null", rows.len());
    }
    eprintln!("demo_parser_agreement_pathological: {} rows -> {} entries (entry_timeout={cap:?})", rows.len(), entries.len());

    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
    let mut samples: Vec<Sample> = Vec::new();
    let (mut printed_membership, mut printed_posix, mut printed_non_posix, mut printed_timeout, mut printed_crash, mut printed_ambiguous) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);

    for entry in &entries {
        if let Err(e) = core_regex(&entry.pattern) {
            eprintln!("skipping entry (pattern did not translate): {} -- {}", entry.pattern, e);
            continue;
        }

        let outcomes = std::array::from_fn(|i| run_parser_isolated(&worker, cap, i, &entry.pattern, &entry.input));

        let tally = tallies.entry(entry.category.clone()).or_default();
        tally.entries += 1;
        if entry.expected_match {
            tally.matches += 1;
        } else {
            tally.non_matches += 1;
        }

        if let Some(names) = timed_out_names(&outcomes) {
            tally.timeouts += 1;
            if printed_timeout < MAX_PRINTED_PER_BUG_KIND {
                println!("TIMEOUT >{}ms  {}  pat={:?}  in={:?}  [{}]", cap.as_millis(), entry.category, truncate(&entry.pattern, 60), truncate(&entry.input, 40), names.join(","));
                push_sample(&mut samples, "timeout", &entry.category, &entry.pattern, &entry.input, format!("timed out after {}ms: [{}]", cap.as_millis(), names.join(",")));
                printed_timeout += 1;
            }
            continue;
        }

        let crashed = crashed_names(&outcomes);
        if !crashed.is_empty() {
            tally.crashes += 1;
            if printed_crash < MAX_PRINTED_PER_BUG_KIND {
                let detail: Vec<String> = crashed.iter().map(|(n, r)| format!("{n}: {r}")).collect();
                println!("CRASH  {}  pat={:?}  in={:?}  {}", entry.category, truncate(&entry.pattern, 60), truncate(&entry.input, 40), detail.join(" | "));
                push_sample(&mut samples, "crash", &entry.category, &entry.pattern, &entry.input, detail.join(" | "));
                printed_crash += 1;
            }
            continue;
        }

        let [rec, loop_, bc, pstd, pbc] = outcomes.map(|o| match o {
            Outcome::Done(r) => r,
            Outcome::TimedOut | Outcome::Crashed(_) => unreachable!("checked above"),
        });

        let all_matched = [rec.is_some(), loop_.is_some(), bc.is_some(), pstd.is_some(), pbc.is_some()];
        let membership_ok = all_matched.iter().all(|&m| m == entry.expected_match);
        if !membership_ok {
            tally.membership_bugs += 1;
            if printed_membership < MAX_PRINTED_PER_BUG_KIND {
                println!(
                    "MEMBERSHIP BUG  {}  pat={:?}  in={:?}  expected={}  got=[rec={} loop={} bc={} pstd={} pbc={}]",
                    entry.category, truncate(&entry.pattern, 60), truncate(&entry.input, 40), entry.expected_match,
                    all_matched[0], all_matched[1], all_matched[2], all_matched[3], all_matched[4]
                );
                push_sample(
                    &mut samples, "membership_bug", &entry.category, &entry.pattern, &entry.input,
                    format!("expected={} got=[rec={} loop={} bc={} pstd={} pbc={}]", entry.expected_match, all_matched[0], all_matched[1], all_matched[2], all_matched[3], all_matched[4]),
                );
                printed_membership += 1;
            }
            continue;
        }

        if !entry.expected_match {
            continue;
        }

        let posix_shape_ok = rec == loop_ && loop_ == bc;
        if !posix_shape_ok {
            tally.posix_shape_bugs += 1;
            if printed_posix < MAX_PRINTED_PER_BUG_KIND {
                println!("POSIX SHAPE BUG  {}  pat={:?}  in={:?}  rec={:?} loop={:?} bc={:?}", entry.category, truncate(&entry.pattern, 60), truncate(&entry.input, 40), rec, loop_, bc);
                push_sample(&mut samples, "posix_shape_bug", &entry.category, &entry.pattern, &entry.input, format!("rec={:?} loop={:?} bc={:?}", rec, loop_, bc));
                printed_posix += 1;
            }
        }

        let non_posix_shape_ok = pstd == pbc;
        if !non_posix_shape_ok {
            tally.non_posix_shape_bugs += 1;
            if printed_non_posix < MAX_PRINTED_PER_BUG_KIND {
                println!("NON-POSIX SHAPE BUG  {}  pat={:?}  in={:?}  pstd={:?} pbc={:?}", entry.category, truncate(&entry.pattern, 60), truncate(&entry.input, 40), pstd, pbc);
                push_sample(&mut samples, "non_posix_shape_bug", &entry.category, &entry.pattern, &entry.input, format!("pstd={:?} pbc={:?}", pstd, pbc));
                printed_non_posix += 1;
            }
        }

        if posix_shape_ok && non_posix_shape_ok {
            tally.cross_compared += 1;
            if bc == pbc {
                tally.cross_same_shape += 1;
            } else {
                tally.cross_different_shape += 1;
                if printed_ambiguous < MAX_PRINTED_PER_BUG_KIND {
                    println!("AMBIGUOUS SHAPE  {}  pat={:?}  in={:?}\n  posix (bc)     ={:?}\n  non-posix (pbc)={:?}", entry.category, truncate(&entry.pattern, 60), truncate(&entry.input, 40), bc, pbc);
                    push_sample(&mut samples, "ambiguous_shape", &entry.category, &entry.pattern, &entry.input, format!("posix (bc)={:?} | non-posix (pbc)={:?}", bc, pbc));
                    printed_ambiguous += 1;
                }
            }
        }
    }

    println!();
    println!(
        "{:<6} {:>8} {:>8} {:>8} {:>9} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "Cat", "Entries", "Matches", "NonMatch", "Timeouts", "Crashes", "MatchAgr%", "ShapeAgr%", "ShapeDis%", "ShapeNAgr", "ShapeNDis"
    );
    println!("{}", "-".repeat(111));

    let mut summary_csv = String::new();
    writeln!(summary_csv, "category,entries,matches,non_matches,timeouts,crashes,match_agree_pct,shape_agree_pct,shape_disagree_pct,shape_n_agree,shape_n_disagree").unwrap();

    let mut total = Tally::default();
    for (category, t) in &tallies {
        println!(
            "{:<6} {:>8} {:>8} {:>8} {:>9} {:>8} {:>9.1}% {:>9.1}% {:>9.1}% {:>10} {:>10}",
            category, t.entries, t.matches, t.non_matches, t.timeouts, t.crashes, t.match_agree_pct(), t.shape_agree_pct(), t.shape_disagree_pct(), t.cross_same_shape, t.cross_different_shape
        );
        writeln!(
            summary_csv, "{},{},{},{},{},{},{:.1},{:.1},{:.1},{},{}",
            category, t.entries, t.matches, t.non_matches, t.timeouts, t.crashes, t.match_agree_pct(), t.shape_agree_pct(), t.shape_disagree_pct(), t.cross_same_shape, t.cross_different_shape
        ).unwrap();
        total.accumulate(t);
    }
    println!("{}", "-".repeat(111));
    println!(
        "{:<6} {:>8} {:>8} {:>8} {:>9} {:>8} {:>9.1}% {:>9.1}% {:>9.1}% {:>10} {:>10}",
        "Total", total.entries, total.matches, total.non_matches, total.timeouts, total.crashes, total.match_agree_pct(), total.shape_agree_pct(), total.shape_disagree_pct(), total.cross_same_shape, total.cross_different_shape
    );
    writeln!(
        summary_csv, "Total,{},{},{},{},{},{:.1},{:.1},{:.1},{},{}",
        total.entries, total.matches, total.non_matches, total.timeouts, total.crashes, total.match_agree_pct(), total.shape_agree_pct(), total.shape_disagree_pct(), total.cross_same_shape, total.cross_different_shape
    ).unwrap();

    println!();
    println!("Same columns as demo_parser_agreement's S2ELab table -- see that tool's own");
    println!("printed legend for what each means. Timeouts are expected here, not bugs: the");
    println!("pathological dataset exists specifically to find each parser's real limit.");

    let bugs = total.membership_bugs + total.posix_shape_bugs + total.non_posix_shape_bugs;
    println!();
    if bugs > 0 {
        println!("{} BUG(S) across {} entries (printed details capped at {} per kind).", bugs, total.entries, MAX_PRINTED_PER_BUG_KIND);
    } else {
        println!("No bugs across {} entries ({} timed out, {} crashed).", total.entries, total.timeouts, total.crashes);
    }

    save("parser_agreement_pathological", "csv", &summary_csv);
    let samples_path = "reports/demos/parser_agreement_pathological_samples.csv";
    if let Err(e) = write_samples_csv(samples_path, &samples) {
        eprintln!("could not write {samples_path}: {e}");
    } else {
        eprintln!("wrote {samples_path}");
    }

    if bugs > 0 {
        std::process::exit(1);
    }
}
