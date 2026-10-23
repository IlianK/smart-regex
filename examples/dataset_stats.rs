//! examples/dataset_stats.rs
//!
//! cargo run --release --example dataset_stats
//!          [--source S] [--verbose] [--no-flags] [file...]
//!
//! Per-source corpus report: extraction counts, acceptance, pattern
//! duplication, match-intent reliability, rejection causes, anchor shape
//! distribution, PCRE flag usage.
//! Defaults: Suricata, SpamAssassin under `data/raw/`.
//! Opt-in: RegexLib, S2ELab, Innovatorved.
//! RegexLib / Innovatorved: crowd-sourced, no correctness guarantee;
//! Innovatorved test text also unlabeled.
//! S2ELab: curated academic benchmark, not a maintained production rule set.
//! Explicit files require one `--source`.
//!
//!   Accepted = `core_regex_tagged` produced a Regex, using the same function
//!              and depth/node caps as `dataset_prepare` (not plain
//!              `parse_pcre_rule`). No approximate-but-accepted case; each
//!              Regex denotes exactly what it was translated from.
//!              Splits into Direct / Explicit; both are accepted and generated for.
//!   Reject   = no Regex produced. Unrepresentable construct: backreference,
//!              lookaround, \b/\B, anchor placement other than a top-level
//!              `^...$` wrap, m flag, x flag, translation exceeding pipeline
//!              safety caps, or another unsupported construct.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use regex_engine::data::extract::{
    extract_innovatorved, extract_regexlib, extract_s2_elab, extract_spamassassin,
    extract_suricata,
};
use regex_engine::data::prepare::{anchor_shape, core_regex_tagged, AnchorShape};
use regex_engine::data::types::{MatchIntent, RuleContext, SourceKind};

/// Whether an extracted row's own given input is empty. Only S2ELab and
/// Innovatorved carry such a field at all, and only S2ELab's is ever used.
fn has_no_given_input(context: &RuleContext) -> bool {
    match context {
        RuleContext::S2ELab { matches, non_matches } => matches.is_empty() && non_matches.is_empty(),
        RuleContext::Innovatorved { test_text } => {
            test_text.as_deref().map(str::trim).unwrap_or("").is_empty()
        }
        RuleContext::Suricata { .. } | RuleContext::SpamAssassin { .. } | RuleContext::RegexLib { .. } => false,
    }
}
use regex_engine::frontend::{parse_ext_pattern, strip_pcre_delimiters};

// ---------------------------------------------------------------------
// Standard corpus locations
// ---------------------------------------------------------------------

/// Default raw directory per source; every top-level file in it is read.
fn default_dir(source: SourceKind) -> &'static str {
    match source {
        SourceKind::Suricata => "data/raw/snort",
        SourceKind::SpamAssassin => "data/raw/spamAssassin",
        SourceKind::RegexLib => "data/raw/regexLib",
        SourceKind::S2ELab => "data/raw/s2-elab",
        SourceKind::Innovatorved => "data/raw/innovatorved",
    }
}

/// Default sweep when no `--source` is given. RegexLib is deliberately
/// excluded: it's crowd-sourced, with no curation or correctness guarantee.
fn all_sources() -> [SourceKind; 2] {
    [SourceKind::Suricata, SourceKind::SpamAssassin]
}

/// Every regular file directly inside `dir`, sorted for determinism.
fn files_in(dir: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            out.push(path);
        }
    }
    out.sort();
    out
}

// ---------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Cause {
    Backreference,
    Lookaround,
    WordBoundary,
    Anchor,
    FlagMultiline,
    FlagExtended,
    Other,
}

impl Cause {
    fn label(&self) -> &'static str {
        match self {
            Cause::Backreference => "backreference",
            Cause::Lookaround => "lookaround",
            Cause::WordBoundary => "word-boundary",
            Cause::Anchor => "anchor",
            Cause::FlagMultiline => "flag-m",
            Cause::FlagExtended => "flag-x",
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
            // Checked before "anchor": the multiline rejection's own message
            // mentions anchors too, so checking "anchor" first would misclassify it.
            Cause::FlagMultiline
        } else if e.contains("extended") {
            Cause::FlagExtended
        } else if e.contains("anchor") {
            Cause::Anchor
        } else {
            Cause::Other
        }
    }

    /// Fixed order for the rejection table.
    const ALL: [Cause; 7] = [
        Cause::Backreference,
        Cause::Lookaround,
        Cause::WordBoundary,
        Cause::Anchor,
        Cause::FlagMultiline,
        Cause::FlagExtended,
        Cause::Other,
    ];
}

/// What `core_regex_tagged` does with one extracted pattern.
#[derive(Debug)]
enum Classification {
    Accepted(MatchIntent),
    Rejected(Cause),
}

fn classify(pattern: &str) -> Classification {
    match core_regex_tagged(pattern) {
        Ok((_, intent)) => Classification::Accepted(intent),
        Err(e) => Classification::Rejected(Cause::classify(&e)),
    }
}

// ---------------------------------------------------------------------
// Per-source statistics
// ---------------------------------------------------------------------

#[derive(Debug, Default)]
struct SourceStats {
    source: Option<SourceKind>,
    files_read: usize,
    extracted: usize,
    /// `ExplicitFullMatch`: pattern wrapped itself in `^...$`,
    /// anchors stripped before translation.
    accepted_explicit: usize,
    /// `UnanchoredAmbiguous`: no anchor; translated and benchmarked as a
    /// full match -- that reading is this project's own choice.
    accepted_ambiguous: usize,
    /// Of the accepted patterns, how many have empty given input
    /// (see `has_no_given_input`); always 0 for sources without one.
    accepted_no_input: usize,
    rejected_by: BTreeMap<Cause, usize>,
    /// Where `^`/`$` sit structurally across every extracted pattern,
    /// independent of accept/reject; explains the anchor-related rows above.
    anchor_shapes: BTreeMap<AnchorShape, usize>,
    flag_usage: BTreeMap<char, usize>,
    flag_accepted: BTreeMap<char, usize>,
    /// Distinct raw pattern text among every extracted pattern, accepted
    /// or not -- a source's own redundancy, independent of translatability.
    extracted_seen: HashSet<String>,
    /// Distinct raw pattern text among ACCEPTED patterns only -- the
    /// subset `dataset_prepare` dedups, so the one that affects weighting.
    accepted_seen: HashSet<String>,
    /// Subset of `accepted_seen` tagged `ExplicitFullMatch`, deduplicated
    /// the same way, unlike `accepted_explicit` which counts rule instances.
    accepted_explicit_seen: HashSet<String>,
    /// Subset of `accepted_seen` tagged `MatchIntent::UnanchoredAmbiguous`.
    accepted_ambiguous_seen: HashSet<String>,
    /// Populated only when --verbose.
    rejections: Vec<(String, Cause)>,
}

impl SourceStats {
    fn new(source: Option<SourceKind>) -> Self {
        SourceStats {
            source,
            ..Default::default()
        }
    }

    fn name(&self) -> String {
        match self.source {
            Some(s) => s.dir_name().to_string(),
            None => "total".to_string(),
        }
    }

    fn accepted(&self) -> usize {
        self.accepted_explicit + self.accepted_ambiguous
    }

    fn distinct_extracted(&self) -> usize {
        self.extracted_seen.len()
    }

    fn duplicate_extracted(&self) -> usize {
        self.extracted.saturating_sub(self.distinct_extracted())
    }

    fn distinct_accepted(&self) -> usize {
        self.accepted_seen.len()
    }

    /// Distinct accepted patterns tagged `ExplicitFullMatch`, unlike
    /// `accepted_explicit` which counts rule instances, not distinct patterns.
    fn distinct_accepted_explicit(&self) -> usize {
        self.accepted_explicit_seen.len()
    }

    /// Distinct accepted patterns tagged `UnanchoredAmbiguous` -- matches
    /// `dataset_prepare`'s own `patterns_unanchored_ambiguous`.
    fn distinct_accepted_ambiguous(&self) -> usize {
        self.accepted_ambiguous_seen.len()
    }

    fn duplicate_accepted(&self) -> usize {
        self.accepted().saturating_sub(self.distinct_accepted())
    }

    fn add_pattern(&mut self, pattern: &str, context: &RuleContext, verbose: bool) {
        self.extracted += 1;
        self.extracted_seen.insert(pattern.to_string());

        // Flags recorded for every extracted pattern, accepted or not.
        // The point is what the corpus contains, not what survives.
        for flag in flags_of(pattern) {
            *self.flag_usage.entry(flag).or_default() += 1;
        }

        // Anchor shape likewise computed independently of accept/reject, so
        // this always sums to `extracted` -- even an unparseable pattern gets a row.
        let (body, _flags) = strip_pcre_delimiters(pattern);
        let shape = parse_ext_pattern(body).map(|ep| anchor_shape(&ep)).unwrap_or(AnchorShape::Unparseable);
        *self.anchor_shapes.entry(shape).or_default() += 1;

        match classify(pattern) {
            Classification::Accepted(intent) => {
                self.accepted_seen.insert(pattern.to_string());
                match intent {
                    MatchIntent::ExplicitFullMatch => {
                        self.accepted_explicit += 1;
                        self.accepted_explicit_seen.insert(pattern.to_string());
                    }
                    MatchIntent::UnanchoredAmbiguous => {
                        self.accepted_ambiguous += 1;
                        self.accepted_ambiguous_seen.insert(pattern.to_string());
                    }
                }
                if has_no_given_input(context) {
                    self.accepted_no_input += 1;
                }
                for flag in flags_of(pattern) {
                    *self.flag_accepted.entry(flag).or_default() += 1;
                }
            }
            Classification::Rejected(cause) => {
                *self.rejected_by.entry(cause).or_default() += 1;
                if verbose {
                    self.rejections.push((pattern.to_string(), cause));
                }
            }
        }
    }

    fn merge_into(&self, total: &mut SourceStats) {
        total.files_read += self.files_read;
        total.extracted += self.extracted;
        total.accepted_explicit += self.accepted_explicit;
        total.accepted_ambiguous += self.accepted_ambiguous;
        total.accepted_no_input += self.accepted_no_input;
        for (cause, n) in &self.rejected_by {
            *total.rejected_by.entry(*cause).or_default() += n;
        }
        for (shape, n) in &self.anchor_shapes {
            *total.anchor_shapes.entry(*shape).or_default() += n;
        }
        for (flag, n) in &self.flag_usage {
            *total.flag_usage.entry(*flag).or_default() += n;
        }
        for (flag, n) in &self.flag_accepted {
            *total.flag_accepted.entry(*flag).or_default() += n;
        }
        // True union, not a sum of per-source counts: a pattern shared across
        // two selected sources should still count once in the combined total.
        total.extracted_seen.extend(self.extracted_seen.iter().cloned());
        total.accepted_seen.extend(self.accepted_seen.iter().cloned());
        total.accepted_explicit_seen.extend(self.accepted_explicit_seen.iter().cloned());
        total.accepted_ambiguous_seen.extend(self.accepted_ambiguous_seen.iter().cloned());
    }

    fn rejected_total(&self) -> usize {
        self.rejected_by.values().sum()
    }
}

/// Flag letters attached to a `/PATTERN/FLAGS` wrapper.
/// Empty for a bare pattern.
fn flags_of(pattern: &str) -> Vec<char> {
    raw_flags(pattern)
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect()
}

/// The raw FLAGS part of `/PATTERN/FLAGS`, or `""` if not wrapped that way.
fn raw_flags(pattern: &str) -> &str {
    let s = pattern.trim();
    if !s.starts_with('/') {
        return "";
    }
    let Some(end) = s.rfind('/') else {
        return "";
    };
    if end == 0 {
        return "";
    }
    &s[end + 1..]
}

// ---------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------

fn read_source(source: SourceKind, paths: &[PathBuf], verbose: bool) -> SourceStats {
    let mut stats = SourceStats::new(Some(source));
    let files = if paths.is_empty() {
        files_in(default_dir(source))
    } else {
        paths.to_vec()
    };

    if files.is_empty() {
        eprintln!(
            "warning: no input files for {} (looked in {})",
            source.dir_name(),
            default_dir(source)
        );
    }

    for path in &files {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("skipping {}: {e}", path.display());
                continue;
            }
        };
        // Some corpora aren't clean UTF-8 (SpamAssassin's locale files); decode
        // lossily rather than drop the file -- a corrupted pattern just gets rejected.
        let text = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "warning: {} is not valid UTF-8, decoding lossily rather than \
                     skipping the file",
                    path.display()
                );
                String::from_utf8_lossy(e.as_bytes()).into_owned()
            }
        };
        stats.files_read += 1;
        let extracted = match source {
            SourceKind::Suricata => extract_suricata(&text),
            SourceKind::SpamAssassin => extract_spamassassin(&text),
            SourceKind::RegexLib => extract_regexlib(&text),
            SourceKind::S2ELab => extract_s2_elab(&text),
            SourceKind::Innovatorved => extract_innovatorved(&text),
        };
        for rule in extracted {
            stats.add_pattern(&rule.raw_pattern, &rule.context, verbose);
        }
    }
    stats
}

// ---------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------

fn header(title: &str) {
    println!();
    println!("{title}");
    println!("{}", "─".repeat(72));
}

fn coverage_table(stats: &[SourceStats], total: &SourceStats) {
    header("Dataset coverage");
    println!(
        "{:<13} {:>6} {:>9} {:>9} {:>7} {:>9} {:>7}",
        "Source", "Files", "Extracted", "Accepted", "%", "Rejected", "%",
    );
    println!("{}", "-".repeat(60));

    let print_row = |s: &SourceStats| {
        let pct = |n: usize| {
            if s.extracted == 0 {
                0.0
            } else {
                100.0 * n as f64 / s.extracted as f64
            }
        };
        println!(
            "{:<13} {:>6} {:>9} {:>9} {:>7.1} {:>9} {:>7.1}",
            s.name(),
            s.files_read,
            s.extracted,
            s.accepted(),
            pct(s.accepted()),
            s.rejected_total(),
            pct(s.rejected_total()),
        );
    };

    for s in stats {
        print_row(s);
    }
    println!("{}", "-".repeat(60));
    print_row(total);
    println!();
    println!("Accepted = core_regex_tagged produced a Regex.");
    println!("Rejected = no Regex produced: unsupported construct.");
    println!("Accepted + Rejected sum to Extracted; each % is against that.");

    if stats.iter().any(|s| s.accepted_no_input > 0) {
        println!();
        println!("Accepted but with no input in the source file:");
        for s in stats {
            if s.accepted_no_input > 0 {
                println!(
                    "  {:<13} {:>6} of {:>6} accepted",
                    s.name(),
                    s.accepted_no_input,
                    s.accepted()
                );
            }
        }
        println!();
        println!("Note: only S2ELab's given `matches`/`non_matches` reach a benchmark;");
        println!("a row with no given input here, from S2ELab or any other source, yields none.");
    }
}

fn duplication_table(stats: &[SourceStats], total: &SourceStats) {
    header("Pattern duplication (byte-identical raw pattern text)");
    println!(
        "{:<13} {:>9} {:>8} {:>6} {:>6} {:>6} {:>9} {:>8} {:>6} {:>6} {:>6}",
        "Source", "Extract.", "Distinct", "Dist%", "Dup", "Dup%", "Accept.", "Distinct", "Dist%", "Dup", "Dup%",
    );
    println!("{}", "-".repeat(93));

    let pct = |n: usize, of: usize| if of == 0 { 0.0 } else { 100.0 * n as f64 / of as f64 };

    let print_row = |s: &SourceStats| {
        println!(
            "{:<13} {:>9} {:>8} {:>6.1} {:>6} {:>6.1} {:>9} {:>8} {:>6.1} {:>6} {:>6.1}",
            s.name(),
            s.extracted,
            s.distinct_extracted(),
            pct(s.distinct_extracted(), s.extracted),
            s.duplicate_extracted(),
            pct(s.duplicate_extracted(), s.extracted),
            s.accepted(),
            s.distinct_accepted(),
            pct(s.distinct_accepted(), s.accepted()),
            s.duplicate_accepted(),
            pct(s.duplicate_accepted(), s.accepted()),
        );
    };

    for s in stats {
        print_row(s);
    }
    println!("{}", "-".repeat(93));
    print_row(total);
    println!();
    println!("Extract = every extracted pattern regardless of translatability");
    println!("Distinct = count of unique raw pattern text");
    println!("Dup = Duplicates");

}

fn match_intent_table(stats: &[SourceStats], total: &SourceStats) {
    header("Accepted by match-intent reliability");
    println!(
        "{:<13} {:>10} {:>7} {:>14} {:>7}",
        "Source", "UnanchoredAmbiguous", "%", "ExplicitFullMatch ^...$", "%",
    );
    println!("{}", "-".repeat(56));

    let print_row = |s: &SourceStats| {
        let distinct = s.distinct_accepted();
        let pct = |n: usize| {
            if distinct == 0 {
                0.0
            } else {
                100.0 * n as f64 / distinct as f64
            }
        };
        println!(
            "{:<13} {:>10} {:>7.1} {:>14} {:>7.1}",
            s.name(),
            s.distinct_accepted_ambiguous(),
            pct(s.distinct_accepted_ambiguous()),
            s.distinct_accepted_explicit(),
            pct(s.distinct_accepted_explicit()),
        );
    };

    for s in stats {
        print_row(s);
    }
    println!("{}", "-".repeat(56));
    print_row(total);
    println!();
    println!("Counts are by DISTINCT accepted patterns");
    println!("Direct         = no anchor; translated as-is.");
    println!("Explicit ^...$ = top-level wrap, anchors stripped; any other placement is rejected.");
    println!();
}

fn rejection_table(stats: &[SourceStats], total: &SourceStats) {
    header("Rejection reasons");
    let causes = Cause::ALL;
    print!("{:<16}", "Source");
    for c in causes {
        print!(" {:>12}", c.label());
    }
    println!();
    println!("{}", "-".repeat(16 + 13 * causes.len()));

    let print_row = |name: &str, s: &SourceStats| {
        print!("{:<16}", name);
        for c in causes {
            print!(" {:>12}", s.rejected_by.get(&c).copied().unwrap_or(0));
        }
        println!();
    };

    for s in stats {
        print_row(&s.name(), s);
    }
    println!("{}", "-".repeat(16 + 13 * causes.len()));
    print_row(&total.name(), total);
}

fn anchor_shape_table(stats: &[SourceStats], total: &SourceStats) {
    header("Anchor shape (every extracted pattern, accepted or not)");
    let shapes = AnchorShape::ALL;
    print!("{:<16}", "Source");
    for s in shapes {
        print!(" {:>14}", s.label());
    }
    println!();
    println!("{}", "-".repeat(16 + 15 * shapes.len()));

    let print_row = |name: &str, s: &SourceStats| {
        print!("{:<16}", name);
        for shape in shapes {
            print!(" {:>14}", s.anchor_shapes.get(&shape).copied().unwrap_or(0));
        }
        println!();
    };

    for s in stats {
        print_row(&s.name(), s);
    }
    println!("{}", "-".repeat(16 + 15 * shapes.len()));
    print_row(&total.name(), total);
    println!();
    println!("Unparseable   = rejected before an ExtPat even exists.");
    println!("Fully-wrapped = recovered by core_regex_tagged");
    println!("start/end/nested = rejected");
}

fn flag_table(
    stats: &[SourceStats],
    total: &SourceStats,
    show_flags: bool,
    title: &str,
    select: impl Fn(&SourceStats) -> &BTreeMap<char, usize>,
) {
    if !show_flags {
        return;
    }
    header(title);

    let mut all_flags: Vec<char> = select(total).keys().copied().collect();
    all_flags.sort_unstable();

    if all_flags.is_empty() {
        println!("(no /.../FLAGS wrappers found)");
        return;
    }

    print!("{:<6}", "Flag");
    for s in stats {
        print!(" {:>14}", s.name());
    }
    print!(" {:>10} {:<48}", "Total", "Effect");
    println!();
    println!("{}", "-".repeat(6 + 15 * stats.len() + 10 + 49));

    for flag in all_flags {
        print!("{:<6}", flag);
        for s in stats {
            print!(" {:>14}", select(s).get(&flag).copied().unwrap_or(0));
        }
        print!(
            " {:>10} {:<48}",
            select(total).get(&flag).copied().unwrap_or(0),
            flag_effect(flag)
        );
        println!();
    }
}

fn flag_effect(flag: char) -> &'static str {
    match flag {
        'i' => "honoured (case-fold)",
        's' => "honoured (dotall: . matches \\n)",
        'm' => "REJECTED (multiline)",
        'x' => "REJECTED (extended)",
        'R' => "ignored (no search left to position relative to)",
        'A' => "ignored (anchors are rejected outright, not honoured)",
        'E' => "ignored ($ is rejected outright, nothing left to tune)",
        _ => "ignored (buffer selector, or unclassified)",
    }
}

// ---------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------

struct Options {
    sources: Vec<SourceKind>,
    explicit_files: Vec<PathBuf>,
    verbose: bool,
    show_flags: bool,
}

fn parse_args() -> Options {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut sources: Vec<SourceKind> = Vec::new();
    let mut explicit_files: Vec<PathBuf> = Vec::new();
    let mut verbose = false;
    let mut show_flags = true;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--source" => {
                i += 1;
                let Some(s) = args.get(i) else {
                    eprintln!("--source needs a value");
                    std::process::exit(2);
                };
                sources.push(parse_source(s));
            }
            "--verbose" => verbose = true,
            "--no-flags" => show_flags = false,
            other if !other.starts_with("--") => {
                // Bare positional: a source name selects it, anything else is
                // a file path, honored only for a single source.
                if let Ok(s) = std::str::from_utf8(other.as_bytes()) {
                    if is_source_name(s) {
                        sources.push(parse_source(s));
                        i += 1;
                        continue;
                    }
                }
                explicit_files.push(PathBuf::from(other));
            }
            other => {
                eprintln!("unknown argument {other:?}");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    if sources.is_empty() {
        sources = all_sources().to_vec();
    }

    Options {
        sources,
        explicit_files,
        verbose,
        show_flags,
    }
}

fn is_source_name(s: &str) -> bool {
    matches!(
        s.to_lowercase().as_str(),
        "suricata" | "snort" | "spamassassin" | "regexlib" | "s2-elab" | "s2elab" | "innovatorved"
    )
}

fn parse_source(s: &str) -> SourceKind {
    match s.to_lowercase().as_str() {
        "suricata" | "snort" => SourceKind::Suricata,
        "spamassassin" => SourceKind::SpamAssassin,
        "regexlib" => SourceKind::RegexLib,
        "s2-elab" | "s2elab" => SourceKind::S2ELab,
        "innovatorved" => SourceKind::Innovatorved,
        other => {
            eprintln!(
                "unknown source {other:?} (expected suricata, spamassassin, regexlib, s2-elab, or innovatorved)"
            );
            std::process::exit(2);
        }
    }
}

fn rejection_detail(stats: &[SourceStats], verbose: bool) {
    if !verbose {
        return;
    }
    header("Per-pattern rejections");
    for s in stats {
        if s.rejections.is_empty() {
            continue;
        }
        println!();
        println!("{} ({} rejections)", s.name(), s.rejections.len());
        println!("{}", "-".repeat(72));
        for (pattern, cause) in &s.rejections {
            let shown: String = pattern.chars().take(58).collect();
            println!("{:<14} {}", cause.label(), shown);
        }
    }
}

// ---------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------

fn main() {
    let opts = parse_args();

    // Explicit files only apply with exactly one source selected.
    // With several, there is no way to know which file belongs to which.
    let explicit_for_one = opts.sources.len() == 1 && !opts.explicit_files.is_empty();
    if !opts.explicit_files.is_empty() && !explicit_for_one {
        eprintln!(
            "warning: explicit file paths are only used with a single --source; \
             ignoring {} path(s) and reading the standard directories instead",
            opts.explicit_files.len()
        );
    }

    let mut per_source: Vec<SourceStats> = Vec::new();
    let mut total = SourceStats::new(None);

    for source in &opts.sources {
        let files: &[PathBuf] = if explicit_for_one {
            &opts.explicit_files
        } else {
            &[]
        };
        let stats = read_source(*source, files, opts.verbose);
        stats.merge_into(&mut total);
        per_source.push(stats);
    }

    coverage_table(&per_source, &total);
    duplication_table(&per_source, &total);
    match_intent_table(&per_source, &total);
    rejection_table(&per_source, &total);
    anchor_shape_table(&per_source, &total);
    flag_table(
        &per_source,
        &total,
        opts.show_flags,
        "PCRE flag distribution (all extracted patterns)",
        |s| &s.flag_usage,
    );
    flag_table(
        &per_source,
        &total,
        opts.show_flags,
        "PCRE flag distribution (accepted patterns only)",
        |s| &s.flag_accepted,
    );
    rejection_detail(&per_source, opts.verbose);
}