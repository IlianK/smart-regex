//! Level 3 rendering for the standard (Brzozowski-derivative, inj-based) parser.

use std::time::Instant;

use crate::types::{Regex, flatten};
use crate::parsers::selection::ParserType;
use crate::parsers::standard::{parse_loop_traced, parse_recursive_traced};
use crate::regex::nullable::standard::nullable;
use crate::diagnostics::DiagConfig;
use crate::diagnostics::replay::partial_tree_standard;
use crate::diagnostics::report::ReportWriter;
use super::{timestamp, render_error_summary};

pub fn render(
    regex_str: &str,
    r: &Regex,
    input: &str,
    config: &DiagConfig,
    w: &mut ReportWriter,
) {
    let chars: Vec<char> = input.chars().collect();
    let start = Instant::now();

    let (result, trace) = match config.parser_type {
        ParserType::DerivRec => parse_recursive_traced(input, r),
        _                    => parse_loop_traced(input, r),
    };

    let elapsed = start.elapsed();

    let parser_label = match config.parser_type {
        ParserType::DerivRec  => "POSIX Standard (recursive parser)",
        ParserType::DerivLoop => "POSIX Standard (loop parser)",
        _                     => unreachable!(),
    };

    let result_label = if result.is_some() { "MATCH" } else { "NO MATCH" };

    // Header
    w.separator();
    w.line("REGEX ENGINE DEBUG REPORT");
    w.separator();
    w.kv("Timestamp", &timestamp());
    w.kv("Mode",      parser_label);
    w.kv("Regex",     regex_str);
    w.kv("Input",     &format!("{:?}", input));
    w.kv("Result",    result_label);

    // Timing
    w.separator();
    w.line("TIMING");
    w.separator();
    w.kv("Parse time",         &format!("{:.3}ms", elapsed.as_secs_f64() * 1000.0));
    w.kv("Expressions stored", &format!(
        "{} ({})",
        trace.expression_count(),
        expression_range_label(trace.expression_count())
    ));

    // Forward pass
    w.separator();
    w.line("FORWARD PASS (Derivatives)");
    w.separator();
    w.line(&format!("r0 = {:?}", trace.expressions[0]));
    w.blank();

    for step in &trace.deriv_steps {
        w.line(&format!(
            "Step {}: ∂(r{}, '{}')",
            step.position, step.position - 1, step.character
        ));
        render_deriv_expansion(&step.before, step.character, w);
        w.line(&format!("r{} = {:?}", step.position, step.after));
        if step.nullable {
            w.line(&format!(
                "  nullable(r{}) = true  ✓ position {} matched",
                step.position, step.position
            ));
        } else {
            w.line(&format!(
                "  nullable(r{}) = false  ✗ position {}",
                step.position, step.position
            ));
        }
        w.blank();
    }

    // Nullability check
    w.separator();
    w.line("NULLABILITY CHECK");
    w.separator();
    let final_idx = trace.expression_count() - 1;
    let final_nullable = result.is_some();
    w.line(&format!(
        "nullable(r{}) = {} → {}",
        final_idx,
        final_nullable,
        if final_nullable { "proceed to backward pass" } else { "no parse tree exists for full input" }
    ));

    if let Some(ref tree) = result {
        // Backward pass
        w.separator();
        w.line("BACKWARD PASS (mkEps + inject)");
        w.separator();

        if let Some(ref mke) = trace.mk_eps_result {
            w.line(&format!("mkEps(r{}):", final_idx));
            render_mk_eps_expansion(&mke.regex, w);
            w.line(&format!("v{} = {}", final_idx, mke.tree));
            w.blank();
        }

        if let Some(ref steps) = trace.inject_steps {
            // Steps stored in forward order (pos 1 first); display backward (pos n first)
            for step in steps.iter().rev() {
                let expr_idx = step.position - 1;
                w.line(&format!(
                    "inject(r{}, '{}', v{}):",
                    expr_idx, step.character, expr_idx + 1
                ));
                w.line(&format!(
                    "  inject({:?}, '{}', {})",
                    trace.expressions[expr_idx], step.character, step.before
                ));
                w.line(&format!("  → {}", step.after));
                w.line(&format!("v{} = {}", expr_idx, step.after));
                w.blank();
            }
        }

        // Result
        w.separator();
        w.line("RESULT");
        w.separator();
        w.kv("Parse tree", &format!("{}", tree));
        w.kv("Flattened",  &format!("{:?}", flatten(tree)));

    } else {
        // Partial recovery
        w.separator();
        w.line("PARTIAL RECOVERY");
        w.separator();

        if let Some(partial) = partial_tree_standard(
            &trace.expressions, &chars, trace.last_nullable_idx
        ) {
            let flat = flatten(&partial);
            let last_idx = trace.last_nullable_idx.unwrap_or(0);
            w.line(&format!(
                "Last nullable derivative: r{} = {:?}  (after position {})",
                last_idx, trace.expressions[last_idx], last_idx
            ));
            w.line(&format!("mkEps(r{}) → {}", last_idx, partial));
            w.blank();

            if last_idx > 0 {
                w.line(&format!("Partial backward pass (positions 1–{} only):", last_idx));
                if let Some(ref steps) = trace.inject_steps {
                    for step in steps.iter().rev().take(last_idx) {
                        w.line(&format!(
                            "  inject(r{}, '{}', ...) → {}",
                            step.position - 1, step.character, step.after
                        ));
                    }
                }
                w.blank();
            }

            w.kv("Partial parse tree", &format!("{}", partial));
            w.kv("Partial match",      &format!("{:?}", flat));
        } else {
            w.line("No prefix matched.");
        }

        render_error_summary(w, input, r);
    }

    w.separator();
    w.line("END OF REPORT");
    w.separator();
}

fn render_deriv_expansion(r: &Regex, c: char, w: &mut ReportWriter) {
    use Regex::*;
    match r {
        Phi => w.line("  rule: ∂(∅, c) = ∅"),
        Eps => w.line("  rule: ∂(ε, c) = ∅"),
        Lit(ch) => {
            if *ch == c {
                w.line(&format!("  rule: ∂('{0}', '{0}') = ε  [literal match]", c));
            } else {
                w.line(&format!("  rule: ∂('{}', '{}') = ∅  [literal mismatch]", ch, c));
            }
        }
        Alt(r1, r2) => {
            w.line(&format!(
                "  rule: ∂(r1 + r2, '{}') = ∂(r1, '{}') + ∂(r2, '{}')", c, c, c
            ));
            w.line(&format!("  ∂({:?}, '{}') - see left branch", r1, c));
            w.line(&format!("  ∂({:?}, '{}') - see right branch", r2, c));
        }
        Seq(r1, _r2) => {
            if nullable(r1) {
                w.line(&format!(
                    "  rule: ∂(r1·r2, '{}')  [nullable(r1)=true]", c
                ));
                w.line(&format!(
                    "       = ∂(r1, '{}')·r2  +  ∂(r2, '{}')", c, c
                ));
            } else {
                w.line(&format!(
                    "  rule: ∂(r1·r2, '{}')  [nullable(r1)=false]", c
                ));
                w.line(&format!(
                    "       = ∂(r1, '{}')·r2", c
                ));
            }
        }
        Star(r1) => {
            w.line(&format!(
                "  rule: ∂(r*, '{}') = ∂(r, '{}') · r*", c, c
            ));
            w.line(&format!("  ∂({:?}, '{}'):", r1, c));
            render_deriv_expansion(r1, c, w);
        }
    }
}

fn render_mk_eps_expansion(r: &Regex, w: &mut ReportWriter) {
    use Regex::*;
    match r {
        Eps        => w.line("  mkEps(ε) → ()"),
        Star(_)    => w.line("  mkEps(r*) → []  [zero iterations]"),
        Alt(r1, _) => {
            if nullable(r1) {
                w.line("  mkEps(r1 + r2): nullable(r1)=true → Left(mkEps(r1))");
            } else {
                w.line("  mkEps(r1 + r2): nullable(r1)=false → Right(mkEps(r2))");
            }
        }
        Seq(_, _)  => w.line("  mkEps(r1 · r2) → Pair(mkEps(r1), mkEps(r2))"),
        Lit(c)     => w.line(&format!("  mkEps('{}') → panic (non-nullable)", c)),
        Phi        => w.line("  mkEps(∅) → panic (non-nullable)"),
    }
}

fn expression_range_label(count: usize) -> String {
    if count == 0 { return "none".to_string(); }
    let labels: Vec<String> = (0..count).map(|i| format!("r{}", i)).collect();
    labels.join(" → ")
}
