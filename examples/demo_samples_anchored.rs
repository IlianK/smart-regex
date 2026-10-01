//! examples/demo_samples_anchored.rs
//!
//! cargo run --release --example demo_samples_anchored -- <source> 
//!          [--seed N] [--data-dir DIR] [--count N] 
//!          [--diag 0|1|2|3] [file...]
//!
//! For patterns carrying `^` and/or `$`: shows what padding buys on each anchor shape. 
//! A generated candidate starts and ends exactly where the core does, 
//! so for a `^`-only or `$`-only pattern it would match both the padded and the plain form,
//! and the padding would appear to do nothing. 
//! Extra text is therefore added on the side(s) that are actually padded 
//! (nothing for `^...$`, which has no padding to test), 
//! and the input is run through both forms: 
//! the padded form still matches on that side, the plain form does not.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

use regex_engine::data::extract::{extract_regexlib, extract_spamassassin, extract_suricata};
use regex_engine::data::generate::{generate_best, generate_neutral, generate_worst_structural};
use regex_engine::data::prepare::core_regex;
use regex_engine::data::types::{Candidate, Category, Provenance, SourceKind};
use regex_engine::diagnostics::{run_parser, DiagConfig, DiagLevel};
use regex_engine::frontend::alphabet::alphabet;
use regex_engine::frontend::{
    case_fold, detect_anchors, parse_ext_pattern, parse_pcre_rule, strip_pcre_delimiters,
    translate,
};
use regex_engine::parsers::ParserType;
use regex_engine::types::Regex;

fn size_regex(r: &Regex) -> usize {
    match r {
        Regex::Phi | Regex::Eps | Regex::Lit(_) => 1,
        Regex::Alt(a, b) | Regex::Seq(a, b) => 1 + size_regex(a) + size_regex(b),
        Regex::Star(a) => 1 + size_regex(a),
    }
}

fn noise(rng: &mut StdRng, len: usize) -> String {
    let chars = alphabet();
    (0..len).map(|_| *chars.choose(rng).expect("alphabet() is non-empty")).collect()
}

/// Same body as the padded regex, case-folded the same way, no padding:
/// so a noise-wrapped input can be checked against the plain form,
/// isolating what padding buys.
fn plain_translation(raw_pattern: &str) -> Regex {
    let (body, flags) = strip_pcre_delimiters(raw_pattern);
    let ep = parse_ext_pattern(body).expect("already accepted by parse_pcre_rule");
    let ep = if flags.case_insensitive { case_fold(&ep) } else { ep };
    translate(&ep).expect("already accepted by parse_pcre_rule")
}

/// Add noise on the side(s) actually padded; `None` for `^...$` (no
/// padding to exercise).
fn wrap_for_anchor_shape(
    text: &str,
    start_anchored: bool,
    end_anchored: bool,
    rng: &mut StdRng,
) -> Option<String> {
    match (start_anchored, end_anchored) {
        (true, true) => None,
        (true, false) => Some(format!("{text}{}", noise(rng, 4))),
        (false, true) => Some(format!("{}{text}", noise(rng, 4))),
        (false, false) => unreachable!("unanchored samples are not drawn from this pool"),
    }
}

struct Sample {
    pattern: String,
    start_anchored: bool,
    end_anchored: bool,
}

/// Fixed sample files under `data_dir`, one set per source; explicit file
/// paths override this list.
fn dataset_files(source: SourceKind, dir: &str) -> Vec<PathBuf> {
    let names: &[&str] = match source {
        SourceKind::Suricata => &["emerging-exploit.rules", "emerging-web_client.rules"],
        SourceKind::SpamAssassin => &["20_body_tests.cf", "20_drugs.cf", "20_head_tests.cf"],
        SourceKind::RegexLib => &["regexlib-manual-processed.sample.txt"],
    };
    names.iter().map(|n| Path::new(dir).join(n)).collect()
}

/// Extract every rule from `paths`; empty `paths` falls back to
/// `dataset_files` for this source.
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

/// Anchored subset of the patterns `parse_pcre_rule` accepts.
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

/// Up to 5 deduplicated candidates: 2 Best, 2 Neutral, then
/// `generate_worst_structural(.., 6, 1)` (one positive, one late-failing
/// negative), same generators `data::run_pipeline` uses.
fn generate_inputs(core: &Regex, rng: &mut StdRng) -> Vec<Candidate> {
    let mut inputs: Vec<Candidate> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let mut absorb = |cands: Vec<Candidate>, inputs: &mut Vec<Candidate>| {
        for c in cands {
            if seen.insert(c.text.clone()) {
                inputs.push(c);
            }
        }
    };

    absorb(generate_best(core, rng, 2), &mut inputs);
    absorb(generate_neutral(core, rng, 2), &mut inputs);
    absorb(generate_worst_structural(core, rng, 6, 1), &mut inputs);

    inputs.truncate(5);
    inputs
}

/// Plain-English reason; `Category::Worst` covers both a maximal-repetition
/// match and a late-failing near-miss, so the raw category name alone
/// doesn't say which.
fn describe(candidate: &Candidate) -> &'static str {
    match (candidate.category, candidate.provenance, candidate.claimed_match) {
        (Category::Best, ..) => "shortest matching input",
        (Category::Neutral, ..) => "typical matching input (moderate repetition)",
        (Category::Worst, Provenance::Structural, true) => {
            "hardest matching input (maximal repetition)"
        }
        (Category::Worst, Provenance::StructuralNegative, false) => {
            "near-miss: corrupted late, must NOT match"
        }
        _ => "generated input",
    }
}

/// Random sample from `pool` run through the five steps; a pattern
/// `parse_pcre_rule` accepts but `core_regex` rejects is skipped with a
/// note on stderr rather than aborting.
fn show(index: usize, total: usize, pool: &[Sample], rng: &mut StdRng, parser: ParserType, diag: DiagLevel) {
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

    let candidates = generate_inputs(&core, rng);
    let padded = parse_pcre_rule(&sample.pattern).expect("already accepted by parse_pcre_rule");
    let plain = plain_translation(&sample.pattern);
    println!(
        "3. Prepared:     core {} nodes -> search-padded {} nodes \
         (data::prepare::core_regex, frontend::parse_pcre_rule)",
        size_regex(&core),
        size_regex(&padded)
    );

    // Noise on the padded side(s) only; without it, a candidate drawn
    // from the core would match both forms identically and show nothing.
    let inputs: Vec<(String, &Candidate)> = candidates
        .iter()
        .map(|c| {
            let text = wrap_for_anchor_shape(&c.text, sample.start_anchored, sample.end_anchored, rng)
                .unwrap_or_else(|| c.text.clone());
            (text, c)
        })
        .collect();

    println!("4. Generated {} input(s) (data::generate):", inputs.len());
    for (text, c) in &inputs {
        let expect = if c.claimed_match { "MATCH   " } else { "NO MATCH" };
        println!("     [{expect}] {:?}  -- {}", text, describe(c));
    }
    println!();

    let fully_anchored = sample.start_anchored && sample.end_anchored;
    println!("5. --diag {} traces:", diag as u8);
    let config = DiagConfig::new(diag, parser, None);
    for (i, (text, c)) in inputs.iter().enumerate() {
        let expect = if c.claimed_match { "MATCH" } else { "NO MATCH" };
        println!("{}", "-".repeat(70));
        println!(
            "input {}/{} [expected {expect}] {:?}  -- {}",
            i + 1,
            inputs.len(),
            text,
            describe(c)
        );
        println!("{}", "-".repeat(70));
        if fully_anchored {
            run_parser(&sample.pattern, &padded, text, &config);
        } else {
            println!(
                "-- against the padded/search regex -- expected {expect}: \
                 the noise sits on the side the padding covers --"
            );
            run_parser(&sample.pattern, &padded, text, &config);
            println!();
            // Always NO MATCH: for a positive candidate the noise breaks
            // plain full-string equality; a negative already failed.
            println!(
                "-- same input, against the plain body with no padding at all -- \
                 expected NO MATCH: nothing absorbs the added noise --"
            );
            run_parser(&sample.pattern, &plain, text, &config);
        }
        println!();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "usage: cargo run --release --example demo_anchored_samples -- \
             <suricata|spamassassin|regexlib> [--seed N] [--data-dir DIR] [--count N] \
             [--diag 0|1|2|3] [file...]"
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
    let mut data_dir = "data/raw/_Samples".to_string();
    let mut count: usize = 3;
    let mut diag = DiagLevel::Basic;
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
            "--diag" => {
                i += 1;
                diag = match args.get(i).map(String::as_str) {
                    Some("0") => DiagLevel::Off,
                    Some("1") => DiagLevel::Basic,
                    Some("2") => DiagLevel::Verbose,
                    Some("3") => DiagLevel::Debug,
                    _ => {
                        eprintln!("--diag needs 0, 1, 2, or 3");
                        std::process::exit(2);
                    }
                };
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
        show(n, count, &anchored, &mut rng, ParserType::DerivStdRec, diag);
    }
}