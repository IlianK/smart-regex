//! regex-engine: derivative and partial-derivative regex parsing, POSIX and Greedy.

// Core data types
pub mod types;

// Dataset preparation 
pub mod data;

// Trace structs 
pub mod trace;

// Algorithms
pub mod regex;
pub mod matchers;
pub mod parsers;
pub mod diagnostics;

// Surface syntax
pub mod frontend;

// Re-export core types
pub use types::{Regex, ARegex, ParseTree, flatten};

// Re-export surface-syntax pattern AST
pub use frontend::{ExtPat, parse_ext_pattern, parse_dataset_pattern, parse_pcre_rule};

// Re-export matchers
pub use matchers::{match_naive, match_deriv, match_pderiv};

// Re-export parsers
pub use parsers::parse;
pub use regex::mk_eps;
pub use parsers::deriv_std::inject;
pub use parsers::{parse_deriv_std_rec, parse_deriv_std_loop, parse_deriv_bc};
pub use parsers::parse_pderiv_bc;

// Re-export traced variants
pub use parsers::parse_loop_traced;
pub use parsers::parse_recursive_traced;
pub use parsers::parse_bitcoded_traced;
pub use parsers::parse_pderiv_std_traced;
pub use parsers::parse_pderiv_bc_traced;

// Re-export diagnostics 
pub use diagnostics::{DiagLevel, DiagConfig, run_parser};