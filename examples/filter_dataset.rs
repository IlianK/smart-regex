//! regex-engine/examples/filter_dataset.rs
//!
//! Extracts every pattern from the given rule files and reports how many
//! `parse_pcre_rule` accepts, how many it rejects, and why.
//!
//! Takes arbitrary paths, so it works against a full downloaded ruleset
//! of any size, not just the tracked samples under `data/raw/`.
//! See `docs/DATASETS.md`.
//!
//! Usage:
//!   cargo run --release --example filter_dataset -- suricata <file>...
//!   cargo run --release --example filter_dataset -- spamassassin <file>...
//!   cargo run --release --example filter_dataset -- regexlib <file>...
//!
//! Flags:
//!   --verbose   list each rejected pattern with its cause
//!   --anchors   list the anchor-usage breakdown of accepted patterns
//!   --flags     list the PCRE flag distribution and per-flag rejection counts

use std::collections::BTreeMap;
use std::path::PathBuf;

use regex_engine::data::extract::{extract_regexlib, extract_spamassassin, extract_suricata};
use regex_engine::data::types::SourceKind;
use regex_engine::frontend::{
    detect_anchors, parse_pcre_rule, strip_pcre_delimiters,
};

#[derive(Debug)]
enum Cause {
    Backreference,
    Lookaround,
    WordBoundary,
    Multiline,
    Extended,
    Other,
}

impl Cause {
    fn label(&self) -> &'static str {
        match self {
            Cause::Backreference => "backreference",
            Cause::Lookaround => "lookaround",
            Cause::WordBoundary => "word-boundary",
            Cause::Multiline => "flag-m",
            Cause::Extended => "flag-x",
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
        } else if e.contains("multiline") {
            Cause::Multiline
        } else if e.contains("extended") {
            Cause::Extended
        } else {
            Cause::Other
        }
    }
}

/// One row per distinct flag-letter seen across the input, counting how
/// many extracted patterns carried it. Independent of accept/reject.
/// Buffer-modifier flags (`U`, `H`, `P`, ...) are reported too; they
/// have no entry in the note table, so they print as "not classified".
fn collect_flag_usage(patterns: &[String]) -> BTreeMap<char, usize> {
    let mut counts: BTreeMap<char, usize> = BTreeMap::new();
    for p in patterns {
        let s = p.trim();
        if !s.starts_with('/') {
            continue;
        }
        if let Some(end) = s.rfind('/') {
            if end > 0 {
                for c in s[end + 1..].chars() {
                    if c.is_ascii_alphabetic() {
                        *counts.entry(c).or_default() += 1;
                    }
                }
            }
        }
    }
    counts
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(source_arg) = args.next() else {
        eprintln!(
            "usage: cargo run --release --example filter_dataset -- \
             <suricata|spamassassin|regexlib> <file>... [--verbose] [--anchors] [--flags]"
        );
        std::process::exit(2);
    };

    let source = match source_arg.as_str() {
        "suricata" | "snort" => SourceKind::Suricata,
        "spamassassin" => SourceKind::SpamAssassin,
        "regexlib" => SourceKind::RegexLib,
        other => {
            eprintln!("unknown source {other:?} (expected suricata, spamassassin, or regexlib)");
            std::process::exit(2);
        }
    };

    let mut files: Vec<PathBuf> = Vec::new();
    let mut verbose = false;
    let mut anchors = false;
    let mut flags_report = false;
    for arg in args {
        match arg.as_str() {
            "--verbose" => verbose = true,
            "--anchors" => anchors = true,
            "--flags" => flags_report = true,
            _ => files.push(PathBuf::from(arg)),
        }
    }
    if files.is_empty() {
        eprintln!("no input files given");
        std::process::exit(2);
    }

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
    let mut accepted: Vec<String> = Vec::new();
    for p in &patterns {
        match parse_pcre_rule(p) {
            Ok(_) => accepted.push(p.clone()),
            Err(e) => rejected.push((p.clone(), Cause::classify(&e))),
        }
    }

    if verbose {
        println!("{:<14} {}", "Cause", "Pattern");
        println!("{}", "-".repeat(90));
        for (pattern, cause) in &rejected {
            let shown: String = pattern.chars().take(74).collect();
            println!("{:<14} {}", cause.label(), shown);
        }
        println!("{}", "-".repeat(90));
    }

    let count = |c: &Cause| {
        rejected
            .iter()
            .filter(|(_, r)| std::mem::discriminant(r) == std::mem::discriminant(c))
            .count()
    };

    println!(
        "{} extracted, {} accepted, {} rejected \
         ({} backreference, {} lookaround, {} word-boundary, \
          {} flag-m, {} flag-x, {} other)",
        patterns.len(),
        accepted.len(),
        rejected.len(),
        count(&Cause::Backreference),
        count(&Cause::Lookaround),
        count(&Cause::WordBoundary),
        count(&Cause::Multiline),
        count(&Cause::Extended),
        count(&Cause::Other),
    );

    if flags_report {
        println!("{}", "-".repeat(90));
        println!("PCRE flag distribution across all extracted patterns:");
        let usage = collect_flag_usage(&patterns);
        if usage.is_empty() {
            println!("  (no /.../FLAGS wrappers found)");
        } else {
            for (flag, n) in &usage {
                let note = match flag {
                    'i' => " (case-insensitive: honoured by case_fold)",
                    's' => " (dot-all: no effect, Dot already covers newlines)",
                    'm' => " (multiline: REJECTED, would change anchor meaning)",
                    'x' => " (extended: REJECTED, whitespace meaning not modelled)",
                    'R' => " (relative: ignored, treated as ordinary search pattern)",
                    _ => " (buffer-selection/normalization or unclassified: ignored)",
                };
                println!("  {flag}: {n}{note}");
            }
        }
    }

    if anchors {
        let mut start_only = 0usize;
        let mut end_only = 0usize;
        let mut both = 0usize;
        let mut neither = 0usize;
        for p in &accepted {
            let (body, _) = strip_pcre_delimiters(p);
            match detect_anchors(body) {
                Ok((true, true)) => both += 1,
                Ok((true, false)) => start_only += 1,
                Ok((false, true)) => end_only += 1,
                Ok((false, false)) => neither += 1,
                Err(e) => {
                    panic!("{p:?} was accepted by parse_pcre_rule but detect_anchors failed: {e}")
                }
            }
        }
        println!("{}", "-".repeat(90));
        println!(
            "of {} accepted: {} anchored ({} one-sided: {} ^-only, {} $-only; {} fully ^...$), \
             {} unanchored",
            accepted.len(),
            start_only + end_only + both,
            start_only + end_only,
            start_only,
            end_only,
            both,
            neither,
        );
    }
}