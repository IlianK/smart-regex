//! src/parsers/deriv_std/parse.rs
//! 
//! Plain-Regex POSIX parsers (deriv_std): recursive and loop variants; traced counterparts in traced.rs.

use crate::types::{Regex, ParseTree};
use crate::regex::deriv::deriv;
use crate::regex::nullable::nullable;
use crate::regex::mk_eps::mk_eps;
use crate::parsers::deriv_std::inject::inject;


// RECURSIVE PARSER

/// Indexes into `chars` instead of collecting the remaining input into a
/// fresh `String` on every call: the old version allocated and copied an
/// O(n), O(n-1), O(n-2)... suffix at each step, O(n^2) total.
fn parse_recursive_helper(r: &Regex, chars: &[char], pos: usize) -> Option<ParseTree> {
    match chars.get(pos) {
        None => {
            if nullable(r) { Some(mk_eps(r)) } else { None }
        }
        Some(&l) => {
            let r_deriv = deriv(r, l);
            let subtree = parse_recursive_helper(&r_deriv, chars, pos + 1)?;
            Some(inject(r, l, subtree))
        }
    }
}

pub fn parse_deriv_std_rec(input: &str, r: &Regex) -> Option<ParseTree> {
    let chars: Vec<char> = input.chars().collect();
    parse_recursive_helper(r, &chars, 0).map(reverse_stars)
}

/// Reverses every `Star`'s items exactly once. `inject`'s Star case
/// (see inject.rs) now `push`es a new iteration onto the existing tail
/// instead of rebuilding the whole vec by `extend`, O(1) amortized
/// instead of O(current length) -- which leaves every Star's items in
/// reverse temporal order until this single O(total tree size)
/// correction pass, applied once here after the full backward pass
/// completes.
pub(super) fn reverse_stars(v: ParseTree) -> ParseTree {
    match v {
        ParseTree::Empty | ParseTree::Char(_) => v,
        ParseTree::Pair(v1, v2) => ParseTree::Pair(Box::new(reverse_stars(*v1)), Box::new(reverse_stars(*v2))),
        ParseTree::Left(v1) => ParseTree::Left(Box::new(reverse_stars(*v1))),
        ParseTree::Right(v1) => ParseTree::Right(Box::new(reverse_stars(*v1))),
        ParseTree::Star(mut items) => {
            items.reverse();
            ParseTree::Star(items.into_iter().map(reverse_stars).collect())
        }
    }
}


// LOOP PARSER

pub fn parse_deriv_std_loop(input: &str, r: &Regex) -> Option<ParseTree> {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();

    let mut expressions = Vec::with_capacity(n + 1);
    expressions.push(r.clone());

    for &c in chars.iter() {
        let current = expressions.last().unwrap();
        let next = deriv(current, c);
        expressions.push(next);
    }

    let final_r = expressions.last().unwrap();
    if !nullable(final_r) {
        return None;
    }

    let mut tree = mk_eps(expressions.last().unwrap());
    for i in (0..n).rev() {
        tree = inject(&expressions[i], chars[i], tree);
    }

    Some(reverse_stars(tree))
}


// Unit tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::flatten;
    use crate::parsers::deriv_bc::parse_deriv_bc;

    #[test]
    fn star_iterations_are_in_left_to_right_order() {
        // (a|b)* on "abab": a bug in the push/reverse rewrite would
        // reverse this to [b,a,b,a] instead of the correct [a,b,a,b].
        let r = Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')));

        let tree_rec = parse_deriv_std_rec("abab", &r).unwrap();
        let tree_loop = parse_deriv_std_loop("abab", &r).unwrap();
        assert_eq!(tree_rec, tree_loop);
        assert_eq!(flatten(&tree_rec), "abab");
        assert_eq!(
            tree_rec,
            ParseTree::Star(vec![
                ParseTree::Left(Box::new(ParseTree::Char('a'))),
                ParseTree::Right(Box::new(ParseTree::Char('b'))),
                ParseTree::Left(Box::new(ParseTree::Char('a'))),
                ParseTree::Right(Box::new(ParseTree::Char('b'))),
            ])
        );
        assert_eq!(tree_rec, parse_deriv_bc("abab", &r).unwrap());
    }

    #[test]
    fn nested_star_iterations_match_bc_order() {
        // ((a|b)(a|b))* on "abba": two-character iterations, so a
        // within-iteration order bug (not just a between-iteration one)
        // would also show up here.
        let pair = Regex::seq(
            Regex::alt(Regex::lit('a'), Regex::lit('b')),
            Regex::alt(Regex::lit('a'), Regex::lit('b')),
        );
        let r = Regex::star(pair);

        let tree_rec = parse_deriv_std_rec("abba", &r).unwrap();
        let tree_loop = parse_deriv_std_loop("abba", &r).unwrap();
        assert_eq!(tree_rec, tree_loop);
        assert_eq!(flatten(&tree_rec), "abba");
        assert_eq!(tree_rec, parse_deriv_bc("abba", &r).unwrap());
    }
}
