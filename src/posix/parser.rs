//! POSIX parser using derivatives (Theorem 1)
//!
//! Provides two implementations:
//! - `parse_recursive`: Pure recursive version (matching Haskell)
//! - `parse_loop`: Iterative with vector storage
//! - `parse_posix`: Default uses recursive 

use crate::basic::{Regex, deriv, nullable};
use super::parse_tree::ParseTree;
use super::mk_eps::mk_eps;
use super::inject::inject;
use crate::debug_println;
use crate::posix::debug::step_reset;


/// Returns true if loop version should be used
fn use_loop_version() -> bool {
    std::env::var("REGEX_USE_LOOP").is_ok()
}


// RECURSIVE
fn parse_recursive_helper(r: &Regex, input: &str, depth: usize, is_forward: bool) -> Option<ParseTree> {
    let indent = "  ".repeat(depth);
    let mut chars = input.chars();
    let n = input.len();
    
    match chars.next() {                                                    // Empty input
        None => {
            // Build empty parse tree
            debug_println!("\n\n[--- Build Empty Parse Tree ---]");
            debug_println!("mkEps(r{}) = ?", n);
            debug_println!("{}mkEps({:?})", indent, r);
            
            // Nullability check
            if nullable(r) {
                let tree = mk_eps(r);                            // mkEps r
                debug_println!("{}v{} = {:?}", indent, n, tree);
                Some(tree)
            } else {
                debug_println!("{}X Not nullable -> no match", indent);
                None                                                        // error "no match"
            }
        }
        Some(l) => {                                                  // l = first char, rest = w
            let rest: String = chars.collect();                             // w
            let remaining_len = rest.len();
            let current_idx = n - 1 - remaining_len;
            let next_idx = n - remaining_len;
            
            // Forward pass (Derivatives)
            if is_forward {
                debug_println!("\n\n[--- Forward pass (Derivatives) ---]");
                debug_println!("r0 = {:?}", r);
            }
            
            debug_println!("{}r{}{} -> r{} = ?", indent, current_idx, l, next_idx);
            let r_deriv = deriv(r, l);                            // deriv l r
            debug_println!("{}  deriv = {:?}", indent, r_deriv);
            
            // Recursive call
            let subtree = parse_recursive_helper(               // parse (deriv l r) w
                &r_deriv, 
                &rest,
                depth + 1, 
                false)?;
            
            // Backward pass (Injection)
            debug_println!("\n\n[--- Backward pass (Injection) ---]");
            debug_println!("inject(r{}, '{}', v{})", current_idx, l, next_idx);
            debug_println!("{}inject({:?}, '{}', {:?})", indent, r, l, subtree);
            
            let result = inject(r, l, subtree);              // inj r (deriv l r) l $ subtree
            debug_println!("{}v{} = {:?}", indent, current_idx, result);
            
            // Final result
            Some(result)
        }
    }
}


/// Debug Prints 
pub fn parse_recursive(input: &str, r: &Regex) -> Option<ParseTree> {
    step_reset();
    debug_println!("\n[parse_recursive] input = \"{}\", regex = {:?}", input, r);
    
    let result = parse_recursive_helper(r, input, 0, true);
    
    if let Some(tree) = &result {
        debug_println!("\n\n[--- Result ---]");
        debug_println!("v0 = {:?}", tree);
        debug_println!("Flattened: \"{}\"", super::flatten(tree));
    } else {
        debug_println!("\n\n[--- Result ---]");
        debug_println!("✗ No match");
    }
    debug_println!("");
    
    result
}


// LOOP 
pub fn parse_loop(input: &str, r: &Regex) -> Option<ParseTree> {
    step_reset();
    
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    
    debug_println!("\n[parse_loop] input = \"{}\", regex = {:?}", input, r);
    
    let mut expressions = Vec::with_capacity(n + 1);
    expressions.push(r.clone());
    
    // Forward pass (Derivatives)
    debug_println!("\n\n[--- Forward pass (Derivatives) ---]");
    debug_println!("r0 = {:?}", r);
    
    for (idx, &c) in chars.iter().enumerate() {
        let current = expressions.last().unwrap();
        let next = deriv(current, c);
        debug_println!("r{}{} -> r{} = {:?}", idx, c, idx + 1, next);
        expressions.push(next);
    }
    
    // Nullability check
    let final_r = expressions.last().unwrap();
    if !nullable(final_r) {
        debug_println!("\n✗ Not nullable -> no match");
        return None;
    }
    
    // Build empty parse tree
    debug_println!("\n\n[--- Build Empty Parse Tree ---]");
    debug_println!("r{} nullable", n);
    debug_println!("mkEps(r{}) = ?", n);
    
    let mut tree = mk_eps(expressions.last().unwrap());
    debug_println!("v{} = {:?}", n, tree);
    
    // Backward pass (Injection)
    debug_println!("\n\n[--- Backward pass (Injection) ---]");
    for i in (0..n).rev() {
        debug_println!("\ninject(r{}, '{}', v{})", i, chars[i], i + 1);
        tree = inject(&expressions[i], chars[i], tree);
        debug_println!("v{} = {:?}", i, tree);
    }
    
    // Final result
    debug_println!("\n\n[--- Result ---]");
    debug_println!("v0 = {:?}", tree);
    debug_println!("Flattened: \"{}\"\n", super::flatten(&tree));
    
    Some(tree)
}


// EXPORTS
pub fn parse_posix(input: &str, r: &Regex) -> Option<ParseTree> {
    if use_loop_version() {
        debug_println!("[parse_posix] Using LOOP parser");
        parse_loop(input, r)
    } else {
        debug_println!("[parse_posix] Using RECURSIVE parser");
        parse_recursive(input, r)
    }
}

pub fn match_posix(input: &str, r: &Regex) -> bool {
    parse_posix(input, r).is_some()
}