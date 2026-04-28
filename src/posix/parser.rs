//! POSIX parser using derivatives (Theorem 1)

use crate::basic::{Regex, deriv, nullable};
use super::parse_tree::ParseTree;
use super::mk_eps::mk_eps;
use super::inject::inject;
use crate::debug_println;
use crate::posix::debug::step_reset;

pub fn parse_posix(input: &str, r: &Regex) -> Option<ParseTree> {
    step_reset();
    
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    
    debug_println!("\n[parse_posix] input = \"{}\", regex = {:?}", input, r);
    
    let mut expressions = Vec::with_capacity(n + 1);
    expressions.push(r.clone());
    
    
    // Forward pass
    debug_println!("\n\n[--- Forward pass ---]");
    debug_println!("r0 = {:?}", r);
    
    for (idx, &c) in chars.iter().enumerate() {
        let current = expressions.last().unwrap();
        let next = deriv(current, c);
        debug_println!("r{}{} -> r{} = {:?}", idx, c, idx + 1, next);
        expressions.push(next);
    }
    
    let final_r = expressions.last().unwrap();
    if !nullable(final_r) {
        debug_println!("\n✗ Not nullable -> no match");
        return None;
    }
    

    // Backward pass
    debug_println!("\n\n[--- Backward pass ---]");
    debug_println!("r{} nullable", n);
    debug_println!("mkEps(r{}) ->", n);
    
    let mut tree = mk_eps(expressions.last().unwrap());
    debug_println!("v{} = {:?}", n, tree);


    // Injection
    debug_println!("\n\n[--- Inject pass ---]");
    for i in (0..n).rev() {
        debug_println!("\ninject(r{}, '{}', v{})", i, chars[i], i + 1);
        tree = inject(&expressions[i], chars[i], tree);
        debug_println!("v{} = {:?}", i, tree);
    }


    // Final result
    debug_println!("\n\n[--- Result ---]");
    debug_println!("Result: v0 = {:?}", tree);
    debug_println!("Flattened: \"{}\"\n", super::flatten(&tree));
    
    Some(tree)
}


pub fn match_posix(input: &str, r: &Regex) -> bool {
    parse_posix(input, r).is_some()
}