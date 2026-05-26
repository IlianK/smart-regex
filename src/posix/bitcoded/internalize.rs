//! internalize: Regex → ARegex
//! fuse: prepend bit-code prefix

use crate::types::{Regex, ARegex};

pub fn fuse(bs: &[bool], ri: ARegex) -> ARegex {
    if bs.is_empty() { return ri; }
    match ri {
        ARegex::Phi => ARegex::Phi,
        ARegex::Eps(p) => ARegex::Eps(combine(bs, p)),
        ARegex::Lit(p, c) => ARegex::Lit(combine(bs, p), c),
        ARegex::Alt(p, r1, r2) => ARegex::Alt(combine(bs, p), r1, r2),
        ARegex::Seq(p, r1, r2) => ARegex::Seq(combine(bs, p), r1, r2),
        ARegex::Star(p, r) => ARegex::Star(combine(bs, p), r),
    }
}

fn combine(bs: &[bool], p: Vec<bool>) -> Vec<bool> {
    let mut new = bs.to_vec();
    new.extend(p);
    new
}

pub fn internalize(r: &Regex) -> ARegex {
    match r {
        Regex::Phi => ARegex::Phi,
        Regex::Eps => ARegex::Eps(vec![]),
        Regex::Lit(c) => ARegex::Lit(vec![], *c),
        Regex::Alt(r1, r2) => {
            let ri1 = fuse(&[false], internalize(r1));
            let ri2 = fuse(&[true], internalize(r2));
            ARegex::Alt(vec![], Box::new(ri1), Box::new(ri2))
        }
        Regex::Seq(r1, r2) => {
            ARegex::Seq(vec![], Box::new(internalize(r1)), Box::new(internalize(r2)))
        }
        Regex::Star(r1) => {
            ARegex::Star(vec![], Box::new(internalize(r1)))
        }
    }
}