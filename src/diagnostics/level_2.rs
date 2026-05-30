//! Level 2 — Verbose diagnostics output.
//!
//! Standard success:
//!   Regex:  a*  |  Input:  "aa"  |  Match:  true  |  Tree:   [a, a]
//!   Time:   0.08ms  |  Steps:  3 derivative expressions computed
//!   Construction steps:
//!     mkEps(a*) → []
//!     inject(a*, 'a', []) → [a]
//!     inject(a*, 'a', [a]) → [a, a]
//!
//! Standard failure:
//!   + partial match, bits-so-far (bitcoded)

use std::time::Instant;

use crate::types::{Regex, flatten};
use crate::posix::bitcoded::parse_bitcoded_traced;
use crate::posix::standard::parse_loop_traced;
use crate::diagnostics::DiagConfig;
use crate::diagnostics::replay::{find_failure, caret_lines, partial_tree_standard};

// ============================================================================
// Parser — Level 2
// ============================================================================

pub fn run_parser(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    if config.is_bitcoded() {
        run_parser_bitcoded(regex_str, r, input);
    } else {
        run_parser_standard(regex_str, r, input, config);
    }
}

// ── Standard path ────────────────────────────────────────────────────────────

fn run_parser_standard(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    // Use parse_loop_traced regardless of recursive/loop setting at Level 2
    // (parse_recursive does not store the expression sequence)
    let start = Instant::now();
    let (result, trace) = parse_loop_traced(input, r);
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

            // mkEps line
            if let Some(ref mke) = trace.mk_eps_result {
                println!("  mkEps({}) → {}", regex_str, mke.tree);
            }

            // inject lines (forward order: position 1 first)
            if let Some(ref steps) = trace.inject_steps {
                for step in steps {
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

            // Partial match recovery
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

// ── Bitcoded path ────────────────────────────────────────────────────────────

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
                let bits_str: String = bits.iter()
                    .map(|b| if *b { '1' } else { '0' })
                    .collect();
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

            // Partial info from last nullable step
            if let Some(ref bits) = trace.bits_at_last_nullable {
                let bits_str: String = bits.iter()
                    .map(|b| if *b { '1' } else { '0' })
                    .collect();
                let partial_input: String = input.chars()
                    .take(trace.last_nullable_idx.unwrap_or(0))
                    .collect();

                println!();
                println!(
                    "Partial match: {:?}  (positions 1–{})",
                    partial_input, trace.last_nullable_idx.unwrap_or(0)
                );
                println!("Bits so far:   [{}]", bits_str);

                // Decode what we can from the last nullable expression
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