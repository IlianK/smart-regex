//! Level 2 rendering for the standard (Brzozowski-derivative, inj-based) parser.

use std::time::Instant;

use crate::types::{Regex, flatten};
use crate::parsers::selection::ParserType;
use crate::parsers::standard::{parse_loop_traced, parse_recursive_traced};
use crate::diagnostics::DiagConfig;
use crate::diagnostics::replay::{error_report, partial_tree_standard};

pub fn run(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    let start = Instant::now();

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

            println!("{}", error_report(input, r));

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
