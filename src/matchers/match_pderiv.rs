//! regex-engine/src/matchers/match_pderiv.rs
//! 
//! Antimirov partial derivative matcher (boolean)

use std::collections::HashSet;
use crate::types::Regex;
use crate::regex::antimirov::standard::pderiv;
use crate::regex::nullable::standard::nullable;

pub fn match_pderiv(input: &str, r: &Regex) -> bool {
    let mut states: HashSet<Regex> = HashSet::new();
    states.insert(r.clone());

    for c in input.chars() {
        let mut next_states: HashSet<Regex> = HashSet::new();
        for state in &states {
            next_states.extend(pderiv(state, c));
        }
        states = next_states;
        if states.is_empty() {
            return false;
        }
    }

    states.iter().any(|r| nullable(r))
}

// ============================================================================
// Tests for match_pderiv
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Regex;

    #[test] fn phi_never_matches()      { assert!(!match_pderiv("",  &Regex::Phi)); }
    #[test] fn eps_matches_empty()      { assert!(match_pderiv("",   &Regex::Eps)); }
    #[test] fn eps_no_nonempty()        { assert!(!match_pderiv("a", &Regex::Eps)); }
    #[test] fn lit_matches_char()       { assert!(match_pderiv("a",  &Regex::lit('a'))); }
    #[test] fn lit_wrong_char()         { assert!(!match_pderiv("b", &Regex::lit('a'))); }
    #[test] fn star_matches_empty()     { assert!(match_pderiv("",   &Regex::star(Regex::lit('a')))); }
    #[test] fn star_matches_repeated()  { assert!(match_pderiv("aaa",&Regex::star(Regex::lit('a')))); }
    #[test] fn seq_matches()            { assert!(match_pderiv("ab", &Regex::seq(Regex::lit('a'), Regex::lit('b')))); }
    #[test] fn seq_too_short()          { assert!(!match_pderiv("a", &Regex::seq(Regex::lit('a'), Regex::lit('b')))); }
    #[test] fn alt_left()               { assert!(match_pderiv("a",  &Regex::alt(Regex::lit('a'), Regex::lit('b')))); }
    #[test] fn alt_right()              { assert!(match_pderiv("b",  &Regex::alt(Regex::lit('a'), Regex::lit('b')))); }
    #[test] fn alt_neither()            { assert!(!match_pderiv("c", &Regex::alt(Regex::lit('a'), Regex::lit('b')))); }

    // Extra: empty state set terminates early and returns false
    #[test]
    fn empty_state_set_on_phi() {
        // After consuming 'a' from Phi the state set is empty -> false
        assert!(!match_pderiv("a", &Regex::Phi));
    }
}