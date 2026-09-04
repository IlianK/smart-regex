//! regex-engine/src/diagnostics/diag_3/mod.rs
//!
//! Level 3 - Debug diagnostics. Full trace: derivation, nullability, mkEps, inject
//!
//! Output goes to REGEX_DIAG_REPORT file if set, otherwise stdout.

mod deriv_std;
mod deriv_bc;
mod pderiv_bc;
mod pderiv_std;

use crate::types::Regex;
use crate::parsers::selection::ParserType;
use crate::diagnostics::DiagConfig;
use crate::diagnostics::replay::{find_failure, caret_lines};
use crate::diagnostics::report::ReportWriter;

pub fn run_parser(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    let mut w = ReportWriter::new(config.report_path.as_deref());

    match config.parser_type {
        ParserType::DerivBc                                 => deriv_bc::render(regex_str, r, input, &mut w),
        ParserType::DerivStdRec | ParserType::DerivStdLoop  => deriv_std::render(regex_str, r, input, config, &mut w),
        ParserType::PDerivBc                                => pderiv_bc::render(regex_str, r, input, &mut w),
        ParserType::PDerivStd                               => pderiv_std::render(regex_str, r, input, &mut w),
    }

    w.flush();
}

/// Shared by all 
pub(super) fn timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// "ERROR SUMMARY" section on failure 
pub(super) fn render_error_summary(w: &mut ReportWriter, input: &str, r: &Regex) {
    w.separator();
    w.line("ERROR SUMMARY");
    w.separator();
    let info = find_failure(input, r);
    w.kv("Failure at position", &format!("{} (1-indexed)", info.position));
    if info.found == '\0' {
        w.kv("Found", "end of input");
    } else {
        w.kv("Found", &format!("'{}'", info.found));
    }
    w.kv("Expected", &info.expected);
    w.line(&caret_lines(input, info.position));
}
