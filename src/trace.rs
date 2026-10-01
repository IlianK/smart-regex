//! src/trace.rs
//!
//! Trace data structures for the `*_traced` parser variants. 
//! One type per parser family; consumed by `src/diagnostics/` 
//! for `--diag 2`/`--diag 3` reports

use crate::types::{Regex, ARegex, ParseTree};


// Standard derivative-based parser (parse_loop_traced / parse_recursive_traced)

/// One forward derivative step: `before`/`after` expressions 
/// around `character`, and whether `after` is nullable.
#[derive(Debug, Clone)]
pub struct DerivStep {
    pub position: usize,
    pub character: char,
    pub before: Regex,
    pub after: Regex,
    pub nullable: bool,
}

/// One backward `inject` step: `before`/`after` trees around `character`.
#[derive(Debug, Clone)]
pub struct InjectStep {
    pub position: usize,
    pub character: char,
    pub before: ParseTree,
    pub after: ParseTree,
}

/// The `mkEps` call that seeds the backward pass.
#[derive(Debug, Clone)]
pub struct MkEpsResult {
    pub regex: Regex,
    pub tree: ParseTree,
}

/// Full trace from `parse_loop_traced` / `parse_recursive_traced`.
#[derive(Debug, Clone)]
pub struct ParseTrace {
    pub expressions: Vec<Regex>,
    pub deriv_steps: Vec<DerivStep>,
    pub mk_eps_result: Option<MkEpsResult>,
    pub inject_steps: Option<Vec<InjectStep>>,
    pub last_nullable_idx: Option<usize>,
}

impl ParseTrace {
    pub fn expression_count(&self) -> usize {
        self.expressions.len()
    }

    pub fn successful_steps(&self) -> usize {
        self.last_nullable_idx.unwrap_or(0)
    }
}

// Bit-coded derivative-based parser (parse_bitcoded_traced)

/// One bit-coded forward step: `before`/`after` annotated expressions
/// around `deriv_bc + simp`, and whether `after` is nullable.
#[derive(Debug, Clone)]
pub struct BitStep {
    pub position: usize,
    pub character: char,
    pub before: ARegex,
    pub after: ARegex,
    pub nullable: bool,
}

/// Full trace from `parse_bitcoded_traced`.
#[derive(Debug, Clone)]
pub struct BitTrace {
    pub internalized: ARegex,
    pub bit_steps: Vec<BitStep>,
    pub final_bits: Option<Vec<bool>>,
    pub last_nullable_idx: Option<usize>,
    pub bits_at_last_nullable: Option<Vec<bool>>,
}

impl BitTrace {
    pub fn successful_steps(&self) -> usize {
        self.last_nullable_idx.unwrap_or(0)
    }
}


// Bit-coded partial-derivative parser (parse_pderiv_bc_traced)

/// One frontier step: every `(residual, accumulated bits)` 
/// pair still alive after consuming `character`, 
/// one per surviving strand of nondeterminism.
#[derive(Debug, Clone)]
pub struct PDerivBitStep {
    pub position: usize,
    pub character: char,
    pub before: Vec<(Regex, Vec<bool>)>,
    pub after: Vec<(Regex, Vec<bool>)>,
    pub nullable: bool,
}

/// Full trace from `parse_pderiv_bc_traced`.
#[derive(Debug, Clone)]
pub struct PDerivBitTrace {
    pub initial: Vec<(Regex, Vec<bool>)>,
    pub steps: Vec<PDerivBitStep>,
    pub final_bits: Option<Vec<bool>>,
    pub last_nullable_idx: Option<usize>,
    pub bits_at_last_nullable: Option<Vec<bool>>,
}

impl PDerivBitTrace {
    pub fn successful_steps(&self) -> usize {
        self.last_nullable_idx.unwrap_or(0)
    }
}


// Standard partial-derivative parser (parse_pderiv_std_traced)

/// One frontier step, residuals only. 
/// Injections are `Rc<dyn Fn(ParseTree) -> ParseTree>` and have no printable form, 
/// so a nullable strand carries its fully-built `ParseTree` directly
#[derive(Debug, Clone)]
pub struct PDerivStdStep {
    pub position: usize,
    pub character: char,
    pub before: Vec<Regex>,
    pub after: Vec<Regex>,
    pub nullable: bool,
}

/// Full trace from `parse_pderiv_std_traced`.
#[derive(Debug, Clone)]
pub struct PDerivStdTrace {
    pub initial: Vec<Regex>,
    pub steps: Vec<PDerivStdStep>,
    pub final_tree: Option<ParseTree>,
    pub last_nullable_idx: Option<usize>,
    pub tree_at_last_nullable: Option<ParseTree>,
}

impl PDerivStdTrace {
    /// Characters matched before failure (0 on a full failure).
    pub fn successful_steps(&self) -> usize {
        self.last_nullable_idx.unwrap_or(0)
    }
}