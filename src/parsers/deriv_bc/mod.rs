//! Bit-coded Brzozowski derivative parser, ARegex (POSIX).

pub mod nullable;
pub mod deriv;
pub mod simplify;
pub mod internalize;
pub mod mk_eps_bc;
pub mod parse;
pub mod traced;

pub use internalize::{internalize, fuse};
pub use mk_eps_bc::mk_eps_bc;
pub use parse::{parse_deriv_bc, parse_bitcoded_recursive, parse_bitcoded_loop};
pub use traced::parse_bitcoded_traced;
