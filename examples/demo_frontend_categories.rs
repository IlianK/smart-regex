//! examples/demo_frontend_categories.rs
//!
//! cargo run --release --example demo_frontend_categories -- <s2-elab-json-file...> [--diag 0|1|2|3]
//!
//! For each verified category of frontend behaviour, finds a real S2ELab
//! pattern that exercises it, then shows the actual behaviour: either the
//! translated `Regex` run against the pattern's own real `matches`/
//! `non_matches` (whichever genuinely verify), or the exact rejection.
//! A category with no real example in the given file(s) is reported and
//! skipped, never faked.

use std::path::PathBuf;

use regex_engine::data::extract::extract_s2_elab;
use regex_engine::data::prepare::{core_regex, verify_candidate};
use regex_engine::data::types::{Candidate, PreparedCase, Provenance, RuleContext, SourceKind};
use regex_engine::diagnostics::{run_parser, DiagConfig, DiagLevel};
use regex_engine::frontend::{parse_ext_pattern, parse_pcre_rule, strip_pcre_delimiters, ExtPat};
use regex_engine::parsers::ParserType;

struct RealPattern {
    raw_pattern: String,
    matches: Vec<String>,
    non_matches: Vec<String>,
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

fn contains_bound(ep: &ExtPat) -> bool {
    match ep {
        ExtPat::Bound(..) => true,
        ExtPat::Or(parts) | ExtPat::Concat(parts) => parts.iter().any(contains_bound),
        ExtPat::Group(inner)
        | ExtPat::GroupNonMarking(inner)
        | ExtPat::Opt(inner)
        | ExtPat::Plus(inner)
        | ExtPat::Star(inner) => contains_bound(inner),
        _ => false,
    }
}

fn contains_or(ep: &ExtPat) -> bool {
    match ep {
        ExtPat::Or(_) => true,
        ExtPat::Concat(parts) => parts.iter().any(contains_or),
        ExtPat::Group(inner)
        | ExtPat::GroupNonMarking(inner)
        | ExtPat::Opt(inner)
        | ExtPat::Plus(inner)
        | ExtPat::Star(inner)
        | ExtPat::Bound(inner, ..) => contains_or(inner),
        _ => false,
    }
}

fn contains_class(ep: &ExtPat) -> bool {
    match ep {
        ExtPat::Any(_) => true,
        ExtPat::Or(parts) | ExtPat::Concat(parts) => parts.iter().any(contains_class),
        ExtPat::Group(inner)
        | ExtPat::GroupNonMarking(inner)
        | ExtPat::Opt(inner)
        | ExtPat::Plus(inner)
        | ExtPat::Star(inner)
        | ExtPat::Bound(inner, ..) => contains_class(inner),
        _ => false,
    }
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

fn accepted_ep(p: &str) -> Option<ExtPat> {
    let (body, _) = strip_pcre_delimiters(p);
    accepted(p).then(|| parse_ext_pattern(body).ok()).flatten()
}

fn rejected_because(p: &str, needle: &str) -> bool {
    matches!(parse_pcre_rule(p), Err(e) if e.contains(needle))
}

// -------------------------------
// Category detectors
// -------------------------------

fn is_dot_without_s(p: &str) -> bool {
    !strip_pcre_delimiters(p).1.dot_all && accepted_ep(p).is_some_and(|ep| contains_dot(&ep))
}

fn is_bare_backslash_zero(p: &str) -> bool {
    accepted(p) && has_bare_backslash_zero(strip_pcre_delimiters(p).0)
}

fn is_bounded_quantifier(p: &str) -> bool {
    accepted_ep(p).is_some_and(|ep| contains_bound(&ep))
}

fn is_alternation(p: &str) -> bool {
    accepted_ep(p).is_some_and(|ep| contains_or(&ep))
}

fn is_character_class(p: &str) -> bool {
    accepted_ep(p).is_some_and(|ep| contains_class(&ep))
}

fn is_backreference_rejected(p: &str) -> bool {
    rejected_because(p, "backreference")
}

fn is_lookahead_rejected(p: &str) -> bool {
    rejected_because(p, "lookahead")
}

fn is_lookbehind_rejected(p: &str) -> bool {
    rejected_because(p, "lookbehind")
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

/// One of `pattern`'s own given examples, verified at the same bar
/// `dataset_prepare` applies; `None` if it doesn't hold up. Never generated.
fn verified_example(pattern: &RealPattern, text: &str, claimed_match: bool) -> Option<PreparedCase> {
    verify_candidate(
        SourceKind::S2ELab,
        &pattern.raw_pattern,
        Candidate { text: text.to_string(), provenance: Provenance::DatasetGiven, claimed_match },
    )
}

fn has_real_example(pattern: &RealPattern) -> bool {
    pattern.matches.iter().any(|t| verified_example(pattern, t, true).is_some())
        || pattern.non_matches.iter().any(|t| verified_example(pattern, t, false).is_some())
}

/// Shows the translated `Regex` run against one of the pattern's own real
/// matches and one of its own real non-matches, whichever verify.
fn demo_accepted(pattern: &RealPattern, diag: DiagLevel) {
    let core = core_regex(&pattern.raw_pattern).expect("category detector already required acceptance");
    let config = DiagConfig::new(diag, ParserType::DerivStdRec, None);

    let positive = pattern.matches.iter().find_map(|t| verified_example(pattern, t, true));
    let negative = pattern.non_matches.iter().find_map(|t| verified_example(pattern, t, false));

    for case in [positive, negative].into_iter().flatten() {
        let label = if case.verified_match { "match" } else { "non-match" };
        println!("  real {label}: {:?}", case.input);
        run_parser(&pattern.raw_pattern, &core, &case.input, &config);
    }
}

fn demo_rejected(pattern: &str) {
    let err = parse_pcre_rule(pattern).expect_err("category detector already required rejection");
    println!("rejected: {err}");
}

struct Entry {
    name: &'static str,
    detect: fn(&str) -> bool,
    kind: Kind,
}

enum Kind {
    Accepted,
    Rejected,
}

fn entries() -> Vec<Entry> {
    vec![
        Entry { name: "accepted: '.' excludes '\\n' (no flag)", detect: is_dot_without_s, kind: Kind::Accepted },
        Entry { name: "accepted: bare '\\0' outside a class", detect: is_bare_backslash_zero, kind: Kind::Accepted },
        Entry { name: "accepted: bounded quantifier '{n,m}'", detect: is_bounded_quantifier, kind: Kind::Accepted },
        Entry { name: "accepted: alternation '|'", detect: is_alternation, kind: Kind::Accepted },
        Entry { name: "accepted: character class '[...]'", detect: is_character_class, kind: Kind::Accepted },
        Entry { name: "rejected: backreference", detect: is_backreference_rejected, kind: Kind::Rejected },
        Entry { name: "rejected: lookahead", detect: is_lookahead_rejected, kind: Kind::Rejected },
        Entry { name: "rejected: lookbehind", detect: is_lookbehind_rejected, kind: Kind::Rejected },
        Entry { name: "rejected: word boundary", detect: is_word_boundary_rejected, kind: Kind::Rejected },
        Entry { name: "rejected: anchor ('^' or '$')", detect: is_anchor_rejected, kind: Kind::Rejected },
    ]
}

fn load_all_patterns(files: &[PathBuf]) -> Vec<RealPattern> {
    let mut out = Vec::new();
    for path in files {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("skipping {}: {e}", path.display());
                continue;
            }
        };
        for rule in extract_s2_elab(&text) {
            if let RuleContext::S2ELab { matches, non_matches } = rule.context {
                out.push(RealPattern { raw_pattern: rule.raw_pattern, matches, non_matches });
            }
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "usage: cargo run --release --example demo_frontend_categories -- \
             <s2-elab-json-file> [file...] [--diag 0|1|2|3]"
        );
        std::process::exit(2);
    }

    let mut diag = DiagLevel::Basic;
    let mut files: Vec<PathBuf> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
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

    for entry in entries() {
        println!("[{}]", entry.name);
        println!("{}", "-".repeat(60));

        let found = patterns
            .iter()
            .filter(|p| (entry.detect)(&p.raw_pattern))
            .filter(|p| matches!(entry.kind, Kind::Rejected) || has_real_example(p))
            .min_by_key(|p| p.raw_pattern.len());

        let Some(pattern) = found else {
            println!("  no example in this dataset -- skipped");
            println!();
            continue;
        };

        println!("  raw: {}", pattern.raw_pattern);
        match entry.kind {
            Kind::Accepted => demo_accepted(pattern, diag),
            Kind::Rejected => demo_rejected(&pattern.raw_pattern),
        }
        println!();
    }
}
