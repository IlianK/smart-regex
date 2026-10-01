pub mod extract;
pub mod generate;
pub mod prepare;
pub mod types;

use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::SeedableRng;

use types::{Category, PreparedCase, RuleContext, SourceKind};

/// `patterns_unusable` is driven by `prepare::core_regex`, not
/// `frontend::parse_pcre_rule`, so it will not exactly match
/// `examples/filter_dataset.rs`'s "Rejected" count: `core_regex` has two
/// extra safety caps `parse_pcre_rule` does not (`MAX_CORE_DEPTH`,
/// `MAX_CORE_NODES`), so a handful of patterns `parse_pcre_rule` accepts
/// -- typically ones with a very large repeat bound, like `{500}` or
/// `{1000,}` -- still count as unusable here, before `patterns_not_faithful`
/// is even checked. That is by design: generating samples for a
/// 20,000-node regex is not worth doing even though the translation
/// itself is fine.
#[derive(Debug, Default, Clone, Copy)]
pub struct PipelineReport {
    pub rules_extracted: usize,
    pub patterns_unusable: usize,
    pub patterns_not_faithful: usize,
    /// Patterns that passed both `core_regex` and `frontend::is_faithful`
    /// and so actually had candidates generated for them: the count a
    /// reader actually wants, rather than `rules_extracted -
    /// patterns_unusable - patterns_not_faithful` computed by hand.
    pub patterns_faithful: usize,
    pub candidates_generated: usize,
    pub candidates_verified: usize,
}

/// Runs the full pipeline for one source: extract every rule from
/// `raw_files`, generate candidates for each faithful rule, verify them,
/// and write the survivors to `<data_root>/<source>/prepared.jsonl`. A
/// rule whose translation is accepted but only an approximation of what
/// the pattern means (`frontend::is_faithful` says so -- currently `R` or
/// a nested anchor) is skipped, the same as a rule that fails to
/// translate at all: this pipeline only ever benchmarks patterns whose
/// produced `Regex` denotes exactly what the source pattern means. The
/// output file is replaced, not appended to, at the start of each run.
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
                eprintln!(
                    "warning: {} is not valid UTF-8, decoding lossily rather than \
                     aborting the run -- a pattern touching the invalid bytes may \
                     come out corrupted, not just this file's progress lost: {}",
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
