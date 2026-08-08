//! regex-engine/src/matchers/match_deriv.rs
//! 
//! Brzozowski derivative matcher (boolean)

use crate::types::Regex;
use crate::regex::deriv::standard::deriv;
use crate::regex::simplify::standard::simplify;
use crate::regex::nullable::standard::nullable;

pub fn match_deriv(input: &str, r: &Regex) -> bool {
    let mut current = r.clone();
    for c in input.chars() {
        current = simplify(deriv(&current, c));
    }
    nullable(&current)
}

// -------------------------------
// Tests for match_deriv
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Regex;

    #[test] fn phi_never_matches()      { assert!(!match_deriv("",  &Regex::Phi)); }
    #[test] fn eps_matches_empty()      { assert!(match_deriv("",   &Regex::Eps)); }
    #[test] fn eps_no_nonempty()        { assert!(!match_deriv("a", &Regex::Eps)); }
    #[test] fn lit_matches_char()       { assert!(match_deriv("a",  &Regex::lit('a'))); }
    #[test] fn lit_wrong_char()         { assert!(!match_deriv("b", &Regex::lit('a'))); }
    #[test] fn star_matches_empty()     { assert!(match_deriv("",   &Regex::star(Regex::lit('a')))); }
    #[test] fn star_matches_repeated()  { assert!(match_deriv("aaa",&Regex::star(Regex::lit('a')))); }
    #[test] fn seq_matches()            { assert!(match_deriv("ab", &Regex::seq(Regex::lit('a'), Regex::lit('b')))); }
    #[test] fn seq_too_short()          { assert!(!match_deriv("a", &Regex::seq(Regex::lit('a'), Regex::lit('b')))); }
    #[test] fn alt_left()               { assert!(match_deriv("a",  &Regex::alt(Regex::lit('a'), Regex::lit('b')))); }
    #[test] fn alt_right()              { assert!(match_deriv("b",  &Regex::alt(Regex::lit('a'), Regex::lit('b')))); }
    #[test] fn alt_neither()            { assert!(!match_deriv("c", &Regex::alt(Regex::lit('a'), Regex::lit('b')))); }
}