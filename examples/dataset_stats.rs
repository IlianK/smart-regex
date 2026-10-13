//! examples/dataset_stats.rs
//!
//! cargo run --release --example dataset_stats
//!          [--source S] [--verbose] [--no-flags] [file...]
//!
//! Per-source corpus report.
//! Covers: extraction counts, acceptance, match-intent reliability,
//! rejection causes, anchor shape distribution, PCRE flag usage.
//! Defaults to Suricata and SpamAssassin under `data/raw/`.
//! RegexLib is opt-in only (`--source regexlib`).
//! It is crowd-sourced, with no correctness guarantee.
//! The other two sources' rule sets are maintained.
//! Explicit files need one `--source`.
//!
//!   Accepted = `core_regex_tagged` produced a Regex.
//!              Same function and same depth/node caps `dataset_prepare`
//!              benchmarks against, not plain `parse_pcre_rule`.
//!              No approximate-but-accepted case exists.
//!              Every accepted pattern's Regex denotes exactly what it
//!              was translated from.
//!              Accepted splits into two kinds: Direct / Explicit
//!              Both are accepted and generated for.
//!   Reject   = no Regex produced at all.
//!              A construct this frontend cannot represent:
//!              backreference, lookaround, \b/\B, an anchor placement
//!              other than a top-level `^...$` wrap, the m flag, the x
//!              flag, a translation too large for the pipeline's safety
//!              caps, or another unsupported construct.

use std::collections::BTreeMap;
use std::path::PathBuf;

use regex_engine::data::extract::{extract_regexlib, extract_spamassassin, extract_suricata};
use regex_engine::data::prepare::{anchor_shape, core_regex_tagged, AnchorShape};
use regex_engine::data::types::{MatchIntent, SourceKind};
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
    }
}

/// Default sweep when no `--source` is given.
/// RegexLib is deliberately not in this list.
/// It is crowd-sourced (regexlib.com via the rxxr2 mirror).
/// It has no curation or correctness guarantee.
/// Suricata's and SpamAssassin's maintained rule sets do.
/// So RegexLib is opt-in only.
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
            // Checked before "anchor": the multiline rejection mentions
            // anchors in its own explanation.
            // Checking "anchor" first would misclassify every multiline
            // rejection.
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
    /// MatchIntent::ExplicitFullMatch.
    /// Pattern wrapped its whole self in `^...$`.
    /// Anchors stripped before translation.
    accepted_explicit: usize,
    /// MatchIntent::UnanchoredAmbiguous.
    /// No anchor at all; translated and benchmarked as a full match.
    /// That reading is this project's own choice.
    /// Not necessarily the source's.
    accepted_ambiguous: usize,
    rejected_by: BTreeMap<Cause, usize>,
    /// Where `^`/`$` sit structurally across *every* extracted pattern.
    /// Independent of accept/reject.
    /// This explains the "anchor" row of `rejected_by`.
    /// It also explains the `accepted_explicit` count.
    anchor_shapes: BTreeMap<AnchorShape, usize>,
    flag_usage: BTreeMap<char, usize>,
    flag_accepted: BTreeMap<char, usize>,
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

    fn add_pattern(&mut self, pattern: &str, verbose: bool) {
        self.extracted += 1;

        // Flags recorded for every extracted pattern, accepted or not.
        // The point is what the corpus contains, not what survives.
        for flag in flags_of(pattern) {
            *self.flag_usage.entry(flag).or_default() += 1;
        }

        // Anchor shape likewise computed independently of accept/reject.
        // The point is what the corpus contains, including shapes that
        // go on to be rejected.
        // A pattern that fails to parse at all still gets a row.
        // (Backreference, lookaround, or a syntax error.)
        // So this always sums to `extracted`.
        let (body, _flags) = strip_pcre_delimiters(pattern);
        let shape = parse_ext_pattern(body).map(|ep| anchor_shape(&ep)).unwrap_or(AnchorShape::Unparseable);
        *self.anchor_shapes.entry(shape).or_default() += 1;

        match classify(pattern) {
            Classification::Accepted(intent) => {
                match intent {
                    MatchIntent::ExplicitFullMatch => self.accepted_explicit += 1,
                    MatchIntent::UnanchoredAmbiguous => self.accepted_ambiguous += 1,
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
        // Some corpora are not clean UTF-8 (SpamAssassin's locale files).
        // Decode lossily rather than drop the whole file.
        // A corrupted pattern is a per-pattern problem classify() can
        // reject.
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
        };
        for rule in extracted {
            stats.add_pattern(&rule.raw_pattern, verbose);
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
}

fn match_intent_table(stats: &[SourceStats], total: &SourceStats) {
    header("Accepted by match-intent reliability");
    println!(
        "{:<13} {:>10} {:>7} {:>14} {:>7}",
        "Source", "Direct", "%", "Explicit ^...$", "%",
    );
    println!("{}", "-".repeat(56));

    let print_row = |s: &SourceStats| {
        let pct = |n: usize| {
            if s.accepted() == 0 {
                0.0
            } else {
                100.0 * n as f64 / s.accepted() as f64
            }
        };
        println!(
            "{:<13} {:>10} {:>7.1} {:>14} {:>7.1}",
            s.name(),
            s.accepted_ambiguous,
            pct(s.accepted_ambiguous),
            s.accepted_explicit,
            pct(s.accepted_explicit),
        );
    };

    for s in stats {
        print_row(s);
    }
    println!("{}", "-".repeat(56));
    print_row(total);
    println!();
    println!("Direct         = no anchor; translated as-is.");
    println!("Explicit ^...$ = top-level ^...$ wrap; anchors stripped as redundant.");
    println!("                 Both tiers are accepted and generated for.");
    println!("Any other anchor placement is rejected (see Rejection reasons).");
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
    println!("Rows sum to Extracted.");
    println!("Unparseable   = rejected before an ExtPat even exists.");
    println!("Fully-wrapped = recovered by core_regex_tagged (Accepted tier).");
    println!("start/end/nested = unrecovered constraint; see Rejection reasons.");
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
                // Bare positional: a source name selects it.
                // Anything else is a file path, only honored for a
                // single source.
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
        "suricata" | "snort" | "spamassassin" | "regexlib"
    )
}

fn parse_source(s: &str) -> SourceKind {
    match s.to_lowercase().as_str() {
        "suricata" | "snort" => SourceKind::Suricata,
        "spamassassin" => SourceKind::SpamAssassin,
        "regexlib" => SourceKind::RegexLib,
        other => {
            eprintln!("unknown source {other:?} (expected suricata, spamassassin, or regexlib)");
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