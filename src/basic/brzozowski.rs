//! Brzozowski derivative matcher

use super::regex::Regex;
use super::common::simplify;
use super::common::{nullable};
use crate::debug_println;

/// Computes derivative of r based on character x
pub fn deriv(r: &Regex, x: char) -> Regex {
    debug_println!("[DEBUG]     deriv({:?}, '{}')", r, x);
    
    let result = match r {
        Regex::Phi => {
            debug_println!("[DEBUG]       -> Φ");
            Regex::Phi
        },
        Regex::Eps => {
            debug_println!("[DEBUG]       -> Φ (ε derivative)");
            Regex::Phi
        },
        Regex::Lit(c) => {
            if *c == x { 
                debug_println!("[DEBUG]       -> ε (literal match)");
                Regex::Eps 
            } else { 
                debug_println!("[DEBUG]       -> Φ (literal mismatch)");
                Regex::Phi 
            }
        },
        Regex::Alt(r, s) => {
            debug_println!("[DEBUG]       -> Alt(deriv(left), deriv(right))");
            Regex::alt(deriv(r, x), deriv(s, x))
        },
        Regex::Seq(r, s) => {
            let dr = Regex::seq(deriv(r, x), *s.clone());
            debug_println!("[DEBUG]       nullable(r) = {}", nullable(r));
            if nullable(r) {
                debug_println!("[DEBUG]       -> Alt(Seq(deriv(r), s), deriv(s))");
                Regex::alt(dr, deriv(s, x))
            } else {
                debug_println!("[DEBUG]       -> Seq(deriv(r), s)");
                dr
            }
        },
        Regex::Star(r) => {
            debug_println!("[DEBUG]       -> Seq(deriv(r), r*)");
            Regex::seq(deriv(r, x), Regex::star(*r.clone()))
        },
    };
    
    debug_println!("[DEBUG]       <- deriv result: {:?}", result);
    result
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