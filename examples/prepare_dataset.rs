//! Runs the dataset preparation pipeline (`src/data/`) end to end for one
//! source: extract patterns from the given raw file(s), generate
//! best/neutral/worst candidates for each, verify every one against the
//! real parser, and write the survivors to
//! `data/processed/<source>/prepared.jsonl`.
//!
//! Usage:
//!   cargo run --release --example prepare_dataset -- suricata data/raw/emerging-web_client.rules [more files...]
//!   cargo run --release --example prepare_dataset -- spamassassin data/raw/20_drugs.cf --corpus-dir path/to/ham_spam
//!   cargo run --release --example prepare_dataset -- regexlib data/raw/regexlib-manual-processed.sample.txt [--variants 5]
//!
//! `--variants N` (default 5) is how many independently verified inputs
//! each pattern contributes per category, not a total across categories:
//! up to N `Best`, N `Neutral`, and 2N `Worst` (N heavily-iterated
//! positives, N late-failing negatives) survive verification per pattern.

use std::path::{Path, PathBuf};

use regex_engine::data::types::SourceKind;
use regex_engine::data::run_pipeline;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "usage: cargo run --example prepare_dataset -- <suricata|spamassassin|regexlib> <file> [file...] [--corpus-dir <dir>]"
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

    let mut files: Vec<PathBuf> = Vec::new();
    let mut corpus_dir: Option<PathBuf> = None;
    let mut variants: usize = 5;
    let mut i = 2;
    while i < args.len() {
        if args[i] == "--corpus-dir" {
            i += 1;
            corpus_dir = args.get(i).map(PathBuf::from);
        } else if args[i] == "--variants" {
            i += 1;
            variants = args.get(i).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                eprintln!("--variants needs a positive integer");
                std::process::exit(2);
            });
        } else {
            files.push(PathBuf::from(&args[i]));
        }
        i += 1;
    }

    let data_root = Path::new("data/processed");
    let seed = 20260101; // fixed, so a re-run reproduces the same dataset

    let report = run_pipeline(source, &files, data_root, corpus_dir.as_deref(), seed, variants)
        .unwrap_or_else(|e| {
            eprintln!("pipeline failed: {e}");
            std::process::exit(1);
        });

    println!("=== {:?} (variants per category: {}) ===", source, variants);
    println!("rules extracted:      {}", report.rules_extracted);
    println!("patterns unusable:    {}", report.patterns_unusable);
    println!("candidates generated: {}", report.candidates_generated);
    println!("candidates verified:  {}", report.candidates_verified);
    println!(
        "-> data/processed/{}/prepared.jsonl",
        source.dir_name()
    );
}
