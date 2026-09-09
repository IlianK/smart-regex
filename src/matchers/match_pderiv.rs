//! Antimirov partial derivative matcher (boolean)

use std::collections::HashSet;
use crate::types::Regex;
use crate::regex::nullable::nullable;
use crate::regex::simplify::smart_seq;

/// Antimirov partial derivatives
fn pderiv(r: &Regex, x: char) -> Vec<Regex> {
    match r {
        Regex::Phi => Vec::new(),
        Regex::Eps => Vec::new(),
        Regex::Lit(c) => {
            if *c == x { vec![Regex::Eps] } else { Vec::new() }
        }
        Regex::Alt(r, s) => {
            let mut out = pderiv(r, x);
            out.extend(pderiv(s, x));
            out
        }
        Regex::Seq(r, s) => {
            let mut out: Vec<Regex> = pderiv(r, x)
                .into_iter()
                .map(|r_prime| smart_seq(r_prime, s))
                .collect();
            if nullable(r) {
                out.extend(pderiv(s, x));
            }
            out
        }
        Regex::Star(r) => {
            pderiv(r, x)
                .into_iter()
                .map(|r_prime| smart_seq(r_prime, &Regex::star(*r.clone())))
                .collect()
        }
    }
}

pub fn match_pderiv(input: &str, r: &Regex) -> bool {
    let mut states: HashSet<Regex> = HashSet::new();
    states.insert(r.clone());

    for c in input.chars() {
        let mut next_states: HashSet<Regex> = HashSet::with_capacity(states.len());
        for state in &states {
            next_states.extend(pderiv(state, c));
        }
        if next_states.is_empty() {
            return false;
        }
        states = next_states;
    }

    states.iter().any(nullable)
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Regex;

    #[test] fn phi_never_matches()      { assert!(!match_pderiv("",  &Regex::Phi)); }
    #[test] fn empty_state_set_on_phi() { assert!(!match_pderiv("a", &Regex::Phi));}
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
}
