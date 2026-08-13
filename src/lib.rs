//! regex-engine/src/lib.rs
//! 
//! Main lib
 
// Core data types
pub mod types;

// Trace structs 
pub mod trace;

// Algorithms
pub mod regex;
pub mod matchers;
pub mod parsers;
pub mod diagnostics;

// Re-export core types
pub use types::{Regex, ARegex, ParseTree, flatten};

// Re-export matchers
pub use matchers::{match_naive, match_deriv, match_pderiv};

// Re-export parsers
pub use parsers::{parse_posix, mk_eps, inject};
pub use parsers::{parse_recursive, parse_loop, parse_bitcoded};
pub use parsers::parse_pderiv_bc;

// Re-export traced variants
pub use parsers::parse_loop_traced;
pub use parsers::parse_recursive_traced;
pub use parsers::parse_bitcoded_traced;
pub use parsers::parse_pderiv_bc_traced;

// Re-export diagnostics entry points
pub use diagnostics::{DiagLevel, DiagConfig, run_parser, run_matcher};