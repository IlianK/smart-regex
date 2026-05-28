//! Brzozowski derivative matcher (boolean match only)

use crate::types::Regex;
use crate::regex::brzozowski::standard::deriv;
use crate::regex::simplify::standard::simplify;
use crate::regex::nullable::standard::nullable;

pub fn match_deriv(input: &str, r: &Regex) -> bool {
    let mut current = r.clone();
    for c in input.chars() {
        current = simplify(deriv(&current, c));
    }
    nullable(&current)
}