//! Level 2 rendering for pderiv_std; mirrors pderiv_bc's but the trace has no bits, residuals only.

use std::time::Instant;

use crate::types::Regex;
use crate::parsers::pderiv_std::parse_pderiv_std_traced;
use crate::diagnostics::replay::error_report;

fn frontier_str(frontier: &[Regex]) -> String {
    frontier
        .iter()
        .map(|r| format!("{:?}", r))
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn run(regex_str: &str, r: &Regex, input: &str) {
    let start = Instant::now();
    let (result, trace) = parse_pderiv_std_traced(input, r);
    let elapsed = start.elapsed();

    println!("Regex:  {}", regex_str);
    println!("Input:  {:?}", input);
    println!("Policy: GREEDY");

    match &result {
        Some(tree) => {
            println!("Match:  true");
            println!("Tree:   {}", tree);
            println!("Time:   {:.3}ms", elapsed.as_secs_f64() * 1000.0);
            println!(
                "Steps:  {} pderiv_std steps computed (frontier size at each step: {})",
                trace.steps.len(),
                trace.steps.iter().map(|s| s.after.len().to_string())
                    .collect::<Vec<_>>().join(" -> ")
            );
            println!();
            println!("Frontier construction (residuals only -- injections aren't printable):");
            println!("  step 0: {}", frontier_str(&trace.initial));

            for step in &trace.steps {
                println!(
                    "  step {} ('{}'): {}",
                    step.position, step.character, frontier_str(&step.after)
                );
            }

            println!("  selected (first nullable, priority order) -> {}", tree);
        }
        None => {
            let successful = trace.successful_steps();
            println!("Match:  false");
            println!("Time:   {:.3}ms", elapsed.as_secs_f64() * 1000.0);
            println!(
                "Steps:  {} pderiv_std steps computed ({} with a nullable residual)",
                trace.steps.len(), successful
            );

            println!("{}", error_report(input, r));

            if let Some(ref tree) = trace.tree_at_last_nullable {
                let partial_input: String = input.chars()
                    .take(trace.last_nullable_idx.unwrap_or(0))
                    .collect();

                println!();
                println!(
                    "Partial match: {:?}  (positions 1–{})",
                    partial_input, trace.last_nullable_idx.unwrap_or(0)
                );
                println!("Tree so far:   {}", tree);
            }
        }
    }
}
