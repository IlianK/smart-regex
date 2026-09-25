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
    pub patterns_unusable: usize,
    pub candidates_generated: usize,
    pub candidates_verified: usize,
}

/// Runs the full pipeline for one source: extract every rule from
/// `raw_files`, generate candidates for each, verify them, and write the
/// survivors to `<data_root>/<source>/prepared.jsonl`
pub fn run_pipeline(
    source: SourceKind,
    raw_files: &[PathBuf],
    data_root: &Path,
    corpus_dir: Option<&Path>,
    seed: u64,
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
        candidates.extend(generate::generate_best(&core, &mut rng, 8));
        candidates.extend(generate::generate_neutral(&core, &mut rng));
        candidates.extend(generate::generate_worst_structural(&core, &mut rng, 6));

        if let RuleContext::Suricata { content_fields, .. } = &rule.context {
            candidates.extend(generate::generate_from_content_fields(
                content_fields,
                &core,
                &mut rng,
            ));
        }

        if source == SourceKind::SpamAssassin {
            if let Some(dir) = corpus_dir.filter(|d| d.is_dir()) {
                candidates.extend(generate::generate_from_corpus_dir(
                    dir,
                    &rule.raw_pattern,
                    3,
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
/// given), optionally narrowed to one [`Category`]
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
