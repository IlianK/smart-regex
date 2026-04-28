//! regex_engine

pub mod basic;
pub mod posix;
pub mod demo; 

pub use basic::{
    Regex, 
    match_naive, 
    match_deriv, 
    match_pderiv,
    nullable,
    deriv,
    deriv_simp,
};

pub use posix::{
    ParseTree,
    parse_posix,
    match_posix,
    flatten,
    mk_eps,
    inject,
};