//! Brzozowski derivative matcher

use super::regex::Regex;
use super::common::{nullable, simplify};
use crate::debug_println;
use crate::posix::debug::{indent_inc, indent_dec};


pub fn deriv(r: &Regex, x: char) -> Regex {
    debug_println!("∂({:?}, {})", r, x);
    indent_inc();
    
    let result = match r {
        Regex::Phi => {
            debug_println!("∅");
            Regex::Phi
        }
        Regex::Eps => {
            debug_println!("∅");
            Regex::Phi
        }
        Regex::Lit(c) => {
            if *c == x {
                debug_println!("ε");
                Regex::Eps
            } else {
                debug_println!("∅");
                Regex::Phi
            }
        }
        Regex::Alt(r1, r2) => {
            debug_println!("∂({:?}) + ∂({:?})", r1, r2);
            Regex::alt(deriv(r1, x), deriv(r2, x))
        }
        Regex::Seq(r1, r2) => {
            if nullable(r1) {
                debug_println!("∂({:?})·{:?} + ∂({:?})", r1, r2, r2);
                let dr1 = Regex::seq(deriv(r1, x), *r2.clone());
                Regex::alt(dr1, deriv(r2, x))
            } else {
                debug_println!("∂({:?})·{:?}", r1, r2);
                Regex::seq(deriv(r1, x), *r2.clone())
            }
        }
        Regex::Star(r1) => {
            debug_println!("∂({:?})·{:?}*", r1, r1);
            Regex::seq(deriv(r1, x), Regex::star(*r1.clone()))
        }
    };
    
    indent_dec();
    debug_println!("= {:?}", result);
    result
}


pub fn deriv_simp(r: &Regex, c: char) -> Regex {
    simplify(deriv(r, c))
}


pub fn match_deriv(input: &str, r: &Regex) -> bool {
    let mut current = r.clone();
    for c in input.chars() {
        current = simplify(deriv(&current, c));
    }
    nullable(&current)
}