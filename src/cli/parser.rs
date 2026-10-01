//! src/cli/parser.rs
//!
//! `parse` subcommand: parse tree with optional diagnostics. `--parser all`

use regex_engine::diagnostics::{DiagConfig, DiagLevel, run_parser};
use regex_engine::{parse_deriv_std_rec, parse_deriv_std_loop, parse_deriv_bc, parse_pderiv_bc, flatten};
use regex_engine::parsers::{ParserType, parse_pderiv_std};
use regex_engine::types::ParseTree;
use regex_engine::frontend::{parse_dataset_pattern, parse_pattern, faithfulness_gaps, FaithfulnessGap};

fn parse(regex_str: &str, search: bool) -> Result<regex_engine::types::Regex, String> {
    let r: Result<regex_engine::Regex, String> = if search { parse_dataset_pattern(regex_str) } else { parse_pattern(regex_str) };
    // Warning about nested anchors, which are a known faithfulness gap.
    if r.is_ok() && faithfulness_gaps(regex_str).contains(&FaithfulnessGap::NestedAnchor) {
        eprintln!(
            "warning: {regex_str:?}: '^'/'$' present but not a top-level anchor; \
            treated as a no-op (see FRONTEND.md, NestedAnchor)"
        );
    }
    r
}

/// Single parser, with diagnostics.
///
/// `diag = Debug` with no `--diag-report` writes to `reports/report.txt`;
/// the CLI's own default, not `demo_parse`'s per-parser path.
pub fn run_parse_single(
    regex_str: &str,
    input: &str,
    parser: ParserType,
    diag: DiagLevel,
    diag_report: Option<String>,
    search: bool,
) {
    let r = match parse(regex_str, search) {
        Ok(r)  => r,
        Err(e) => { eprintln!("Regex parse error: {}", e); std::process::exit(2); }
    };

    let report_path = match (&diag_report, diag) {
        (Some(path), _)          => Some(path.clone()),
        (None, DiagLevel::Debug) => Some("reports/report.txt".to_string()),
        (None, _)                => None,
    };

    let config = DiagConfig::new(diag, parser, report_path);
    run_parser(regex_str, &r, input, &config);
}

/// All five parsers in one table, then a summary block.
///
/// Three checks, all bugs if they fail: POSIX parsers agree with each
/// other, Greedy parsers agree with each other, and all five agree on
/// membership. The fourth line, `Greedy vs POSIX`, is informational: the
/// two policies are allowed to differ on tree shape (see `PARSERS.md`).
pub fn run_parse_all(regex_str: &str, input: &str, search: bool) {
    let r = match parse(regex_str, search) {
        Ok(r)  => r,
        Err(e) => {
            eprintln!("Regex parse error: {}", e);
            std::process::exit(2);
        }
    };

    println!("Regex: {}", regex_str);
    println!("Input: {:?}", input);
    println!();
    println!("  {:<14} | {:<6} | {:<8} | {}", "parser", "family", "match", "tree");
    println!("  {:-<14}-+-{:-<6}-+-{:-<8}-+-{:-<30}", "", "", "", "");

    type ParserFn = fn(&str, &regex_engine::Regex) -> Option<ParseTree>;

    let rows: [(&str, ParserFn, &str); 5] = [
        (ParserType::DerivStdRec.name(),  parse_deriv_std_rec,  "POSIX"),
        (ParserType::DerivStdLoop.name(), parse_deriv_std_loop, "POSIX"),
        (ParserType::DerivBc.name(),      parse_deriv_bc,       "POSIX"),
        (ParserType::PDerivStd.name(),    parse_pderiv_std,     "Greedy"),
        (ParserType::PDerivBc.name(),     parse_pderiv_bc,      "Greedy"),
    ];

    let mut results: Vec<Option<ParseTree>> = Vec::new();
    for (label, f, family) in rows {
        let result = f(input, &r);
        match &result {
            Some(tree) => println!("  {:<14} | {:<6} | {:<8} | {:?}", label, family, flatten(tree), tree),
            None       => println!("  {:<14} | {:<6} | {:<8} | no match", label, family, ""),
        }
        results.push(result);
    }

    // results[0..3] POSIX, results[3..5] Greedy, in `rows` order.
    let posix  = &results[0..3];
    let greedy = &results[3..5];

    let posix_agree  = posix.windows(2).all(|w| w[0] == w[1]);
    let greedy_agree = greedy.windows(2).all(|w| w[0] == w[1]);
    let membership_agree =
        results.iter().map(|t| t.is_some()).all(|m| m == results[0].is_some());

    // Informational: whether pderiv_bc produced the same tree as deriv_bc.
    let greedy_vs_posix = if results[4] == results[2] { "agree" } else { "disagree" };

    let membership = if membership_agree {
        if results[0].is_some() { "[ok] all agree: match" } else { "[ok] all agree: no match" }
    } else {
        "[FAIL] parsers disagree on membership"
    };

    println!();
    println!("  summary");
    println!("    membership       : {}", membership);
    println!("    POSIX parsers    : {}", if posix_agree  { "[ok] agree" } else { "[FAIL] DISAGREE" });
    println!("    Greedy parsers   : {}", if greedy_agree { "[ok] agree" } else { "[FAIL] DISAGREE" });
    println!("    Greedy vs POSIX  : {}", greedy_vs_posix);
}