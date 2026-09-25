//! Shared types for the dataset preparation pipeline: what a rule looks like
//! per source, what a generated input candidate is, and how a verified case
//! is categorized and recorded.

use serde::{Deserialize, Serialize};

/// Which corpus a rule came from. Each source needs its own extraction and
/// generation strategy, so this tag threads through the whole pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceKind {
    Suricata,
    SpamAssassin,
    RegexLib,
}

impl SourceKind {
    /// The directory name this source's files live under, `data/<name>/`.
    pub fn dir_name(self) -> &'static str {
        match self {
            SourceKind::Suricata => "snort",
            SourceKind::SpamAssassin => "spamassassin",
            SourceKind::RegexLib => "regexlib",
        }
    }
}

/// One `content:` match field from a Suricata/Snort rule, decoded from its
/// `text` / `|hex bytes|` mixed syntax into raw bytes, with the modifiers
/// that constrain where it may appear relative to the rest of the rule.
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
    /// The raw `/PATTERN/FLAGS` (or bare pattern) text, exactly as it would
    /// be handed to `parse_pcre_rule`/`parse_dataset_pattern`.
    pub raw_pattern: String,
    pub context: RuleContext,
}

/// Where a candidate input came from. Kept alongside every generated string
/// so a benchmark result can be traced back to how trustworthy its input is:
/// a real corpus snippet is a stronger claim than a structurally sampled one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provenance {
    /// Sampled from a real, separately downloaded corpus (SpamAssassin
    /// ham/spam). Never fabricated at runtime; absent if no corpus is
    /// configured.
    RealCorpus,
    /// Assembled from the rule's own `content:` fields (Suricata).
    ContentDerived,
    /// Sampled by walking the pattern's own structure.
    Structural,
    /// A structural positive, corrupted near the end to produce a
    /// late-failing negative.
    StructuralNegative,
}

/// The three benchmark categories, decided per pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    /// Short, unambiguous, cheap: establishes a baseline cost.
    Best,
    /// A realistic-length input with no engineered pathology.
    Neutral,
    /// Engineered to be expensive: either genuinely ambiguous/long, or a
    /// negative that only fails late.
    Worst,
}

/// One generated input, not yet checked against a real parser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub text: String,
    pub category: Category,
    pub provenance: Provenance,
    /// What the generator believes the outcome should be. `prepare` checks
    /// this rather than trusting it.
    pub claimed_match: bool,
}

/// A candidate after verification: the ground truth this pipeline actually
/// produces, safe to hand to a benchmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedCase {
    pub source: SourceKind,
    pub pattern: String,
    pub input: String,
    pub category: Category,
    pub provenance: Provenance,
    /// The real outcome, from actually running the parser. This is the
    /// fact this whole pipeline exists to establish; `claimed_match` above
    /// is only ever a proposal.
    pub verified_match: bool,
    /// For a worst-case negative: how many characters were consumed before
    /// the parse was known to be impossible. `None` for anything that
    /// matched, or wasn't checked for lateness.
    pub failed_after_chars: Option<usize>,
}
