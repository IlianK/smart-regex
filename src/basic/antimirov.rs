//! Antimirov partial derivative matcher (NFA)

use std::collections::HashSet;
use super::regex::Regex;
use super::common::{nullable, smart_seq};

/// Computes set of partial derivatives
pub fn pderiv(r: &Regex, x: char) -> HashSet<Regex> {
    match r {
        Regex::Phi => HashSet::new(),
        Regex::Eps => HashSet::new(),
        Regex::Lit(c) => {
            let mut set = HashSet::new();
            if *c == x { set.insert(Regex::Eps); }
            set
        }
        Regex::Alt(r, s) => {
            let mut set = pderiv(r, x);
            set.extend(pderiv(s, x));
            set
        }
        Regex::Seq(r, s) => {
            let mut set: HashSet<Regex> = pderiv(r, x)
                .into_iter()
                .map(|r_prime| smart_seq(r_prime, s))
                .collect();
            if nullable(r) {
                set.extend(pderiv(s, x));
            }
            set
        }
        Regex::Star(r) => {
            pderiv(r, x)
                .into_iter()
                .map(|r_prime| smart_seq(r_prime, &Regex::star(*r.clone())))
                .collect()
        }
    }
}

/// Match input using partial derivatives
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