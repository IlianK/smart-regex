//! Brzozowski derivative matcher

use super::regex::Regex;
use super::common::simplify;
use super::common::{nullable};

/// Computes derivative of r based on character x
pub fn deriv(r: &Regex, x: char) -> Regex {
    match r {
        Regex::Phi => Regex::Phi,
        Regex::Eps => Regex::Phi,
        Regex::Lit(c) => {
            if *c == x { Regex::Eps } else { Regex::Phi }
        }
        Regex::Alt(r, s) => {
            Regex::alt(deriv(r, x), deriv(s, x))
        }
        Regex::Seq(r, s) => {
            let dr = Regex::seq(deriv(r, x), *s.clone());
            if nullable(r) {
                Regex::alt(dr, deriv(s, x))
            } else {
                dr
            }
        }
        Regex::Star(r) => {
            Regex::seq(deriv(r, x), Regex::star(*r.clone()))
        }
    }
}

/// Derivative with simplification
pub fn deriv_simp(r: &Regex, c: char) -> Regex {
    simplify(deriv(r, c))
}

/// Match input using Brzozowski derivatives
pub fn match_deriv(input: &str, r: &Regex) -> bool {
    let mut current = r.clone();
    for c in input.chars() {
        current = simplify(deriv(&current, c));
    }
    nullable(&current)
}