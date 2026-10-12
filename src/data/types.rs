//! src/data/types.rs
//!
//! Shared types for the dataset preparation pipeline.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceKind {
    Suricata,
    SpamAssassin,
    RegexLib,
}

impl SourceKind {
    /// Dir under `data/processed/` (not the raw corpus dir name).
    pub fn dir_name(self) -> &'static str {
        match self {
            SourceKind::Suricata => "snort",
            SourceKind::SpamAssassin => "spamassassin",
            SourceKind::RegexLib => "regexlib",
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