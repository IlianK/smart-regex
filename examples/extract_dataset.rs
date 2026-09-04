//! regex-engine/examples/extract_dataset.rs

use regex_engine::frontend::parse_pcre_rule;
use std::collections::HashSet;
use std::fs;
use std::io::Write;

#[derive(Debug, Default)]
struct Tally {
    total: usize,
    ok: usize,
    backreference: usize,
    lookaround: usize,
    other_rejected: usize,
}

fn classify_rejection(msg: &str) -> &'static str {
    if msg.contains("backreference") {
        "backreference"
    } else if msg.contains("lookahead") || msg.contains("lookbehind") {
        "lookaround"
    } else {
        "other"
    }
}

/// Suricata/Snort `.rules`: one rule per line, `pcre:"/PATTERN/FLAGS";`
fn extract_suricata(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(start) = line.find("pcre:\"") else { continue };
        let rest = &line[start + 6..];
        // Suricata's value is quoted -- require the closing '"' right
        // after the flag run, or this isn't a well-formed pcre: option.
        if let Some(end) = find_pcre_span_end(rest) {
            if rest.as_bytes().get(end) == Some(&b'"') {
                out.push(rest[..end].to_string());
            }
        }
    }
    out
}

/// Finds the end of a `/PATTERN/FLAGS` span at the start of `s`,
/// respecting escaped internal slashes (`\/`) 
fn find_pcre_span_end(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'/') {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() && bytes[i] != b'/' {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
        } else {
            i += 1;
        }
    }
    if i >= bytes.len() {
        return None; // no closing '/' found
    }
    i += 1; // past the closing '/'
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    Some(i)
}

/// SpamAssassin `.cf`: `body`/`header` rules, `Field =~ /PATTERN/FLAGS`.
/// Skips commented-out lines (`#header ...`).
fn extract_spamassassin(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        if !(trimmed.starts_with("body ") || trimmed.starts_with("header ")) {
            continue;
        }
        let Some(op) = line.find("=~") else { continue };
        let rest = line[op + 2..].trim_start();
        // Unlike Suricata's quoted value, nothing has to follow the flag
        // run here -- the pattern just ends at whatever comes next
        // (end of line, trailing whitespace/comment).
        if let Some(end) = find_pcre_span_end(rest) {
            out.push(rest[..end].to_string());
        }
    }
    out
}

/// RegexLib's comment-delimited corpus: blank-line-separated entries
fn extract_regexlib(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: cargo run --example extract_dataset -- <suricata|spamassassin|regexlib> <file> [file...]");
        std::process::exit(2);
    }
    let format = args[1].as_str();
    let files = &args[2..];

    let mut patterns: Vec<String> = Vec::new();
    for path in files {
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("skipping {}: {}", path, e);
                continue;
            }
        };
        let extracted = match format {
            "suricata" => extract_suricata(&text),
            "spamassassin" => extract_spamassassin(&text),
            "regexlib" => extract_regexlib(&text),
            other => {
                eprintln!("unknown format {:?} (expected suricata, spamassassin, or regexlib)", other);
                std::process::exit(2);
            }
        };
        println!("{}: {} pattern(s) extracted", path, extracted.len());
        patterns.extend(extracted);
    }

    let mut tally = Tally { total: patterns.len(), ..Default::default() };
    let mut accepted: Vec<String> = Vec::new();
    let mut examples: Vec<(String, String, &'static str)> = Vec::new(); // (pattern, error, category)

    for p in &patterns {
        match parse_pcre_rule(p) {
            Ok(_) => {
                tally.ok += 1;
                accepted.push(p.clone());
            }
            Err(e) => {
                let cat = classify_rejection(&e);
                match cat {
                    "backreference" => tally.backreference += 1,
                    "lookaround" => tally.lookaround += 1,
                    _ => tally.other_rejected += 1,
                }
                if examples.len() < 8 {
                    examples.push((p.clone(), e, cat));
                }
            }
        }
    }

    // Dedupe (some datasets repeat identical patterns across rules) and append to corpus file.
    let existing: HashSet<String> = fs::read_to_string("data/corpus_patterns.txt")
        .unwrap_or_default()
        .lines()
        .map(|s| s.to_string())
        .collect();
    let mut new_unique: Vec<&String> = accepted
        .iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .filter(|p| !existing.contains(*p))
        .collect();
    new_unique.sort();

    if !new_unique.is_empty() {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("data/corpus_patterns.txt")
            .expect("failed to open data/corpus_patterns.txt for appending");
        for p in &new_unique {
            writeln!(f, "{}", p).expect("write failed");
        }
    }

    println!();
    println!("=== Triage summary ({}) ===", format);
    println!("Total extracted:        {}", tally.total);
    println!("Accepted (parseable):   {}", tally.ok);
    println!("Rejected -- backreference: {}", tally.backreference);
    println!("Rejected -- lookaround:    {}", tally.lookaround);
    println!("Rejected -- other:         {}", tally.other_rejected);
    println!("New unique -> data/corpus_patterns.txt: {}", new_unique.len());

    if !examples.is_empty() {
        println!();
        println!("Sample rejections:");
        for (p, e, cat) in examples.iter().take(5) {
            let shown = if p.chars().count() > 70 {
                format!("{}...", p.chars().take(70).collect::<String>())
            } else {
                p.clone()
            };
            println!("  [{}] {} -- {}", cat, shown, e);
        }
    }
}
