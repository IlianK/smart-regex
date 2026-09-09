//! regex-engine/src/cli/parser.rs
//!
//! Parser command logic

use regex_engine::diagnostics::{DiagConfig, DiagLevel, run_parser};
use regex_engine::{parse_deriv_std_rec, parse_deriv_std_loop, parse_deriv_bc, parse_pderiv_bc, flatten};
use regex_engine::parsers::{ParserType, parse_pderiv_std};
use regex_engine::types::ParseTree;
use regex_engine::frontend::parse_pattern;


// Runs with chosen parser
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
        (None, DiagLevel::Debug) => Some("reports/report.txt".to_string()), // default report path
        (None, _)                => None,
    };

    let config = DiagConfig::new(diag, parser, report_path);
    run_parser(regex_str, &r, input, &config);
}


// Runs with all parsers
pub fn run_parse_all(regex_str: &str, input: &str) {
    let r = match parse_pattern(regex_str) {
        Ok(r)  => r,
        Err(e) => {
            eprintln!("Regex parse error: {}", e);
            std::process::exit(2);
        }
    };

    // Comparison table
    println!("Regex: {}", regex_str);
    println!("Input: {:?}", input);
    println!();
    println!("{:12} | {:6} | {}", "Parser", "Policy", "Result");
    println!("{:-<12}-+-{:-<6}-+-{:-<30}", "", "", "");

    type ParserFn = fn(&str, &regex_engine::Regex) -> Option<ParseTree>;

    // POSIX Brzozowski-derivative parsers
    let posix_parsers: Vec<(&str, ParserFn)> = vec![
        ("DERIV_REC",  parse_deriv_std_rec),
        ("DERIV_LOOP", parse_deriv_std_loop),
        ("DERIV_BC",   parse_deriv_bc),
    ];

    let mut posix_results: Vec<Option<ParseTree>> = Vec::new();

    for (name, parser) in &posix_parsers {
        let result = parser(input, &r);

        match &result {
            Some(tree) => {
                println!(
                    "{:12} | {:6} | {} → {:?}",
                    name,
                    "POSIX",
                    tree,
                    flatten(tree)
                );
            }
            None => {
                println!("{:12} | {:6} | No match", name, "POSIX");
            }
        }

        posix_results.push(result);
    }

    // GREEDY Antimirov-partial derivative parsers
    let greedy_parsers: Vec<(&str, ParserFn)> = vec![
        ("PDERIV_STD", parse_pderiv_std),
        ("PDERIV_BC",  parse_pderiv_bc),
    ];

    let mut greedy_results: Vec<Option<ParseTree>> = Vec::new();

    for (name, parser) in &greedy_parsers {
        let result = parser(input, &r);

        match &result {
            Some(tree) => println!(
                "{:12} | {:6} | {} → {:?}",
                name,
                "GREEDY",
                tree,
                flatten(tree)
            ),
            None => println!(
                "{:12} | {:6} | No match",
                name,
                "GREEDY"
            ),
        }

        greedy_results.push(result);
    }

    println!();

    check_parser_agreement(
        &posix_results,
        &greedy_results,
    );

}


// Agreement checks
fn check_parser_agreement(
    posix_results: &[Option<ParseTree>],
    greedy_results: &[Option<ParseTree>],
) {
    // POSIX parsers must produce identical trees
    let posix_agree = posix_results.windows(2).all(|w| w[0] == w[1]);

    if posix_agree {
        println!("POSIX parsers agree");
    } else {
        println!("POSIX PARSERS DISAGREE");
    }

    // GREEDY parsers must produce identical trees
    let greedy_agree = greedy_results.windows(2).all(|w| w[0] == w[1]);

    if greedy_agree {
        println!("PDERIV parsers agree");
    } else {
        println!("PDERIV PARSERS DISAGREE (bug)");
    }

    // All must agree on membership
    let membership_agrees =
        posix_results
            .iter()
            .chain(greedy_results.iter())
            .map(|t| t.is_some())
            .all(|matched| matched == posix_results[0].is_some());

    if membership_agrees {
        println!("GREEDY agrees with POSIX on membership");
    } else {
        println!("MEMBERSHIP DISAGREEMENT (bug)");
    }
}
