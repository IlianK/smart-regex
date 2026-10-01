//! src/data/types.rs
//!
//! Shared types for the dataset preparation pipeline: `ExtractedRule`
//! (from `extract`), `Candidate` (from `generate`), `PreparedCase` (from
//! `prepare`), plus the `Category`, `Provenance`, and `SourceKind` each
//! one carries.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceKind {
    Suricata,
    SpamAssassin,
    RegexLib,
}

impl SourceKind {
    /// Directory under `data/processed/` this source writes to. Not the
    /// same as the raw corpus directory name.
    pub fn dir_name(self) -> &'static str {
        match self {
            SourceKind::Suricata => "snort",
            SourceKind::SpamAssassin => "spamassassin",
            SourceKind::RegexLib => "regexlib",
        }
    }
}

/// A Suricata `content:"..."` payload plus its modifiers.
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

/// Source-specific context an `ExtractedRule` carries, one variant per
/// `SourceKind`.
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
}

/// One rule as read from a raw source file: the pattern text plus context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedRule {
    pub source: SourceKind,
    pub raw_pattern: String,
    pub context: RuleContext,
}

/// Where a candidate's input came from, for the reader of a benchmark result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provenance {
    /// Real message sampled from a local corpus (SpamAssassin only).
    RealCorpus,
    /// Assembled from the rule's own `content:` fields (Suricata only).
    ContentDerived,
    /// Structural sample with a claimed match.
    Structural,
    /// Structural sample with a claimed non-match, built by corrupting a
    /// positive (see `generate::sample_negative_late`).
    StructuralNegative,
}

/// Benchmark category, decided per pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Best,
    Neutral,
    Worst,
}

/// One generated input, not yet checked against a real parser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub text: String,
    pub category: Category,
    pub provenance: Provenance,
    pub claimed_match: bool,
}

/// A candidate after `prepare::verify_candidate` accepted it: the claimed
/// outcome was checked against the real parser and held.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedCase {
    pub source: SourceKind,
    pub pattern: String,
    pub input: String,
    pub category: Category,
    pub provenance: Provenance,
    pub verified_match: bool,
    /// For a non-match, the number of characters consumed before rejection
    /// became structurally impossible; `None` for a match.
    pub failed_after_chars: Option<usize>,
}