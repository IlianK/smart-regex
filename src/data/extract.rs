
use crate::data::types::{ContentField, ExtractedRule, RuleContext, SaField, SourceKind};

/// Finds the end of a `/PATTERN/FLAGS` span at the start of `s`, respecting
/// escaped internal slashes (`\/`).
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

// -------------------------------
// Suricata / Snort
// -------------------------------

/// Extracts every `pcre:"..."` rule from Suricata/Snort `.rules` text,
/// paired with that rule's `content:` fields, `msg`, and `sid`.
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
        let pattern = rest[..end].to_string();
        out.push(ExtractedRule {
            source: SourceKind::Suricata,
            raw_pattern: pattern,
            context: RuleContext::Suricata {
                msg: extract_option_value(line, "msg"),
                sid: extract_option_value(line, "sid"),
                content_fields: extract_content_fields(line),
            },
        });
    }
    out
}

/// Reads a `key:"value";` or `key:value;` rule option's value.
fn extract_option_value(line: &str, key: &str) -> Option<String> {
    let needle = format!("{key}:");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    if rest.starts_with('"') {
        let end = rest[1..].find('"')? + 1;
        Some(rest[1..end].to_string())
    } else {
        let end = rest.find(';').unwrap_or(rest.len());
        Some(rest[..end].trim().to_string())
    }
}

/// Parses every `content:"..."` field on the line, with the modifiers
/// (`nocase`, `distance`, `within`, `depth`, `offset`) that appear between
/// it and the next `content:`/`pcre:`/end of the rule options.
fn extract_content_fields(line: &str) -> Vec<ContentField> {
    let mut fields = Vec::new();
    let mut search_from = 0usize;
    while let Some(rel) = line[search_from..].find("content:\"") {
        let content_start = search_from + rel + "content:\"".len();
        let Some(rel_end) = find_unescaped_quote(&line[content_start..]) else { break };
        let content_end = content_start + rel_end;
        let raw = &line[content_start..content_end];
        let bytes = decode_content_bytes(raw);

        // Modifiers run from just after the closing quote up to the next
        // `content:` (or `pcre:`, or end of string), so this field's own
        // slice never bleeds into the next one's.
        let mods_start = content_end + 1;
        let next_content = line[mods_start..]
            .find("content:")
            .map(|p| mods_start + p);
        let next_pcre = line[mods_start..].find("pcre:").map(|p| mods_start + p);
        let mods_end = [next_content, next_pcre]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(line.len());
        let modifiers = &line[mods_start..mods_end];

        fields.push(ContentField {
            bytes,
            nocase: modifiers.contains("nocase"),
            distance: read_i64_modifier(modifiers, "distance"),
            within: read_u64_modifier(modifiers, "within"),
            depth: read_u64_modifier(modifiers, "depth"),
            offset: read_u64_modifier(modifiers, "offset"),
        });

        search_from = content_end + 1;
    }
    fields
}

/// Finds the first `"` in `s` not preceded by a backslash.
fn find_unescaped_quote(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' && (i == 0 || bytes[i - 1] != b'\\') {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Decodes a Suricata `content:` payload: literal characters pass through
/// as their bytes, and `|XX XX XX|` groups are hex byte pairs.
fn decode_content_bytes(raw: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '|' {
            let mut hex_group = String::new();
            for hc in chars.by_ref() {
                if hc == '|' {
                    break;
                }
                hex_group.push(hc);
            }
            for pair in hex_group.split_whitespace() {
                if let Ok(byte) = u8::from_str_radix(pair, 16) {
                    out.push(byte);
                }
            }
        } else if c == '\\' {
            if let Some(&escaped) = chars.peek() {
                out.extend(escaped.to_string().as_bytes());
                chars.next();
            }
        } else {
            out.extend(c.to_string().as_bytes());
        }
    }
    out
}

fn read_i64_modifier(modifiers: &str, key: &str) -> Option<i64> {
    read_modifier_str(modifiers, key)?.parse().ok()
}

fn read_u64_modifier(modifiers: &str, key: &str) -> Option<u64> {
    read_modifier_str(modifiers, key)?.parse().ok()
}

fn read_modifier_str<'a>(modifiers: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("{key}:");
    let start = modifiers.find(&needle)? + needle.len();
    let rest = &modifiers[start..];
    let end = rest.find(';').unwrap_or(rest.len());
    Some(rest[..end].trim())
}

// -------------------------------
// SpamAssassin
// -------------------------------

/// Extracts every `body`/`header` rule from a SpamAssassin `.cf` file.
pub fn extract_spamassassin(text: &str) -> Vec<ExtractedRule> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        let field = if trimmed.starts_with("body ") {
            SaField::Body
        } else if trimmed.starts_with("header ") {
            SaField::Header
        } else {
            continue;
        };
        let Some(op) = line.find("=~") else { continue };
        let rule_name = trimmed
            .split_whitespace()
            .nth(1)
            .unwrap_or("")
            .to_string();
        let rest = line[op + 2..].trim_start();
        let Some(end) = find_pcre_span_end(rest) else { continue };
        out.push(ExtractedRule {
            source: SourceKind::SpamAssassin,
            raw_pattern: rest[..end].to_string(),
            context: RuleContext::SpamAssassin { rule_name, field },
        });
    }
    out
}

// -------------------------------
// RegexLib
// -------------------------------

/// Extracts RegexLib's comment-delimited entries: a block of `#`-prefixed
/// description lines followed by the bare pattern, separated by blank lines.
pub fn extract_regexlib(text: &str) -> Vec<ExtractedRule> {
    let mut out = Vec::new();
    let mut description_lines: Vec<&str> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            description_lines.clear();
            continue;
        }
        if let Some(desc) = trimmed.strip_prefix('#') {
            description_lines.push(desc.trim());
            continue;
        }
        out.push(ExtractedRule {
            source: SourceKind::RegexLib,
            raw_pattern: trimmed.to_string(),
            context: RuleContext::RegexLib {
                description: description_lines.join(" "),
            },
        });
        description_lines.clear();
    }
    out
}

// -------------------------------
// Tests
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suricata_extracts_pattern_and_content_fields() {
        let line = r##"alert http $EXTERNAL_NET $HTTP_PORTS -> $HOME_NET any (msg:"ET WEB_CLIENT Possible Microsoft Internet Explorer URI Validation Remote Code Execution Attempt"; flow:established,to_client; content:"#|3A|../../"; content:"C|3A 5C|"; nocase; within:50; pcre:"/\x2E\x2E\x2F\x2E\x2E\x2F.+C\x3A\x5C[a-z]/si"; reference:url,www.securityfocus.com/bid/37884; sid:2010798; rev:4;)"##;
        let rules = extract_suricata(line);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].raw_pattern, r"/\x2E\x2E\x2F\x2E\x2E\x2F.+C\x3A\x5C[a-z]/si");
        let RuleContext::Suricata { sid, content_fields, .. } = &rules[0].context else {
            panic!("expected Suricata context")
        };
        assert_eq!(sid.as_deref(), Some("2010798"));
        assert_eq!(content_fields.len(), 2);
        assert_eq!(content_fields[0].bytes, b"#:../../");
        assert_eq!(content_fields[1].bytes, b"C:\\");
        assert!(content_fields[1].nocase);
        assert_eq!(content_fields[1].within, Some(50));
    }

    #[test]
    fn suricata_decodes_hex_and_literal_content() {
        let bytes = decode_content_bytes("http|3a| -J-jar -J|5C 5C 5C 5C|");
        assert_eq!(bytes, b"http: -J-jar -J\\\\\\\\");
    }

    #[test]
    fn spamassassin_extracts_pattern_and_kind() {
        let line = r"header SUBJECT_DRUG_GAP_X	Subject =~ /x.{0,2}a.{0,2}n.{0,2}a.{0,2}x/i";
        let rules = extract_spamassassin(line);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].raw_pattern, r"/x.{0,2}a.{0,2}n.{0,2}a.{0,2}x/i");
        let RuleContext::SpamAssassin { rule_name, field } = &rules[0].context else {
            panic!("expected SpamAssassin context")
        };
        assert_eq!(rule_name, "SUBJECT_DRUG_GAP_X");
        assert_eq!(*field, SaField::Header);
    }

    #[test]
    fn spamassassin_skips_commented_lines() {
        let text = "#header FOO Subject =~ /a/\nbody BAR\tSubject =~ /b/";
        let rules = extract_spamassassin(text);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].raw_pattern, "/b/");
    }

    #[test]
    fn regexlib_extracts_pattern_and_description() {
        let text = "# Matches an ISBN\n# ID: 3642\n/[0-9]{10}/\n";
        let rules = extract_regexlib(text);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].raw_pattern, "/[0-9]{10}/");
        let RuleContext::RegexLib { description } = &rules[0].context else {
            panic!("expected RegexLib context")
        };
        assert!(description.contains("Matches an ISBN"));
    }
}
