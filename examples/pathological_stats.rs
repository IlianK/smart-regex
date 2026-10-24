//! examples/pathological_stats.rs
//!
//! Transparency report for `data/pathological/pathological_patterns.json`:
//! how many patterns exist, per category, and how many concrete test
//! cases the default sweep actually produces -- "how much data is
//! there" is not the pattern count alone, since one pattern expands
//! into several test cases (one per tested input size, times match +
//! non-match).
//!
//! Reuses `select_rows`/`build_entries`-equivalent logic directly
//! against `src/data/pathological.rs::expand`, not a separate estimate.
//!
//! cargo run --release --example pathological_stats
//! Saves reports/data/pathological_stats.csv and
//! reports/data/pathological_rows.csv (the per-pattern catalogue).

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::PathBuf;

use regex_engine::data::pathological::{self, PathologicalDoc, PathologicalRow};

const DEFAULT_JSON_PATH: &str = "data/pathological/pathological_patterns.json";
const REPORTS_DIR: &str = "reports/data";

/// Printed before the table so every column name is self-explanatory
/// without cross-referencing the JSON schema or the bench source.
const LEGEND: &str = "\
Patterns          = distinct hand-written regexes in this category.
Variable-size      = of those, how many have no single fixed regex text --
                     they're built from a template at a chosen pattern
                     size (P) instead, e.g. \"pick P words and alternate
                     them\". 0 for every category except J/K.
Input sizes (N)    = how many times the input repeats its stressed unit,
                     by default for this category -- N=20 means \"20
                     copies of the unit\", not \"20 bytes\". The unit is
                     a single character for most rows (so N=20 on (a+)+
                     is literally \"aaaaaaaaaaaaaaaaaaaa\") but a whole
                     generated word (e.g. \"w000001\") for word-based rows.
Pattern sizes (P)  = for variable-size patterns only: how many
                     alternatives/groups to build into the pattern
                     itself, by default (e.g. P=100 on a word-alternation
                     row builds a 100-way alternation).
Test cases         = patterns x input sizes x pattern sizes x (match +
                     non-match sides that exist) -- the actual number of
                     (pattern, input) pairs a default run measures. This
                     is always >= Patterns, often by a lot.";

fn json_path() -> PathBuf {
    std::env::var("PATHO_JSON_PATH").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(DEFAULT_JSON_PATH))
}

/// `(row, section_name)` for every row in the doc.
fn all_rows(doc: &PathologicalDoc) -> Vec<(&PathologicalRow, &'static str)> {
    let mut out = Vec::new();
    out.extend(doc.pathological_patterns.iter().map(|r| (r, "pathological_patterns")));
    out.extend(doc.baseline_and_edge_patterns.iter().map(|r| (r, "baseline_and_edge_patterns")));
    out.extend(doc.generated_patterns.iter().map(|r| (r, "generated_patterns")));
    out
}

/// Test cases a default sweep (no env overrides) produces for one
/// pattern, mirroring `bench_pathological_time.rs::build_entries` exactly.
fn test_case_count(doc: &PathologicalDoc, row: &PathologicalRow) -> usize {
    let cat = doc.categories.get(&row.category);
    let n_list = row.suggested_n.clone().or_else(|| cat.and_then(|c| c.suggested_n.clone())).unwrap_or_else(|| vec![pathological::DEFAULT_N]);
    let is_variable_size = row.generator.is_some();
    let p_list = if is_variable_size {
        row.suggested_p.clone().or_else(|| cat.and_then(|c| c.suggested_p.clone())).unwrap_or_else(|| vec![pathological::DEFAULT_P])
    } else {
        vec![pathological::DEFAULT_P]
    };
    let mut count = 0;
    for &p in &p_list {
        for &n in &n_list {
            let (_, m, nm) = pathological::expand(row, n, p);
            count += m.is_some() as usize + nm.is_some() as usize;
        }
    }
    count
}

fn csv_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn fmt_list(v: &Option<Vec<u32>>) -> String {
    match v {
        None => "-".to_string(),
        Some(xs) => xs.iter().map(u32::to_string).collect::<Vec<_>>().join(";"),
    }
}

/// One printed/aligned table row; widths are wide enough for the
/// longest real `input_sizes_n`/`pattern_sizes_p` lists in the dataset.
fn row_line(cat: &str, name: &str, patterns: &str, variable: &str, n: &str, p: &str, cases: &str) -> String {
    format!("{cat:<4} {name:<32} {patterns:>8} {variable:>13} {n:<26} {p:<22} {cases:>10}")
}

struct CategoryStats {
    category: String,
    name: String,
    patterns: usize,
    variable_size_patterns: usize,
    input_sizes_n: String,
    pattern_sizes_p: String,
    test_cases: usize,
}

fn main() {
    let path = json_path();
    let doc = pathological::load(&path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));
    let rows = all_rows(&doc);

    if let Err(e) = std::fs::create_dir_all(REPORTS_DIR) {
        eprintln!("could not create {REPORTS_DIR}: {e}");
        return;
    }

    // Per-pattern catalogue.
    let rows_path = format!("{REPORTS_DIR}/pathological_rows.csv");
    let mut rows_f = std::fs::File::create(&rows_path).expect("creating pathological_rows.csv");
    writeln!(rows_f, "id,category,section,variable_size,test_cases").unwrap();
    for (row, section) in &rows {
        writeln!(
            rows_f,
            "{},{},{},{},{}",
            row.id,
            row.category,
            section,
            row.generator.is_some(),
            test_case_count(&doc, row),
        )
        .unwrap();
    }

    // Per-category summary.
    let mut by_category: BTreeMap<String, Vec<&PathologicalRow>> = BTreeMap::new();
    for (row, _) in &rows {
        by_category.entry(row.category.clone()).or_default().push(row);
    }

    let mut cat_stats: Vec<CategoryStats> = Vec::new();
    for (category, cat_rows) in &by_category {
        let info = doc.categories.get(category);
        let variable_size_patterns = cat_rows.iter().filter(|r| r.generator.is_some()).count();
        let test_cases: usize = cat_rows.iter().map(|r| test_case_count(&doc, r)).sum();
        cat_stats.push(CategoryStats {
            category: category.clone(),
            name: info.map(|c| c.name.clone()).unwrap_or_else(|| "?".to_string()),
            patterns: cat_rows.len(),
            variable_size_patterns,
            input_sizes_n: info.map(|c| fmt_list(&c.suggested_n)).unwrap_or_else(|| "-".to_string()),
            pattern_sizes_p: info.map(|c| fmt_list(&c.suggested_p)).unwrap_or_else(|| "-".to_string()),
            test_cases,
        });
    }

    let summary_path = format!("{REPORTS_DIR}/pathological_stats.csv");
    let mut f = std::fs::File::create(&summary_path).expect("creating pathological_stats.csv");
    writeln!(f, "category,name,patterns,variable_size_patterns,input_sizes_n,pattern_sizes_p,test_cases").unwrap();

    println!("Pathological dataset: {path:?}");
    println!();
    println!("{LEGEND}");
    println!();
    let header = row_line("Cat", "Category", "Patterns", "Variable-size", "Input sizes (N)", "Pattern sizes (P)", "Test cases");
    println!("{header}");
    println!("{}", "-".repeat(header.len()));

    let (mut total_patterns, mut total_variable, mut total_cases) = (0usize, 0usize, 0usize);
    for s in &cat_stats {
        println!(
            "{}",
            row_line(
                &s.category,
                &s.name,
                &s.patterns.to_string(),
                &s.variable_size_patterns.to_string(),
                &s.input_sizes_n,
                &s.pattern_sizes_p,
                &s.test_cases.to_string(),
            )
        );
        writeln!(
            f,
            "{},{},{},{},{},{},{}",
            s.category,
            csv_quote(&s.name),
            s.patterns,
            s.variable_size_patterns,
            s.input_sizes_n,
            s.pattern_sizes_p,
            s.test_cases
        )
        .unwrap();
        total_patterns += s.patterns;
        total_variable += s.variable_size_patterns;
        total_cases += s.test_cases;
    }
    println!("{}", "-".repeat(header.len()));
    println!(
        "{}",
        row_line("", "total", &total_patterns.to_string(), &total_variable.to_string(), "", "", &total_cases.to_string())
    );
    writeln!(f, "total,,{total_patterns},{total_variable},,,{total_cases}").unwrap();

    println!();
    println!(
        "{total_patterns} patterns ({} pathological, {} baseline/edge, {} generated), \
         {total_cases} test cases under each category's own default sweep",
        doc.pathological_patterns.len(),
        doc.baseline_and_edge_patterns.len(),
        doc.generated_patterns.len(),
    );
    println!("wrote {summary_path}");
    println!("wrote {rows_path}");
}
