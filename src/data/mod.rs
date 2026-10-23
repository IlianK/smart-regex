//! src/data/mod.rs
//!
//! Pipeline entry points. `run_pipeline` is S2ELab-only: it extracts,
//! translates, and writes every usable pattern's own given examples,
//! each checked with one direct `prepare::matches_bounded` call against
//! its claimed label -- not the deleted generated-candidate verification
//! machinery, just a boolean agree/disagree check. After that check, the
//! longer of the surviving match/non-match sides is truncated down to
//! the shorter side's count, so every usable pattern contributes an
//! equal number of matches and non-matches -- an imbalanced pattern
//! would otherwise skew aggregate parser-timing/memory stats toward
//! whichever outcome dominates it. `dataset_stats` separately still
//! uses the other four extractors for raw-corpus diagnostics unrelated
//! to this pipeline, which is why they still exist in `extract.rs`.
//! `load_prepared_cases` reads the result back for benchmarks.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub mod extract;
pub mod pathological;
pub mod prepare;
pub mod types;

use types::{MatchIntent, PreparedCase, RuleContext, SourceKind};

#[derive(Debug, Default, Clone)]
pub struct PipelineReport {
    pub patterns_extracted: usize,
    /// Of `patterns_extracted`, byte-identical `raw_pattern` text to a
    /// pattern already seen earlier in this run; skipped before
    /// translation, since its fate is already decided by the first one.
    pub patterns_duplicate: usize,
    /// Rejected by `prepare::core_regex_tagged`. Of the distinct patterns.
    pub patterns_unusable: usize,
    /// Distinct patterns `core_regex_tagged` could translate.
    pub patterns_accepted: usize,
    /// Of `patterns_accepted`, top-level `^...$` wraps.
    pub patterns_explicit_full_match: usize,
    /// Of `patterns_accepted`, patterns with no anchor at all.
    pub patterns_unanchored_ambiguous: usize,
    /// Of `patterns_accepted`, how many have at least one given match and
    /// one given non-match left after the label check below -- what a
    /// benchmark actually sees.
    pub patterns_usable: usize,
    /// Total (pattern, input) rows written to `prepared.jsonl`.
    pub inputs_usable: usize,
    /// Given examples dropped because the real translated `Regex`
    /// disagreed with the dataset's own claimed label (or the check
    /// couldn't be confirmed within its safety bound).
    pub inputs_label_mismatch: usize,
    /// Given examples dropped, per pattern, to balance that one
    /// pattern's own match/non-match counts after the label check above
    /// -- truncating its longer side down to its shorter side's count.
    /// Distinct from `inputs_label_mismatch`.
    pub inputs_dropped_for_pattern_balance: usize,
    /// The smallest per-pattern balanced count (`k_i`) across every
    /// usable pattern; every usable pattern is truncated down to this
    /// many matches and this many non-matches, so every one contributes
    /// exactly `2 * balance_k_global` rows dataset-wide. `0` if no
    /// pattern is usable.
    pub balance_k_global: usize,
    /// Given examples dropped by that further, dataset-wide truncation
    /// down to `balance_k_global`. Distinct from
    /// `inputs_dropped_for_pattern_balance`, which happens first.
    pub inputs_dropped_for_global_balance: usize,
    /// Of `patterns_usable`, how many had their own per-pattern balanced
    /// count (`k_i`) above `balance_k_global` and so lost rows in that
    /// second, global truncation.
    pub patterns_truncated_for_global_balance: usize,
}

/// `(text, is_match)` for every given example, unchanged and uncapped.
/// Duplicate strings are kept once; a string in both lists is kept only
/// as a positive, the stronger claim.
fn given_examples(matches: &[String], non_matches: &[String]) -> Vec<(String, bool)> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    for text in matches {
        if seen.insert(text.as_str()) {
            out.push((text.clone(), true));
        }
    }
    for text in non_matches {
        if seen.insert(text.as_str()) {
            out.push((text.clone(), false));
        }
    }
    out
}

/// One usable pattern's own matches/non-matches, already truncated to
/// its own per-pattern balanced count (`matches.len() == non_matches.len()`).
struct UsablePattern {
    pattern: String,
    match_intent: MatchIntent,
    matches: Vec<String>,
    non_matches: Vec<String>,
}

/// Extracts S2ELab, translates each distinct pattern, label-checks its
/// given examples, and writes every usable one to
/// `<data_root>/s2-elab/prepared.jsonl`, truncated in two stages so every
/// usable pattern ends up contributing the same total row count: first
/// each pattern's own longer side down to its shorter side, then every
/// pattern down to the smallest such count (`balance_k_global`) seen
/// anywhere in this run.
pub fn run_pipeline(raw_files: &[PathBuf], data_root: &Path) -> std::io::Result<PipelineReport> {
    let mut report = PipelineReport::default();

    let mut rules = Vec::new();
    for file in raw_files {
        let bytes = std::fs::read(file)?;
        let text = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => {
                // Decode lossily rather than aborting.
                // A corpus can carry legacy bytes; a pattern touching
                // them is a per-pattern problem `core_regex` can reject.
                eprintln!(
                    "warning: {} is not valid UTF-8; decoding lossily",
                    file.display()
                );
                String::from_utf8_lossy(e.as_bytes()).into_owned()
            }
        };
        rules.extend(extract::extract_s2_elab(&text));
    }
    report.patterns_extracted = rules.len();

    let out_path = data_root.join(SourceKind::S2ELab.dir_name()).join("prepared.jsonl");
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(&out_path);

    // Stage 1 (per pattern): label-check, then balance each pattern's
    // own matches/non-matches. Held in memory, not written yet -- stage
    // 2 below needs every pattern's `k_i` before any of them can be
    // truncated to the dataset-wide minimum.
    let mut usable: Vec<UsablePattern> = Vec::new();

    let mut seen_patterns: HashSet<&str> = HashSet::new();
    for rule in &rules {
        if !seen_patterns.insert(rule.raw_pattern.as_str()) {
            // Byte-identical to a rule already processed above in this
            // same run -- same translation and given examples, guaranteed.
            report.patterns_duplicate += 1;
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

        // Only ever `S2ELab` here: `rules` comes solely from
        // `extract_s2_elab` above. Treat anything else as no examples.
        let (matches, non_matches) = match &rule.context {
            RuleContext::S2ELab { matches, non_matches } => (matches.as_slice(), non_matches.as_slice()),
            _ => (&[][..], &[][..]),
        };

        // Drop any given example the real `Regex` disagrees with --
        // a direct boolean check, not generated-candidate verification.
        let examples: Vec<(String, bool)> = given_examples(matches, non_matches)
            .into_iter()
            .filter(|(text, claimed_match)| {
                let agrees = prepare::matches_bounded(text, &core) == Some(*claimed_match);
                if !agrees {
                    report.inputs_label_mismatch += 1;
                }
                agrees
            })
            .collect();

        // Preserves `given_examples`'s own order: every match in its
        // original order, then every non-match in its original order.
        let (mut pattern_matches, mut pattern_non_matches): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
        for (text, is_match) in examples {
            if is_match {
                pattern_matches.push(text);
            } else {
                pattern_non_matches.push(text);
            }
        }

        let k_i = pattern_matches.len().min(pattern_non_matches.len());
        report.inputs_dropped_for_pattern_balance +=
            (pattern_matches.len() - k_i) + (pattern_non_matches.len() - k_i);
        if k_i == 0 {
            continue;
        }
        pattern_matches.truncate(k_i);
        pattern_non_matches.truncate(k_i);

        report.patterns_usable += 1;
        usable.push(UsablePattern {
            pattern: rule.raw_pattern.clone(),
            match_intent,
            matches: pattern_matches,
            non_matches: pattern_non_matches,
        });
    }

    // Stage 2 (dataset-wide): truncate every usable pattern down to the
    // smallest `k_i` seen anywhere, so every one contributes the same
    // total row count. `k_i == matches.len() == non_matches.len()` here.
    let k_global = usable.iter().map(|u| u.matches.len()).min().unwrap_or(0);
    report.balance_k_global = k_global;

    for mut u in usable {
        let k_i = u.matches.len();
        if k_i > k_global {
            report.patterns_truncated_for_global_balance += 1;
            report.inputs_dropped_for_global_balance += 2 * (k_i - k_global);
        }
        u.matches.truncate(k_global);
        u.non_matches.truncate(k_global);

        let pattern = u.pattern;
        let match_intent = u.match_intent;
        let prepared: Vec<PreparedCase> = u
            .matches
            .into_iter()
            .map(|input| (input, true))
            .chain(u.non_matches.into_iter().map(|input| (input, false)))
            .map(|(input, expected_match)| PreparedCase {
                source: SourceKind::S2ELab,
                pattern: pattern.clone(),
                input,
                expected_match,
                match_intent,
            })
            .collect();

        report.inputs_usable += prepared.len();
        prepare::write_prepared_cases(&out_path, &prepared)?;
    }

    Ok(report)
}

/// Loads every prepared case for the given sources.
/// Empty `sources` means S2ELab, the only source `run_pipeline` produces.
/// The other four are opt-in only -- selecting one yields nothing today,
/// since `run_pipeline` never extracts them, but this still reads
/// whatever `prepared.jsonl` happens to be there if one is.
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
    fn given_examples_wrap_matches_and_non_matches_unchanged() {
        let matches = vec!["12345".to_string(), "00000".to_string()];
        let non_matches = vec!["abcde".to_string()];
        let examples = given_examples(&matches, &non_matches);
        assert_eq!(examples.len(), 3);
        let positives: Vec<_> = examples.iter().filter(|(_, is_match)| *is_match).collect();
        let negatives: Vec<_> = examples.iter().filter(|(_, is_match)| !*is_match).collect();
        assert_eq!(positives.len(), 2);
        assert_eq!(negatives.len(), 1);
        assert_eq!(negatives[0].0, "abcde");
    }

    #[test]
    fn given_examples_are_not_capped() {
        let matches: Vec<String> = (0..100).map(|i| format!("m{i}")).collect();
        let non_matches: Vec<String> = (0..100).map(|i| format!("n{i}")).collect();
        let examples = given_examples(&matches, &non_matches);
        assert_eq!(examples.len(), 200);
        assert_eq!(examples.iter().filter(|(_, is_match)| *is_match).count(), 100);
        assert_eq!(examples.iter().filter(|(_, is_match)| !*is_match).count(), 100);
    }

    #[test]
    fn given_examples_keep_a_string_in_both_lists_only_as_a_positive() {
        let matches = vec!["x".to_string()];
        let non_matches = vec!["x".to_string(), "y".to_string()];
        let examples = given_examples(&matches, &non_matches);
        assert_eq!(examples.len(), 2);
        let x = examples.iter().find(|(text, _)| text == "x").expect("x present");
        assert!(x.1);
    }

    #[test]
    fn given_examples_with_nothing_given_yields_nothing() {
        assert!(given_examples(&[], &[]).is_empty());
    }

    #[test]
    fn duplicate_pattern_text_is_skipped_not_double_counted() {
        let dir = std::env::temp_dir().join(format!("data-pipeline-dedup-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rules_path = dir.join("dedup.json");
        // Two entries share byte-identical pattern text (this dedup
        // exists for exactly this real-world shape); a third is
        // genuinely distinct. Each carries its own given examples.
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
        let report = run_pipeline(&[rules_path], &out_root).unwrap();

        assert_eq!(report.patterns_extracted, 3);
        assert_eq!(report.patterns_duplicate, 1, "the second \"abc\" entry should be skipped, not re-derived");
        assert_eq!(report.patterns_accepted, 2, "two distinct patterns: abc and xyz");
        assert_eq!(report.patterns_usable, 2);
        assert_eq!(report.inputs_usable, 4, "2 distinct patterns x (1 match + 1 non-match) each");
        assert_eq!(report.balance_k_global, 1, "both patterns already have k_i == 1");
        assert_eq!(report.patterns_truncated_for_global_balance, 0);

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
        let report = run_pipeline(&[rules_path], &out_root).unwrap();

        assert_eq!(report.patterns_accepted, 1);
        assert_eq!(report.patterns_usable, 0, "no non-match side -- not benchmarkable");
        assert!(!out_root.join("s2-elab").join("prepared.jsonl").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_mislabeled_given_example_is_dropped_and_counted_not_written() {
        let dir = std::env::temp_dir().join(format!("data-pipeline-mismatch-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rules_path = dir.join("mismatch.json");
        // "xyz" is claimed as a match for "abc" but plainly isn't one;
        // "abX" is a genuine non-match.
        std::fs::write(
            &rules_path,
            r#"[{"expression": "abc", "matches": ["abc", "xyz"], "non_matches": ["abX"]}]"#,
        )
        .unwrap();

        let out_root = dir.join("processed");
        let report = run_pipeline(&[rules_path], &out_root).unwrap();

        assert_eq!(report.inputs_label_mismatch, 1, "the mislabeled \"xyz\" claim should be dropped");
        assert_eq!(report.patterns_usable, 1, "the real match and non-match still make this usable");
        assert_eq!(report.inputs_usable, 2);
        assert_eq!(report.balance_k_global, 1, "the one usable pattern already has k_i == 1");

        let cases = prepare::read_prepared_cases(&out_root.join("s2-elab").join("prepared.jsonl")).unwrap();
        assert!(!cases.iter().any(|c| c.input == "xyz"), "the mislabeled example should never be written");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pattern_with_more_matches_than_non_matches_is_truncated_to_the_shorter_side() {
        let dir = std::env::temp_dir().join(format!("data-pipeline-balance-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rules_path = dir.join("balance.json");
        // 3 genuine matches, 1 genuine non-match: k_i == 1, so only the
        // first match (original order) and the one non-match survive.
        std::fs::write(
            &rules_path,
            r#"[{"expression": "abc|abd|abe", "matches": ["abc", "abd", "abe"], "non_matches": ["xyz"]}]"#,
        )
        .unwrap();

        let out_root = dir.join("processed");
        let report = run_pipeline(&[rules_path], &out_root).unwrap();

        assert_eq!(report.patterns_usable, 1);
        assert_eq!(report.inputs_dropped_for_pattern_balance, 2, "abd and abe dropped to match the 1 non-match");
        assert_eq!(report.balance_k_global, 1, "the only usable pattern's own k_i");
        assert_eq!(report.inputs_dropped_for_global_balance, 0, "nothing left to drop at the global stage");
        assert_eq!(report.inputs_usable, 2);

        let cases = prepare::read_prepared_cases(&out_root.join("s2-elab").join("prepared.jsonl")).unwrap();
        let kept_match = cases.iter().find(|c| c.expected_match).expect("one match kept");
        assert_eq!(kept_match.input, "abc", "the first match in original order, not abd or abe");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_usable_pattern_is_truncated_to_the_dataset_wide_minimum() {
        let dir = std::env::temp_dir().join(format!("data-pipeline-globalbalance-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rules_path = dir.join("globalbalance.json");
        // "abc|abd|abe" is already balanced at k_i == 3 on its own; "xyz"
        // is balanced at k_i == 1. The dataset-wide minimum is 1, so the
        // first pattern loses 2 matches + 2 non-matches it didn't lose
        // to its own per-pattern balance.
        std::fs::write(
            &rules_path,
            r#"[
                {"expression": "abc|abd|abe", "matches": ["abc", "abd", "abe"], "non_matches": ["x1", "x2", "x3"]},
                {"expression": "xyz", "matches": ["xyz"], "non_matches": ["abX"]}
            ]"#,
        )
        .unwrap();

        let out_root = dir.join("processed");
        let report = run_pipeline(&[rules_path], &out_root).unwrap();

        assert_eq!(report.patterns_usable, 2);
        assert_eq!(report.inputs_dropped_for_pattern_balance, 0, "both patterns already balanced on their own");
        assert_eq!(report.balance_k_global, 1, "the smaller of the two patterns' k_i (3 and 1)");
        assert_eq!(report.patterns_truncated_for_global_balance, 1, "only abc|abd|abe had k_i > 1");
        assert_eq!(report.inputs_dropped_for_global_balance, 4, "2 matches + 2 non-matches from abc|abd|abe");
        assert_eq!(report.inputs_usable, 4, "2 patterns x (1 match + 1 non-match) each");

        let cases = prepare::read_prepared_cases(&out_root.join("s2-elab").join("prepared.jsonl")).unwrap();
        let first_pattern_cases: Vec<_> = cases.iter().filter(|c| c.pattern == "abc|abd|abe").collect();
        assert_eq!(first_pattern_cases.len(), 2, "truncated down to 1 match + 1 non-match");
        let kept_match = first_pattern_cases.iter().find(|c| c.expected_match).expect("one match kept");
        assert_eq!(kept_match.input, "abc", "the first match in original order");
        let kept_non_match = first_pattern_cases.iter().find(|c| !c.expected_match).expect("one non-match kept");
        assert_eq!(kept_non_match.input, "x1", "the first non-match in original order");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pattern_that_loses_every_match_to_the_label_check_is_unusable() {
        let dir = std::env::temp_dir().join(format!("data-pipeline-allmismatch-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rules_path = dir.join("allmismatch.json");
        // Every claimed match is actually wrong; accepted by the frontend
        // but left with no verified match side after the label check.
        std::fs::write(
            &rules_path,
            r#"[{"expression": "abc", "matches": ["xyz", "qqq"], "non_matches": ["abX"]}]"#,
        )
        .unwrap();

        let out_root = dir.join("processed");
        let report = run_pipeline(&[rules_path], &out_root).unwrap();

        assert_eq!(report.patterns_accepted, 1);
        assert_eq!(report.inputs_label_mismatch, 2);
        assert_eq!(report.patterns_usable, 0, "no verified match side -- not benchmarkable");
        assert!(!out_root.join("s2-elab").join("prepared.jsonl").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
