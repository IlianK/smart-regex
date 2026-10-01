//! examples/dataset_stats.rs
//!
//! cargo run --release --example dataset_stats 
//!          [--source S] [--verbose] [--no-flags] 
//!          [--no-anchors] [--no-caveats] [file...]
//!
//! Per-source corpus report: extraction counts, acceptance, rejection
//! causes, faithful/approx split, PCRE flag and anchor usage. 
//! Defaults to all three sources under `data/raw/`; explicit files need one `--source`.
//!
//! "Faithful" (from `frontend::faithfulness_gaps`) is exactly what
//! `data::run_pipeline` feeds the benchmark; a buffer-selector flag
//! (U/H/P/C/...) does not make a pattern approx. 
//! `faithful + approx == accepted`classifies the *raw* corpus
//! `dataset_prepare` keeps only the faithful subset, so `prepared.jsonl` contains no approx cases.

use std::collections::BTreeMap;
use std::path::PathBuf;

use regex_engine::data::extract::{extract_regexlib, extract_spamassassin, extract_suricata};
use regex_engine::data::types::SourceKind;
use regex_engine::frontend::{
    detect_anchors, faithfulness_gaps, parse_pcre_rule, strip_pcre_delimiters, FaithfulnessGap,
};

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

fn all_sources() -> [SourceKind; 3] {
    [
        SourceKind::Suricata,
        SourceKind::SpamAssassin,
        SourceKind::RegexLib,
    ]
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
            Cause::FlagMultiline
        } else if e.contains("extended") {
            Cause::FlagExtended
        } else {
            Cause::Other
        }
    }

    /// Fixed order for the rejection table.
    const ALL: [Cause; 6] = [
        Cause::Backreference,
        Cause::Lookaround,
        Cause::WordBoundary,
        Cause::FlagMultiline,
        Cause::FlagExtended,
        Cause::Other,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum AnchorShape {
    None,
    StartOnly,
    EndOnly,
    Both,
}

impl AnchorShape {
    fn from_anchors(start: bool, end: bool) -> Self {
        match (start, end) {
            (true, true) => AnchorShape::Both,
            (true, false) => AnchorShape::StartOnly,
            (false, true) => AnchorShape::EndOnly,
            (false, false) => AnchorShape::None,
        }
    }

    const ALL: [AnchorShape; 4] = [
        AnchorShape::None,
        AnchorShape::StartOnly,
        AnchorShape::EndOnly,
        AnchorShape::Both,
    ];

    fn label(&self) -> &'static str {
        match self {
            AnchorShape::None => "none",
            AnchorShape::StartOnly => "^ only",
            AnchorShape::EndOnly => "$ only",
            AnchorShape::Both => "^...$",
        }
    }
}

/// A property an accepted pattern may carry. Relative and NestedAnchor move
/// a pattern to approx; BufferFlag does not (see `Caveat::TABLED`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Caveat {
    /// `R`: position is relative to the rule's previous `content:` match;
    /// the frontend cannot see that from the pattern string alone.
    Relative,
    /// Suricata buffer-selector flag (U/H/P/C/...): names the buffer the
    /// rule targets, not what the pattern denotes.
    BufferFlag,
    /// `^`/`$` not present on every alternation branch; lowered to Eps,
    /// so search padding can fire it at any stopping position.
    NestedAnchor,
}

impl Caveat {
    fn label(&self) -> &'static str {
        match self {
            Caveat::Relative => "R (relative)",
            Caveat::BufferFlag => "buffer flag",
            Caveat::NestedAnchor => "nested anchor",
        }
    }

    /// Only the two that move a pattern from faithful to approx.
    const TABLED: [Caveat; 2] = [Caveat::Relative, Caveat::NestedAnchor];
}

/// What `parse_pcre_rule` does with one extracted pattern.
#[derive(Debug)]
enum Classification {
    Accepted { anchor_shape: AnchorShape },
    Rejected(Cause),
}

fn classify(pattern: &str) -> Classification {
    match parse_pcre_rule(pattern) {
        Ok(_) => {
            let (body, _flags) = strip_pcre_delimiters(pattern);
            // Cannot fail: parse_pcre_rule ran the same check.
            let (start, end) = detect_anchors(body).unwrap_or((false, false));
            Classification::Accepted {
                anchor_shape: AnchorShape::from_anchors(start, end),
            }
        }
        Err(e) => Classification::Rejected(Cause::classify(&e)),
    }
}

/// Caveats an accepted pattern carries; empty means faithful.
fn caveats_of(pattern: &str) -> Vec<Caveat> {
    let mut out: Vec<Caveat> = faithfulness_gaps(pattern)
        .into_iter()
        .map(|gap| match gap {
            FaithfulnessGap::Relative => Caveat::Relative,
            FaithfulnessGap::NestedAnchor => Caveat::NestedAnchor,
        })
        .collect();

    // Any letter outside "imsxRAE" is assumed to be a Suricata
    // buffer-selector/no-op flag; wrong for a corpus using an
    // undocumented PCRE flag, which would be silently ignored here.
    let raw = raw_flags(pattern);
    if raw
        .chars()
        .any(|c| c.is_ascii_alphabetic() && !"imsxRAE".contains(c))
    {
        out.push(Caveat::BufferFlag);
    }

    out
}

// ---------------------------------------------------------------------
// Per-source statistics
// ---------------------------------------------------------------------

#[derive(Debug, Default)]
struct SourceStats {
    source: Option<SourceKind>,
    files_read: usize,
    extracted: usize,
    accepted: usize,
    /// Accepted with no R and no nested anchor; a buffer-selector flag
    /// does not disqualify a pattern from faithful.
    faithful: usize,
    /// Accepted, but only an approximation. `faithful + approx == accepted`.
    approx: usize,
    rejected_by: BTreeMap<Cause, usize>,
    anchors: BTreeMap<AnchorShape, usize>,
    caveats: BTreeMap<Caveat, usize>,
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

    fn add_pattern(&mut self, pattern: &str, verbose: bool) {
        self.extracted += 1;

        // Flag usage is recorded for every extracted pattern, accepted or
        // not -- the point is what the corpus contains, not what survives.
        for flag in flags_of(pattern) {
            *self.flag_usage.entry(flag).or_default() += 1;
        }

        match classify(pattern) {
            Classification::Accepted { anchor_shape } => {
                self.accepted += 1;
                *self.anchors.entry(anchor_shape).or_default() += 1;
                for flag in flags_of(pattern) {
                    *self.flag_accepted.entry(flag).or_default() += 1;
                }

                let caveats = caveats_of(pattern);
                for c in &caveats {
                    *self.caveats.entry(*c).or_default() += 1;
                }
                // R or a nested anchor make the produced Regex an
                // approximation; a buffer-selector flag alone does not.
                if caveats.contains(&Caveat::Relative) || caveats.contains(&Caveat::NestedAnchor) {
                    self.approx += 1;
                } else {
                    self.faithful += 1;
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
        total.accepted += self.accepted;
        total.faithful += self.faithful;
        total.approx += self.approx;
        for (cause, n) in &self.rejected_by {
            *total.rejected_by.entry(*cause).or_default() += n;
        }
        for (shape, n) in &self.anchors {
            *total.anchors.entry(*shape).or_default() += n;
        }
        for (caveat, n) in &self.caveats {
            *total.caveats.entry(*caveat).or_default() += n;
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

/// Flag letters attached to a `/PATTERN/FLAGS` wrapper; empty for a bare
/// pattern.
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
        // Some corpora are not clean UTF-8 (SpamAssassin's locale-specific
        // files). Decode lossily rather than drop the whole file: a
        // corrupted pattern is a per-pattern problem classify() can reject.
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
    println!("{}", "=".repeat(72));
    println!("{title}");
    println!("{}", "=".repeat(72));
}

fn coverage_table(stats: &[SourceStats], total: &SourceStats) {
    header("Dataset coverage");
    println!(
        "{:<13} {:>6} {:>10} {:>9} {:>7} {:>9} {:>11} {:>12} {:>11}",
        "Source",
        "Files",
        "Extracted",
        "Faithful",
        "Approx",
        "Rejected",
        "Faithful %",
        "Faith+App %",
        "Rejected %"
    );
    println!("{}", "-".repeat(96));

    let print_row = |s: &SourceStats| {
        let pct = |n: usize| {
            if s.extracted == 0 {
                0.0
            } else {
                100.0 * n as f64 / s.extracted as f64
            }
        };
        println!(
            "{:<13} {:>6} {:>10} {:>9} {:>7} {:>9} {:>11.1} {:>12.1} {:>11.1}",
            s.name(),
            s.files_read,
            s.extracted,
            s.faithful,
            s.approx,
            s.rejected_total(),
            pct(s.faithful),
            pct(s.faithful + s.approx),
            pct(s.rejected_total()),
        );
    };

    for s in stats {
        print_row(s);
    }
    println!("{}", "-".repeat(96));
    print_row(total);
    println!();
    println!(
        "Faithful    = translated Regex denotes exactly what the pattern means\n\
         Approx      = translated Regex drops a semantic detail (R, nested anchor)\n\
         Rejected    = no Regex produced at all\n\
         Faith+App % = (Faithful + Approx) / Extracted"
    );
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

fn caveats_table(stats: &[SourceStats], total: &SourceStats, show_caveats: bool) {
    if !show_caveats {
        return;
    }
    header("Remaining caveats (accepted patterns, not mutually exclusive)");
    let caveats = Caveat::TABLED;
    print!("{:<16}", "Source");
    for c in caveats {
        print!(" {:>14}", c.label());
    }
    print!(" {:>14}", "faithful");
    print!(" {:>14}", "approx");
    println!();
    println!("{}", "-".repeat(16 + 15 * caveats.len() + 30));

    let print_row = |name: &str, s: &SourceStats| {
        print!("{:<16}", name);
        for c in caveats {
            print!(" {:>14}", s.caveats.get(&c).copied().unwrap_or(0));
        }
        print!(" {:>14}", s.faithful);
        print!(" {:>14}", s.approx);
        println!();
    };

    for s in stats {
        print_row(&s.name(), s);
    }
    println!("{}", "-".repeat(16 + 15 * caveats.len() + 30));
    print_row(&total.name(), total);
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
    print!(" {:>10} {:<40}", "Total", "Effect");
    println!();
    println!("{}", "-".repeat(6 + 15 * stats.len() + 10 + 41));

    for flag in all_flags {
        print!("{:<6}", flag);
        for s in stats {
            print!(" {:>14}", select(s).get(&flag).copied().unwrap_or(0));
        }
        print!(
            " {:>10} {:<40}",
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
        'R' => "ignored (position hint: R)",
        'A' => "honoured (anchored: treated like a leading ^)",
        'E' => "honoured (dollar-endonly: $ forbids a trailing \\n)",
        _ => "ignored (buffer selector, or unclassified)",
    }
}

/// Buffer-selector flags name the buffer a rule targets, not what the
/// pattern denotes, so they do not affect faithfulness -- but the
/// generated inputs are not necessarily shaped like that buffer.
fn buffer_flag_note(total: &SourceStats) {
    let n = total.caveats.get(&Caveat::BufferFlag).copied().unwrap_or(0);
    if n == 0 {
        return;
    }
    println!();
    println!("Note: {n} accepted patterns carry a buffer-selector flag (U, H, P, C, ...).");
    println!("These name the buffer the rule targets, not what the pattern denotes, so they");
    println!("do not affect faithfulness. They do mean the generated inputs are not guaranteed");
    println!("to be suitable for the target buffer -- a `/x/U` rule is tested against generic");
    println!("text, not a URI-normalized buffer. The pattern is right, but the input may not fit.");
}

fn anchor_table(stats: &[SourceStats], total: &SourceStats, show_anchors: bool) {
    if !show_anchors {
        return;
    }
    header("Anchor usage (accepted patterns only)");

    print!("{:<16}", "Source");
    for shape in AnchorShape::ALL {
        print!(" {:>10}", shape.label());
    }
    print!(" {:>14}", "Total anchored");
    println!();
    println!("{}", "-".repeat(16 + 11 * AnchorShape::ALL.len() + 15));

    let print_row = |name: &str, s: &SourceStats| {
        print!("{:<16}", name);
        let mut anchored = 0usize;
        for shape in AnchorShape::ALL {
            let n = s.anchors.get(&shape).copied().unwrap_or(0);
            if shape != AnchorShape::None {
                anchored += n;
            }
            print!(" {:>10}", n);
        }
        print!(" {:>14}", anchored);
        println!();
    };

    for s in stats {
        print_row(&s.name(), s);
    }
    println!("{}", "-".repeat(16 + 11 * AnchorShape::ALL.len() + 15));
    print_row(&total.name(), total);
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
// Argument parsing
// ---------------------------------------------------------------------

struct Options {
    sources: Vec<SourceKind>,
    explicit_files: Vec<PathBuf>,
    verbose: bool,
    show_flags: bool,
    show_anchors: bool,
    show_caveats: bool,
}

fn parse_args() -> Options {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut sources: Vec<SourceKind> = Vec::new();
    let mut explicit_files: Vec<PathBuf> = Vec::new();
    let mut verbose = false;
    let mut show_flags = true;
    let mut show_anchors = true;
    let mut show_caveats = true;

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
            "--no-anchors" => show_anchors = false,
            "--no-caveats" => show_caveats = false,
            other if !other.starts_with("--") => {
                // Bare positional: a source name selects it, anything
                // else is a file path (only honored for a single source).
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
        show_anchors,
        show_caveats,
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

// ---------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------

fn main() {
    let opts = parse_args();

    // Explicit files only apply with exactly one source selected: with
    // several, there is no way to know which file belongs to which.
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
    rejection_table(&per_source, &total);
    caveats_table(&per_source, &total, opts.show_caveats);
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
    if opts.show_flags {
        buffer_flag_note(&total);
    }
    anchor_table(&per_source, &total, opts.show_anchors);
    rejection_detail(&per_source, opts.verbose);
}