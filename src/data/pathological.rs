//! src/data/pathological.rs
//!
//! Loads and expands the hand-curated stress-pattern dataset at
//! `data/pathological/pathological_patterns.json`. A row carries either
//! a concrete `regex`, or a `pattern_template` + `generator` pair that
//! `expand` resolves at bench time. `match_input`/`nomatch_input`
//! expand the same way, parameterized by a swept input length `n` and,
//! for generator rows, a pattern size `p`.

use std::collections::HashMap;
use std::io;
use std::path::Path;

use serde::Deserialize;

/// Fallback input length when a row's category has no `suggested_n`.
pub const DEFAULT_N: u32 = 20;
/// Fallback pattern size when a row's category has no `suggested_p`.
pub const DEFAULT_P: u32 = 100;

/// Deterministic generated-word text: word `i` is `w{i:06}`.
fn word(i: u32) -> String {
    format!("w{i:06}")
}

#[derive(Debug, Clone, Deserialize)]
pub struct CategoryInfo {
    pub name: String,
    pub mechanism: String,
    pub stress_axis: String,
    #[serde(default)]
    pub suggested_n: Option<Vec<u32>>,
    #[serde(default)]
    pub suggested_p: Option<Vec<u32>>,
}

/// A `count` field: a fixed integer, or a `{N}`/`{P}` template,
/// optionally offset by a literal `+K`/`-K` (e.g. `{P}-1`).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CountSpec {
    Fixed(u32),
    Template(String),
}

impl CountSpec {
    pub fn resolve(&self, n: u32, p: u32) -> u32 {
        match self {
            CountSpec::Fixed(v) => *v,
            CountSpec::Template(s) => resolve_count_template(s, n, p),
        }
    }
}

fn resolve_count_template(s: &str, n: u32, p: u32) -> u32 {
    if s == "{N}" {
        return n;
    }
    if s == "{P}" {
        return p;
    }
    if let Some(rest) = s.strip_prefix("{P}-") {
        return p.saturating_sub(rest.parse().unwrap_or_else(|_| panic!("bad count offset: {s:?}")));
    }
    if let Some(rest) = s.strip_prefix("{P}+") {
        return p + rest.parse::<u32>().unwrap_or_else(|_| panic!("bad count offset: {s:?}"));
    }
    if let Some(rest) = s.strip_prefix("{N}-") {
        return n.saturating_sub(rest.parse().unwrap_or_else(|_| panic!("bad count offset: {s:?}")));
    }
    if let Some(rest) = s.strip_prefix("{N}+") {
        return n + rest.parse::<u32>().unwrap_or_else(|_| panic!("bad count offset: {s:?}"));
    }
    panic!("unexpected count template: {s:?}")
}

/// One `match_input`/`nomatch_input` value. `repeat_word`/`and_word`
/// are only ever present on `generated_patterns` rows.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum InputSpec {
    Literal {
        literal: String,
    },
    Repeat {
        repeat: String,
        count: CountSpec,
        #[serde(default)]
        prefix: Option<String>,
        #[serde(default)]
        suffix: Option<String>,
    },
    RepeatWord {
        repeat_word: u32,
        count: CountSpec,
        #[serde(default)]
        suffix: Option<String>,
        /// When set, the input is `word(repeat_word) x count` followed
        /// by `word(and_word)` -- checked before the plain `suffix` case.
        #[serde(default)]
        and_word: Option<u32>,
    },
}

impl InputSpec {
    pub fn expand(&self, n: u32, p: u32) -> String {
        match self {
            InputSpec::Literal { literal } => literal.clone(),
            InputSpec::Repeat { repeat, count, prefix, suffix } => format!(
                "{}{}{}",
                prefix.as_deref().unwrap_or(""),
                repeat.repeat(count.resolve(n, p) as usize),
                suffix.as_deref().unwrap_or(""),
            ),
            InputSpec::RepeatWord { repeat_word, count, suffix, and_word } => {
                let base = word(*repeat_word).repeat(count.resolve(n, p) as usize);
                match and_word {
                    Some(w) => format!("{base}{}", word(*w)),
                    None => format!("{base}{}", suffix.as_deref().unwrap_or("")),
                }
            }
        }
    }
}

/// One `generated_patterns` generator. `word_fn`/`p_values` in the raw
/// JSON are documentation only; `word` above and the caller's own `p`
/// sweep are used instead, the same as every other row's `n`/`p`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Generator {
    WordAlternation,
    FixedWidthTrie { width: usize, alphabet: String },
    SequentialGroups { body: String },
}

impl Generator {
    fn expand(&self, template: &str, p: u32) -> String {
        match self {
            Generator::WordAlternation => {
                let words: Vec<String> = (1..=p).map(word).collect();
                template.replace("{WORDS}", &words.join("|"))
            }
            Generator::FixedWidthTrie { width, alphabet } => {
                let alphabet: Vec<char> = alphabet.chars().collect();
                let base = alphabet.len() as u64;
                let mut words = Vec::with_capacity(p as usize);
                let mut i: u64 = 0;
                while words.len() < p as usize {
                    let mut s = String::new();
                    let mut k = i;
                    for _ in 0..*width {
                        s.insert(0, alphabet[(k % base) as usize]);
                        k /= base;
                    }
                    words.push(s);
                    i += 1;
                }
                template.replace("{WORDS}", &words.join("|"))
            }
            Generator::SequentialGroups { body } => body.repeat(p as usize),
        }
    }
}

/// One row from any of the three pattern arrays. `regex` is set for a
/// concrete pattern; a `generated_patterns` row that needs expansion
/// carries `pattern_template` + `generator` instead.
#[derive(Debug, Clone, Deserialize)]
pub struct PathologicalRow {
    pub id: String,
    pub category: String,
    #[serde(default)]
    pub regex: Option<String>,
    #[serde(default)]
    pub pattern_template: Option<String>,
    #[serde(default)]
    pub generator: Option<Generator>,
    #[serde(default)]
    pub match_input: Option<InputSpec>,
    #[serde(default)]
    pub nomatch_input: Option<InputSpec>,
    pub stress_axis: String,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Meta {
    pub purpose: String,
    pub engine_notes: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathologicalDoc {
    pub meta: Meta,
    pub categories: HashMap<String, CategoryInfo>,
    pub pathological_patterns: Vec<PathologicalRow>,
    pub baseline_and_edge_patterns: Vec<PathologicalRow>,
    pub generated_patterns: Vec<PathologicalRow>,
}

pub fn load(path: &Path) -> io::Result<PathologicalDoc> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// A row's regex, plus its expanded `match_input`/`nomatch_input`, for
/// one `(n, p)` sweep point. `p` is unused by a row with no `generator`.
pub fn expand(row: &PathologicalRow, n: u32, p: u32) -> (String, Option<String>, Option<String>) {
    let regex = match (&row.regex, &row.pattern_template, &row.generator) {
        (Some(r), _, _) => r.clone(),
        (None, Some(tmpl), Some(gen)) => gen.expand(tmpl, p),
        (None, Some(tmpl), None) => tmpl.clone(),
        (None, None, _) => panic!("row {} has neither `regex` nor `pattern_template`", row.id),
    };
    let match_input = row.match_input.as_ref().map(|spec| spec.expand(n, p));
    let nomatch_input = row.nomatch_input.as_ref().map(|spec| spec.expand(n, p));
    (regex, match_input, nomatch_input)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(json: &str) -> PathologicalRow {
        serde_json::from_str(json).expect("row should parse")
    }

    #[test]
    fn plain_n_templated_row_expands_at_two_n_values() {
        let r = row(
            r#"{
            "id": "P01", "category": "A", "regex": "(a+)+",
            "match_input": {"repeat": "a", "count": "{N}"},
            "nomatch_input": {"repeat": "a", "count": "{N}", "suffix": "b"},
            "stress_axis": "derivative-state explosion"
        }"#,
        );
        let (regex, m, nm) = expand(&r, 5, 0);
        assert_eq!(regex, "(a+)+");
        assert_eq!(m, Some("aaaaa".to_string()));
        assert_eq!(nm, Some("aaaaab".to_string()));

        let (_, m, nm) = expand(&r, 10, 0);
        assert_eq!(m, Some("a".repeat(10)));
        assert_eq!(nm, Some(format!("{}b", "a".repeat(10))));
    }

    #[test]
    fn null_match_input_yields_no_match_side() {
        let r = row(
            r#"{
            "id": "E22", "category": "L", "regex": "∅",
            "match_input": null,
            "nomatch_input": {"literal": "x"},
            "stress_axis": "constant"
        }"#,
        );
        let (regex, m, nm) = expand(&r, 20, 0);
        assert_eq!(regex, "\u{2205}");
        assert_eq!(m, None);
        assert_eq!(nm, Some("x".to_string()));
    }

    #[test]
    fn generator_less_generated_row_uses_its_own_regex() {
        let r = row(
            r#"{
            "id": "E42", "category": "K", "regex": "(?:a{100})+",
            "match_input": {"repeat": "a", "count": "{N}"},
            "nomatch_input": {"repeat": "a", "count": "{N}", "suffix": "b"},
            "stress_axis": "large pattern + large input"
        }"#,
        );
        assert!(r.generator.is_none() && r.pattern_template.is_none());
        let (regex, m, nm) = expand(&r, 3, 0);
        assert_eq!(regex, "(?:a{100})+");
        assert_eq!(m, Some("aaa".to_string()));
        assert_eq!(nm, Some("aaab".to_string()));
    }

    #[test]
    fn word_alternation_generator_substitutes_words() {
        let r = row(
            r#"{
            "id": "E36", "category": "J",
            "pattern_template": "(?:{WORDS})+",
            "generator": {"kind": "word_alternation"},
            "match_input": {"repeat_word": 1, "count": "{N}"},
            "nomatch_input": {"repeat_word": 1, "count": "{N}", "suffix": "!"},
            "stress_axis": "compile blow-up (large pat)"
        }"#,
        );
        let (regex, m, nm) = expand(&r, 2, 3);
        assert_eq!(regex, "(?:w000001|w000002|w000003)+");
        assert_eq!(m, Some("w000001w000001".to_string()));
        assert_eq!(nm, Some("w000001w000001!".to_string()));
    }

    #[test]
    fn fixed_width_trie_generator_enumerates_lexicographically() {
        let r = row(
            r#"{
            "id": "E37", "category": "J",
            "pattern_template": "(?:{WORDS})",
            "generator": {"kind": "fixed_width_trie", "width": 2, "alphabet": "ab"},
            "match_input": {"literal": "aa"},
            "nomatch_input": {"literal": "zz"},
            "stress_axis": "compile blow-up (large pat)"
        }"#,
        );
        let (regex, _, _) = expand(&r, 0, 4);
        assert_eq!(regex, "(?:aa|ab|ba|bb)");
    }

    #[test]
    fn sequential_groups_generator_repeats_the_body() {
        let r = row(
            r#"{
            "id": "E39", "category": "J",
            "pattern_template": "{P_GROUPS_OF_A}",
            "generator": {"kind": "sequential_groups", "body": "(a)"},
            "match_input": {"repeat": "a", "count": "{P}"},
            "nomatch_input": {"repeat": "a", "count": "{P}-1"},
            "stress_axis": "compile blow-up (large pat)"
        }"#,
        );
        let (regex, m, nm) = expand(&r, 0, 3);
        assert_eq!(regex, "(a)(a)(a)");
        assert_eq!(m, Some("aaa".to_string()));
        assert_eq!(nm, Some("aa".to_string()));
    }

    #[test]
    fn and_word_concatenates_two_literal_words() {
        let r = row(
            r#"{
            "id": "E38", "category": "J",
            "pattern_template": "(?:{WORDS})",
            "generator": {"kind": "word_alternation"},
            "match_input": {"repeat_word": 1, "count": 1},
            "nomatch_input": {"repeat_word": 1, "count": 1, "and_word": 2},
            "stress_axis": "compile blow-up (large pat)"
        }"#,
        );
        let (_, m, nm) = expand(&r, 0, 2);
        assert_eq!(m, Some("w000001".to_string()));
        assert_eq!(nm, Some("w000001w000002".to_string()));
    }

    #[test]
    fn the_real_dataset_file_loads_with_the_expected_section_sizes() {
        let doc = load(Path::new("data/pathological/pathological_patterns.json")).expect("should load");
        assert_eq!(doc.pathological_patterns.len(), 56);
        assert_eq!(doc.baseline_and_edge_patterns.len(), 43);
        assert_eq!(doc.generated_patterns.len(), 8);
        assert!(doc.categories.contains_key("A"));
        assert!(doc.categories.contains_key("L"));
    }
}
