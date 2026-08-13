//! regex-engine/src/diagnostics/level3/mod.rs
//!
//! Level 3 - Debug diagnostics. Full trace: derivation, nullability, mkEps, inject
//!
//! Output goes to REGEX_DIAG_REPORT file if set, otherwise stdout.
//! 
mod standard;
mod bitcoded;
mod pderiv_bc;

use crate::types::Regex;
use crate::parsers::selection::ParserType;
use crate::diagnostics::DiagConfig;
use crate::diagnostics::replay::{find_failure, caret_lines};
use crate::diagnostics::report::ReportWriter;

pub fn run_parser(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    let mut w = ReportWriter::new(config.report_path.as_deref());

    match config.parser_type {
        ParserType::DerivBC => bitcoded::render(regex_str, r, input, &mut w),
        ParserType::DerivRec | ParserType::DerivLoop => standard::render(regex_str, r, input, config, &mut w),
        ParserType::PDeriv | ParserType::PDerivBC => pderiv_bc::render(regex_str, r, input, &mut w),
    }

    w.flush();
}

/// Shared by all three renderers for the report header's Timestamp line.
pub(super) fn timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// The "ERROR SUMMARY" section every renderer prints identically on
/// failure -- separator, four key-value lines, then the caret lines.
/// (Distinct from replay::error_report, which is the more compact
/// single-line form Levels 1-2 use; Level 3's report format spells out
/// Failure/Found/Expected as separate fields.)
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
