//! regex_engine

// Modules 
pub mod types;
pub mod regex;
pub mod matchers;
pub mod posix;

// pub mod diagnostics;

// Re-export 
pub use types::{Regex, ARegex, ParseTree, flatten};
pub use matchers::{match_naive, match_deriv, match_pderiv};
pub use posix::{parse_posix, match_posix, mk_eps, inject};