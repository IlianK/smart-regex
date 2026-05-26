//! POSIX disambiguation policy for regular expression parsing

mod mk_eps;
mod inject;
mod parser;
pub mod debug;
pub mod bitcoded;

pub use crate::types::{ParseTree, flatten};
pub use mk_eps::mk_eps;
pub use inject::inject;
pub use parser::{parse_posix, match_posix, parse_recursive, parse_loop};
pub use bitcoded::parse_bitcoded;
pub use debug::debug_enabled;

pub mod tests {
    use crate::types::Regex;
    
    pub fn fmt_regex(r: &Regex) -> String {
        match r {
            Regex::Phi => "∅".to_string(),
            Regex::Eps => "ε".to_string(),
            Regex::Lit(c) => format!("'{}'", c),
            Regex::Seq(r1, r2) => format!("({}·{})", fmt_regex(r1), fmt_regex(r2)),
            Regex::Alt(r1, r2) => format!("({}+{})", fmt_regex(r1), fmt_regex(r2)),
            Regex::Star(r1) => format!("({})*", fmt_regex(r1)),
        }
    }
}