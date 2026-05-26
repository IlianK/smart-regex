//! Brzozowski derivative matcher (boolean match only)

use crate::types::Regex;
use crate::derivatives::standard::{deriv, simplify, nullable};

pub fn match_deriv(input: &str, r: &Regex) -> bool {
    let mut current = r.clone();
    for c in input.chars() {
        current = simplify(deriv(&current, c));
    }
    nullable(&current)
}