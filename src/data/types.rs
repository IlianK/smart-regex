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

/// Source-specific context, one variant per `SourceKind`. Suricata,
/// SpamAssassin and RegexLib carry no payload: nothing reads one, since
/// only `dataset_stats`'s raw-corpus diagnostics use those three extractors,
/// and it only needs each rule's raw pattern text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleContext {
    Suricata,
    SpamAssassin,
    RegexLib,
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

/// One S2ELab-given (pattern, input) pair that `core_regex_tagged` accepted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedCase {
    pub source: SourceKind,
    pub pattern: String,
    pub input: String,
    /// The dataset author's own label; never independently re-checked.
    pub expected_match: bool,
    /// See `MatchIntent`.
    pub match_intent: MatchIntent,
}
