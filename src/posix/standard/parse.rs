//! Standard POSIX parsers (recursive and loop)

use crate::types::{Regex, ParseTree};
use crate::regex::brzozowski::standard::deriv;
use crate::regex::nullable::standard::nullable;
use crate::posix::standard::{mk_eps, inject};

// ============================================================================
// RECURSIVE PARSER
// ============================================================================

fn parse_recursive_helper(r: &Regex, input: &str) -> Option<ParseTree> {
    let mut chars = input.chars();
    
    match chars.next() {
        None => {
            if nullable(r) {
                Some(mk_eps(r))
            } else {
                None
            }
        }
        Some(l) => {
            let rest: String = chars.collect();
            let r_deriv = deriv(r, l);
            let subtree = parse_recursive_helper(&r_deriv, &rest)?;
            Some(inject(r, l, subtree))
        }
    }
}

pub fn parse_recursive(input: &str, r: &Regex) -> Option<ParseTree> {
    parse_recursive_helper(r, input)
}

// ============================================================================
// LOOP PARSER
// ============================================================================

pub fn parse_loop(input: &str, r: &Regex) -> Option<ParseTree> {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    
    let mut expressions = Vec::with_capacity(n + 1);
    expressions.push(r.clone());
    
    // Forward pass
    for (_idx, &c) in chars.iter().enumerate() {
        let current = expressions.last().unwrap();
        let next = deriv(current, c);
        expressions.push(next);
    }
    
    let final_r = expressions.last().unwrap();
    if !nullable(final_r) {
        return None;
    }
    
    // Backward pass
    let mut tree = mk_eps(expressions.last().unwrap());
    for i in (0..n).rev() {
        tree = inject(&expressions[i], chars[i], tree);
    }
    
    Some(tree)
}