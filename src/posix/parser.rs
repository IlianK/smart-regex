//! POSIX parser using derivatives (Theorem 1)

use crate::basic::{Regex, deriv, nullable};
use super::parse_tree::ParseTree;
use super::mk_eps::mk_eps;
use super::inject::inject;

/// Parses input string according to POSIX disambiguation policy
///
/// Algorithm:
/// 1. Repeatedly apply derivative to get r0 -> r1 -> ... -> rn
/// 2. Build empty parse tree vn = mkEps(rn)
/// 3. Backwards inject letters: vi = inj(ri, li+1, vi+1)
/// 4. Return v0 as the POSIX parse tree
///
/// Returns None if input is not in language of r
/// 
/// Example: parse_posix("ab", a·b)
pub fn parse_posix(input: &str, r: &Regex) -> Option<ParseTree> {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    
    // Store expressions (unsimplified for injection)
    let mut expressions = Vec::with_capacity(n + 1);
    expressions.push(r.clone());
    

    // 1. Forward pass: compute derivatives 
    // deriv(Seq(r1, r2), c) = Seq(deriv(r1, c), r2)
    for &c in &chars {
        let next = deriv(&expressions.last().unwrap(), c);
        expressions.push(next);
    }

    // Iteration 1: Process 'a'
    // r0 = a·b;  c1 = 'a', 
    // r1 = deriv(a·b, 'a') = ε·b

    // Iteration 2: Process b (r1 nullable)
    // r1 = ε·b
    // r2 = deriv(r1, 'b') 
    //
    // Since nullable(ε) = true, Seq rule: 
    //  = deriv(Seq(r_left, r_right), c) 
    //  = deriv(Seq((ε, b), b))
    //
    //  = Alt(Seq(deriv(r_left, c), r_right), deriv(r_right, c))
    //  = Alt(Seq(deriv(ε, 'b'), b), deriv(b, 'b'))
    //  
    //  = deriv(ε, 'b'), b) = (φ, b)   LEFT
    //  = deriv(b, 'b') = ε            RIGHT
    //
    //  r2 = Alt(Seq((φ, b), ε)
    

    // Check if final expression is nullable (input in language)
    if !nullable(expressions.last().unwrap()) {
        return None;
    }
    
    // LEFT:    nullable(Seq((φ, b)) = nullable(φ) && nullable(b) = false && false = false
    // RIGHGT:  nullable(ε) = true
    // -> Input in language


    // 2. Backward pass: build parse tree by injecting letters (start with empty)
    let mut tree = mk_eps(expressions.last().unwrap());

    // mk_eps processes exp[2] = Alt(Seq((φ, b), ε)
    // -> right alternative was chosen (left not nullable)

    
    // Inject letters c from last to first
    for i in (0..n).rev() {
        let r_i = &expressions[i];
        let c = chars[i];
        tree = inject(r_i, c, tree);
    }

    // Loop i=1: inject(r1, 'b', tree)
    // r_i = exp[1] = Seq(ε, b)
    // c = chars[1] = b
    // tree = Right(EMPTY) -> from mk_eps

    // Inject gets: Seq(ε, b) with nullable(r1) = true
    // v2 = Empty
    // v2_inj = inject(Lit('b'), 'b', Empty) -> sub inject for Lit b
    // v1_eps = mk_eps(r1) = mk_eps(Eps) = Empty
    // returns Char b: Pair(Empty, Char('b'))
      

    // Loop i=0: inject(r0, 'a', tree)
    // r_i = exp[0] = Seq(Lit('a'), Lit('b'))
    // c = chars[0] = 'a'
    // tree = Pair(Empty, Char('b'))

    // Inject gets: Seq(Lit('a'), Lit('b')) with nullable(r1) = false
    // Lit cannot match empty string
    // v1 = Empty
    // v2 = Char b
    // v1_inj = inject(Lit('a'), 'a', Empty) -> sub inject for a
    // returns Char a: Pair(Char('a'), Char('b'))
    
    Some(tree)
    // Flattened ab
}


/// Parse and check if match (boolean)
pub fn match_posix(input: &str, r: &Regex) -> bool {
    parse_posix(input, r).is_some()
}