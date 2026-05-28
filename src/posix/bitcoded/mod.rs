//! Bit-coded incremental POSIX parsing

pub mod internalize;
pub mod mk_eps_bc;
pub mod decode;
pub mod parse;

pub use internalize::{internalize, fuse};
pub use mk_eps_bc::mk_eps_bc;
pub use decode::decode;
pub use parse::parse_bitcoded;

// Re-export from regex modules for convenience
pub use crate::regex::nullable::annotated::{nullable_bc, is_phi};
pub use crate::regex::brzozowski::annotated::deriv_bc;
pub use crate::regex::simplify::annotated::simp;