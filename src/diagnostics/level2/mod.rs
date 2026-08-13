//! regex-engine/src/diagnostics/level2/mod.rs
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
//! One submodule per parser family, mirroring src/posix's own
//! standard/bitcoded/bitcoded::pderiv split -- each renders its own trace
//! type (ParseTrace / BitTrace / PDerivBitTrace) independently, since the
//! three have different shapes (single expression per step vs. a bit-coded
//! single expression vs. a whole frontier per step).

mod standard;
mod bitcoded;
mod pderiv_bc;

use crate::types::Regex;
use crate::parsers::selection::ParserType;
use crate::diagnostics::DiagConfig;

pub fn run_parser(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    match config.parser_type {
        ParserType::DerivBC => bitcoded::run(regex_str, r, input),
        ParserType::DerivRec | ParserType::DerivLoop => standard::run(regex_str, r, input, config),
        ParserType::PDeriv | ParserType::PDerivBC => pderiv_bc::run(regex_str, r, input),
    }
}
