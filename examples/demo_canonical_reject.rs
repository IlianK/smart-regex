//! examples/demo_canonical_reject.rs
//!
//! cargo run --release --example demo_canonical_reject [-- <file...>]
//!
//! One example per rejection cause `dataset_stats.rs` counts.
//! Causes: backreference, lookaround, word-boundary, 
//! anchor, m flag, x flag, other.
//!
//! Replaces `demo_canonical_approx_reject.rs`.
//! The Approx half (R flag, nested anchor) no longer applies.
//! This frontend has no accepted-but-approximate case left.
//! Only accepted or rejected.

use std::path::PathBuf;

use regex_engine::data::extract::{extract_regexlib, extract_spamassassin, extract_suricata};
use regex_engine::data::types::SourceKind;
use regex_engine::frontend::parse_pcre_rule;

// ---------------------------------------------------------------------
// Corpus loading -- same default layout as dataset_stats.rs
// ---------------------------------------------------------------------

fn default_dir(source: SourceKind) -> &'static str {
    match source {
        SourceKind::Suricata => "data/raw/snort",
        SourceKind::SpamAssassin => "data/raw/spamAssassin",
        SourceKind::RegexLib => "data/raw/regexLib",
    }
}

fn files_in(dir: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            out.push(path);
        }
    }
    out.sort();
    out
}

fn classify_file(path: &PathBuf) -> SourceKind {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rules") => SourceKind::Suricata,
        Some("cf") => SourceKind::SpamAssassin,
        _ => SourceKind::RegexLib,
    }
}

/// `(pattern, source name)` for every extracted pattern in `files`.
fn load_patterns(files: &[(PathBuf, SourceKind)]) -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    for (path, source) in files {
        let text = match std::fs::read(path) {
            Ok(bytes) => String::from_utf8(bytes.clone())
                .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).into_owned()),
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
        for rule in extracted {
            out.push((rule.raw_pattern, source.dir_name()));
        }
    }
    out
}

/// Shortest pattern for which `detect` holds; `None` if none does.
/// Deterministic: does not depend on extraction order.
fn shortest_matching<'a>(
    patterns: &'a [(String, &'static str)],
    detect: impl Fn(&str) -> bool,
) -> Option<&'a (String, &'static str)> {
    patterns.iter().filter(|(p, _)| detect(p)).min_by_key(|(p, _)| p.len())
}

fn rejected_because(p: &str, needle: &str) -> bool {
    matches!(parse_pcre_rule(p), Err(e) if e.to_lowercase().contains(needle))
}

// ---------------------------------------------------------------------
// Reject: one section per Cause, mirroring dataset_stats.rs's Cause enum
// ---------------------------------------------------------------------

struct RejectCause {
    label: &'static str,
    detect: fn(&str) -> bool,
}

const REJECT_CAUSES: &[RejectCause] = &[
    RejectCause { label: "backreference", detect: |p| rejected_because(p, "backreference") },
    RejectCause { label: "lookaround", detect: |p| rejected_because(p, "lookahead") || rejected_because(p, "lookbehind") },
    RejectCause { label: "word-boundary", detect: |p| rejected_because(p, "word boundary") },
    RejectCause { label: "anchor ('^' or '$')", detect: |p| rejected_because(p, "anchor") },
    RejectCause { label: "flag-m (multiline)", detect: |p| rejected_because(p, "multiline") },
    RejectCause { label: "flag-x (extended)", detect: |p| rejected_because(p, "extended") },
    RejectCause {
        label: "other",
        detect: |p| {
            matches!(parse_pcre_rule(p), Err(e) if {
                let e = e.to_lowercase();
                !e.contains("backreference") && !e.contains("lookahead") && !e.contains("lookbehind")
                    && !e.contains("word boundary") && !e.contains("anchor")
                    && !e.contains("multiline") && !e.contains("extended")
            })
        },
    },
];

fn show_reject(patterns: &[(String, &'static str)]) {
    println!("REJECT: no Regex produced at all");
    println!("{}", "-".repeat(74));
    println!("dataset_stats.rs's seven Reject causes.");

    for cause in REJECT_CAUSES {
        println!();
        println!("[{}]", cause.label);
        match shortest_matching(patterns, cause.detect) {
            None => println!("  (no example in this corpus)"),
            Some((pattern, source)) => {
                let err = parse_pcre_rule(pattern).expect_err("detector already required rejection");
                println!("  source:  {source}");
                println!("  pattern: {pattern}");
                println!("  err:     {err}");
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let files: Vec<(PathBuf, SourceKind)> = if args.is_empty() {
        [SourceKind::Suricata, SourceKind::SpamAssassin, SourceKind::RegexLib]
            .into_iter()
            .flat_map(|s| files_in(default_dir(s)).into_iter().map(move |p| (p, s)))
            .collect()
    } else {
        args.iter().map(PathBuf::from).map(|p| { let s = classify_file(&p); (p, s) }).collect()
    };

    if files.is_empty() {
        eprintln!("no corpus files found (looked under data/raw/{{snort,spamAssassin,regexLib}})");
        std::process::exit(2);
    }

    let patterns = load_patterns(&files);
    println!("{} patterns extracted from {} file(s)", patterns.len(), files.len());
    println!();

    show_reject(&patterns);
}