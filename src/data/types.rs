//! src/data/types.rs
//!
//! Shared types for the dataset preparation pipeline.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceKind {
    Suricata,
    SpamAssassin,
    RegexLib,
    S2ELab,
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

/// Only `data/prepare.rs::core_regex_tagged` strips anchors.
/// Every other entry point rejects `^`/`$` unconditionally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchIntent {
    /// Top-level `^...$` wrap; redundant under full-string matching.
    ExplicitFullMatch,
    /// No anchor anywhere; source tools search by default.
    UnanchoredAmbiguous,
}

/// Where a candidate's input came from.
///
/// `Structural`/`StructuralNegative` are produced only by the structural
/// sampler in `data::generate` (`sample_positive`/`sample_negative_late`),
/// which is no longer part of the benchmark/dataset pipeline (`run_pipeline`
/// never calls it) -- it survives only as a fixture generator for
/// `prepare.rs`'s own unit tests and for the illustrative
/// `demo_frontend_categories` example, neither of which makes a research
/// claim about parser correctness over generated input. `DatasetGiven` is
/// the only provenance `run_pipeline` produces today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provenance {
    /// Structural sample, claimed match. Test/demo fixture only.
    Structural,
    /// Structural sample, claimed non-match. Test/demo fixture only.
    StructuralNegative,
    /// S2ELab: one of the pattern author's own `matches`/`non_matches`.
    DatasetGiven,
    /// Innovatorved: one line of the pattern's free-form test text,
    DatasetTextClassified,
}

impl Provenance {
    /// Whether a *negative* (`claimed_match == false`) candidate of this
    /// provenance must additionally fail late (past
    /// `LATE_FAILURE_THRESHOLD`) to survive verification.
    pub fn requires_late_failure(self) -> bool {
        match self {
            Provenance::StructuralNegative
            | Provenance::DatasetGiven
            | Provenance::DatasetTextClassified => true,
            Provenance::Structural => false,
        }
    }

    /// Whether this candidate's *text* came from the source's own data
    /// or was being generated to fill a gap.
    pub fn is_from_dataset(self) -> bool {
        match self {
            Provenance::DatasetGiven | Provenance::DatasetTextClassified => true,
            Provenance::Structural | Provenance::StructuralNegative => false,
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
        assert!(!Provenance::Structural.requires_late_failure());
    }

    #[test]
    fn from_dataset_is_true_only_for_the_two_given_or_classified_provenances() {
        assert!(Provenance::DatasetGiven.is_from_dataset());
        assert!(Provenance::DatasetTextClassified.is_from_dataset());
    }

    #[test]
    fn from_dataset_is_false_for_every_generated_provenance() {
        assert!(!Provenance::Structural.is_from_dataset());
        assert!(!Provenance::StructuralNegative.is_from_dataset());
    }
}