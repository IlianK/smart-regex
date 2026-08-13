//! Level 2 rendering for the bit-coded Brzozowski-derivative parser.

use std::time::Instant;

use crate::types::Regex;
use crate::parsers::bitcoded::parse_bitcoded_traced;
use crate::diagnostics::replay::error_report;
use crate::diagnostics::format::bits_str;

pub fn run(regex_str: &str, r: &Regex, input: &str) {
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
                println!("  mkEpsBC → bits: {}", bits_str(bits));
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
