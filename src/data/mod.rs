//! src/data/mod.rs
//!
//! Pipeline entry points.
//! `run_pipeline` drives extract → verify for one source, using only
//! that source's own given match/non-match examples -- no input is
//! generated, derived, or assembled. Today that means S2ELab only
//! (`RuleContext::S2ELab`'s own `matches`/`non_matches`); every other
//! source's raw-extraction code (`extract::extract_*`) still runs, so a
//! future source that ships its own given examples can be wired in the
//! same way, but none of the other four produce any usable case right
//! now -- that is expected, not a bug to route around.
//! `load_prepared_cases` reads the result back for benchmarks.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub mod extract;
pub mod generate;
pub mod prepare;
pub mod types;

use types::{MatchIntent, PreparedCase, RuleContext, SourceKind};

#[derive(Debug, Default, Clone)]
pub struct PipelineReport {
    pub rules_extracted: usize,
    /// Of `rules_extracted`, how many carried byte-identical
    /// `raw_pattern` text to a rule already seen earlier in this same
    /// run. Checked -- and skipped -- before translation is even
    /// attempted: a duplicate's fate (accepted or not, usable or not)
    /// is already decided by its first occurrence, so re-deriving it
    /// would only double-count the same answer and, worse, write a
    /// second independently-generated input group for the same regex
    /// into `prepared.jsonl`, overweighting that pattern in any
    /// benchmark that reads it. `dataset_stats` reports the same
    /// duplication on the raw corpus without running generation or
    /// verification to get there.
    pub rules_duplicate_pattern: usize,
    /// Rejected by `prepare::core_regex_tagged`.
    /// Of `rules_extracted - rules_duplicate_pattern`, i.e. of distinct
    /// patterns only.
    pub patterns_unusable: usize,
    /// Distinct patterns `core_regex_tagged` could translate.
    /// Not what landed in `prepared.jsonl`; see `patterns_usable`.
    pub patterns_accepted: usize,
    /// Of `patterns_accepted`, top-level `^...$` wraps.
    pub patterns_explicit_full_match: usize,
    /// Of `patterns_accepted`, patterns with no anchor at all.
    pub patterns_unanchored_ambiguous: usize,
    /// Candidates considered before verification: every one of the
    /// source's own given examples for `patterns_accepted` patterns.
    pub inputs_generated: usize,
    /// Of `inputs_generated`, how many verified against the real parser.
    pub inputs_verified: usize,
    /// Of `inputs_generated`, how many were examined by `verify_one` and
    /// rejected: either the claim did not hold, or a negative failed
    /// before `LATE_FAILURE_THRESHOLD`.
    pub inputs_rejected: usize,
    /// Of `patterns_accepted`, how many ended up with zero verified
    /// matches or zero verified non-matches (or had no given examples at
    /// all, which is every pattern from a source with no given-input
    /// extraction wired in). These contribute nothing to `prepared.jsonl`
    /// -- a pattern needs at least one of each side to be benchmarkable.
    pub patterns_unusable_at_generation: usize,
    /// `patterns_accepted - patterns_unusable_at_generation`.
    /// What a benchmark over this source's `prepared.jsonl` actually sees.
    pub patterns_usable: usize,
    /// Total (pattern, input) rows written to `prepared.jsonl`: every
    /// verified given example of every usable pattern, not a fixed count.
    pub inputs_usable: usize,
    /// Of `inputs_usable`, how many came from the source's own data.
    /// Equals `inputs_usable` for every source today, since nothing is
    /// generated anymore; kept as its own field for when a source mixes
    /// given input with something else in the future.
    pub inputs_usable_from_source: usize,
    /// Of `patterns_usable`, top-level `^...$` wraps.
    pub usable_explicit_full_match: usize,
    /// Of `patterns_usable`, patterns with no anchor at all.
    pub usable_unanchored_ambiguous: usize,
    /// Raw text of every pattern counted in `patterns_unusable_at_generation`.
    /// Always collected; printing is the caller's choice.
    pub unusable_at_generation_patterns: Vec<String>,
}

impl PipelineReport {
    /// `inputs_generated - inputs_verified - inputs_rejected`.
    /// Candidates never examined, because `verify_until` already had
    /// `per_side` on both sides or their claimed side was full.
    pub fn inputs_skipped(&self) -> usize {
        self.inputs_generated
            .saturating_sub(self.inputs_verified)
            .saturating_sub(self.inputs_rejected)
    }

    /// `inputs_usable - inputs_usable_from_source`. Always 0 today --
    /// nothing is generated to fill a gap anymore -- kept for when a
    /// future source mixes given input with something else.
    pub fn inputs_usable_synthetic(&self) -> usize {
        self.inputs_usable - self.inputs_usable_from_source
    }

    /// `inputs_verified - inputs_usable`.
    /// Verified candidates that did not reach `prepared.jsonl` because
    /// their pattern ended up with zero on the other side.
    pub fn inputs_dropped_with_pattern(&self) -> usize {
        self.inputs_verified.saturating_sub(self.inputs_usable)
    }
}

/// Runs extract → verify for one source, using only that source's own
/// given match/non-match examples.
/// Writes to `<data_root>/<source>/prepared.jsonl`, replacing any existing file.
pub fn run_pipeline(
    source: SourceKind,
    raw_files: &[PathBuf],
    data_root: &Path,
) -> std::io::Result<PipelineReport> {
    let mut report = PipelineReport::default();

    let mut rules = Vec::new();
    for file in raw_files {
        let bytes = std::fs::read(file)?;
        let text = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => {
                // Decode lossily rather than aborting.
                // A corpus can carry legacy bytes.
                // A pattern touching them is a per-pattern problem
                // `core_regex` can reject on its own.
                eprintln!(
                    "warning: {} is not valid UTF-8; decoding lossily",
                    file.display()
                );
                String::from_utf8_lossy(e.as_bytes()).into_owned()
            }
        };
        let extracted = match source {
            SourceKind::Suricata => extract::extract_suricata(&text),
            SourceKind::SpamAssassin => extract::extract_spamassassin(&text),
            SourceKind::RegexLib => extract::extract_regexlib(&text),
            SourceKind::S2ELab => extract::extract_s2_elab(&text),
            SourceKind::Innovatorved => extract::extract_innovatorved(&text),
        };
        rules.extend(extracted);
    }
    report.rules_extracted = rules.len();

    let out_path = data_root.join(source.dir_name()).join("prepared.jsonl");
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(&out_path);

    let mut seen_patterns: HashSet<&str> = HashSet::new();
    for rule in &rules {
        if !seen_patterns.insert(rule.raw_pattern.as_str()) {
            // Byte-identical to a rule already processed above in this
            // same run -- same translation result, same generate/verify
            // outcome, guaranteed. Skip rather than re-derive it a
            // second time and write a second independent input group
            // for the same regex into `prepared.jsonl`.
            report.rules_duplicate_pattern += 1;
            continue;
        }
        let Ok((core, match_intent)) = prepare::core_regex_tagged(&rule.raw_pattern) else {
            report.patterns_unusable += 1;
            continue;
        };
        report.patterns_accepted += 1;
        match match_intent {
            MatchIntent::ExplicitFullMatch => report.patterns_explicit_full_match += 1,
            MatchIntent::UnanchoredAmbiguous => report.patterns_unanchored_ambiguous += 1,
        }

        // Only a source with its own given examples produces candidates.
        // Every other context yields none, so its patterns always end up
        // in `patterns_unusable_at_generation` below -- expected, not a
        // bug: there is nothing to generate anymore.
        let candidates = match &rule.context {
            RuleContext::S2ELab { matches, non_matches } => {
                generate::given_examples(matches, non_matches)
            }
            _ => Vec::new(),
        };

        report.inputs_generated += candidates.len();
        let candidates_len = candidates.len();
        let prepared: Vec<PreparedCase> = candidates
            .into_iter()
            .filter_map(|c| prepare::verify_one(source, &rule.raw_pattern, &core, match_intent, c))
            .collect();
        let prepared_len = prepared.len();
        report.inputs_verified += prepared_len;
        report.inputs_rejected += candidates_len - prepared_len;

        let verified_matches = prepared.iter().filter(|c| c.verified_match).count();
        let verified_non_matches = prepared_len - verified_matches;

        if verified_matches == 0 || verified_non_matches == 0 {
            report.patterns_unusable_at_generation += 1;
            report.unusable_at_generation_patterns.push(rule.raw_pattern.clone());
        } else {
            report.patterns_usable += 1;
            report.inputs_usable += prepared_len;
            report.inputs_usable_from_source +=
                prepared.iter().filter(|c| c.provenance.is_from_dataset()).count();
            match match_intent {
                MatchIntent::ExplicitFullMatch => report.usable_explicit_full_match += 1,
                MatchIntent::UnanchoredAmbiguous => report.usable_unanchored_ambiguous += 1,
            }
            prepare::write_prepared_cases(&out_path, &prepared)?;
        }
    }

    Ok(report)
}

/// Loads every prepared case for the given sources.
/// Empty `sources` means S2ELab, the one source `run_pipeline` can
/// currently produce any usable case for. Suricata, SpamAssassin,
/// RegexLib, and Innovatorved are opt-in only -- selecting them yields
/// nothing today, since none of them has a given-input extraction wired
/// in yet, but `load_prepared_cases` still reads whatever
/// `prepared.jsonl` happens to be there if one is.
pub fn load_prepared_cases(
    data_root: &Path,
    sources: &[SourceKind],
) -> std::io::Result<Vec<PreparedCase>> {
    let sources: &[SourceKind] = if sources.is_empty() {
        &[SourceKind::S2ELab]
    } else {
        sources
    };
    let mut out = Vec::new();
    for &source in sources {
        let path = data_root.join(source.dir_name()).join("prepared.jsonl");
        if !path.exists() {
            continue;
        }
        let cases = prepare::read_prepared_cases(&path)?;
        out.extend(cases);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_pattern_text_is_skipped_not_double_counted() {
        let dir = std::env::temp_dir().join(format!("data-pipeline-dedup-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rules_path = dir.join("dedup.json");
        // Two entries share byte-identical pattern text (this dedup
        // exists for exactly this real-world shape); a third is
        // genuinely distinct. Each carries its own given examples,
        // S2ELab-style. Non-matches are chosen to fail *late* (past
        // two of three characters): a given negative still has to
        // clear `LATE_FAILURE_THRESHOLD` like any other, so an
        // immediately-failing one such as "xyz" against "abc" would
        // be rejected by that real quality filter, not by this test.
        std::fs::write(
            &rules_path,
            r#"[
                {"expression": "abc", "matches": ["abc"], "non_matches": ["abX"]},
                {"expression": "abc", "matches": ["abc"], "non_matches": ["abX"]},
                {"expression": "xyz", "matches": ["xyz"], "non_matches": ["xyX"]}
            ]"#,
        )
        .unwrap();

        let out_root = dir.join("processed");
        let report = run_pipeline(SourceKind::S2ELab, &[rules_path], &out_root).unwrap();

        assert_eq!(report.rules_extracted, 3);
        assert_eq!(report.rules_duplicate_pattern, 1, "the second \"abc\" entry should be skipped, not re-derived");
        assert_eq!(report.patterns_accepted, 2, "two distinct patterns: abc and xyz");
        assert_eq!(report.patterns_usable, 2);
        assert_eq!(report.inputs_usable, 4, "2 distinct patterns x (1 match + 1 non-match) each");

        let cases = prepare::read_prepared_cases(&out_root.join("s2-elab").join("prepared.jsonl")).unwrap();
        let mut patterns: Vec<&str> = cases.iter().map(|c| c.pattern.as_str()).collect();
        patterns.sort_unstable();
        patterns.dedup();
        assert_eq!(patterns, vec!["abc", "xyz"], "prepared.jsonl holds one input group per distinct pattern, not per entry");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pattern_with_only_matches_or_only_non_matches_is_unusable() {
        let dir = std::env::temp_dir().join(format!("data-pipeline-onesided-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rules_path = dir.join("onesided.json");
        std::fs::write(
            &rules_path,
            r#"[{"expression": "abc", "matches": ["abc"], "non_matches": []}]"#,
        )
        .unwrap();

        let out_root = dir.join("processed");
        let report = run_pipeline(SourceKind::S2ELab, &[rules_path], &out_root).unwrap();

        assert_eq!(report.patterns_accepted, 1);
        assert_eq!(report.patterns_usable, 0, "no non-match side -- not benchmarkable");
        assert_eq!(report.patterns_unusable_at_generation, 1);
        assert!(!out_root.join("s2-elab").join("prepared.jsonl").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}