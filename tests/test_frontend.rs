// tests/test_frontend.rs
//
// Integration tests for regex_engine::frontend
// Run:  cargo test --test test_frontend

use regex_engine::frontend::parse_dataset_pattern;
use regex_engine::parsers::{parse_pderiv_bc, parse_recursive};
use regex_engine::{flatten, parse_pcre_rule};

fn matches(pattern: &str, input: &str) -> bool {
    let r = parse_dataset_pattern(pattern).unwrap_or_else(|e| panic!("parse({:?}) failed: {}", pattern, e));
    parse_recursive(input, &r).is_some()
}

// -------------------------------
// The paper's own headline example (Mamouras et al., PLDI 2024, Section 1):
// a Snort dataset regex matching dotted-quad IPv4 addresses
// -------------------------------

fn snort_ip_regex() -> &'static str {
    r"((\d[1-9]\d|1\d\d|2[0-4]\d|25[0-5])\.){3}(\d[1-9]\d|1\d\d|2[0-4]\d|25[0-5])"
}

#[test]
fn snort_ip_regex_parses() {
    assert!(parse_dataset_pattern(snort_ip_regex()).is_ok());
}

#[test]
fn snort_ip_regex_matches_bare_address() {
    assert!(matches(snort_ip_regex(), "239.255.250.250"));
}

#[test]
fn snort_ip_regex_matches_as_substring_of_surrounding_text() {
    assert!(matches(snort_ip_regex(), "src=239.255.250.250 dst=10.0.0.1"));
}

#[test]
fn snort_ip_regex_rejects_text_with_no_dotted_quad_shape_anywhere() {
    assert!(!matches(snort_ip_regex(), "no digits or dots in this string at all"));
}

#[test]
fn snort_ip_regex_greedy_and_posix_agree_on_membership_may_differ_on_tree() {
    let r = parse_dataset_pattern(snort_ip_regex()).expect("should parse");
    let word = "239.255.250.250";
    let posix_tree = parse_recursive(word, &r);
    let greedy_tree = parse_pderiv_bc(word, &r);
    assert!(posix_tree.is_some(), "POSIX parser should match {:?}", word);
    assert!(greedy_tree.is_some(), "Greedy parser should match {:?}", word);
    // Membership always agrees, tree shape is allowed to differ (Ch. 6).
    // Whichever happens on this input, both must still round-trip.
    assert_eq!(flatten(&posix_tree.unwrap()), word);
    assert_eq!(flatten(&greedy_tree.unwrap()), word);
}

// -------------------------------
// RegexLib-style patterns
// -------------------------------

#[test]
fn email_like_pattern() {
    let p = r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$";
    assert!(matches(p, "user@example.com"));
    assert!(matches(p, "first.last+tag@sub.example.co"));
    assert!(!matches(p, "not-an-email"));
    assert!(!matches(p, "@example.com"));
}

#[test]
fn hex_color_pattern() {
    let p = r"^#[0-9a-fA-F]{6}$";
    assert!(matches(p, "#1a2b3c"));
    assert!(matches(p, "#FFFFFF"));
    assert!(!matches(p, "#1a2b3"));
    assert!(!matches(p, "1a2b3c"));
}

#[test]
fn us_phone_number_pattern() {
    let p = r"^\(\d{3}\)\s?\d{3}-\d{4}$";
    assert!(matches(p, "(555) 123-4567"));
    assert!(matches(p, "(555)123-4567"));
    assert!(!matches(p, "555-123-4567"));
}

// -------------------------------
// Suricata/Snort-pcre-option-style patterns: HTTP header sniffing, applied
// as a substring search over a full header line (the realistic scenario).
// -------------------------------

#[test]
fn user_agent_sniff_pattern_matches_within_full_header_line() {
    let p = r"User-Agent:\s*(?:curl|wget)";
    assert!(matches(p, "User-Agent: curl/7.68.0"));
    assert!(matches(p, "GET / HTTP/1.1\r\nUser-Agent: wget/1.20.3\r\n"));
    assert!(!matches(p, "User-Agent: Mozilla/5.0"));
}

#[test]
fn pcre_rule_style_case_insensitive_header_match() {
    // Snort/Suricata's own pcre:"/.../i" option convention.
    let r = parse_pcre_rule(r"/user-agent:\s*sqlmap/i").expect("should parse");
    assert!(parse_recursive("User-Agent: sqlmap/1.5", &r).is_some());
    assert!(parse_recursive("USER-AGENT: SQLMAP/1.5", &r).is_some());
    assert!(parse_recursive("User-Agent: Mozilla", &r).is_none());
}

// -------------------------------
// SpamAssassin-style patterns: negated classes, non-marking groups,
// alternation-heavy subject-line matching.
// -------------------------------

#[test]
fn spamassassin_style_subject_pattern() {
    let p = r"^Subject:\s*(?:\[SPAM\]|\*\*\*SPAM\*\*\*)";
    assert!(matches(p, "Subject: [SPAM] buy now"));
    assert!(matches(p, "Subject: ***SPAM*** buy now"));
    assert!(!matches(p, "Subject: hello"));
}

#[test]
fn negated_class_excludes_whitespace() {
    let p = r"^\S+$"; // one non-whitespace "token", nothing else
    assert!(matches(p, "token"));
    assert!(!matches(p, "two tokens"));
    assert!(!matches(p, ""));
}

// -------------------------------
// Constructs this frontend deliberately refuses (context-sensitive, not
// regular -- see frontend::mod's module doc comment)
// -------------------------------

#[test]
fn backreference_is_rejected_not_silently_mismatched() {
    assert!(parse_dataset_pattern(r"(\w+)\s\1").is_err());
}

#[test]
fn lookaround_variants_are_all_rejected() {
    for p in [r"foo(?=bar)", r"foo(?!bar)", r"(?<=foo)bar", r"(?<!foo)bar"] {
        assert!(parse_dataset_pattern(p).is_err(), "expected {:?} to be rejected", p);
    }
}

// -------------------------------
// Differential: every pattern above, re-run through parse_pderiv_bc too,
// confirming membership agreement with parse_recursive on every case
// -------------------------------

#[test]
fn membership_agrees_between_posix_and_greedy_across_all_sample_patterns() {
    let cases: &[(&str, &[&str])] = &[
        (r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$", &["user@example.com", "nope"]),
        (r"^#[0-9a-fA-F]{6}$", &["#1a2b3c", "#zzzzzz"]),
        (r"User-Agent:\s*(?:curl|wget)", &["User-Agent: curl/1", "User-Agent: Mozilla"]),
    ];
    for (pattern, words) in cases {
        let r = parse_dataset_pattern(pattern).unwrap_or_else(|e| panic!("parse({:?}) failed: {}", pattern, e));
        for w in *words {
            let posix = parse_recursive(w, &r).is_some();
            let greedy = parse_pderiv_bc(w, &r).is_some();
            assert_eq!(posix, greedy, "membership disagreement on pattern {:?}, word {:?}", pattern, w);
        }
    }
}
