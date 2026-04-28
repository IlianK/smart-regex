//! Naive recursive matcher (exponential time)

use super::regex::Regex;

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