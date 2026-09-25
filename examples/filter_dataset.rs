//! Filters a downloaded rule corpus for the patterns `parse_pcre_rule`
//! rejects outright (backreference, lookaround, word boundary, or another
//! unsupported construct), and prints them as a table.
//!
//! Takes arbitrary file paths, so it works against a full downloaded
//! ruleset of any size, not just the small tracked samples under
//! `data/raw/`. See `docs/DATASETS.md`.
//!
//! Usage:
//!   cargo run --release --example filter_dataset -- suricata <file>...
//!   cargo run --release --example filter_dataset -- spamassassin <file>...
//!   cargo run --release --example filter_dataset -- regexlib <file>...

use std::path::PathBuf;

use regex_engine::data::extract::{extract_regexlib, extract_spamassassin, extract_suricata};
use regex_engine::data::types::SourceKind;
use regex_engine::frontend::parse_pcre_rule;

enum Cause {
    Backreference,
    Lookaround,
    WordBoundary,
    Other,
}

impl Cause {
    fn label(&self) -> &'static str {
        match self {
            Cause::Backreference => "backreference",
            Cause::Lookaround => "lookaround",
            Cause::WordBoundary => "word-boundary",
            Cause::Other => "other",
        }
    }

    fn classify(err: &str) -> Cause {
        let e = err.to_lowercase();
        if e.contains("backreference") {
            Cause::Backreference
        } else if e.contains("lookahead") || e.contains("lookbehind") {
            Cause::Lookaround
        } else if e.contains("word boundary") {
            Cause::WordBoundary
        } else {
            Cause::Other
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: cargo run --release --example filter_dataset -- <suricata|spamassassin|regexlib> <file> [file...]");
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

    let files: Vec<PathBuf> = args[2..].iter().map(PathBuf::from).collect();
    let mut patterns: Vec<String> = Vec::new();
    for file in &files {
        let text = match std::fs::read_to_string(file) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("skipping {}: {}", file.display(), e);
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

    let mut rejected: Vec<(String, Cause)> = Vec::new();
    let mut accepted = 0usize;
    for p in &patterns {
        match parse_pcre_rule(p) {
            Ok(_) => accepted += 1,
            Err(e) => rejected.push((p.clone(), Cause::classify(&e))),
        }
    }

    println!("{:<8} {:<70} {}", "Cause", "Pattern", "");
    println!("{}", "-".repeat(90));
    for (pattern, cause) in &rejected {
        let shown: String = pattern.chars().take(68).collect();
        println!("{:<8} {:<70}", cause.label(), shown);
    }
    println!("{}", "-".repeat(90));
    println!(
        "{} extracted, {} accepted, {} rejected ({} backreference, {} lookaround, {} word-boundary, {} other)",
        patterns.len(),
        accepted,
        rejected.len(),
        rejected.iter().filter(|(_, c)| matches!(c, Cause::Backreference)).count(),
        rejected.iter().filter(|(_, c)| matches!(c, Cause::Lookaround)).count(),
        rejected.iter().filter(|(_, c)| matches!(c, Cause::WordBoundary)).count(),
        rejected.iter().filter(|(_, c)| matches!(c, Cause::Other)).count(),
    );
}
