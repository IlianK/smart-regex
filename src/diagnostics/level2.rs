//! regex-engine/src/diagnostics/level2.rs
//!
//! Level 2 - Verbose diagnostics output.
//!
//! Standard success:
//!   Regex:  a*
//!   Input:  "aaa"
//!   Match:  true
//!   Tree:   [a, a, a]
//!   Time:   0.08ms
//!   Steps:  4 derivative expressions computed
//!
//!   Construction steps:
//!     mkEps(r3) → Right Right ((), [])
//!     inject(a*, 'a', Right Right ((), [])) → Right ((), [a])    ← position 3 (backward start)
//!     inject(a*, 'a', Right ((), [a])) → ((), [a, a])            ← position 2
//!     inject(a*, 'a', ((), [a, a])) → [a, a, a]                  ← position 1 (backward end)

use std::time::Instant;

use crate::types::{Regex, flatten};
use crate::posix::selection::ParserType;
use crate::posix::standard::{parse_loop_traced, parse_recursive_traced};
use crate::posix::bitcoded::parse_bitcoded_traced;
use crate::diagnostics::DiagConfig;
use crate::diagnostics::replay::{find_failure, caret_lines, partial_tree_standard};


// -------------------------------
// Entry point
// -------------------------------

pub fn run_parser(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    match config.parser_type {
        ParserType::DerivBC => run_parser_bitcoded(regex_str, r, input),
        ParserType::DerivRec | ParserType::DerivLoop => run_parser_standard(regex_str, r, input, config),
        ParserType::PDeriv | ParserType::PDerivBC => {
            unimplemented!("Level 2 diagnostics for pderiv-based parsing - not yet implemented")
        }
    }
}


// -------------------------------
// Standard Path
// -------------------------------

fn run_parser_standard(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    let start = Instant::now();

    // Respect REGEX_PARSER - use the traced variant matching the selected parser
    // (only DerivRec/DerivLoop reach here - caller in run_parser() already routed
    // DerivBC/PDeriv/PDerivBC elsewhere)
    let (result, trace) = match config.parser_type {
        ParserType::DerivRec => parse_recursive_traced(input, r),
        _                    => parse_loop_traced(input, r),
    };

    let elapsed = start.elapsed();
    let chars: Vec<char> = input.chars().collect();

    println!("Regex:  {}", regex_str);
    println!("Input:  {:?}", input);

    match &result {
        Some(tree) => {
            println!("Match:  true");
            println!("Tree:   {}", tree);
            println!("Time:   {:.3}ms", elapsed.as_secs_f64() * 1000.0);
            println!("Steps:  {} derivative expressions computed", trace.expression_count());
            println!();
            println!("Construction steps:");

            // mkEps applied to the final derivative expression rN
            if let Some(ref mke) = trace.mk_eps_result {
                let final_idx = trace.expression_count() - 1;
                println!("  mkEps(r{}) → {}", final_idx, mke.tree);
            }

            // inject lines - backward-pass order: highest position first (n → 1)
            if let Some(ref steps) = trace.inject_steps {
                let mut display_steps: Vec<_> = steps.iter().collect();
                display_steps.sort_by_key(|s| std::cmp::Reverse(s.position));
                for step in display_steps {
                    println!(
                        "  inject({}, '{}', {}) → {}",
                        regex_str, step.character, step.before, step.after
                    );
                }
            }
        }
        None => {
            let successful = trace.successful_steps();
            println!("Match:  false");
            println!("Time:   {:.3}ms", elapsed.as_secs_f64() * 1000.0);
            println!(
                "Steps:  {} derivative expressions computed ({} successful, {} failed)",
                trace.expression_count(),
                successful,
                trace.expression_count().saturating_sub(successful + 1)
            );

            let info = find_failure(input, r);
            if info.found == '\0' {
                println!(
                    "Error:  position {}: unexpected end of input, expected {}",
                    info.position, info.expected
                );
            } else {
                println!(
                    "Error:  position {}: found '{}', expected {}",
                    info.position, info.found, info.expected
                );
            }
            println!("{}", caret_lines(input, info.position));

            if let Some(partial) = partial_tree_standard(
                &trace.expressions, &chars, trace.last_nullable_idx
            ) {
                let flat = flatten(&partial);
                if !flat.is_empty() {
                    println!();
                    println!(
                        "Partial match: {:?}  (positions 1–{})",
                        flat, trace.last_nullable_idx.unwrap_or(0)
                    );
                    println!("Partial tree:  {}  (recovered from last nullable derivative)", partial);
                }
            }
        }
    }
}


// -------------------------------
// Bitcoded Path
// -------------------------------

fn run_parser_bitcoded(regex_str: &str, r: &Regex, input: &str) {
    let start = Instant::now();
    let (result, trace) = parse_bitcoded_traced(input, r);
    let elapsed = start.elapsed();

    println!("Regex:  {}", regex_str);
    println!("Input:  {:?}", input);

    match &result {
        Some(tree) => {
            println!("Match:  true");
            println!("Tree:   {}", tree);
            println!("Time:   {:.3}ms", elapsed.as_secs_f64() * 1000.0);
            println!("Steps:  {} derivative steps computed", trace.bit_steps.len());
            println!();
            println!("Bit construction:");
            println!("  internalize({}) → {}", regex_str, trace.internalized);

            for step in &trace.bit_steps {
                println!(
                    "  step {} ('{}'): deriv_bc + simp → {}",
                    step.position, step.character, step.after
                );
            }

            if let Some(ref bits) = trace.final_bits {
                let bits_str = bits.iter()
                    .map(|b| if *b { "1" } else { "0" })
                    .collect::<Vec<_>>()
                    .join(",");
                println!("  mkEpsBC → bits: [{}]", bits_str);
                println!("  decode  → {}", tree);
            }
        }
        None => {
            let successful = trace.successful_steps();
            println!("Match:  false");
            println!("Time:   {:.3}ms", elapsed.as_secs_f64() * 1000.0);
            println!(
                "Steps:  {} derivative steps computed ({} successful, 1 failed)",
                trace.bit_steps.len(), successful
            );

            let info = find_failure(input, r);
            if info.found == '\0' {
                println!(
                    "Error:  position {}: unexpected end of input, expected {}",
                    info.position, info.expected
                );
            } else {
                println!(
                    "Error:  position {}: found '{}', expected {}",
                    info.position, info.found, info.expected
                );
            }
            println!("{}", caret_lines(input, info.position));

            if let Some(ref bits) = trace.bits_at_last_nullable {
                let bits_str = bits.iter()
                    .map(|b| if *b { "1" } else { "0" })
                    .collect::<Vec<_>>()
                    .join(",");
                let partial_input: String = input.chars()
                    .take(trace.last_nullable_idx.unwrap_or(0))
                    .collect();

                println!();
                println!(
                    "Partial match: {:?}  (positions 1–{})",
                    partial_input, trace.last_nullable_idx.unwrap_or(0)
                );
                println!("Bits so far:   [{}]", bits_str);

                if let Some(idx) = trace.last_nullable_idx {
                    if idx > 0 {
                        let last_ri = &trace.bit_steps[idx - 1].after;
                        println!("Last nullable: {}  (after step {})", last_ri, idx);
                    }
                }
            }
        }
    }
}