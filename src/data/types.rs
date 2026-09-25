
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceKind {
    Suricata,
    SpamAssassin,
    RegexLib,
}

impl SourceKind {
    pub fn dir_name(self) -> &'static str {
        match self {
            SourceKind::Suricata => "snort",
            SourceKind::SpamAssassin => "spamassassin",
            SourceKind::RegexLib => "regexlib",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentField {
    pub bytes: Vec<u8>,
    pub nocase: bool,
    pub distance: Option<i64>,
    pub within: Option<u64>,
    pub depth: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaField {
    Body,
    Header,
}

/// Source-specific context extracted alongside the bare pattern text.
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

/// A pattern as extracted from its source, before any input is generated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedRule {
    pub source: SourceKind,
    pub raw_pattern: String,
    pub context: RuleContext,
}

/// Where a candidate input came from. 
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provenance {
    RealCorpus,
    ContentDerived,
    Structural,
    StructuralNegative,
}

/// The three benchmark categories, decided per pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Best,
    Neutral,
    Worst,
}

/// One generated input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub text: String,
    pub category: Category,
    pub provenance: Provenance,
    pub claimed_match: bool,
}

/// A candidate after verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedCase {
    pub source: SourceKind,
    pub pattern: String,
    pub input: String,
    pub category: Category,
    pub provenance: Provenance,
    pub verified_match: bool,
    pub failed_after_chars: Option<usize>,
}
