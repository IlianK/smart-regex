//! examples/demo_samples_unanchored.rs
//!
//! cargo run --release --example demo_samples_unanchored -- <source> 
//!          [--seed N] [--data-dir DIR] [--count N] 
//!          [--diag 0|1|2|3] [file...]
//!
//! For unanchored patterns: 
//! shows that padding both sides with Sigma* does mean "match anywhere, not just at the start". 
//! A minimal match for the pattern is wrapped in arbitrary text on both sides, 
//! then run through two versions of the regex.
//! The search-padded version matches (Sigma* absorbs the extra text); 
//! the plain version does not (it requires an exact full-string match). 
//! The input is identical in both traces (only the regex differs) 
//! so the difference in outcome is the padding's doing.

use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

use regex_engine::data::extract::{extract_regexlib, extract_spamassassin, extract_suricata};
use regex_engine::data::generate::generate_best;
use regex_engine::data::prepare::core_regex;
use regex_engine::data::types::SourceKind;
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

/// Fixed sample files under `data_dir`, one set per source.
fn dataset_files(source: SourceKind, dir: &str) -> Vec<PathBuf> {
    let names: &[&str] = match source {
        SourceKind::Suricata => &["emerging-exploit.rules", "emerging-web_client.rules"],
        SourceKind::SpamAssassin => &["20_body_tests.cf", "20_drugs.cf", "20_head_tests.cf"],
        SourceKind::RegexLib => &["regexlib-manual-processed.sample.txt"],
    };
    names.iter().map(|n| Path::new(dir).join(n)).collect()
}

/// Extract every rule from `paths`; empty `paths` falls back to
/// `dataset_files`.
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

/// Patterns `parse_pcre_rule` accepts with no `^` and no `$`.
fn unanchored_patterns(patterns: &[String]) -> Vec<String> {
    patterns
        .iter()
        .filter(|p| {
            if parse_pcre_rule(p).is_err() {
                return false;
            }
            let (body, _) = strip_pcre_delimiters(p);
            matches!(detect_anchors(body), Ok((false, false)))
        })
        .cloned()
        .collect()
}

fn noise(rng: &mut StdRng, len: usize) -> String {
    let chars = alphabet();
    (0..len).map(|_| *chars.choose(rng).expect("alphabet() is non-empty")).collect()
}

/// Same body as the padded regex, case-folded the same way, no padding:
/// `parse_pattern` alone would drop the `i` flag, so this keeps the two
/// traces a controlled comparison.
fn plain_translation(raw_pattern: &str) -> Regex {
    let (body, flags) = strip_pcre_delimiters(raw_pattern);
    let ep = parse_ext_pattern(body).expect("already accepted by parse_pcre_rule");
    let ep = if flags.case_insensitive { case_fold(&ep) } else { ep };
    translate(&ep).expect("already accepted by parse_pcre_rule")
}

/// Random sample: minimal core match wrapped in noise, traced against
/// padded and plain forms. Skips patterns `core_regex` or `generate_best`
/// can't handle, with a note on stderr.
fn show(
    index: usize,
    total: usize,
    pool: &[String],
    rng: &mut StdRng,
    parser: ParserType,
    diag: DiagLevel,
) {
    let mut order: Vec<usize> = (0..pool.len()).collect();
    order.shuffle(rng);

    let (pattern, core, core_match) = 'pick: {
        for idx in &order {
            let pattern = &pool[*idx];
            let core = match core_regex(pattern) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!(
                        "skipping {pattern:?} for unanchored sample {index}/{total} \
                         (core_regex: {e}), trying another sample"
                    );
                    continue;
                }
            };
            let Some(candidate) = generate_best(&core, rng, 1).into_iter().next() else {
                eprintln!(
                    "skipping {pattern:?} for unanchored sample {index}/{total} \
                     (generate_best produced nothing), trying another sample"
                );
                continue;
            };
            break 'pick (pattern, core, candidate.text);
        }
        panic!("no sample in this pool of {} could be prepared", pool.len());
    };

    let prefix = noise(rng, 4);
    let suffix = noise(rng, 4);
    let wrapped = format!("{prefix}{core_match}{suffix}");

    println!("{}", "=".repeat(70));
    println!("UNANCHORED SAMPLE {index}/{total}");
    println!("{}", "=".repeat(70));
    println!("Raw pattern:   {pattern}");

    let padded = parse_pcre_rule(pattern).expect("already accepted by parse_pcre_rule");
    let plain = plain_translation(pattern);
    println!(
        "Core: {} nodes   Padded (Sigma* . core . Sigma*): {} nodes",
        size_regex(&core),
        size_regex(&padded)
    );
    println!("Minimal match for the core: {core_match:?}");
    println!("Wrapped in noise:           {wrapped:?}  ({prefix:?} + match + {suffix:?})");
    println!();

    let config = DiagConfig::new(diag, parser, None);

    println!(
        "-- against the padded/search regex (parse_pcre_rule) -- \
         expected MATCH: Sigma* absorbs the noise on both sides --"
    );
    run_parser(pattern, &padded, &wrapped, &config);
    println!();

    println!(
        "-- same input, against the same body with no padding \
         (parse_pattern on the folded body) -- expected NO MATCH: \
         nothing absorbs the noise, the input must equal the pattern exactly --"
    );
    run_parser(pattern, &plain, &wrapped, &config);
    println!();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "usage: cargo run --release --example demo_samples_unanchored -- \
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
    let unanchored = unanchored_patterns(&patterns);
    if unanchored.is_empty() {
        eprintln!("no unanchored accepted patterns found");
        std::process::exit(1);
    }

    let mut rng = match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    };

    println!("{} unanchored accepted patterns available\n", unanchored.len());

    for n in 1..=count {
        show(n, count, &unanchored, &mut rng, ParserType::DerivStdRec, diag);
    }
}