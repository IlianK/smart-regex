//! regex-engine/src/diagnostics/diag_2/mod.rs
//!
//! Level 2 - Verbose diagnostics output.
//!
//! Standard success:
//!   Regex:  a*
//!   Input:  "aaa"
//!   Match:  true
//!   Tree:   [a, a, a]
//!   Time:   0.08ms
//!   Steps:  4 derivative expressions computed
//!
//!   Construction steps:
//!     mkEps(r3) → Right Right ((), [])
//!     inject(a*, 'a', Right Right ((), [])) → Right ((), [a])    ← position 3 (backward start)
//!     inject(a*, 'a', Right ((), [a])) → ((), [a, a])            ← position 2
//!     inject(a*, 'a', ((), [a, a])) → [a, a, a]                  ← position 1 (backward end)
//!

mod deriv_std;
mod deriv_bc;
mod pderiv_bc;
mod pderiv_std;

use crate::types::Regex;
use crate::parsers::selection::ParserType;
use crate::diagnostics::DiagConfig;

pub fn run_parser(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    match config.parser_type {
        ParserType::DerivBc                                 => deriv_bc::run(regex_str, r, input),
        ParserType::DerivStdRec | ParserType::DerivStdLoop  => deriv_std::run(regex_str, r, input, config),
        ParserType::PDerivBc                                => pderiv_bc::run(regex_str, r, input),
        ParserType::PDerivStd                               => pderiv_std::run(regex_str, r, input),
    }
}
