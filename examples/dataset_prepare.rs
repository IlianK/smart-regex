//! examples/dataset_prepare.rs
//!
//! cargo run --release --example dataset_prepare -- <source> <file...> [--corpus-dir DIR] [--variants N]
//!
//! Extract patterns from the given raw file(s).
//! Generate up to N positive and N negative samples per pattern, verify
//! each, and write survivors to `data/processed/<source>/prepared.jsonl`.
//!
//! `--variants N` (default 5) is samples per pattern: up to N positives
//! and N negatives, not a combined total.

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
    let seed = 20260101; // fixed, so re-runs reproduce the same dataset

    let report = run_pipeline(source, &files, data_root, corpus_dir.as_deref(), seed, variants)
        .unwrap_or_else(|e| {
            eprintln!("pipeline failed: {e}");
            std::process::exit(1);
        });

    println!("{:?}  ({} variants/pattern)", source, variants);
    println!("  extracted   {:>6} rules",     report.rules_extracted);
    println!("  unusable    {:>6} patterns",  report.patterns_unusable);
    println!(
        "  accepted    {:>6} patterns  [{} direct, {} full-match]",
        report.patterns_accepted,
        report.patterns_unanchored_ambiguous,
        report.patterns_explicit_full_match,
    );
    if report.patterns_explicit_full_match_skipped > 0 {
        println!(
            "    skipped   {:>6} full-match (generation is direct-only)",
            report.patterns_explicit_full_match_skipped
        );
    }
    println!("  generated   {:>6} candidates", report.candidates_generated);
    println!("  verified    {:>6} candidates", report.candidates_verified);
    println!("  wrote       data/processed/{}/prepared.jsonl", source.dir_name());
}