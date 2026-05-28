//! Antimirov partial derivative matcher (boolean match only)

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