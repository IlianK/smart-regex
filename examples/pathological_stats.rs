//! examples/pathological_stats.rs
//!
//! Transparency report for `data/pathological/pathological_patterns.json`:
//! how many rows exist, per section and category, and how many concrete
//! (pattern, input) entries the default sweep (each category's own
//! `suggested_n`/`suggested_p`, same as `bench_pathological_time.rs`'s
//! own default) actually produces -- "how much data is there" is not
//! the row count alone, since one row expands into many swept entries.
//!
//! Reuses `select_rows`/`build_entries`-equivalent logic directly
//! against `src/data/pathological.rs::expand`, not a separate estimate.
//!
//! cargo run --release --example pathological_stats
//! Saves reports/data/pathological_stats.csv and
//! reports/data/pathological_rows.csv (the per-row catalogue).

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::PathBuf;

use regex_engine::data::pathological::{self, PathologicalDoc, PathologicalRow};

const DEFAULT_JSON_PATH: &str = "data/pathological/pathological_patterns.json";
const REPORTS_DIR: &str = "reports/data";

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

/// Entries a default sweep (no env overrides) produces for one row,
/// mirroring `bench_pathological_time.rs::build_entries` exactly.
fn swept_entry_count(doc: &PathologicalDoc, row: &PathologicalRow) -> usize {
    let cat = doc.categories.get(&row.category);
    let n_list = cat.and_then(|c| c.suggested_n.clone()).unwrap_or_else(|| vec![pathological::DEFAULT_N]);
    let needs_p = row.generator.is_some();
    let p_list = if needs_p {
        cat.and_then(|c| c.suggested_p.clone()).unwrap_or_else(|| vec![pathological::DEFAULT_P])
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
        None => String::new(),
        Some(xs) => xs.iter().map(u32::to_string).collect::<Vec<_>>().join(";"),
    }
}

struct CategoryStats {
    category: String,
    name: String,
    rows: usize,
    generator_rows: usize,
    suggested_n: String,
    suggested_p: String,
    swept_entries: usize,
}

fn main() {
    let path = json_path();
    let doc = pathological::load(&path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));
    let rows = all_rows(&doc);

    if let Err(e) = std::fs::create_dir_all(REPORTS_DIR) {
        eprintln!("could not create {REPORTS_DIR}: {e}");
        return;
    }

    // Per-row catalogue: id, category, section, has generator, swept entries.
    let rows_path = format!("{REPORTS_DIR}/pathological_rows.csv");
    let mut rows_f = std::fs::File::create(&rows_path).expect("creating pathological_rows.csv");
    writeln!(rows_f, "id,category,section,has_generator,swept_entries").unwrap();
    for (row, section) in &rows {
        writeln!(
            rows_f,
            "{},{},{},{},{}",
            row.id,
            row.category,
            section,
            row.generator.is_some(),
            swept_entry_count(&doc, row),
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
        let generator_rows = cat_rows.iter().filter(|r| r.generator.is_some()).count();
        let swept_entries: usize = cat_rows.iter().map(|r| swept_entry_count(&doc, r)).sum();
        cat_stats.push(CategoryStats {
            category: category.clone(),
            name: info.map(|c| c.name.clone()).unwrap_or_else(|| "?".to_string()),
            rows: cat_rows.len(),
            generator_rows,
            suggested_n: info.map(|c| fmt_list(&c.suggested_n)).unwrap_or_default(),
            suggested_p: info.map(|c| fmt_list(&c.suggested_p)).unwrap_or_default(),
            swept_entries,
        });
    }

    let summary_path = format!("{REPORTS_DIR}/pathological_stats.csv");
    let mut f = std::fs::File::create(&summary_path).expect("creating pathological_stats.csv");
    writeln!(f, "category,name,rows,generator_rows,suggested_n,suggested_p,swept_entries_default").unwrap();

    println!("Pathological dataset: {path:?}");
    println!();
    println!(
        "{:<4} {:<28} {:>5} {:>6} {:<20} {:<12} {:>14}",
        "Cat", "Name", "Rows", "GenRow", "suggested_n", "suggested_p", "SweptEntries"
    );
    println!("{}", "-".repeat(94));

    let (mut total_rows, mut total_gen, mut total_swept) = (0usize, 0usize, 0usize);
    for s in &cat_stats {
        println!(
            "{:<4} {:<28} {:>5} {:>6} {:<20} {:<12} {:>14}",
            s.category, s.name, s.rows, s.generator_rows, s.suggested_n, s.suggested_p, s.swept_entries
        );
        writeln!(
            f,
            "{},{},{},{},{},{},{}",
            s.category, csv_quote(&s.name), s.rows, s.generator_rows, s.suggested_n, s.suggested_p, s.swept_entries
        )
        .unwrap();
        total_rows += s.rows;
        total_gen += s.generator_rows;
        total_swept += s.swept_entries;
    }
    println!("{}", "-".repeat(94));
    println!("{:<4} {:<28} {:>5} {:>6} {:<20} {:<12} {:>14}", "", "total", total_rows, total_gen, "", "", total_swept);
    writeln!(f, "total,,{total_rows},{total_gen},,,{total_swept}").unwrap();

    println!();
    println!(
        "{} rows (pathological_patterns {}, baseline_and_edge_patterns {}, generated_patterns {}), \
         {total_swept} entries under each category's own default sweep",
        total_rows,
        doc.pathological_patterns.len(),
        doc.baseline_and_edge_patterns.len(),
        doc.generated_patterns.len(),
    );
    println!("wrote {summary_path}");
    println!("wrote {rows_path}");
}
