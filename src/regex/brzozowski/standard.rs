//! Brzozowski derivative for standard Regex

use crate::types::Regex;
use crate::regex::nullable::standard::nullable;

/// Computes derivative of r based on character x
pub fn deriv(r: &Regex, x: char) -> Regex {
    match r {
        Regex::Phi => Regex::Phi,
        Regex::Eps => Regex::Phi,
        Regex::Lit(c) => {
            if *c == x { Regex::Eps } else { Regex::Phi }
        }
        Regex::Alt(r1, r2) => {
            Regex::alt(deriv(r1, x), deriv(r2, x))
        }
        Regex::Seq(r1, r2) => {
            if nullable(r1) {
                let dr1 = Regex::seq(deriv(r1, x), *r2.clone());
                Regex::alt(dr1, deriv(r2, x))
            } else {
                Regex::seq(deriv(r1, x), *r2.clone())
            }
        }
        Regex::Star(r1) => {
            Regex::seq(deriv(r1, x), Regex::star(*r1.clone()))
        }
    }
}