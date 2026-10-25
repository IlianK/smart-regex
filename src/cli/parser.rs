//! src/cli/parser.rs
//!
//! `parse` subcommand: parse tree with optional diagnostics.
//! `--parser all` runs every parser.

use regex_engine::diagnostics::{DiagConfig, DiagLevel, run_parser};
use regex_engine::{parse_deriv_std_rec, parse_deriv_std_loop, parse_deriv_bc, parse_pderiv_bc, flatten};
use regex_engine::parsers::{ParserType, parse_pderiv_std};
use regex_engine::types::ParseTree;
use regex_engine::frontend::parse_pattern;

/// Single parser, with diagnostics.
///
/// `diag = Debug` with no `--diag-report` writes to `reports/diag/report.txt`.
/// That is the CLI's own default, not `demo_parse`'s per-parser path.
pub fn run_parse_single(
    regex_str: &str,
    input: &str,
    parser: ParserType,
    diag: DiagLevel,
    diag_report: Option<String>,
) {
    let r = match parse_pattern(regex_str) {
        Ok(r)  => r,
        Err(e) => { eprintln!("Regex parse error: {}", e); std::process::exit(2); }
    };

    let report_path = match (&diag_report, diag) {
        (Some(path), _)          => Some(path.clone()),
        (None, DiagLevel::Debug) => Some("reports/diag/report.txt".to_string()),
        (None, _)                => None,
    };

    let config = DiagConfig::new(diag, parser, report_path);
    run_parser(regex_str, &r, input, &config);
}


/// All five parsers in one table, then a summary block.
///
/// Three checks, all bugs if they fail:
/// POSIX parsers agree with each other;
/// Greedy parsers agree with each other;
/// All five agree on membership.
///
/// The fourth line, `Greedy vs POSIX`, is informational: the two policies
/// may differ on tree shape.
pub fn run_parse_all(regex_str: &str, input: &str) {
    let r = match parse_pattern(regex_str) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Regex parse error: {}", e);
            std::process::exit(2);
        }
    };

    println!("Regex: {}", regex_str);
    println!("Input: {:?}", input);
    println!();

    // Table column widths.
    const PARSER_W: usize = 14;
    const FAMILY_W: usize = 10;
    const MATCH_W: usize = 8;

    println!(
        "  {:<PARSER_W$} | {:<FAMILY_W$} | {:<MATCH_W$} | {}",
        "parser", "family", "match", "tree"
    );
    println!(
        "  {:-<PARSER_W$}-+-{:-<FAMILY_W$}-+-{:-<MATCH_W$}-+-{:-<30}",
        "", "", "", ""
    );

    type ParserFn = fn(&str, &regex_engine::Regex) -> Option<ParseTree>;

    let rows: [(&str, ParserFn, &str); 5] = [
        (ParserType::DerivStdRec.name(),  parse_deriv_std_rec,  "POSIX"),
        (ParserType::DerivStdLoop.name(), parse_deriv_std_loop, "POSIX"),
        (ParserType::DerivBc.name(),      parse_deriv_bc,       "POSIX"),
        (ParserType::PDerivStd.name(),    parse_pderiv_std,     "Non-POSIX"),
        (ParserType::PDerivBc.name(),     parse_pderiv_bc,      "Non-POSIX"),
    ];

    let mut results: Vec<Option<ParseTree>> = Vec::with_capacity(rows.len());

    for (label, f, family) in rows {
        let result = f(input, &r);

        match &result {
            Some(tree) => println!(
                "  {:<PARSER_W$} | {:<FAMILY_W$} | {:<MATCH_W$} | {:?}",
                label,
                family,
                flatten(tree),
                tree
            ),
            None => println!(
                "  {:<PARSER_W$} | {:<FAMILY_W$} | {:<MATCH_W$} | no match",
                label,
                family,
                ""
            ),
        }

        results.push(result);
    }

    // results[0..3] POSIX, results[3..5] Non-POSIX, matching `rows` order.
    let posix = &results[0..3];
    let non_posix = &results[3..5];

    let posix_agree = posix.windows(2).all(|w| w[0] == w[1]);
    let non_posix_agree = non_posix.windows(2).all(|w| w[0] == w[1]);

    let membership_agree = results
        .iter()
        .all(|t| t.is_some() == results[0].is_some());

    // Informational: does pderiv_bc produce the same tree as deriv_bc?
    let non_posix_vs_posix = if results[4] == results[2] {
        "agree"
    } else {
        "disagree"
    };

    let membership = if membership_agree {
        if results[0].is_some() {
            "[ok] all agree: match"
        } else {
            "[ok] all agree: no match"
        }
    } else {
        "[FAIL] parsers disagree on membership"
    };

    println!();
    println!("  summary");

    // Align all summary colons.
    const SUMMARY_W: usize = 20;

    println!("    {:<SUMMARY_W$}: {}", "membership", membership);
    println!(
        "    {:<SUMMARY_W$}: {}",
        "POSIX parsers",
        if posix_agree { "[ok] agree" } else { "[FAIL] DISAGREE" }
    );
    println!(
        "    {:<SUMMARY_W$}: {}",
        "Non-POSIX parsers",
        if non_posix_agree {
            "[ok] agree"
        } else {
            "[FAIL] DISAGREE"
        }
    );
    println!(
        "    {:<SUMMARY_W$}: {}",
        "Non-POSIX vs POSIX",
        non_posix_vs_posix
    );
}
