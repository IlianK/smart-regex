//! Traced deriv_bc parser (REGEX_DIAG=2/3): re-implements parse.rs's loop variant, recording a BitTrace.

use crate::types::{Regex, ParseTree};
use crate::parsers::deriv_bc::nullable::nullable_bc;
use crate::parsers::deriv_bc::deriv::deriv_bc;
use crate::parsers::deriv_bc::simplify::simp;
use super::internalize::internalize;
use super::mk_eps_bc::mk_eps_bc;
use crate::regex::decode::decode;
use crate::trace::{BitStep, BitTrace};

/// Identical in behaviour to parse_bitcoded_loop but also returns a BitTrace.
pub fn parse_bitcoded_traced(input: &str, r: &Regex) -> (Option<ParseTree>, BitTrace) {
    let chars: Vec<char> = input.chars().collect();
    let ri0 = internalize(r);

    let mut ri = ri0.clone();
    let mut bit_steps: Vec<BitStep> = Vec::with_capacity(chars.len());
    let mut last_nullable_idx: Option<usize> = if nullable_bc(&ri) { Some(0) } else { None };
    let mut bits_at_last_nullable: Option<Vec<bool>> = None;

    // Forward pass
    for (idx, &c) in chars.iter().enumerate() {
        let before = ri.clone();
        ri = simp(deriv_bc(ri, c));
        let is_nullable = nullable_bc(&ri);

        bit_steps.push(BitStep {
            position:  idx + 1,
            character: c,
            before,
            after:     ri.clone(),
            nullable:  is_nullable,
        });

        if is_nullable {
            last_nullable_idx = Some(idx + 1);
            bits_at_last_nullable = Some(mk_eps_bc(&ri));
        }
    }

    // Nullability check
    if !nullable_bc(&ri) {
        let trace = BitTrace {
            internalized: ri0,
            bit_steps,
            final_bits: None,
            last_nullable_idx,
            bits_at_last_nullable,
        };
        return (None, trace);
    }

    // mkEpsBC + decode
    let bits = mk_eps_bc(&ri);
    let tree = decode(r, &bits);

    let trace = BitTrace {
        internalized: ri0,
        bit_steps,
        final_bits: Some(bits),
        last_nullable_idx,
        bits_at_last_nullable,
    };

    (Some(tree), trace)
}


// Unit tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ARegex;
    use super::super::parse::parse_deriv_bc;

    #[test]
    fn traced_matches_untraced_on_success() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_deriv_bc("aaa", &r);
        let (traced, trace) = parse_bitcoded_traced("aaa", &r);
        assert_eq!(plain, traced);
        assert!(traced.is_some());
        assert!(trace.final_bits.is_some());
        assert_eq!(trace.bit_steps.len(), 3);
    }

    #[test]
    fn traced_matches_untraced_on_failure() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_deriv_bc("aab", &r);
        let (traced, trace) = parse_bitcoded_traced("aab", &r);
        assert_eq!(plain, traced);
        assert!(traced.is_none());
        assert!(trace.final_bits.is_none());
        assert_eq!(trace.last_nullable_idx, Some(2));
        assert!(trace.bits_at_last_nullable.is_some());
    }

    #[test]
    fn traced_records_internalized_expression() {
        let r = Regex::star(Regex::lit('a'));
        let (_, trace) = parse_bitcoded_traced("a", &r);
        assert!(matches!(trace.internalized, ARegex::Star(ref bs, _) if bs.is_empty()));
    }
}
