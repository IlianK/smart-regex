//! src/data/mod.rs
//!
//! Pipeline entry points.
//! `run_pipeline` drives extract → generate → prepare for one source
//! and writes `prepared.jsonl`. `load_prepared_cases` reads the result back for benchmarks.

pub mod extract;
pub mod generate;
pub mod prepare;
pub mod types;

use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::SeedableRng;

use types::{MatchIntent, PreparedCase, RuleContext, SourceKind};

#[derive(Debug, Default, Clone)]
pub struct PipelineReport {
    pub rules_extracted: usize,
    /// Rejected by `prepare::core_regex_tagged`.
    /// Same set as `frontend::parse_pcre_rule` rejects, plus the two
    /// safety caps (`MAX_CORE_DEPTH`, `MAX_CORE_NODES`).
    pub patterns_unusable: usize,
    /// Patterns `core_regex_tagged` could translate. Not the same as
    /// "what landed in `prepared.jsonl`": see `patterns_usable` below,
    /// and `patterns_unusable_at_generation` for why the two differ.
    pub patterns_accepted: usize,
    /// Of `patterns_accepted`, top-level `^...$` wraps.
    pub patterns_explicit_full_match: usize,
    /// Of `patterns_accepted`, patterns with no anchor at all.
    /// See `types::MatchIntent`.
    pub patterns_unanchored_ambiguous: usize,
    /// Inputs produced before verification, summed over every accepted
    /// pattern -- an activity count, not a quality one. Bounded per
    /// pattern by `slack_for(source) * per_side * (number of strategies
    /// the source uses)`; see `slack_for`.
    pub inputs_generated: usize,
    /// Of `inputs_generated`, how many verified against the real parser
    /// -- still before the balancing step below, so this can (and for
    /// some patterns does) include more matches than non-matches or
    /// vice versa. `inputs_generated - inputs_verified` is how many
    /// were discarded for failing verification.
    pub inputs_verified: usize,
    /// Of `patterns_accepted`, how many did **not** reach `per_side`
    /// verified matches *and* `per_side` verified non-matches (where
    /// `per_side` is half of the CLI's `--inputs N`), and so contribute
    /// nothing to `prepared.jsonl` (see `prepare::balance_cases`).
    /// This is the only way a pattern ends up excluded from the
    /// benchmark corpus; "zero verified cases" is just the extreme end
    /// of it, not a separate condition. Two typical causes: the
    /// pattern's language is too small to have `per_side` distinct
    /// members at all (`a{3}` has exactly one positive); or it had
    /// enough candidates but not enough of one side survived
    /// verification -- for example, a source's given examples assume
    /// substring-search semantics while this frontend's matching is
    /// full-string-only, so every claimed match fails, and every
    /// claimed non-match fails too early (before
    /// `prepare::LATE_FAILURE_THRESHOLD`) to count as a meaningful
    /// negative either. Neither cause is a translation bug; both are
    /// checked empirically here, not inferred from the source schema.
    pub patterns_unusable_at_generation: usize,
    /// `patterns_accepted - patterns_unusable_at_generation`: patterns
    /// that actually appear in `prepared.jsonl`, each with exactly
    /// `per_side` matches and `per_side` non-matches -- no more, no
    /// fewer, and the same split for every one of them. This, not
    /// `patterns_accepted`, is what a benchmark over this source's
    /// `prepared.jsonl` actually sees.
    pub patterns_usable: usize,
    /// `patterns_usable * 2 * per_side` (`== patterns_usable` times the
    /// CLI's own `--inputs N`): total (pattern, input) rows written to
    /// `prepared.jsonl`.
    pub inputs_usable: usize,
    /// Of `inputs_usable`, how many carry a `Provenance::is_from_dataset`
    /// tag -- came from the source's own given or classified data, not
    /// generated to fill a gap. Always `0` for Suricata, SpamAssassin,
    /// and RegexLib (they never carry such a provenance); always equal
    /// to `inputs_usable` for S2ELab (it never generates); for
    /// Innovatorved, the two can mix within the same pattern -- see
    /// `inputs_usable_synthetic` for the number worth disclosing
    /// alongside any figure computed over its corpus.
    pub inputs_usable_from_source: usize,
    /// Of `patterns_usable`, top-level `^...$` wraps. Not necessarily
    /// in the same proportion as `patterns_explicit_full_match` is to
    /// `patterns_accepted` -- balancing can affect the two
    /// `MatchIntent` tiers unevenly.
    pub usable_explicit_full_match: usize,
    /// Of `patterns_usable`, patterns with no anchor at all.
    pub usable_unanchored_ambiguous: usize,
    /// Raw pattern text of every pattern counted in
    /// `patterns_unusable_at_generation`, in encounter order. Always
    /// collected (the cost is trivial); printing it is the caller's
    /// choice, e.g. `dataset_prepare --verbose`.
    pub unusable_at_generation_patterns: Vec<String>,
}

impl PipelineReport {
    /// `inputs_generated - inputs_verified`: candidates that failed
    /// verification against the real parser.
    pub fn inputs_discarded(&self) -> usize {
        self.inputs_generated - self.inputs_verified
    }

    /// `inputs_usable - inputs_usable_from_source`: of the inputs that
    /// made it into `prepared.jsonl`, how many were generated rather than
    /// taken from the source. A pattern whose inputs are mostly or
    /// entirely synthetic is not testing the source's own data, just a
    /// regex extracted from it -- this is the number that makes that
    /// distinction checkable instead of asserted.
    pub fn inputs_usable_synthetic(&self) -> usize {
        self.inputs_usable - self.inputs_usable_from_source
    }
}

/// Per-side candidate generation cap, as a multiple of `per_side`.
/// Every source generates at most `slack * per_side` candidates per
/// side, per strategy, before verification; `prepare::balance_cases`
/// keeps `per_side` per side after. The slack differs by how reliable
/// the source's candidates are:
///
/// - Structural sampling walks the pattern's own language, so its
///   outputs match by construction. `RegexLib` uses only this, so 1x.
/// - Suricata's content fields and SpamAssassin's corpus messages are
///   real bytes already filtered by the pattern, so most verify; 3x
///   absorbs the occasional mismatch from the concatenation or
///   full-string-matching step.
/// - S2ELab's given examples and Innovatorved's classified lines
///   frequently fail under full-string matching (the source data often
///   assumes substring search), so 10x.
///
/// Increasing a source's slack increases the corpus size only up to the
/// point where `per_side` verified cases per side already exist; beyond
/// that, the extra candidates are trimmed by `balance_cases` and only
/// cost verification time.
fn slack_for(source: SourceKind) -> usize {
    match source {
        SourceKind::RegexLib => 1,
        SourceKind::Suricata => 1,
        SourceKind::SpamAssassin => 1,
        SourceKind::S2ELab => 10,
        SourceKind::Innovatorved => 10,
    }
}

/// Runs extract → generate → prepare → balance for one source, writing
/// the balanced cases to `<data_root>/<source>/prepared.jsonl`
/// (replaced, not appended). See `prepare::balance_cases` for why this
/// last step runs, `slack_for` for the generation cap, and
/// `PipelineReport` for how to read its counts.
///
/// `inputs` is the CLI's `--inputs N`: the *total* verified inputs each
/// surviving pattern contributes, split evenly. It must be even; this
/// is `debug_assert`ed here as a safety net, but the real, user-facing
/// check belongs at the CLI boundary (`dataset_prepare`), not here.
pub fn run_pipeline(
    source: SourceKind,
    raw_files: &[PathBuf],
    data_root: &Path,
    corpus_dir: Option<&Path>,
    seed: u64,
    inputs: usize,
) -> std::io::Result<PipelineReport> {
    debug_assert!(inputs % 2 == 0, "--inputs must be even; got {inputs}");
    let per_side = (inputs / 2).max(1);
    let slack = slack_for(source);
    let cap = slack * per_side;

    let mut rng = StdRng::seed_from_u64(seed);
    let mut report = PipelineReport::default();

    let mut rules = Vec::new();
    for file in raw_files {
        let bytes = std::fs::read(file)?;
        let text = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => {
                // Decode lossily rather than aborting: a corpus can carry
                // legacy bytes, and a pattern touching them is a
                // per-pattern problem `core_regex` can reject on its own.
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

    for rule in &rules {
        let Ok((core, match_intent)) = prepare::core_regex_tagged(&rule.raw_pattern) else {
            report.patterns_unusable += 1;
            continue;
        };
        report.patterns_accepted += 1;
        match match_intent {
            MatchIntent::ExplicitFullMatch => report.patterns_explicit_full_match += 1,
            MatchIntent::UnanchoredAmbiguous => report.patterns_unanchored_ambiguous += 1,
        }

        // Each strategy below generates at most `cap` per side. `cap`
        // is `slack * per_side`, so total candidates per pattern are
        // bounded by `cap` times the number of strategies this source
        // uses, regardless of how much data the raw file ships.
        let mut candidates = Vec::new();

        match &rule.context {
            // S2ELab carries real matches *and* non-matches of its own;
            // structural sampling would only dilute them with synthetic
            // input a dataset chosen specifically to avoid that does not
            // need.
            RuleContext::S2ELab { matches, non_matches } => {
                candidates.extend(generate::generate_from_given_examples(
                    matches, non_matches, cap,
                ));
            }
            // Innovatorved carries only unlabeled free-form text, split
            // into lines and classified below: each line lands on
            // exactly one side, so unlike S2ELab this alone cannot be
            // trusted to cover both. A structural top-up runs alongside
            // it, unconditionally, so both sides are always present in
            // the candidate pool even for a pattern whose text gave
            // nothing usable. `balance_cases` prefers the real
            // (DatasetTextClassified) cases within each side, so the
            // top-up never displaces a real input that exists; the
            // trade-off is measured in `PipelineReport::inputs_usable_from_source`.
            RuleContext::Innovatorved { test_text } => {
                if let Some(text) = test_text {
                    let lines: Vec<String> =
                        text.lines().map(str::to_string).collect();
                    candidates.extend(generate::generate_from_dataset_text(
                        &lines, &core, cap, &mut rng,
                    ));
                }
                candidates.extend(generate::generate_sample(&core, &mut rng, cap, cap));
            }
            _ => {
                candidates.extend(generate::generate_sample(&core, &mut rng, cap, cap));

                if let RuleContext::Suricata { content_fields, .. } = &rule.context {
                    candidates.extend(generate::generate_from_content_fields(
                        content_fields,
                        &core,
                        &mut rng,
                        cap,
                    ));
                }

                if source == SourceKind::SpamAssassin {
                    if let Some(dir) = corpus_dir.filter(|d| d.is_dir()) {
                        candidates.extend(generate::generate_from_corpus_dir(
                            dir,
                            &rule.raw_pattern,
                            cap,
                            &mut rng,
                        ));
                    }
                }
            }
        }

        report.inputs_generated += candidates.len();
        let prepared = prepare::verify_all(source, &rule.raw_pattern, candidates);
        report.inputs_verified += prepared.len();

        let balanced = prepare::balance_cases(prepared, per_side);
        if balanced.is_empty() {
            report.patterns_unusable_at_generation += 1;
            report.unusable_at_generation_patterns.push(rule.raw_pattern.clone());
        } else {
            report.patterns_usable += 1;
            report.inputs_usable += balanced.len();
            report.inputs_usable_from_source +=
                balanced.iter().filter(|c| c.provenance.is_from_dataset()).count();
            match match_intent {
                MatchIntent::ExplicitFullMatch => report.usable_explicit_full_match += 1,
                MatchIntent::UnanchoredAmbiguous => report.usable_unanchored_ambiguous += 1,
            }
        }
        prepare::write_prepared_cases(&out_path, &balanced)?;
    }

    Ok(report)
}

/// Loads every prepared case for the given sources.
/// Empty `sources` means Suricata + SpamAssassin.
/// RegexLib, S2ELab, and Innovatorved are opt-in only; pass them
/// explicitly.
pub fn load_prepared_cases(
    data_root: &Path,
    sources: &[SourceKind],
) -> std::io::Result<Vec<PreparedCase>> {
    let sources: &[SourceKind] = if sources.is_empty() {
        &[SourceKind::Suricata, SourceKind::SpamAssassin]
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
    fn every_source_has_at_least_one_slack() {
        for source in [
            SourceKind::Suricata,
            SourceKind::SpamAssassin,
            SourceKind::RegexLib,
            SourceKind::S2ELab,
            SourceKind::Innovatorved,
        ] {
            assert!(slack_for(source) >= 1, "{source:?} has zero slack");
        }
    }
}