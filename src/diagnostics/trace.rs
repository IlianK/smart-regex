//! Trace data structures populated by parse_loop_traced and parse_bitcoded_traced.
//!
//! These are passed to the Level 2 and Level 3 formatters.
//! The core parsers do not depend on this module.

use crate::types::{Regex, ARegex, ParseTree};

// ============================================================================
// Standard parser trace  (parse_loop_traced)
// ============================================================================

/// One step in the forward derivative pass.
#[derive(Debug, Clone)]
pub struct DerivStep {
    /// 1-indexed position in the input
    pub position: usize,
    /// Character consumed at this step
    pub character: char,
    /// Expression before this derivative step (rᵢ)
    pub before: Regex,
    /// Expression after deriv + simplify (rᵢ₊₁)
    pub after: Regex,
    /// Whether the resulting expression is nullable
    pub nullable: bool,
}

/// One step in the backward inject pass.
#[derive(Debug, Clone)]
pub struct InjectStep {
    /// 1-indexed position (counting backward from n down to 1)
    pub position: usize,
    /// Character injected
    pub character: char,
    /// Parse tree before injection (vᵢ₊₁)
    pub before: ParseTree,
    /// Parse tree after injection (vᵢ)
    pub after: ParseTree,
}

/// mkEps result recorded during the backward pass.
#[derive(Debug, Clone)]
pub struct MkEpsResult {
    /// The nullable expression mkEps was called on (rₙ)
    pub regex: Regex,
    /// The resulting empty parse tree
    pub tree: ParseTree,
}

/// Full trace from parse_loop_traced.
#[derive(Debug, Clone)]
pub struct ParseTrace {
    /// All expressions r0..rn stored during the forward pass
    pub expressions: Vec<Regex>,
    /// All derivative steps (one per character)
    pub deriv_steps: Vec<DerivStep>,
    /// mkEps result (None if parse failed)
    pub mk_eps_result: Option<MkEpsResult>,
    /// All inject steps in backward order (None if parse failed)
    pub inject_steps: Option<Vec<InjectStep>>,
    /// Index of the last nullable expression (for partial recovery on failure)
    pub last_nullable_idx: Option<usize>,
}

impl ParseTrace {
    /// Total derivative expressions computed (= input length + 1)
    pub fn expression_count(&self) -> usize {
        self.expressions.len()
    }

    /// Number of successful (nullable) steps before failure
    pub fn successful_steps(&self) -> usize {
        self.last_nullable_idx.map(|i| i).unwrap_or(0)
    }
}

// ============================================================================
// Bitcoded parser trace  (parse_bitcoded_traced)
// ============================================================================

/// One step in the bitcoded forward pass.
#[derive(Debug, Clone)]
pub struct BitStep {
    /// 1-indexed position
    pub position: usize,
    /// Character consumed
    pub character: char,
    /// Annotated expression before deriv_bc + simp (riᵢ)
    pub before: ARegex,
    /// Annotated expression after deriv_bc + simp (riᵢ₊₁)
    pub after: ARegex,
    /// Whether the resulting expression is nullable
    pub nullable: bool,
}

/// Full trace from parse_bitcoded_traced.
#[derive(Debug, Clone)]
pub struct BitTrace {
    /// Internalized expression (ri₀)
    pub internalized: ARegex,
    /// All bit steps (one per character)
    pub bit_steps: Vec<BitStep>,
    /// Bit sequence produced by mkEpsBC (None if parse failed)
    pub final_bits: Option<Vec<bool>>,
    /// Index of the last nullable riᵢ (for partial recovery on failure)
    pub last_nullable_idx: Option<usize>,
    /// Accumulated bits at the last nullable step (for failure reporting)
    pub bits_at_last_nullable: Option<Vec<bool>>,
}

impl BitTrace {
    pub fn successful_steps(&self) -> usize {
        self.last_nullable_idx.map(|i| i).unwrap_or(0)
    }
}