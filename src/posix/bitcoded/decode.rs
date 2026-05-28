//! Decoder: bit-code → ParseTree
//!
//! Based on Figure 4 of the paper

use crate::types::{Regex, ParseTree};

pub fn decode(r: &Regex, bs: &[bool]) -> ParseTree {
    let (v, rest) = decode_inner(r, bs);
    assert!(
        rest.is_empty(),
        "decode: {} leftover bit(s) after decoding {:?}",
        rest.len(),
        r
    );
    v
}

fn decode_inner<'a>(r: &Regex, bs: &'a [bool]) -> (ParseTree, &'a [bool]) {
    match r {
        Regex::Eps => (ParseTree::Empty, bs),
        Regex::Lit(c) => (ParseTree::Char(*c), bs),
        Regex::Alt(r1, r2) => match bs {
            [false, rest @ ..] => {
                let (v, remaining) = decode_inner(r1, rest);
                (ParseTree::Left(Box::new(v)), remaining)
            }
            [true, rest @ ..] => {
                let (v, remaining) = decode_inner(r2, rest);
                (ParseTree::Right(Box::new(v)), remaining)
            }
            [] => panic!("decode: unexpected end of bits at Alt"),
        },
        Regex::Seq(r1, r2) => {
            let (v1, after_r1) = decode_inner(r1, bs);
            let (v2, after_r2) = decode_inner(r2, after_r1);
            (ParseTree::Pair(Box::new(v1), Box::new(v2)), after_r2)
        }
        Regex::Star(r1) => {
            let mut iterations = Vec::new();
            let mut remaining = bs;
            loop {
                match remaining {
                    [true, rest @ ..] => {
                        remaining = rest;
                        break;
                    }
                    [false, rest @ ..] => {
                        let (v, after) = decode_inner(r1, rest);
                        iterations.push(v);
                        remaining = after;
                    }
                    [] => panic!("decode: unexpected end of bits inside Star"),
                }
            }
            (ParseTree::Star(iterations), remaining)
        }
        Regex::Phi => panic!("decode: called on Phi"),
    }
}