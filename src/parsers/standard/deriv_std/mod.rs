//! Plain-Regex Brzozowski-derivative parser (POSIX).

pub mod inject;
pub mod parse;

pub use inject::inject;
pub use parse::{parse_recursive, parse_loop, parse_loop_traced, parse_recursive_traced};
