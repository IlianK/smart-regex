//! regex-engine/examples/filter_dataset.rs
//!
//! Reads the standard rule corpora (Suricata, SpamAssassin, RegexLib) and
//! reports a coverage comparison: how many patterns each source yields,
//! how many the frontend accepts, how the rejections and PCRE flags break
//! down, and -- separately from "accepted" -- how many of the accepted
//! patterns are "faithful" (the produced `Regex` exactly denotes what the
//! source pattern means) versus "approx" (accepted, but a known
//! simplification was applied: a relative-match flag, or an anchor nested
//! inside a non-fully-anchored alternation). A bare `.` without the
//! dotall flag used to be a third case -- this engine let `.` match `\n`
//! regardless of the flag -- but `parse_pcre_rule` now excludes `\n` from
//! `.` whenever `s` is absent (`frontend::strip_dot_newline`), so it no
//! longer needs tracking here: it is a real engine fix, not a documented
//! approximation.
//!
//! A buffer-selector flag (`U`, `H`, `P`, `C`, ...) does not make a
//! pattern "approx": it names which buffer the rule is tested against,
//! not what the pattern denotes, so the translated `Regex` is still
//! faithful. What it does affect is whether the generated inputs are
//! *suitable* for the target buffer, which is a question about input
//! relevance rather than translation fidelity. That distinction is
//! stated once, in the note printed under the accepted-patterns flag
//! table; the caveats table lists only the two caveats that actually
//! move a pattern from faithful to approx.
//!
//! "Accepted" only means `parse_pcre_rule` did not hard-reject the
//! pattern; `faithful + approx == accepted`, always.
//!
//! The faithful/approx split here is read from `frontend::faithfulness_gaps`,
//! the same function `data::run_pipeline` calls to decide what to leave
//! out of the real dataset (`data/processed/<source>/prepared.jsonl`
//! contains only faithful patterns, as of the review that drew this
//! line: everything else is now either rejected outright or excluded as
//! a known approximation, never silently included). So this report's
//! "Faithful" row is not just a statistic about the corpus, it is
//! exactly the set of patterns the real benchmark is built from.
//!
//! Default: reads all three sources from their standard locations under
//! `data/raw/`. `--source S` restricts to one. Explicit file paths on the
//! command line override the standard locations for whichever sources
//! they name.
//!
//! Usage:
//!   cargo run --release --example filter_dataset
//!   cargo run --release --example filter_dataset -- --source snort
//!   cargo run --release --example filter_dataset -- snort data/raw/snort/*.rules
//!
//! Flags:
//!   --source S    restrict to one source (suricata, spamassassin, regexlib)
//!   --verbose     list each rejected pattern with its cause
//!   --no-anchors  skip the anchor-usage table
//!   --no-flags    skip the PCRE flag-distribution tables and their note
//!   --no-caveats  skip the remaining-caveats table

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

/// Where each source's raw files live by default. Matching is by
/// directory: every file in the directory (top level only) is read.
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

    /// Every variant, so tables can iterate in a fixed order.
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

/// A property an *accepted* pattern can carry. `Relative` and
/// `NestedAnchor` move a pattern from `faithful` to `approx`;
/// `BufferFlag` does not, and is deliberately excluded from the caveats
/// table (see `Caveat::TABLED` and `buffer_flag_note`). The variant stays
/// so `caveats_of` can still record its count in `SourceStats::caveats`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Caveat {
    /// `R`: match position is relative to the rule's previous
    /// `content:` match. The frontend sees only the pattern string, not
    /// the surrounding rule, so it cannot see that constraint; the
    /// pattern is treated as an ordinary search pattern instead.
    Relative,
    /// A Suricata buffer-selector flag (`U`, `H`, `P`, `C`, ...): which
    /// normalized buffer the pattern targets. Does *not* affect
    /// faithfulness -- it names where the pattern is tested, not what it
    /// denotes -- but it does mean the generated inputs are not
    /// guaranteed to be shaped like the buffer the rule expects.
    BufferFlag,
    /// `^` or `$` appears somewhere inside the pattern but not on every
    /// branch of the alternation it sits in, so `detect_anchors` does
    /// not treat the whole pattern as anchored on that side. `translate`
    /// still lowers that nested anchor to `Eps` unconditionally, so once
    /// the pattern is search-padded it can fire at any position the
    /// padding stops at, not just the true buffer boundary.
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

    /// Variants the caveats table iterates. `BufferFlag` is deliberately
    /// absent: a buffer selector does not move a pattern from faithful to
    /// approx, and its count is printed by `buffer_flag_note` under the
    /// flag tables instead. `Relative` and `NestedAnchor` are the two
    /// that do move a pattern from faithful to approx. A bare `.` without
    /// the dotall (`s`) flag used to be a third: `parse_pcre_rule` now
    /// excludes `\n` from `.` whenever `s` is absent
    /// (`frontend::strip_dot_newline`), so that case is gone -- it is a
    /// real engine fix, not just a tracked approximation anymore.
    const TABLED: [Caveat; 2] = [Caveat::Relative, Caveat::NestedAnchor];
}

/// What happens to one extracted pattern when `parse_pcre_rule` is asked
/// to translate it for search.
#[derive(Debug)]
enum Classification {
    Accepted { anchor_shape: AnchorShape },
    Rejected(Cause),
}

fn classify(pattern: &str) -> Classification {
    match parse_pcre_rule(pattern) {
        Ok(_) => {
            let (body, _flags) = strip_pcre_delimiters(pattern);
            // `detect_anchors` cannot fail here: `parse_pcre_rule` has
            // already run the same flag check and body parse.
            let (start, end) = detect_anchors(body).unwrap_or((false, false));
            Classification::Accepted {
                anchor_shape: AnchorShape::from_anchors(start, end),
            }
        }
        Err(e) => Classification::Rejected(Cause::classify(&e)),
    }
}

/// The caveats an accepted `pattern` still carries (see `Caveat`).
/// Returns an empty `Vec` for a pattern that is faithful. `Relative` and
/// `NestedAnchor` are read from `frontend::faithfulness_gaps` -- the same
/// function `data::run_pipeline` calls to decide what to exclude from the
/// real dataset, so this report can never drift from what the pipeline
/// actually does. `BufferFlag` is not a faithfulness gap at all (see its
/// doc comment) and has no library equivalent; it stays a local,
/// informational-only check.
fn caveats_of(pattern: &str) -> Vec<Caveat> {
    let mut out: Vec<Caveat> = faithfulness_gaps(pattern)
        .into_iter()
        .map(|gap| match gap {
            FaithfulnessGap::Relative => Caveat::Relative,
            FaithfulnessGap::NestedAnchor => Caveat::NestedAnchor,
        })
        .collect();

    // Any flag letter not in "imsxRAE" is assumed to be one of Suricata's
    // documented buffer-selector/no-op flags (U, H, P, Q, I, D, M, C, S,
    // Y, V, B, O, ...; confirmed against Suricata's own pcre-keyword
    // docs, doc/userguide/rules/payload-keywords.rst). That assumption
    // covers every letter this frontend has actually seen in these three
    // corpora (checked against the flag tables below). It would be wrong
    // for a corpus using a PCRE flag outside {i, m, s, x} that Suricata
    // does not document, since such a letter would be silently ignored
    // here rather than flagged.
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
    /// Accepted, and the produced `Regex` exactly denotes what the
    /// source pattern means: no `R` (position hint dropped) and no
    /// nested-anchor ambiguity. A buffer-selector flag (`U`/`H`/`P`/...)
    /// does NOT disqualify a pattern from faithful: it says which
    /// buffer the pattern should be tested against, not what the
    /// pattern itself denotes as a regular language.
    faithful: usize,
    /// Accepted, but not faithful: `R` or a nested anchor was found, so
    /// the produced `Regex` is a known approximation of what the
    /// pattern means. `faithful + approx == accepted`, always.
    approx: usize,
    rejected_by: BTreeMap<Cause, usize>,
    anchors: BTreeMap<AnchorShape, usize>,
    caveats: BTreeMap<Caveat, usize>,
    flag_usage: BTreeMap<char, usize>,
    flag_accepted: BTreeMap<char, usize>,
    /// Every rejected (pattern, cause), only populated when --verbose.
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

        // Flag usage is recorded for *all* extracted patterns, whether or
        // not the translation succeeds -- the point is to see what the
        // corpus contains, not what survives.
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
                // approximation of the source pattern; a buffer-selector
                // flag alone does not (see the `faithful` field doc).
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

/// The flag letters attached to a `/PATTERN/FLAGS` wrapper, in whatever
/// order they appear. Returns an empty iterator for a bare pattern.
fn flags_of(pattern: &str) -> Vec<char> {
    raw_flags(pattern)
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect()
}

/// The raw `FLAGS` part of a `/PATTERN/FLAGS` wrapper, or `""` if
/// `pattern` isn't wrapped that way.
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
        // A real corpus is not guaranteed to be clean UTF-8 (SpamAssassin's
        // locale-specific rule files, e.g. 30_text_de.cf, carry legacy
        // Latin-1 diacritics). Decoding lossily instead of skipping the
        // whole file keeps every other rule in it: a pattern whose own
        // bytes were invalid may come out corrupted, but that is a
        // per-pattern problem `classify` can reject on its own merits,
        // not a reason to silently drop the rest of the file.
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

    // Union of all flag letters seen, sorted.
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

/// A short explanation of the buffer-selector flags, printed once under
/// the accepted-patterns flag table. These flags name the buffer a rule
/// targets (HTTP URI, HTTP header, POST body, and so on), not what the
/// pattern denotes; ignoring them does not make the translated `Regex`
/// unfaithful, but it does mean the generated inputs are not necessarily
/// shaped like the buffer the rule expects. That is a difference in input
/// relevance, not in translation fidelity, and this note is where the
/// report says so.
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
                // A bare positional: if it looks like a file, treat it as
                // one; if it names a source, treat it as a source selector.
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

    // Explicit files apply only when exactly one source is selected; with
    // multiple sources there is no way to know which file belongs to
    // which, so the files are ignored with a warning.
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