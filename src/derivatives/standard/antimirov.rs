//! Antimirov partial derivatives for standard Regex

use std::collections::HashSet;
use crate::types::Regex;
use super::nullable::nullable;
use super::simplify::smart_seq;

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