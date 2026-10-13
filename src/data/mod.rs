//! src/data/mod.rs
//!
//! Pipeline entry points.
//! `run_pipeline` drives extract → generate → prepare for one source.
//! `load_prepared_cases` reads the result back for benchmarks.

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
    pub patterns_unusable: usize,
    /// Patterns `core_regex_tagged` could translate.
    /// Not what landed in `prepared.jsonl`; see `patterns_usable`.
    pub patterns_accepted: usize,
    /// Of `patterns_accepted`, top-level `^...$` wraps.
    pub patterns_explicit_full_match: usize,
    /// Of `patterns_accepted`, patterns with no anchor at all.
    pub patterns_unanchored_ambiguous: usize,
    /// Candidates produced before verification.
    /// Bounded per pattern by `slack_for(source)` times the strategies used.
    pub inputs_generated: usize,
    /// Of `inputs_generated`, how many verified against the real parser.
    /// `verify_until` stops as soon as `per_side` has been reached on both
    /// sides, so this equals `inputs_usable` plus whatever the below-target
    /// patterns happened to verify before their batch ran out.
    pub inputs_verified: usize,
    /// Of `inputs_generated`, how many were examined by `verify_one` and
    /// rejected: either the claim did not hold, or a negative failed
    /// before `LATE_FAILURE_THRESHOLD`.
    pub inputs_rejected: usize,
    /// Of `patterns_accepted`, how many never reached `per_side` verified
    /// matches and `per_side` verified non-matches.
    /// These contribute nothing to `prepared.jsonl`.
    /// Two common causes:
    /// too-small language, or examples assuming substring search while
    /// this frontend's matching is full-string-only.
    pub patterns_unusable_at_generation: usize,
    /// `patterns_accepted - patterns_unusable_at_generation`.
    /// What a benchmark over this source's `prepared.jsonl` actually sees.
    /// Each contributes exactly `per_side` matches and `per_side` non-matches.
    pub patterns_usable: usize,
    /// `patterns_usable` times the CLI's `--inputs N`.
    /// Total (pattern, input) rows written to `prepared.jsonl`.
    pub inputs_usable: usize,
    /// Of `inputs_usable`, how many came from the source's own data.
    /// Always 0 for Suricata, SpamAssassin, RegexLib.
    /// Always equal to `inputs_usable` for S2ELab.
    /// Innovatorved can mix within a pattern.
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

    /// `inputs_usable - inputs_usable_from_source`.
    pub fn inputs_usable_synthetic(&self) -> usize {
        self.inputs_usable - self.inputs_usable_from_source
    }

    /// `inputs_verified - inputs_usable`.
    /// Verified candidates that did not reach `prepared.jsonl` because
    /// their pattern fell below target.
    pub fn inputs_dropped_with_pattern(&self) -> usize {
        self.inputs_verified.saturating_sub(self.inputs_usable)
    }
}

/// Per-side candidate generation cap, as a multiple of `per_side`.
/// Every strategy generates at most `slack * per_side` per side.
/// Slack reflects how reliably the source's candidates verify:
/// - structural sampling walks the pattern's own language, so 1 is enough;
/// - content-derived and corpus messages are real bytes, mostly verifiable;
/// - given examples and classified lines often fail.
fn slack_for(source: SourceKind) -> usize {
    match source {
        SourceKind::Suricata => 1,
        SourceKind::SpamAssassin => 1,
        SourceKind::RegexLib => 3,
        SourceKind::S2ELab => 5,
        SourceKind::Innovatorved => 10,
    }
}

/// Runs extract → generate → prepare for one source.
/// Writes to `<data_root>/<source>/prepared.jsonl`, replacing any existing file.
///
/// `inputs` is the CLI's `--inputs N`: total verified inputs per surviving
/// pattern, split evenly.
/// Must be even; the CLI enforces this, and there is a debug assert as a
/// safety net here.
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
    let cap = slack_for(source) * per_side;

    let mut rng = StdRng::seed_from_u64(seed);
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

        // Each strategy below generates at most `cap` per side.
        // Total candidates per pattern stay bounded regardless of raw file size.
        let mut candidates = Vec::new();

        match &rule.context {
            // S2ELab carries real matches and non-matches of its own.
            // Structural sampling would dilute them with synthetic input
            // a dataset chosen to avoid that does not need.
            RuleContext::S2ELab { matches, non_matches } => {
                candidates.extend(generate::generate_from_given_examples(
                    matches, non_matches, cap,
                ));
            }
            // Innovatorved carries unlabeled free-form text.
            // Classify the text lines first, then top up only the shortfall
            // on each side.
            // A pattern whose text already fills both sides pays nothing
            // for structural sampling.
            // A pattern whose text is empty or all on one side pays only
            // for the other.
            // Real candidates are pushed before the top-up, so
            // `verify_until`'s first-come order prefers them.
            RuleContext::Innovatorved { test_text } => {
                let real: Vec<_> = test_text
                    .as_deref()
                    .map(|text| generate::generate_from_dataset_text(text, &core, cap, &mut rng))
                    .unwrap_or_default();
                let real_matches = real.iter().filter(|c| c.claimed_match).count();
                let real_non_matches = real.iter().filter(|c| !c.claimed_match).count();
                candidates.extend(real);
                candidates.extend(generate::generate_positives(
                    &core,
                    &mut rng,
                    cap.saturating_sub(real_matches),
                ));
                candidates.extend(generate::generate_negatives(
                    &core,
                    &mut rng,
                    cap.saturating_sub(real_non_matches),
                ));
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
        let (prepared, rejected) = prepare::verify_until(
            source,
            &rule.raw_pattern,
            &core,
            match_intent,
            candidates,
            per_side,
        );
        let prepared_len = prepared.len();
        report.inputs_verified += prepared_len;
        report.inputs_rejected += rejected;

        if prepared_len < per_side * 2 {
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
/// Empty `sources` means Suricata + SpamAssassin.
/// RegexLib, S2ELab, and Innovatorved are opt-in only.
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