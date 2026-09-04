//! Bit-coded Brzozowski-derivative parser (POSIX).

pub mod internalize;
pub mod mk_eps_bc;
pub mod parse;

pub use internalize::{internalize, fuse};
pub use mk_eps_bc::mk_eps_bc;
pub use parse::{parse_bitcoded, parse_bitcoded_recursive, parse_bitcoded_loop, parse_bitcoded_traced};
