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
pub mod posix;
pub mod diagnostics;

// Re-export core types
pub use types::{Regex, ARegex, ParseTree, flatten};

// Re-export matchers
pub use matchers::{match_naive, match_deriv, match_pderiv};

// Re-export parsers
pub use posix::{parse_posix, match_posix, mk_eps, inject};
pub use posix::{parse_recursive, parse_loop, parse_bitcoded};

// Re-export traced variants
pub use posix::parse_loop_traced;
pub use posix::parse_recursive_traced;
pub use posix::parse_bitcoded_traced;

// Re-export diagnostics entry points
pub use diagnostics::{DiagLevel, DiagConfig, run_parser, run_matcher};