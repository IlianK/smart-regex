//! src/data/extract.rs
//!
//! Per-source extraction: read a source's raw rule files and produce
//! `ExtractedRule` values carrying the bare pattern plus whatever
//! source-specific context the rest of the crate still reads (S2ELab's
//! given examples, Innovatorved's free-form test text). Suricata,
//! SpamAssassin and RegexLib carry no context payload -- nothing reads
//! one -- but their extractors stay: `dataset_stats`'s raw-corpus
//! diagnostics (coverage, dedup, rejection reasons, anchor shape, flags)
//! use all five, independent of whether a source feeds a benchmark.

use crate::data::types::{ExtractedRule, RuleContext, SourceKind};
use serde::Deserialize;

/// End of the `/PATTERN/FLAGS` span at the start of `s`.
/// Respects escaped internal slashes (`\/`).
fn find_pcre_span_end(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'/') {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() && bytes[i] != b'/' {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
        } else {
            i += 1;
        }
    }
    if i >= bytes.len() {
        return None;
    }
    i += 1;
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    Some(i)
}

/// Skip `n` leading whitespace-separated tokens; return the rest with
/// leading whitespace trimmed. Stands in for `SplitWhitespace::as_str`.
fn skip_whitespace_tokens(s: &str, n: usize) -> &str {
    let mut rest = s;
    for _ in 0..n {
        rest = rest.trim_start();
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        rest = &rest[end..];
    }
    rest.trim_start()
}

// -------------------------------
// Suricata / Snort
// -------------------------------

/// Every `pcre:"..."` rule's pattern in Suricata/Snort `.rules` text.
pub fn extract_suricata(text: &str) -> Vec<ExtractedRule> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        let Some(start) = line.find("pcre:\"") else { continue };
        let rest = &line[start + 6..];
        let Some(end) = find_pcre_span_end(rest) else { continue };
        if rest.as_bytes().get(end) != Some(&b'"') {
            continue;
        }
        out.push(ExtractedRule {
            source: SourceKind::Suricata,
            raw_pattern: rest[..end].to_string(),
            context: RuleContext::Suricata,
        });
    }
    out
}

// -------------------------------
// SpamAssassin
// -------------------------------

/// Every `body`/`header` rule's pattern from a SpamAssassin `.cf` file.
///
/// The two kinds differ: `header NAME Field =~ /pattern/flags` has the
/// pattern after `=~`, while `body NAME /pattern/flags` has no operator.
pub fn extract_spamassassin(text: &str) -> Vec<ExtractedRule> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        let rest = if trimmed.starts_with("body ") {
            skip_whitespace_tokens(trimmed, 2)
        } else if trimmed.starts_with("header ") {
            let Some(op) = line.find("=~") else { continue };
            line[op + 2..].trim_start()
        } else {
            continue;
        };
        let Some(end) = find_pcre_span_end(rest) else { continue };
        out.push(ExtractedRule {
            source: SourceKind::SpamAssassin,
            raw_pattern: rest[..end].to_string(),
            context: RuleContext::SpamAssassin,
        });
    }
    out
}

// -------------------------------
// RegexLib
// -------------------------------

/// RegexLib's comment-delimited entries: the bare pattern line, with
/// `#`-prefixed description lines and blank separators skipped.
pub fn extract_regexlib(text: &str) -> Vec<ExtractedRule> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        out.push(ExtractedRule {
            source: SourceKind::RegexLib,
            raw_pattern: trimmed.to_string(),
            context: RuleContext::RegexLib,
        });
    }
    out
}

// -------------------------------
// S2ELab (s2e-lab/RegexEval)
// -------------------------------

/// One entry of https://github.com/s2e-lab/RegexEval's
/// `DatasetCollection/RegexEval.json`: a bare (unwrapped) pattern plus
/// the author's own labeled match/non-match examples.
#[derive(Deserialize)]
struct S2ELabEntry {
    expression: String,
    #[serde(default)]
    matches: Vec<String>,
    #[serde(default)]
    non_matches: Vec<String>,
}

/// Every entry in a RegexEval-format JSON array.
/// A file that is not a JSON array of this shape yields no rules rather
/// than erroring, consistent with every other extractor here silently
/// skipping lines/entries it cannot read.
pub fn extract_s2_elab(text: &str) -> Vec<ExtractedRule> {
    let Ok(entries) = serde_json::from_str::<Vec<S2ELabEntry>>(text) else {
        return Vec::new();
    };
    entries
        .into_iter()
        .map(|e| ExtractedRule {
            source: SourceKind::S2ELab,
            raw_pattern: e.expression,
            context: RuleContext::S2ELab { matches: e.matches, non_matches: e.non_matches },
        })
        .collect()
}

// -------------------------------
// Innovatorved (innovatorved/regex_dataset)
// -------------------------------

/// One entry of https://github.com/innovatorved/regex_dataset's
/// `data.json`: a pattern plus free-form, *unlabeled* test text.
/// `text` is `null` for roughly one entry in nine in the real file, so
/// it is read as `Option<String>` rather than defaulted to `""`: a
/// present-but-empty string and an absent field are the same case here
/// either way, but the field is genuinely nullable in the source.
#[derive(Deserialize)]
struct InnovatorvedEntry {
    regex: Option<String>,
    text: Option<String>,
}

/// Every entry in an innovatorved/regex_dataset-format JSON array.
/// Entries with no pattern at all (seen twice in the real 8551-entry
/// file) are dropped here rather than reaching `core_regex_tagged` as
/// an empty string.
pub fn extract_innovatorved(text: &str) -> Vec<ExtractedRule> {
    let Ok(entries) = serde_json::from_str::<Vec<InnovatorvedEntry>>(text) else {
        return Vec::new();
    };
    entries
        .into_iter()
        .filter_map(|e| {
            let pattern = e.regex?;
            if pattern.trim().is_empty() {
                return None;
            }
            Some(ExtractedRule {
                source: SourceKind::Innovatorved,
                raw_pattern: pattern,
                context: RuleContext::Innovatorved { test_text: e.text },
            })
        })
        .collect()
}

// -------------------------------
// Tests
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suricata_extracts_the_pcre_pattern() {
        let line = r##"alert http $EXTERNAL_NET $HTTP_PORTS -> $HOME_NET any (msg:"ET WEB_CLIENT Possible Microsoft Internet Explorer URI Validation Remote Code Execution Attempt"; flow:established,to_client; content:"#|3A|../../"; content:"C|3A 5C|"; nocase; within:50; pcre:"/\x2E\x2E\x2F\x2E\x2E\x2F.+C\x3A\x5C[a-z]/si"; reference:url,www.securityfocus.com/bid/37884; sid:2010798; rev:4;)"##;
        let rules = extract_suricata(line);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].raw_pattern, r"/\x2E\x2E\x2F\x2E\x2E\x2F.+C\x3A\x5C[a-z]/si");
        assert_eq!(rules[0].context, RuleContext::Suricata);
    }

    #[test]
    fn spamassassin_extracts_a_header_pattern() {
        let line = r"header SUBJECT_DRUG_GAP_X	Subject =~ /x.{0,2}a.{0,2}n.{0,2}a.{0,2}x/i";
        let rules = extract_spamassassin(line);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].raw_pattern, r"/x.{0,2}a.{0,2}n.{0,2}a.{0,2}x/i");
        assert_eq!(rules[0].context, RuleContext::SpamAssassin);
    }

    #[test]
    fn spamassassin_skips_commented_lines() {
        let text = "#header FOO Subject =~ /a/\nbody BAR\t/b/";
        let rules = extract_spamassassin(text);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].raw_pattern, "/b/");
    }

    #[test]
    fn spamassassin_extracts_body_rule_with_no_operator() {
        let text = "body WEIRD_QUOTING\t/[\\042\\223]{2}/";
        let rules = extract_spamassassin(text);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].raw_pattern, "/[\\042\\223]{2}/");
    }

    #[test]
    fn spamassassin_skips_eval_body_rule() {
        let text = "body MPART_ALT_DIFF\teval:multipart_alternative_difference('99', '100')";
        assert_eq!(extract_spamassassin(text).len(), 0);
    }

    #[test]
    fn regexlib_extracts_the_bare_pattern() {
        let text = "# Matches an ISBN\n# ID: 3642\n/[0-9]{10}/\n";
        let rules = extract_regexlib(text);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].raw_pattern, "/[0-9]{10}/");
        assert_eq!(rules[0].context, RuleContext::RegexLib);
    }

    #[test]
    fn s2_elab_extracts_pattern_and_examples() {
        let text = r#"[
            {
                "expression": "^\\d{5}$",
                "raw_prompt": "A zip code.",
                "refined_prompt": "A zip code.",
                "matches": ["12345", "00000"],
                "non_matches": ["abcde", "1234"],
                "id": 1
            }
        ]"#;
        let rules = extract_s2_elab(text);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].raw_pattern, r"^\d{5}$");
        let RuleContext::S2ELab { matches, non_matches } = &rules[0].context else {
            panic!("expected S2ELab context")
        };
        assert_eq!(matches, &vec!["12345".to_string(), "00000".to_string()]);
        assert_eq!(non_matches, &vec!["abcde".to_string(), "1234".to_string()]);
    }

    #[test]
    fn s2_elab_not_a_json_array_yields_no_rules() {
        assert_eq!(extract_s2_elab("not json").len(), 0);
        assert_eq!(extract_s2_elab("{}").len(), 0);
    }

    #[test]
    fn innovatorved_extracts_pattern_and_test_text() {
        let text = r#"[
            {
                "created_at": "2019-07-30T16:27:46.000Z",
                "title": "ex1",
                "description": "",
                "text": ">>Sofa<<312.23!3\n>>TV<<300!5",
                "regex": ">>(?<name>[A-Za-z]+)<<(?<price>\\d+\\.?\\d+)!(?<quantity>\\d+)"
            }
        ]"#;
        let rules = extract_innovatorved(text);
        assert_eq!(rules.len(), 1);
        assert!(rules[0].raw_pattern.starts_with(">>"));
        let RuleContext::Innovatorved { test_text } = &rules[0].context else {
            panic!("expected Innovatorved context")
        };
        assert_eq!(test_text.as_deref(), Some(">>Sofa<<312.23!3\n>>TV<<300!5"));
    }

    #[test]
    fn innovatorved_drops_entries_with_no_pattern_but_keeps_null_text() {
        let text = r#"[
            {"created_at": "x", "title": "empty", "description": "", "text": null, "regex": ""},
            {"created_at": "x", "title": "null-regex", "description": "", "text": null, "regex": null},
            {"created_at": "x", "title": "kept", "description": "", "text": null, "regex": "a+"}
        ]"#;
        let rules = extract_innovatorved(text);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].raw_pattern, "a+");
        let RuleContext::Innovatorved { test_text } = &rules[0].context else {
            panic!("expected Innovatorved context")
        };
        assert_eq!(*test_text, None);
    }
}
