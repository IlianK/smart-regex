//! examples/demo_translate_categories.rs
//!
//! cargo run --release --example demo_translate_categories
//!
//! One hand-authored pattern per accepted frontend construct, each run
//! against a chosen match and non-match input through one real parser.
//! Breadth, not a real corpus: every row is expected to pass -- a
//! failure here is a bug in this file, not a dataset gap, so there is
//! no "skipped" escape hatch.
//!
//! Writes a compact per-category summary to
//! reports/demos/translate_categories.csv (name, pattern, checks
//! passed/total, pass) and the full listing -- the translated `Regex`
//! for each category plus every individual check -- separately, to
//! reports/demos/translate_categories_patterns.json: several of these
//! `Regex`es are very large (a `Debug`-rendered `.` alone is a couple
//! hundred `Alt` nodes), exactly the kind of listing-of-patterns this
//! project's report files keep out of their summary table.

#[path = "common/thesis_figures.rs"]
mod common;

use regex_engine::frontend::{parse_pattern, parse_pcre_rule};
use regex_engine::parsers::parse_deriv_std_rec;
use regex_engine::types::Regex;

use common::{csv_quote, save};

/// `pattern` goes through `parse_pcre_rule` when it carries a
/// `/PATTERN/FLAGS` wrapper, `parse_pattern` otherwise -- the same
/// choice `core_regex` itself makes per-pattern.
fn translate_for(pattern: &str) -> Result<Regex, String> {
    if pattern.starts_with('/') {
        parse_pcre_rule(pattern)
    } else {
        parse_pattern(pattern)
    }
}

struct Category {
    name: &'static str,
    pattern: &'static str,
    /// (input, expected `is_some()` of the real parser's result).
    checks: &'static [(&'static str, bool)],
}

/// One row per construct `src/frontend/translate.rs` and `parse.rs`
/// accept. Grouped roughly as the Background section orders them.
fn categories() -> Vec<Category> {
    vec![
        Category { name: "literal character", pattern: "a", checks: &[("a", true), ("b", false)] },
        Category {
            name: "'.' (bare parse_pattern: includes '\\n')",
            pattern: ".",
            checks: &[("\n", true), ("", false)],
        },
        Category {
            name: "'.' under /PATTERN/ (no s flag: excludes '\\n')",
            pattern: "/./",
            checks: &[("x", true), ("\n", false)],
        },
        Category { name: "escape \\d (digit)", pattern: r"\d", checks: &[("7", true), ("x", false)] },
        Category { name: "escape \\D (non-digit)", pattern: r"\D", checks: &[("x", true), ("7", false)] },
        Category { name: "escape \\w (word)", pattern: r"\w", checks: &[("a", true), ("-", false)] },
        Category { name: "escape \\W (non-word)", pattern: r"\W", checks: &[("-", true), ("a", false)] },
        Category { name: "escape \\s (space)", pattern: r"\s", checks: &[(" ", true), ("a", false)] },
        Category { name: "escape \\S (non-space)", pattern: r"\S", checks: &[("a", true), (" ", false)] },
        Category {
            name: "\\n \\t \\r literal escapes",
            pattern: "\\n\\t\\r",
            checks: &[("\n\t\r", true), ("abc", false)],
        },
        Category { name: "\\xHH hex escape", pattern: r"\x41", checks: &[("A", true), ("B", false)] },
        Category {
            name: "octal escape outside a class (\\0 family)",
            pattern: r"\012",
            checks: &[("\n", true), ("a", false)],
        },
        Category {
            name: "octal escape inside a class (\\0-\\7)",
            pattern: r"[\101]",
            checks: &[("A", true), ("B", false)],
        },
        Category {
            name: "character class with a range",
            pattern: "[a-c]",
            checks: &[("b", true), ("d", false)],
        },
        Category { name: "negated class", pattern: "[^abc]", checks: &[("d", true), ("a", false)] },
        Category {
            name: "bounded quantifier {m,n}",
            pattern: "a{2,4}",
            checks: &[("aaa", true), ("a", false)],
        },
        Category {
            name: "bounded quantifier {m,}",
            pattern: "a{2,}",
            checks: &[("aaaaa", true), ("a", false)],
        },
        Category { name: "bounded quantifier {m}", pattern: "a{3}", checks: &[("aaa", true), ("aa", false)] },
        Category { name: "alternation '|'", pattern: "a|b", checks: &[("b", true), ("c", false)] },
        Category { name: "star '*'", pattern: "a*", checks: &[("aaa", true), ("b", false)] },
        Category { name: "plus '+'", pattern: "a+", checks: &[("aaa", true), ("", false)] },
        Category { name: "opt '?'", pattern: "a?", checks: &[("a", true), ("aa", false)] },
        Category {
            name: "lazy marker discarded (a*? behaves as a*)",
            pattern: "a*?",
            checks: &[("aa", true), ("ab", false)],
        },
        Category {
            name: "capturing group '(...)' (no capture semantics kept)",
            pattern: "(ab)",
            checks: &[("ab", true), ("a", false)],
        },
        Category {
            name: "non-capturing group '(?:...)'",
            pattern: "(?:ab)",
            checks: &[("ab", true), ("a", false)],
        },
        Category {
            name: "named group, either syntax: (?<name>...) / (?P<name>...)",
            pattern: "(?P<year>ab)",
            checks: &[("ab", true), ("a", false)],
        },
        Category {
            name: "case-folding via /i",
            pattern: "/abc/i",
            checks: &[("AbC", true), ("abd", false)],
        },
        Category { name: "'ε' token (empty word)", pattern: "ε", checks: &[("", true), ("a", false)] },
        Category {
            name: "'∅' token (empty language: nothing matches)",
            pattern: "∅",
            checks: &[("", false), ("a", false)],
        },
    ]
}

/// One category's outcome: whether every check passed, its translated
/// `Regex` (absent on a translate error), and each check's own result.
struct CategoryResult {
    pass: bool,
    checks_passed: usize,
    regex: Result<Regex, String>,
    check_results: Vec<(&'static str, bool, bool)>, // (input, expected, actual)
}

fn run_category(cat: &Category) -> CategoryResult {
    println!("[{}]", cat.name);
    println!("  pattern: {}", cat.pattern);

    let regex = match translate_for(cat.pattern) {
        Ok(r) => r,
        Err(e) => {
            println!("  FAIL: translate error: {e}");
            println!();
            return CategoryResult { pass: false, checks_passed: 0, regex: Err(e), check_results: Vec::new() };
        }
    };
    println!("  regex:   {:?}", regex);

    let mut pass = true;
    let mut checks_passed = 0usize;
    let mut check_results = Vec::new();
    for &(input, expected) in cat.checks {
        let actual = parse_deriv_std_rec(input, &regex).is_some();
        let ok = actual == expected;
        pass &= ok;
        if ok {
            checks_passed += 1;
        }
        println!(
            "  {}  input={:?}  expected_match={}  actual_match={}",
            if ok { "pass" } else { "FAIL" },
            input,
            expected,
            actual
        );
        check_results.push((input, expected, actual));
    }
    println!();
    CategoryResult { pass, checks_passed, regex: Ok(regex), check_results }
}

fn main() {
    println!("Frontend translation breadth: one hand-authored example per accepted construct.\n");
    let cats = categories();

    let mut summary_csv = String::new();
    summary_csv.push_str("name,pattern,checks_passed,checks_total,pass\n");
    let mut listing = Vec::new();

    let mut passed = 0usize;
    for cat in &cats {
        let result = run_category(cat);
        if result.pass {
            passed += 1;
        }
        summary_csv.push_str(&format!(
            "{},{},{},{},{}\n",
            csv_quote(cat.name),
            csv_quote(cat.pattern),
            result.checks_passed,
            cat.checks.len(),
            result.pass,
        ));
        listing.push(serde_json::json!({
            "name": cat.name,
            "pattern": cat.pattern,
            "regex": result.regex.as_ref().map(|r| format!("{:?}", r)).ok(),
            "translate_error": result.regex.as_ref().err(),
            "checks": result.check_results.iter().map(|&(input, expected, actual)| {
                serde_json::json!({
                    "input": input,
                    "expected_match": expected,
                    "actual_match": actual,
                    "pass": expected == actual,
                })
            }).collect::<Vec<_>>(),
            "pass": result.pass,
        }));
    }
    println!("{}/{} categories pass", passed, cats.len());

    save("translate_categories", "csv", &summary_csv);
    save(
        "translate_categories_patterns",
        "json",
        &serde_json::to_string_pretty(&listing).expect("serializing the listing"),
    );
    if passed != cats.len() {
        std::process::exit(1);
    }
}
