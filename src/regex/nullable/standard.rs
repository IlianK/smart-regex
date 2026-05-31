//! regex-engine/src/regex/nullable/standard.rs
//! 
//! Nullability for standard Regex

use crate::types::Regex;

/// Decides whether epsilon is in L(r)
pub fn nullable(r: &Regex) -> bool {
    match r {
        Regex::Phi => false,
        Regex::Eps => true,
        Regex::Lit(_) => false,
        Regex::Alt(r, s) => nullable(r) || nullable(s),
        Regex::Seq(r, s) => nullable(r) && nullable(s),
        Regex::Star(_) => true,
    }
}


// ============================================================================
// Tests for nullable
// ============================================================================ 

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Regex;
 
    #[test] fn phi_not_nullable()   { assert!(!nullable(&Regex::Phi)); }
    #[test] fn eps_nullable()       { assert!(nullable(&Regex::Eps)); }
    #[test] fn lit_not_nullable()   { assert!(!nullable(&Regex::lit('a'))); }
    #[test] fn star_always_nullable() {
        assert!(nullable(&Regex::star(Regex::lit('a'))));
        assert!(nullable(&Regex::star(Regex::Phi)));
    }
    #[test] fn seq_both_nullable()  { assert!(nullable(&Regex::seq(Regex::Eps, Regex::Eps))); }
    #[test] fn seq_one_not()        { assert!(!nullable(&Regex::seq(Regex::lit('a'), Regex::Eps))); }
    #[test] fn alt_left_nullable()  { assert!(nullable(&Regex::alt(Regex::Eps, Regex::lit('a')))); }
    #[test] fn alt_right_nullable() { assert!(nullable(&Regex::alt(Regex::lit('a'), Regex::Eps))); }
    #[test] fn alt_neither()        { assert!(!nullable(&Regex::alt(Regex::lit('a'), Regex::lit('b')))); }
}
