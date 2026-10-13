//! examples/demo_frontend_categories.rs
//!
//! cargo run --release --example demo_frontend_categories -- <file...> [--seed N] [--diag 0|1|2|3]
//!
//! For each verified category of frontend behaviour, finds a real pattern
//! in the given corpus files that exercises it.
//! Categories: flags, class/escape handling, outright rejections.
//! Then shows the actual behaviour: either the translated Regex with
//! traces, or the exact rejection.
//! A category with no real example is reported and skipped.
//!
//! Files classified by extension:
//! `.rules` -> Suricata, `.cf` -> SpamAssassin, anything else -> RegexLib.
//! `--seed N` makes candidate generation deterministic.
//! `--diag 0|1|2|3` sets trace verbosity (default 1).

use std::path::PathBuf;

use rand::rngs::StdRng;
use rand::SeedableRng;

use regex_engine::data::extract::{
    extract_innovatorved, extract_regexlib, extract_s2_elab, extract_spamassassin,
    extract_suricata,
};
use regex_engine::data::generate::generate_sample;
use regex_engine::data::prepare::core_regex;
use regex_engine::data::types::SourceKind;
use regex_engine::diagnostics::{run_parser, DiagConfig, DiagLevel};
use regex_engine::frontend::{parse_ext_pattern, parse_pcre_rule, strip_pcre_delimiters, ExtPat};
use regex_engine::parsers::ParserType;

fn invert_ascii_case(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_lowercase() {
                c.to_ascii_uppercase()
            } else if c.is_ascii_uppercase() {
                c.to_ascii_lowercase()
            } else {
                c
            }
        })
        .collect()
}

fn contains_dot(ep: &ExtPat) -> bool {
    match ep {
        ExtPat::Dot => true,
        ExtPat::Or(parts) | ExtPat::Concat(parts) => parts.iter().any(contains_dot),
        ExtPat::Group(inner)
        | ExtPat::GroupNonMarking(inner)
        | ExtPat::Opt(inner)
        | ExtPat::Plus(inner)
        | ExtPat::Star(inner)
        | ExtPat::Bound(inner, ..) => contains_dot(inner),
        _ => false,
    }
}

/// Textual scan for a backslash-octal escape inside a `[...]` class.
/// Not a full PCRE parse; just enough to pick one real corpus example.
fn has_octal_in_class(body: &str) -> bool {
    let chars: Vec<char> = body.chars().collect();
    let mut in_class = false;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '\\' if in_class && chars.get(i + 1).is_some_and(|c| ('0'..='7').contains(c)) => {
                return true
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// Textual scan for a bare `\0` outside any `[...]` class.
fn has_bare_backslash_zero(body: &str) -> bool {
    let chars: Vec<char> = body.chars().collect();
    let mut in_class = false;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '\\' if !in_class && chars.get(i + 1) == Some(&'0') => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

fn accepted(p: &str) -> bool {
    parse_pcre_rule(p).is_ok()
}

fn rejected_because(p: &str, needle: &str) -> bool {
    matches!(parse_pcre_rule(p), Err(e) if e.contains(needle))
}

// -------------------------------
// Category detectors
// -------------------------------

fn is_case_insensitive(p: &str) -> bool {
    accepted(p) && strip_pcre_delimiters(p).1.case_insensitive
}

fn is_dot_without_s(p: &str) -> bool {
    if !accepted(p) {
        return false;
    }
    let (body, flags) = strip_pcre_delimiters(p);
    !flags.dot_all && parse_ext_pattern(body).map(|ep| contains_dot(&ep)).unwrap_or(false)
}

fn is_dot_with_s(p: &str) -> bool {
    if !accepted(p) {
        return false;
    }
    let (body, flags) = strip_pcre_delimiters(p);
    flags.dot_all && parse_ext_pattern(body).map(|ep| contains_dot(&ep)).unwrap_or(false)
}

/// Real `[:name:]` POSIX class, rejected outright.
/// Checks the rejection reason, not a `[:` substring: `[:*:]` contains
/// that text but is an ordinary class of three literals, accepted.
fn is_posix_class_rejected(p: &str) -> bool {
    rejected_because(p, "POSIX class")
}

fn is_octal_in_class(p: &str) -> bool {
    accepted(p) && has_octal_in_class(strip_pcre_delimiters(p).0)
}

fn is_bare_backslash_zero(p: &str) -> bool {
    accepted(p) && has_bare_backslash_zero(strip_pcre_delimiters(p).0)
}

fn is_multiline_rejected(p: &str) -> bool {
    strip_pcre_delimiters(p).1.multiline && rejected_because(p, "multiline")
}

fn is_extended_rejected(p: &str) -> bool {
    strip_pcre_delimiters(p).1.extended && rejected_because(p, "extended")
}

fn is_backreference_rejected(p: &str) -> bool {
    rejected_because(p, "backreference")
}

fn is_lookahead_rejected(p: &str) -> bool {
    rejected_because(p, "lookahead")
}

fn is_word_boundary_rejected(p: &str) -> bool {
    rejected_because(p, "word boundary")
}

fn is_anchor_rejected(p: &str) -> bool {
    rejected_because(p, "anchor")
}

// -------------------------------
// Demo kinds
// -------------------------------

/// Generic accepted pattern: translated size, then up to 3 generated
/// candidates traced against the real regex (full-string match only).
fn demo_accepted(pattern: &str, rng: &mut StdRng, diag: DiagLevel) {
    let core = core_regex(pattern).expect("category detector already required acceptance");

    let candidates = generate_sample(&core, rng, 3, 0);
    let config = DiagConfig::new(diag, ParserType::DerivStdRec, None);
    println!("generated {} input(s)", candidates.len());
    for c in &candidates {
        println!("  match?  {:?}", c.text);
        run_parser(pattern, &core, &c.text, &config);
    }
}

/// Targeted extra check: flips the ASCII case of every letter in a
/// generated match and shows it still matches.
/// Isolates what `case_fold` buys, rather than hoping a generic candidate
/// differs in case.
fn extra_case_insensitive(pattern: &str, rng: &mut StdRng, diag: DiagLevel) {
    let core = core_regex(pattern).expect("category detector already required acceptance");
    let Some(sample) = generate_sample(&core, rng, 1, 0).into_iter().next() else {
        println!("extra check: no generated input available, skipped");
        return;
    };
    let flipped = invert_ascii_case(&sample.text);
    println!("case-flip: {:?} -> {:?}", sample.text, flipped);
    let config = DiagConfig::new(diag, ParserType::DerivStdRec, None);
    run_parser(pattern, &core, &flipped, &config);
}

fn demo_rejected(pattern: &str) {
    let err = parse_pcre_rule(pattern).expect_err("category detector already required rejection");
    println!("rejected: {err}");
}

struct Entry {
    name: &'static str,
    detect: fn(&str) -> bool,
    kind: Kind,
    extra: Option<fn(&str, &mut StdRng, DiagLevel)>,
}

enum Kind {
    Accepted,
    Rejected,
}

fn entries() -> Vec<Entry> {
    vec![
        Entry {
            name: "case-insensitive ('i' flag)",
            detect: is_case_insensitive,
            kind: Kind::Accepted,
            extra: Some(extra_case_insensitive),
        },
        Entry {
            name: "'.' excludes '\\n' (no 's' flag)",
            detect: is_dot_without_s,
            kind: Kind::Accepted,
            extra: None,
        },
        Entry {
            name: "'.' includes '\\n' ('s' flag)",
            detect: is_dot_with_s,
            kind: Kind::Accepted,
            extra: None,
        },
        Entry {
            name: "octal escape inside a class",
            detect: is_octal_in_class,
            kind: Kind::Accepted,
            extra: None,
        },
        Entry {
            name: "bare '\\0' outside a class",
            detect: is_bare_backslash_zero,
            kind: Kind::Accepted,
            extra: None,
        },
        Entry {
            name: "rejected: multiline ('m' flag)",
            detect: is_multiline_rejected,
            kind: Kind::Rejected,
            extra: None,
        },
        Entry {
            name: "rejected: extended ('x' flag)",
            detect: is_extended_rejected,
            kind: Kind::Rejected,
            extra: None,
        },
        Entry {
            name: "rejected: backreference",
            detect: is_backreference_rejected,
            kind: Kind::Rejected,
            extra: None,
        },
        Entry {
            name: "rejected: lookahead",
            detect: is_lookahead_rejected,
            kind: Kind::Rejected,
            extra: None,
        },
        Entry {
            name: "rejected: word boundary",
            detect: is_word_boundary_rejected,
            kind: Kind::Rejected,
            extra: None,
        },
        Entry {
            name: "rejected: anchor ('^' or '$')",
            detect: is_anchor_rejected,
            kind: Kind::Rejected,
            extra: None,
        },
        Entry {
            name: "rejected: POSIX class (e.g. [:alpha:])",
            detect: is_posix_class_rejected,
            kind: Kind::Rejected,
            extra: None,
        },
    ]
}

fn classify_file(path: &PathBuf) -> SourceKind {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rules") => SourceKind::Suricata,
        Some("cf") => SourceKind::SpamAssassin,
        _ => SourceKind::RegexLib,
    }
}

fn load_all_patterns(files: &[PathBuf]) -> Vec<String> {
    let mut patterns = Vec::new();
    for path in files {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("skipping {}: {e}", path.display());
                continue;
            }
        };
        let extracted = match classify_file(path) {
            SourceKind::Suricata => extract_suricata(&text),
            SourceKind::SpamAssassin => extract_spamassassin(&text),
            SourceKind::RegexLib => extract_regexlib(&text),
            // `classify_file` only ever returns these three (`.rules`,
            // `.cf`, or the RegexLib fallback) -- it has no extension
            // rule telling an S2ELab `.json` file apart from an
            // Innovatorved one, so neither is reachable here. Handled
            // for exhaustiveness, not because this demo supports them.
            SourceKind::S2ELab => extract_s2_elab(&text),
            SourceKind::Innovatorved => extract_innovatorved(&text),
        };
        patterns.extend(extracted.into_iter().map(|r| r.raw_pattern));
    }
    patterns
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "usage: cargo run --release --example demo_frontend_categories -- \
             <file> [file...] [--seed N] [--diag 0|1|2|3]"
        );
        std::process::exit(2);
    }

    let mut seed: Option<u64> = None;
    let mut diag = DiagLevel::Basic;
    let mut files: Vec<PathBuf> = Vec::new();
    let mut i = 1;
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
            other => files.push(PathBuf::from(other)),
        }
        i += 1;
    }

    if files.is_empty() {
        eprintln!("no dataset files given");
        std::process::exit(2);
    }

    let patterns = load_all_patterns(&files);
    println!("{} patterns from {} file(s)", patterns.len(), files.len());
    println!();

    let mut rng = match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    };

    for entry in entries() {
        println!("[{}]", entry.name);
        println!("{}", "-".repeat(60));

        // Shortest real example; independent of --seed (which only
        // controls candidate generation below).
        let found = patterns.iter().filter(|p| (entry.detect)(p)).min_by_key(|p| p.len());

        let Some(pattern) = found else {
            println!("  no example in this dataset -- skipped");
            println!();
            continue;
        };

        println!("  raw: {pattern}");
        match entry.kind {
            Kind::Accepted => demo_accepted(pattern, &mut rng, diag),
            Kind::Rejected => demo_rejected(pattern),
        }
        if let Some(extra) = entry.extra {
            extra(pattern, &mut rng, diag);
        }
        println!();
    }
}