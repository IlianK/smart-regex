//! Traced pderiv_bc parser (REGEX_DIAG=2/3): wraps parse.rs's step_frontier/select_bits, recording a PDerivBitTrace.

use crate::types::{Regex, ParseTree};
use crate::trace::{PDerivBitStep, PDerivBitTrace};
use super::parse::{step_frontier, select_bits};
use crate::regex::decode::decode;

pub fn parse_pderiv_bc_traced(input: &str, r: &Regex) -> (Option<ParseTree>, PDerivBitTrace) {
    let chars: Vec<char> = input.chars().collect();
    let initial = vec![(r.clone(), Vec::new())];

    let mut frontier = initial.clone();
    let mut steps: Vec<PDerivBitStep> = Vec::with_capacity(chars.len());
    let mut last_nullable_idx: Option<usize> = None;
    let mut bits_at_last_nullable: Option<Vec<bool>> = None;

    if let Some(bits) = select_bits(&frontier) {
        last_nullable_idx = Some(0);
        bits_at_last_nullable = Some(bits);
    }

    for (idx, &c) in chars.iter().enumerate() {
        let before = frontier.clone();
        frontier = step_frontier(&frontier, c);
        let bits_here = select_bits(&frontier);

        steps.push(PDerivBitStep {
            position: idx + 1,
            character: c,
            before,
            after: frontier.clone(),
            nullable: bits_here.is_some(),
        });

        if let Some(ref bits) = bits_here {
            last_nullable_idx = Some(idx + 1);
            bits_at_last_nullable = Some(bits.clone());
        }
    }

    let final_bits = select_bits(&frontier);
    let tree = final_bits.as_ref().map(|bits| decode(r, bits));

    let trace = PDerivBitTrace {
        initial,
        steps,
        final_bits,
        last_nullable_idx,
        bits_at_last_nullable,
    };

    (tree, trace)
}


// Unit tests

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parse::parse_pderiv_bc;

    #[test]
    fn traced_matches_untraced_on_success() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_pderiv_bc("aaa", &r);
        let (traced, trace) = parse_pderiv_bc_traced("aaa", &r);
        assert_eq!(plain, traced);
        assert!(traced.is_some());
        assert!(trace.final_bits.is_some());
        assert_eq!(trace.steps.len(), 3);
    }

    #[test]
    fn traced_matches_untraced_on_failure() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_pderiv_bc("aab", &r);
        let (traced, trace) = parse_pderiv_bc_traced("aab", &r);
        assert_eq!(plain, traced);
        assert!(traced.is_none());
        assert!(trace.final_bits.is_none());
        assert_eq!(trace.last_nullable_idx, Some(2));
        assert!(trace.bits_at_last_nullable.is_some());
        // still records a step for every character, even after the frontier dies out
        assert_eq!(trace.steps.len(), 3);
    }
}
