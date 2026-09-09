//! Level 3 rendering for pderiv_std; mirrors pderiv_bc's but the trace has no bits, residuals only.

use std::time::Instant;

use crate::types::{Regex, flatten};
use crate::parsers::pderiv_std::{parse_pderiv_std_traced, pderiv_tree};
use crate::regex::nullable::nullable;
use crate::diagnostics::report::ReportWriter;
use super::{timestamp, render_error_summary};

pub fn render(regex_str: &str, r: &Regex, input: &str, w: &mut ReportWriter) {
    let start = Instant::now();
    let (result, trace) = parse_pderiv_std_traced(input, r);
    let elapsed = start.elapsed();

    let result_label = if result.is_some() { "MATCH" } else { "NO MATCH" };

    // Header
    w.separator();
    w.line("REGEX ENGINE DEBUG REPORT");
    w.separator();
    w.kv("Timestamp", &timestamp());
    w.kv("Mode",      "Standard Partial-Derivative (pderiv_std)");
    w.kv("Regex",     regex_str);
    w.kv("Input",     &format!("{:?}", input));
    w.kv("Result",    result_label);
    w.kv("Policy",    "GREEDY");

    // Timing
    w.separator();
    w.line("TIMING");
    w.separator();
    w.kv("Parse time",  &format!("{:.3}ms", elapsed.as_secs_f64() * 1000.0));
    w.kv("Steps",       &format!("{}", trace.steps.len()));

    // Initial frontier
    w.separator();
    w.line("INITIAL FRONTIER");
    w.separator();
    w.line(&format!("[r0]  where r0 = {:?}", r));
    if nullable(r) {
        w.line("nullable(r0) = true  -- empty input would already match");
    }

    // Forward pass
    w.separator();
    w.line("FORWARD PASS (pderiv_tree per residual, frontier flattened)");
    w.separator();

    for step in &trace.steps {
        w.line(&format!(
            "Step {} (char: '{}', position {})",
            step.position, step.character, step.position
        ));
        w.line(&format!("  frontier before ({} residual(s)):", step.before.len()));
        for (i, r_i) in step.before.iter().enumerate() {
            w.line(&format!("    [{}] {:?}", i, r_i));
        }
        w.line(&format!("  pderiv_tree('{}', ·) applied to each:", step.character));
        for (i, r_i) in step.before.iter().enumerate() {
            w.line(&format!("    from [{}]:", i));
            render_pderiv_tree_expansion(r_i, step.character, w);
        }
        w.line(&format!("  frontier after ({} residual(s)):", step.after.len()));
        for (i, r_i) in step.after.iter().enumerate() {
            w.line(&format!("    [{}] {:?}", i, r_i));
        }
        if step.nullable {
            w.line("  a residual in this frontier is nullable  ✓  parse tree available");
        } else {
            w.line("  no nullable residual in this frontier  ✗");
        }
        w.blank();
    }

    // Nullability check / selection
    w.separator();
    w.line("SELECTION (first nullable residual, priority/list order)");
    w.separator();
    w.line(&format!(
        "any nullable residual in final frontier = {} -> {}",
        result.is_some(),
        if result.is_some() { "proceed to injection (already applied by select)" } else { "no full parse tree exists" }
    ));

    if let Some(ref tree) = result {
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

        if let Some(ref tree) = trace.tree_at_last_nullable {
            let last_idx = trace.last_nullable_idx.unwrap_or(0);
            let partial_str: String = input.chars().take(last_idx).collect();

            w.line(&format!(
                "Last nullable frontier: after position {}", last_idx
            ));
            w.blank();
            w.kv("Partial match", &format!("{:?}  (positions 1–{})", partial_str, last_idx));
            w.kv("Partial tree",  &format!("{}", tree));
        } else {
            w.line("No prefix matched.");
        }

        render_error_summary(w, input, r);
    }

    w.separator();
    w.line("END OF REPORT");
    w.separator();
}

fn render_pderiv_tree_expansion(r: &Regex, c: char, w: &mut ReportWriter) {
    use Regex::*;
    match r {
        Phi | Eps => w.line("      rule: pderiv_tree(c, φ|ε) = []"),
        Lit(ch) => {
            if *ch == c {
                w.line(&format!(
                    "      rule: pderiv_tree('{0}', '{0}') = [(ε, inj: _ -> Char('{0}'))]", c
                ));
            } else {
                w.line(&format!(
                    "      rule: pderiv_tree('{}', '{}') = []  [mismatch]", ch, c
                ));
            }
        }
        Alt(_, _) => {
            w.line(&format!(
                "      rule: pderiv_tree('{0}', r1+r2) = tag(Left, pderiv_tree('{0}',r1)) ++ tag(Right, pderiv_tree('{0}',r2)), nub_tree",
                c
            ));
        }
        Seq(r1, _) => {
            if nullable(r1) {
                w.line(&format!(
                    "      rule: pderiv_tree('{0}', r1·r2)  [nullable(r1)=true]", c
                ));
                w.line("           = nub_tree $ [(r1'·r2, inj: continue r1) | (r1',_) <- pderiv_tree(c,r1)]");
                w.line("                       ++ [(r2', inj: mk_eps(r1) paired with r2's result) | (r2',_) <- pderiv_tree(c,r2)]");
            } else {
                w.line(&format!(
                    "      rule: pderiv_tree('{0}', r1·r2)  [nullable(r1)=false]", c
                ));
                w.line("           = [(r1'·r2, inj: continue r1) | (r1',_) <- pderiv_tree(c,r1)]");
            }
        }
        Star(_) => {
            w.line(&format!(
                "      rule: pderiv_tree('{0}', r*) = nub_tree $ [(r'·r*, inj: unroll one more iteration) | (r',_) <- pderiv_tree('{0}',r)]",
                c
            ));
        }
    }
    let results = pderiv_tree(r, c);
    if results.is_empty() {
        w.line("      -> []");
    } else {
        for (r_, _inj) in &results {
            w.line(&format!("      -> {:?}  (+ injection closure, not printable)", r_));
        }
    }
}
