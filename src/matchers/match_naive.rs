//! regex-engine/src/matchers/match_naive.rs
//! 
//! Naive recursive matcher (boolean)

use crate::types::Regex;

fn match_naive_range(word: &[char], i: usize, j: usize, r: &Regex) -> bool {
    match r {
        Regex::Phi => false,
        Regex::Eps => i == j,
        Regex::Lit(c) => j == i + 1 && word.get(i) == Some(c),
        Regex::Alt(r, s) => {
            match_naive_range(word, i, j, r) || match_naive_range(word, i, j, s)
        }
        Regex::Seq(r, s) => {
            (i..=j).any(|k| {
                match_naive_range(word, i, k, r) && match_naive_range(word, k, j, s)
            })
        }
        Regex::Star(r) => {
            i == j
            || (i + 1..=j).any(|k| {
                match_naive_range(word, i, k, r)
                && match_naive_range(word, k, j, &Regex::star(*r.clone()))
            })
        }
    }
}

pub fn match_naive(input: &str, r: &Regex) -> bool {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    match_naive_range(&chars, 0, len, r)
}

// -------------------------------
// Tests for match_naive
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Regex;

    #[test] fn phi_never_matches()      { assert!(!match_naive("",  &Regex::Phi)); }
    #[test] fn eps_matches_empty()      { assert!(match_naive("",   &Regex::Eps)); }
    #[test] fn eps_no_nonempty()        { assert!(!match_naive("a", &Regex::Eps)); }
    #[test] fn lit_matches_char()       { assert!(match_naive("a",  &Regex::lit('a'))); }
    #[test] fn lit_wrong_char()         { assert!(!match_naive("b", &Regex::lit('a'))); }
    #[test] fn star_matches_empty()     { assert!(match_naive("",   &Regex::star(Regex::lit('a')))); }
    #[test] fn star_matches_repeated()  { assert!(match_naive("aaa",&Regex::star(Regex::lit('a')))); }
    #[test] fn seq_matches()            { assert!(match_naive("ab", &Regex::seq(Regex::lit('a'), Regex::lit('b')))); }
    #[test] fn seq_too_short()          { assert!(!match_naive("a", &Regex::seq(Regex::lit('a'), Regex::lit('b')))); }
    #[test] fn alt_left()               { assert!(match_naive("a",  &Regex::alt(Regex::lit('a'), Regex::lit('b')))); }
    #[test] fn alt_right()              { assert!(match_naive("b",  &Regex::alt(Regex::lit('a'), Regex::lit('b')))); }
    #[test] fn alt_neither()            { assert!(!match_naive("c", &Regex::alt(Regex::lit('a'), Regex::lit('b')))); }
}