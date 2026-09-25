//! Dataset preparation pipeline: extract patterns from Suricata/Snort,
//! SpamAssassin, and RegexLib source files, generate best/neutral/worst
//! input candidates for each, verify every candidate against the real
//! parser, and write the survivors out per source under
//! `data/processed/<source>/` (the caller picks the root; see
//! `examples/prepare_dataset.rs`).
//!
//! The three stages are deliberately separate modules:
//!
//! - [`extract`]: turns raw rule files into [`types::ExtractedRule`]
//!   values, one pattern plus whatever source-specific context (Suricata
//!   `content:` fields, SpamAssassin rule kind, RegexLib description)
//!   later stages need.
//! - [`generate`]: turns one extracted rule into [`types::Candidate`]
//!   inputs, using a strategy chosen for that source. Nothing here is
//!   trusted; every candidate carries a claimed outcome that `prepare`
//!   checks rather than assumes.
//! - [`prepare`]: verifies every candidate against the real parser this
//!   pipeline exists to feed, discards what does not hold up (including a
//!   worst-case negative that fails too early to count as a worst case),
//!   and writes the survivors out as [`types::PreparedCase`] records.
//!
//! [`run_pipeline`] drives all three for one source. A benchmark reads the
//! result with [`prepare::read_prepared_cases`] and decides for itself
//! whether to use one source, several, or filter by [`types::Category`].

pub mod extract;
pub mod generate;
pub mod prepare;
pub mod types;

use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::SeedableRng;

use types::{Category, PreparedCase, RuleContext, SourceKind};

/// Counts from one `run_pipeline` call, for reporting what happened
/// without having to re-read the output file just to know.
#[derive(Debug, Default, Clone, Copy)]
pub struct PipelineReport {
    pub rules_extracted: usize,
    pub patterns_unusable: usize,
    pub candidates_generated: usize,
    pub candidates_verified: usize,
}

/// Runs the full pipeline for one source: extract every rule from
/// `raw_files`, generate candidates for each, verify them, and write the
/// survivors to `<data_root>/<source>/prepared.jsonl`. The output file is
/// replaced, not appended to, at the start of each run: everything in it
/// was verified by the generator code that produced this run, not
/// accumulated silently across runs that may have generated differently.
///
/// `corpus_dir`, used only for [`SourceKind::SpamAssassin`], points at a
/// local directory of real messages (one per file); pass `None`, or a path
/// that does not exist, to fall back to the structural generator alone,
/// since this pipeline never fetches or fabricates corpus data on its own.
///
/// `variants` is how many independently verified inputs each pattern
/// contributes per category (`Best`, `Neutral`, and each of `Worst`'s two
/// structural sub-kinds), not a total across categories; a pattern with
/// fewer than `variants` distinct samples available (a short, unambiguous
/// pattern like `a`) contributes however many distinct ones actually
/// exist rather than padding with duplicates.
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
        let text = std::fs::read_to_string(file)?;
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
        let Ok(core) = prepare::core_regex(&rule.raw_pattern) else {
            report.patterns_unusable += 1;
            continue;
        };

        let mut candidates = Vec::new();
        candidates.extend(generate::generate_best(&core, &mut rng, variants));
        candidates.extend(generate::generate_neutral(&core, &mut rng, variants));
        candidates.extend(generate::generate_worst_structural(&core, &mut rng, 6, variants));

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

/// Loads every prepared case for the given sources (all three if none are
/// given), optionally narrowed to one [`Category`], the shape a benchmark
/// actually wants: one source or several, one category or every one.
pub fn load_prepared_cases(
    data_root: &Path,
    sources: &[SourceKind],
    category: Option<Category>,
) -> std::io::Result<Vec<PreparedCase>> {
    let sources: &[SourceKind] = if sources.is_empty() {
        &[SourceKind::Suricata, SourceKind::SpamAssassin, SourceKind::RegexLib]
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
        out.extend(cases.into_iter().filter(|c| category.is_none_or(|cat| c.category == cat)));
    }
    Ok(out)
}
