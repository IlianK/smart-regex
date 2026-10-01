//! src/data/mod.rs
//!
//! Pipeline entry points: `run_pipeline` drives extract → generate →
//! prepare for one source and writes `prepared.jsonl`; `load_prepared_cases`
//! reads the result back for benchmarks.

pub mod extract;
pub mod generate;
pub mod prepare;
pub mod types;

use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::SeedableRng;

use types::{Category, PreparedCase, RuleContext, SourceKind};

#[derive(Debug, Default, Clone, Copy)]
pub struct PipelineReport {
    pub rules_extracted: usize,
    /// Rejected by `prepare::core_regex`. Not the same set as
    /// `frontend::parse_pcre_rule` rejects: `core_regex` adds two safety
    /// caps (`MAX_CORE_DEPTH`, `MAX_CORE_NODES`) that reject a few
    /// patterns translation alone accepts, typically ones with a very
    /// large repeat bound like `{500}`. Generating samples for a
    /// 20,000-node regex is not worth doing, even though translating it
    /// is fine.
    pub patterns_unusable: usize,
    /// Accepted by `core_regex` but not by `frontend::is_faithful`:
    /// currently `R` or a nested anchor. Excluded from the benchmark.
    pub patterns_not_faithful: usize,
    /// Passed both checks, and so had candidates generated. This is the
    /// count of patterns actually in `prepared.jsonl`.
    pub patterns_faithful: usize,
    pub candidates_generated: usize,
    pub candidates_verified: usize,
}

/// Runs extract → generate → prepare for one source, writing verified
/// cases to `<data_root>/<source>/prepared.jsonl` (replaced, not
/// appended). Only patterns for which `core_regex` succeeds and
/// `frontend::is_faithful` is true are benchmarked: an accepted
/// translation that is only an approximation of the source pattern's
/// meaning is skipped, the same as one that fails to translate.
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
                // legacy bytes (e.g. SpamAssassin's locale files), and a
                // pattern touching the invalid bytes is a per-pattern
                // problem `core_regex` can reject on its own merits.
                eprintln!(
                    "warning: {} is not valid UTF-8, decoding lossily: {}",
                    file.display(),
                    e
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
        let Ok(core) = prepare::core_regex(&rule.raw_pattern) else {
            report.patterns_unusable += 1;
            continue;
        };
        if !crate::frontend::is_faithful(&rule.raw_pattern) {
            report.patterns_not_faithful += 1;
            continue;
        }
        report.patterns_faithful += 1;

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

/// Loads every prepared case for the given sources (all three if empty),
/// optionally narrowed to one `Category`.
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