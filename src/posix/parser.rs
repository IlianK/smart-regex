//! POSIX parser using derivatives (Theorem 1)

use crate::types::{Regex, ParseTree};
use crate::derivatives::standard::{deriv, nullable};
use crate::posix::{mk_eps, inject};
use crate::debug_println;
use crate::posix::debug::step_reset;

fn use_loop_version() -> bool {
    std::env::var("REGEX_USE_LOOP").is_ok()
}

// ============================================================================
// RECURSIVE PARSER
// ============================================================================

fn parse_recursive_helper(r: &Regex, input: &str, depth: usize, is_forward: bool) -> Option<ParseTree> {
    let indent = "  ".repeat(depth);
    let mut chars = input.chars();
    let n = input.len();
    
    match chars.next() {
        None => {
            debug_println!("\n\n[--- Build Empty Parse Tree ---]");
            debug_println!("mkEps(r{}) = ?", n);
            debug_println!("{}mkEps({:?})", indent, r);
            
            if nullable(r) {
                let tree = mk_eps(r);
                debug_println!("{}v{} = {:?}", indent, n, tree);
                Some(tree)
            } else {
                debug_println!("{}X Not nullable -> no match", indent);
                None
            }
        }
        Some(l) => {
            let rest: String = chars.collect();
            let remaining_len = rest.len();
            let current_idx = n - 1 - remaining_len;
            let next_idx = n - remaining_len;
            
            if is_forward {
                debug_println!("\n\n[--- Forward pass (Derivatives) ---]");
                debug_println!("r0 = {:?}", r);
            }
            
            debug_println!("{}r{}{} -> r{} = ?", indent, current_idx, l, next_idx);
            let r_deriv = deriv(r, l);
            debug_println!("{}  deriv = {:?}", indent, r_deriv);
            
            let subtree = parse_recursive_helper(&r_deriv, &rest, depth + 1, false)?;
            
            debug_println!("\n\n[--- Backward pass (Injection) ---]");
            debug_println!("inject(r{}, '{}', v{})", current_idx, l, next_idx);
            debug_println!("{}inject({:?}, '{}', {:?})", indent, r, l, subtree);
            
            let result = inject(r, l, subtree);
            debug_println!("{}v{} = {:?}", indent, current_idx, result);
            
            Some(result)
        }
    }
}

pub fn parse_recursive(input: &str, r: &Regex) -> Option<ParseTree> {
    step_reset();
    debug_println!("\n[parse_recursive] input = \"{}\", regex = {:?}", input, r);
    
    let result = parse_recursive_helper(r, input, 0, true);
    
    if let Some(tree) = &result {
        debug_println!("\n\n[--- Result ---]");
        debug_println!("v0 = {:?}", tree);
        debug_println!("Flattened: \"{}\"", crate::types::flatten(tree));
    } else {
        debug_println!("\n\n[--- Result ---]");
        debug_println!("✗ No match");
    }
    debug_println!("");
    
    result
}

// ============================================================================
// LOOP PARSER
// ============================================================================

pub fn parse_loop(input: &str, r: &Regex) -> Option<ParseTree> {
    step_reset();
    
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    
    debug_println!("\n[parse_loop] input = \"{}\", regex = {:?}", input, r);
    
    let mut expressions = Vec::with_capacity(n + 1);
    expressions.push(r.clone());
    
    debug_println!("\n\n[--- Forward pass (Derivatives) ---]");
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
    
    debug_println!("\n\n[--- Build Empty Parse Tree ---]");
    debug_println!("r{} nullable", n);
    debug_println!("mkEps(r{}) = ?", n);
    
    let mut tree = mk_eps(expressions.last().unwrap());
    debug_println!("v{} = {:?}", n, tree);
    
    debug_println!("\n\n[--- Backward pass (Injection) ---]");
    for i in (0..n).rev() {
        debug_println!("\ninject(r{}, '{}', v{})", i, chars[i], i + 1);
        tree = inject(&expressions[i], chars[i], tree);
        debug_println!("v{} = {:?}", i, tree);
    }
    
    debug_println!("\n\n[--- Result ---]");
    debug_println!("v0 = {:?}", tree);
    debug_println!("Flattened: \"{}\"\n", crate::types::flatten(&tree));
    
    Some(tree)
}

// ============================================================================
// EXPORTS
// ============================================================================

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