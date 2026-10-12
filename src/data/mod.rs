//! src/data/mod.rs
//!
//! Pipeline entry points.
//! `run_pipeline` drives extract → generate → prepare for one source and
//! writes `prepared.jsonl`.
//! `load_prepared_cases` reads the result back for benchmarks.

pub mod extract;
pub mod generate;
pub mod prepare;
pub mod types;

use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::SeedableRng;

use types::{MatchIntent, PreparedCase, RuleContext, SourceKind};

#[derive(Debug, Default, Clone, Copy)]
pub struct PipelineReport {
    pub rules_extracted: usize,
    /// Rejected by `prepare::core_regex_tagged`.
    /// Same set as `frontend::parse_pcre_rule` rejects, plus the two
    /// safety caps (`MAX_CORE_DEPTH`, `MAX_CORE_NODES`).
    pub patterns_unusable: usize,
    /// Accepted patterns, i.e. what landed in `prepared.jsonl`.
    pub patterns_accepted: usize,
    /// Of `patterns_accepted`, top-level `^...$` wraps.
    pub patterns_explicit_full_match: usize,
    /// Of `patterns_accepted`, patterns with no anchor at all.
    /// See `types::MatchIntent`.
    pub patterns_unanchored_ambiguous: usize,
    /// Of `patterns_explicit_full_match`, how many were skipped for
    /// candidate generation (see `run_pipeline`).
    /// Always equal to `patterns_explicit_full_match`.
    pub patterns_explicit_full_match_skipped: usize,
    pub candidates_generated: usize,
    pub candidates_verified: usize,
}

/// Runs extract → generate → prepare for one source, writing verified
/// cases to `<data_root>/<source>/prepared.jsonl` (replaced, not
/// appended).
///
/// Generation only runs for `MatchIntent::UnanchoredAmbiguous` patterns.
/// `ExplicitFullMatch` patterns are counted but not generated for yet;
/// keeping this first pass to the plainer case avoids mixing two
/// generation stories while that tier is still thin.
pub fn run_pipeline(
    source: SourceKind,
    raw_files: &[PathBuf],
    data_root: &Path,
    corpus_dir: Option<&Path>,
    seed: u64,
    variants: usize,
) -> std::io::Result<PipelineReport> {
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
        if match_intent == MatchIntent::ExplicitFullMatch {
            report.patterns_explicit_full_match += 1;
            report.patterns_explicit_full_match_skipped += 1;
            continue;
        }
        report.patterns_unanchored_ambiguous += 1;

        let mut candidates = Vec::new();
        candidates.extend(generate::generate_sample(&core, &mut rng, variants, variants));

        if let RuleContext::Suricata { content_fields, .. } = &rule.context {
            candidates.extend(generate::generate_from_content_fields(
                content_fields,
                &core,
                &mut rng,
                variants,
            ));
        }

        if source == SourceKind::SpamAssassin {
            if let Some(dir) = corpus_dir.filter(|d| d.is_dir()) {
                candidates.extend(generate::generate_from_corpus_dir(
                    dir,
                    &rule.raw_pattern,
                    variants,
                    &mut rng,
                ));
            }
        }

        report.candidates_generated += candidates.len();
        let prepared = prepare::verify_all(source, &rule.raw_pattern, candidates);
        report.candidates_verified += prepared.len();
        prepare::write_prepared_cases(&out_path, &prepared)?;
    }

    Ok(report)
}

/// Loads every prepared case for the given sources.
/// Empty `sources` means Suricata + SpamAssassin.
/// RegexLib is opt-in only; pass it explicitly.
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