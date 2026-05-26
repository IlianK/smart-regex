//! Bit-coded incremental POSIX parsing

mod internalize;
mod mk_eps_bc;
mod simp;
mod decode;
mod parse_bc;

pub use internalize::{internalize, fuse};
pub use mk_eps_bc::mk_eps_bc;
pub use simp::simp;
pub use decode::decode;
pub use parse_bc::parse_bitcoded;

// Re-export from derivatives::bitcoded
pub use crate::derivatives::bitcoded::{nullable_bc, is_phi, deriv_bc};