//! src/data/types.rs
//!
//! Shared types for the dataset preparation pipeline.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceKind {
    Suricata,
    SpamAssassin,
    RegexLib,
    /// github.com/s2e-lab/RegexEval: each pattern ships with
    /// author-labeled `matches`/`non_matches`, so generation here is
    /// "use what's given", not structural sampling. Named for the org
    /// rather than the repo, to match this project's own `data/raw`
    /// layout. Opt-in only, like `RegexLib`.
    S2ELab,
    /// github.com/innovatorved/regex_dataset: each pattern ships with
    /// free-form, *unlabeled* test text (a mix of matches and
    /// non-matches with no marker saying which). Opt-in only, like
    /// `RegexLib`.
    Innovatorved,
}

impl SourceKind {
    /// Dir under `data/processed/` (not the raw corpus dir name).
    pub fn dir_name(self) -> &'static str {
        match self {
            SourceKind::Suricata => "snort",
            SourceKind::SpamAssassin => "spamassassin",
            SourceKind::RegexLib => "regexlib",
            SourceKind::S2ELab => "s2-elab",
            SourceKind::Innovatorved => "innovatorved",
        }
    }
}

/// Suricata `content:"..."` payload plus modifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentField {
    pub bytes: Vec<u8>,
    pub nocase: bool,
    pub distance: Option<i64>,
    pub within: Option<u64>,
    pub depth: Option<u64>,
    pub offset: Option<u64>,
}

/// Which part of a message a SpamAssassin rule inspects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaField {
    Body,
    Header,
}

/// Source-specific context, one variant per `SourceKind`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleContext {
    Suricata {
        msg: Option<String>,
        sid: Option<String>,
        content_fields: Vec<ContentField>,
    },
    SpamAssassin {
        rule_name: String,
        field: SaField,
    },
    RegexLib {
        description: String,
    },
    S2ELab {
        matches: Vec<String>,
        non_matches: Vec<String>,
    },
    Innovatorved {
        /// Raw multi-line test text as given; `None` when the entry has
        /// none (`generate` then produces zero candidates for it).
        test_text: Option<String>,
    },
}

/// One rule read from a raw source file: pattern text plus context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedRule {
    pub source: SourceKind,
    pub raw_pattern: String,
    pub context: RuleContext,
}

/// How reliably a pattern's full-string-match reading reflects the
/// original author's intent (about the *pattern*, not the *input*).
///
/// Only `data/prepare.rs::core_regex_tagged` strips anchors.
/// Every other frontend entry point rejects `^`/`$` unconditionally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchIntent {
    /// Top-level `^...$` wrap; redundant under full-string matching.
    ExplicitFullMatch,
    /// No anchor anywhere; source tools search by default.
    UnanchoredAmbiguous,
}

/// Where a candidate's input came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provenance {
    /// Real corpus message (SpamAssassin only).
    RealCorpus,
    /// Assembled from the rule's own `content:` fields (Suricata only).
    ContentDerived,
    /// Structural sample, claimed match.
    Structural,
    /// Structural sample, claimed non-match (see `sample_negative_late`).
    StructuralNegative,
    /// S2ELab: one of the pattern author's own `matches`/`non_matches`.
    /// `claimed_match` says which; both are given, not sampled.
    DatasetGiven,
    /// Innovatorved: one line of the pattern's free-form test text,
    /// unlabeled in the source, classified against the real parser
    /// before being handed to `verify_candidate` as any other claim.
    DatasetTextClassified,
}

impl Provenance {
    /// Whether a *negative* (`claimed_match == false`) candidate of this
    /// provenance must additionally fail late (past
    /// `LATE_FAILURE_THRESHOLD`) to survive verification.
    /// `RealCorpus` and `ContentDerived` never produce negatives, so they
    /// are irrelevant here; listed explicitly rather than falling through
    /// a wildcard so a future provenance must make this choice too.
    pub fn requires_late_failure(self) -> bool {
        match self {
            Provenance::StructuralNegative
            | Provenance::DatasetGiven
            | Provenance::DatasetTextClassified => true,
            Provenance::RealCorpus | Provenance::ContentDerived | Provenance::Structural => false,
        }
    }

    /// Whether this candidate's *text* came from the source's own data,
    /// rather than being generated to fill a gap.
    /// `DatasetGiven` (S2ELab's own `matches`/`non_matches`) and
    /// `DatasetTextClassified` (Innovatorved's own free-form test lines)
    /// qualify; everything else -- including `RealCorpus`, which samples
    /// real external text but not text the pattern's own author supplied
    /// as a labeled example for it -- is generated.
    /// `run_pipeline` orders real candidates before the structural top-up
    /// so `verify_until` reaches them first.
    /// `PipelineReport` uses it to measure how much of a source's final
    /// corpus is real versus generated.
    /// Listed explicitly, as above, so a future provenance must make this
    /// choice too.
    pub fn is_from_dataset(self) -> bool {
        match self {
            Provenance::DatasetGiven | Provenance::DatasetTextClassified => true,
            Provenance::RealCorpus
            | Provenance::ContentDerived
            | Provenance::Structural
            | Provenance::StructuralNegative => false,
        }
    }
}

/// One generated input, not yet verified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub text: String,
    pub provenance: Provenance,
    pub claimed_match: bool,
}

/// A candidate after `prepare::verify_candidate` accepted it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedCase {
    pub source: SourceKind,
    pub pattern: String,
    pub input: String,
    pub provenance: Provenance,
    pub verified_match: bool,
    /// Chars consumed before rejection became impossible; `None` for a match.
    pub failed_after_chars: Option<usize>,
    /// See `MatchIntent`.
    pub match_intent: MatchIntent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_failure_is_required_for_every_negative_producing_provenance() {
        assert!(Provenance::StructuralNegative.requires_late_failure());
        assert!(Provenance::DatasetGiven.requires_late_failure());
        assert!(Provenance::DatasetTextClassified.requires_late_failure());
    }

    #[test]
    fn late_failure_is_not_required_for_positive_only_provenances() {
        assert!(!Provenance::RealCorpus.requires_late_failure());
        assert!(!Provenance::ContentDerived.requires_late_failure());
        assert!(!Provenance::Structural.requires_late_failure());
    }

    #[test]
    fn from_dataset_is_true_only_for_the_two_given_or_classified_provenances() {
        assert!(Provenance::DatasetGiven.is_from_dataset());
        assert!(Provenance::DatasetTextClassified.is_from_dataset());
    }

    #[test]
    fn from_dataset_is_false_for_every_generated_provenance() {
        assert!(!Provenance::RealCorpus.is_from_dataset());
        assert!(!Provenance::ContentDerived.is_from_dataset());
        assert!(!Provenance::Structural.is_from_dataset());
        assert!(!Provenance::StructuralNegative.is_from_dataset());
    }
}