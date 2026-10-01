//! examples/dataset_prepare.rs
//!
//! cargo run --release --example dataset_prepare -- <source> <file...> [--corpus-dir DIR] [--variants N]
//!
//! Extract patterns from the given raw file(s), 
//! generate best/neutral/worst candidates for each, verify every one, 
//! and write the survivors to `data/processed/<source>/prepared.jsonl`.
//!
//! `--variants N` (default 5) is inputs per category per pattern, not a
//! total: up to N Best, N Neutral, and 2N Worst (N positive, N negative).

use std::path::{Path, PathBuf};

use regex_engine::data::types::SourceKind;
use regex_engine::data::run_pipeline;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "usage: cargo run --example dataset_prepare -- <suricata|spamassassin|regexlib> <file> [file...] [--corpus-dir <dir>]"
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
    println!(
        "patterns not faithful: {} (accepted, but only an approximation -- R or a nested anchor; \
         skipped, not included in the dataset)",
        report.patterns_not_faithful
    );
    println!("patterns faithful (used): {}", report.patterns_faithful);
    println!("candidates generated: {}", report.candidates_generated);
    println!("candidates verified:  {}", report.candidates_verified);
    println!(
        "-> data/processed/{}/prepared.jsonl",
        source.dir_name()
    );
}