//! Level 2 rendering for the bit-coded partial-derivative parser (pDerivBC).

use std::time::Instant;

use crate::types::Regex;
use crate::parsers::bitcoded::pderiv::parse_pderiv_bc_traced;
use crate::diagnostics::replay::error_report;
use crate::diagnostics::format::bits_str;

fn frontier_str(frontier: &[(Regex, Vec<bool>)]) -> String {
    frontier
        .iter()
        .map(|(r, bs)| format!("({:?}, {})", r, bits_str(bs)))
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn run(regex_str: &str, r: &Regex, input: &str) {
    let start = Instant::now();
    let (result, trace) = parse_pderiv_bc_traced(input, r);
    let elapsed = start.elapsed();

    println!("Regex:  {}", regex_str);
    println!("Input:  {:?}", input);
    println!("Note:   pDerivBC computes GREEDY leftmost priority, not POSIX,");
    println!("        on inputs with an A1-relevant ambiguity (see docs)");

    match &result {
        Some(tree) => {
            println!("Match:  true");
            println!("Tree:   {}", tree);
            println!("Time:   {:.3}ms", elapsed.as_secs_f64() * 1000.0);
            println!(
                "Steps:  {} pDerivBC steps computed (frontier size at each step: {})",
                trace.steps.len(),
                trace.steps.iter().map(|s| s.after.len().to_string())
                    .collect::<Vec<_>>().join(" -> ")
            );
            println!();
            println!("Frontier construction:");
            println!("  step 0: {}", frontier_str(&trace.initial));

            for step in &trace.steps {
                println!(
                    "  step {} ('{}'): {}",
                    step.position, step.character, frontier_str(&step.after)
                );
            }

            if let Some(ref bits) = trace.final_bits {
                println!("  selected (first nullable, priority order) -> bits: {}", bits_str(bits));
                println!("  decode  -> {}", tree);
            }
        }
        None => {
            let successful = trace.successful_steps();
            println!("Match:  false");
            println!("Time:   {:.3}ms", elapsed.as_secs_f64() * 1000.0);
            println!(
                "Steps:  {} pDerivBC steps computed ({} with a nullable residual)",
                trace.steps.len(), successful
            );

            println!("{}", error_report(input, r));

            if let Some(ref bits) = trace.bits_at_last_nullable {
                let partial_input: String = input.chars()
                    .take(trace.last_nullable_idx.unwrap_or(0))
                    .collect();

                println!();
                println!(
                    "Partial match: {:?}  (positions 1–{})",
                    partial_input, trace.last_nullable_idx.unwrap_or(0)
                );
                println!("Bits so far:   {}", bits_str(bits));
            }
        }
    }
}
