//! examples/demo_peak_offenders.rs
//!
//! For a benchmark's `..._entries.csv` (one row per (parser, pattern,
//! input) measurement, written by bench_corpus_time/bench_corpus_memory
//! or their bench_pathological_* counterparts), finds the top-N entries
//! per parser by a chosen metric column (e.g. `micros`, `peak_bytes`) --
//! "which pattern+input caused parser X's worst case". Those entries
//! CSVs already carry everything this needs; this just groups, sorts,
//! and extracts the top of each group instead of asking a human to
//! grep/sort them by hand.
//!
//! Reads the CSV generically by column name, not a fixed schema: corpus
//! entries (source,parser,pattern,input,expected_match,outcome,micros,...)
//! and pathological entries (row_id,category,parser,n,p,side,pattern,
//! input,outcome,micros,...) have different columns, so this only
//! requires `parser` and whichever metric column was asked for; every
//! other column in a row is carried through unchanged into the output.
//!
//! cargo run --release --example demo_peak_offenders -- <entries_csv> <metric_column> [top_n]
//!
//! e.g.:
//!   cargo run --release --example demo_peak_offenders -- \
//!     reports/benches/bench_corpus_time_CT1_entries.csv micros 5
//!   cargo run --release --example demo_peak_offenders -- \
//!     reports/benches/bench_corpus_memory_CT1_entries.csv peak_bytes 5
//!
//! Writes reports/demos/peak_offenders_<csv-basename>_<metric>.csv,
//! same columns as the input plus a leading `rank` (1 = worst), rows
//! grouped by parser in the input's own first-seen order.

use std::collections::HashMap;
use std::io::Write;

/// Splits one CSV line into fields. Every text field in this project's
/// entries CSVs is unconditionally wrapped in quotes by their own
/// `csv_quote` (see examples/common/thesis_figures.rs and
/// benches/bench_corpus_time.rs's own copy), with `"` doubled inside;
/// numeric/enum fields never are. That convention, not RFC 4180 in
/// general, is what this parses.
fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut chars = line.chars().peekable();
    while chars.peek().is_some() {
        let field = if chars.peek() == Some(&'"') {
            chars.next();
            let mut s = String::new();
            while let Some(c) = chars.next() {
                if c == '"' {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        s.push('"');
                    } else {
                        break;
                    }
                } else {
                    s.push(c);
                }
            }
            s
        } else {
            let mut s = String::new();
            while let Some(&c) = chars.peek() {
                if c == ',' {
                    break;
                }
                s.push(c);
                chars.next();
            }
            s
        };
        fields.push(field);
        if chars.peek() == Some(&',') {
            chars.next();
        }
    }
    fields
}

fn csv_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "..."
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: demo_peak_offenders <entries_csv> <metric_column> [top_n, default 5]");
        std::process::exit(2);
    }
    let csv_path = &args[1];
    let metric_col = &args[2];
    let top_n: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(5);

    let text = std::fs::read_to_string(csv_path).unwrap_or_else(|e| {
        eprintln!("could not read {csv_path}: {e}");
        std::process::exit(2);
    });
    let mut lines = text.lines();
    let header_line = lines.next().unwrap_or_else(|| {
        eprintln!("{csv_path} is empty");
        std::process::exit(2);
    });
    let header = split_csv_line(header_line);
    let col_idx = |name: &str| -> usize {
        header.iter().position(|h| h == name).unwrap_or_else(|| {
            eprintln!("{csv_path} has no column {name:?}; columns are: {}", header.join(","));
            std::process::exit(2);
        })
    };
    let parser_i = col_idx("parser");
    let metric_i = col_idx(metric_col);
    let pattern_i = header.iter().position(|h| h == "pattern");
    let input_i = header.iter().position(|h| h == "input");

    // parser -> first-seen order, and its rows with a parsed metric value.
    let mut parser_order: Vec<String> = Vec::new();
    let mut by_parser: HashMap<String, Vec<(f64, Vec<String>)>> = HashMap::new();
    let mut skipped_unparseable = 0usize;

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let fields = split_csv_line(line);
        let Some(parser) = fields.get(parser_i).cloned() else { continue };
        let Some(metric_raw) = fields.get(metric_i) else { continue };
        let Ok(value) = metric_raw.parse::<f64>() else {
            // Empty (timed_out/crashed rows leave this column blank) or
            // otherwise non-numeric -- not rankable, not an error.
            skipped_unparseable += 1;
            continue;
        };
        if !by_parser.contains_key(&parser) {
            parser_order.push(parser.clone());
        }
        by_parser.entry(parser).or_default().push((value, fields));
    }

    if by_parser.is_empty() {
        eprintln!("no rows in {csv_path} had a numeric {metric_col:?}");
        std::process::exit(1);
    }

    println!("Top {top_n} by {metric_col} per parser, from {csv_path}:\n");
    let mut out_rows: Vec<Vec<String>> = Vec::new();
    for parser in &parser_order {
        let rows = by_parser.get_mut(parser).unwrap();
        rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        rows.truncate(top_n);

        println!("{parser}:");
        for (rank, (value, fields)) in rows.iter().enumerate() {
            let pattern = pattern_i.and_then(|i| fields.get(i)).map(|s| truncate(s, 60)).unwrap_or_default();
            let input = input_i.and_then(|i| fields.get(i)).map(|s| truncate(s, 40)).unwrap_or_default();
            println!("  #{:<2} {metric_col}={:<12.3} pattern={pattern:?}  input={input:?}", rank + 1, value);

            let mut row = vec![(rank + 1).to_string()];
            row.extend(fields.iter().cloned());
            out_rows.push(row);
        }
        println!();
    }
    if skipped_unparseable > 0 {
        println!("({skipped_unparseable} row(s) had no numeric {metric_col} -- timed out, crashed, or not applicable -- excluded from ranking.)\n");
    }

    let base = std::path::Path::new(csv_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("entries");
    let out_path = format!("reports/demos/peak_offenders_{base}_{metric_col}.csv");
    if let Some(parent) = std::path::Path::new(&out_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut f = std::fs::File::create(&out_path).unwrap_or_else(|e| {
        eprintln!("could not create {out_path}: {e}");
        std::process::exit(2);
    });
    let mut out_header = vec!["rank".to_string()];
    out_header.extend(header.iter().cloned());
    writeln!(f, "{}", out_header.join(",")).unwrap();
    for row in &out_rows {
        let quoted: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, v)| {
                // Column 0 is the synthetic `rank`; columns 1.. line up
                // with `header` at index i-1. Quote exactly the same
                // fields the source CSV quoted (pattern/input/detail-like
                // text), so re-parsing this file works the same way.
                if i == 0 {
                    v.clone()
                } else if header.get(i - 1).map(|h| h == "pattern" || h == "input").unwrap_or(false) {
                    csv_quote(v)
                } else {
                    v.clone()
                }
            })
            .collect();
        writeln!(f, "{}", quoted.join(",")).unwrap();
    }
    println!("wrote {out_path}");
}
