//! regex-engine/examples/demo_anchored_samples.rs
//!
//! For patterns that carry a `^` and/or `$`, shows how the frontend's
//! padding rule (`frontend::translate_as_search`) applies to that pattern's
//! own anchor shape, running it through the project's own dataset pipeline
//! (`src/data/`, the same code `examples/prepare_dataset.rs` drives):
//!
//!   1. the original raw regex
//!   2. which anchors it has, via `frontend::detect_anchors`
//!   3. the core `Regex`, via `data::prepare::core_regex`
//!   4. up to 5 generated candidate inputs, via `data::generate`
//!   5. `--diag 2` on each input against the search-padded regex,
//!      via `frontend::parse_pcre_rule`
//!
//! Only patterns that carry at least one anchor are drawn from: a
//! pattern with neither is what `examples/demo_unanchored_samples.rs`
//! covers. `--count` picks how many to show per run.
//!
//! Run:
//!   cargo run --release --example demo_anchored_samples -- regexlib \
//!     --data-dir data/raw/_Samples --seed 1 --count 3
//!
//! `--seed N` makes sample selection and input generation deterministic;
//! omit it for a fresh set each run. `--data-dir DIR` overrides the sample
//! directory (default `data/_Samples`). `--count N` is how many samples
//! to show (default 3).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

use regex_engine::data::extract::{extract_regexlib, extract_spamassassin, extract_suricata};
use regex_engine::data::generate::{generate_best, generate_neutral, generate_worst_structural};
use regex_engine::data::prepare::core_regex;
use regex_engine::data::types::{Candidate, Category, SourceKind};
use regex_engine::diagnostics::{run_parser, DiagConfig, DiagLevel};
use regex_engine::frontend::{detect_anchors, parse_pcre_rule, strip_pcre_delimiters};
use regex_engine::parsers::ParserType;
use regex_engine::types::Regex;

fn size_regex(r: &Regex) -> usize {
    match r {
        Regex::Phi | Regex::Eps | Regex::Lit(_) => 1,
        Regex::Alt(a, b) | Regex::Seq(a, b) => 1 + size_regex(a) + size_regex(b),
        Regex::Star(a) => 1 + size_regex(a),
    }
}

struct Sample {
    pattern: String,
    start_anchored: bool,
    end_anchored: bool,
}

/// Fixed sample-file names under `data_dir`, one set per source. Explicit
/// file paths on the command line override this list (see `load_patterns`).
fn dataset_files(source: SourceKind, dir: &str) -> Vec<PathBuf> {
    let names: &[&str] = match source {
        SourceKind::Suricata => &["emerging-exploit.rules", "emerging-web_client.rules"],
        SourceKind::SpamAssassin => &["20_body_tests.cf", "20_drugs.cf", "20_head_tests.cf"],
        SourceKind::RegexLib => &["regexlib-manual-processed.sample.txt"],
    };
    names.iter().map(|n| Path::new(dir).join(n)).collect()
}

/// Read and extract every rule from each path. If `paths` is empty, fall
/// back to the fixed sample file names under `data_dir` for this source.
fn load_patterns(source: SourceKind, data_dir: &str, paths: &[PathBuf]) -> Vec<String> {
    let files: Vec<PathBuf> = if paths.is_empty() {
        dataset_files(source, data_dir)
    } else {
        paths.to_vec()
    };

    let mut patterns = Vec::new();
    for path in &files {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("skipping {}: {e}", path.display());
                continue;
            }
        };
        let extracted = match source {
            SourceKind::Suricata => extract_suricata(&text),
            SourceKind::SpamAssassin => extract_spamassassin(&text),
            SourceKind::RegexLib => extract_regexlib(&text),
        };
        patterns.extend(extracted.into_iter().map(|r| r.raw_pattern));
    }
    patterns
}

/// The anchored subset of the patterns `parse_pcre_rule` accepts, using
/// the same `strip_pcre_delimiters` + `detect_anchors` pair
/// `examples/filter_dataset.rs` uses.
fn anchored_patterns(patterns: &[String]) -> Vec<Sample> {
    let mut anchored = Vec::new();
    for p in patterns {
        if parse_pcre_rule(p).is_err() {
            continue;
        }
        let (body, _) = strip_pcre_delimiters(p);
        let Ok((start, end)) = detect_anchors(body) else { continue };
        if start || end {
            anchored.push(Sample {
                pattern: p.clone(),
                start_anchored: start,
                end_anchored: end,
            });
        }
    }
    anchored
}

/// Up to 5 deduplicated candidate inputs from an already-prepared core
/// `Regex`, using the same generators `data::run_pipeline` calls:
/// 2 Best, 2 Neutral, then `generate_worst_structural(.., 6, 1)`, which
/// yields at most one positive and one late-failing negative.
fn generate_inputs(core: &Regex, rng: &mut StdRng) -> Vec<(String, Category)> {
    let mut inputs: Vec<(String, Category)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let mut absorb = |cands: Vec<Candidate>, inputs: &mut Vec<(String, Category)>| {
        for c in cands {
            if seen.insert(c.text.clone()) {
                inputs.push((c.text, c.category));
            }
        }
    };

    absorb(generate_best(core, rng, 2), &mut inputs);
    absorb(generate_neutral(core, rng, 2), &mut inputs);
    absorb(generate_worst_structural(core, rng, 6, 1), &mut inputs);

    inputs.truncate(5);
    inputs
}

/// Picks a random sample from `pool` and runs it through all 5 steps. A
/// pattern `parse_pcre_rule` accepts can still be rejected by
/// `core_regex`; when that happens, retry with another sample from the
/// same pool rather than aborting, noting the skip on stderr.
fn show(index: usize, total: usize, pool: &[Sample], rng: &mut StdRng, parser: ParserType) {
    let mut order: Vec<usize> = (0..pool.len()).collect();
    order.shuffle(rng);

    let (sample, core) = 'pick: {
        for idx in &order {
            let sample = &pool[*idx];
            match core_regex(&sample.pattern) {
                Ok(core) => break 'pick (sample, core),
                Err(e) => eprintln!(
                    "skipping {:?} for anchored sample {index}/{total} (core_regex: {e}), \
                     trying another sample",
                    sample.pattern
                ),
            }
        }
        panic!("no sample in this pool of {} could be prepared by core_regex", pool.len());
    };

    println!("{}", "=".repeat(70));
    println!("ANCHORED SAMPLE {index}/{total}");
    println!("{}", "=".repeat(70));

    println!("1. Raw pattern:  {}", sample.pattern);

    println!(
        "2. Anchors:      {}",
        match (sample.start_anchored, sample.end_anchored) {
            (true, true) => "^...$ (full match, no padding either side)",
            (true, false) => "^... (padded on the right only)",
            (false, true) => "...$ (padded on the left only)",
            (false, false) => unreachable!("unanchored samples are not drawn from this pool"),
        }
    );

    let inputs = generate_inputs(&core, rng);
    let padded = parse_pcre_rule(&sample.pattern).expect("already accepted by parse_pcre_rule");
    println!(
        "3. Prepared:     core {} nodes -> search-padded {} nodes \
         (data::prepare::core_regex, frontend::parse_pcre_rule)",
        size_regex(&core),
        size_regex(&padded)
    );
    println!("4. Generated {} input(s) (data::generate):", inputs.len());
    for (text, category) in &inputs {
        println!("     [{category:?}] {text:?}");
    }
    println!();

    println!("5. --diag 2 traces:");
    let config = DiagConfig::new(DiagLevel::Verbose, parser, None);
    for (i, (text, category)) in inputs.iter().enumerate() {
        println!("{}", "-".repeat(70));
        println!("input {}/{} [{category:?}]: {text:?}", i + 1, inputs.len());
        println!("{}", "-".repeat(70));
        run_parser(&sample.pattern, &padded, text, &config);
        println!();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "usage: cargo run --release --example demo_anchored_samples -- \
             <suricata|spamassassin|regexlib> [--seed N] [--data-dir DIR] [--count N] [file...]"
        );
        std::process::exit(2);
    }

    let source = match args[1].as_str() {
        "suricata" | "snort" => SourceKind::Suricata,
        "spamassassin" => SourceKind::SpamAssassin,
        "regexlib" => SourceKind::RegexLib,
        other => {
            eprintln!("unknown source {other:?} (expected suricata, spamassassin, or regexlib)");
            std::process::exit(2);
        }
    };

    let mut seed: Option<u64> = None;
    let mut data_dir = "data/_Samples".to_string();
    let mut count: usize = 3;
    let mut explicit_files: Vec<PathBuf> = Vec::new();
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => {
                i += 1;
                let Some(s) = args.get(i).and_then(|s| s.parse().ok()) else {
                    eprintln!("--seed needs a number");
                    std::process::exit(2);
                };
                seed = Some(s);
            }
            "--data-dir" => {
                i += 1;
                data_dir = args.get(i).cloned().unwrap_or_else(|| {
                    eprintln!("--data-dir needs a path");
                    std::process::exit(2);
                });
            }
            "--count" => {
                i += 1;
                let Some(c) = args.get(i).and_then(|s| s.parse().ok()) else {
                    eprintln!("--count needs a number");
                    std::process::exit(2);
                };
                count = c;
            }
            other if !other.starts_with("--") => explicit_files.push(PathBuf::from(other)),
            other => {
                eprintln!("unknown argument {other:?}");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let patterns = load_patterns(source, &data_dir, &explicit_files);
    let anchored = anchored_patterns(&patterns);
    if anchored.is_empty() {
        eprintln!("no anchored accepted patterns found");
        std::process::exit(1);
    }

    let mut rng = match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    };

    println!("{} anchored accepted patterns available\n", anchored.len());

    for n in 1..=count {
        show(n, count, &anchored, &mut rng, ParserType::DerivStdRec);
    }
}