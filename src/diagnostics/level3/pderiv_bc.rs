//! Level 3 rendering for the bit-coded partial-derivative parser (pDerivBC).

use std::time::Instant;

use crate::types::{Regex, flatten};
use crate::parsers::bitcoded::pderiv::parse_pderiv_bc_traced;
use crate::regex::nullable::standard::nullable;
use crate::regex::pderiv::annotated::pderiv_bc;
use crate::diagnostics::format::bits_str;
use crate::diagnostics::report::ReportWriter;
use super::{timestamp, render_error_summary};

pub fn render(regex_str: &str, r: &Regex, input: &str, w: &mut ReportWriter) {
    let start = Instant::now();
    let (result, trace) = parse_pderiv_bc_traced(input, r);
    let elapsed = start.elapsed();

    let result_label = if result.is_some() { "MATCH" } else { "NO MATCH" };

    // Header
    w.separator();
    w.line("REGEX ENGINE DEBUG REPORT");
    w.separator();
    w.kv("Timestamp", &timestamp());
    w.kv("Mode",      "Bitcoded Partial-Derivative (pDerivBC)");
    w.kv("Regex",     regex_str);
    w.kv("Input",     &format!("{:?}", input));
    w.kv("Result",    result_label);
    w.blank();
    w.line("NOTE: pDerivBC computes GREEDY leftmost priority, not POSIX");
    w.line("leftmost-longest, on inputs with an A1-relevant ambiguity --");
    w.line("this is a property of the reference construction itself, not");
    w.line("a bug (see regex::pderiv::annotated's module doc comment).");

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
    w.line(&format!("[(r0, [])]  where r0 = {:?}", r));
    if nullable(r) {
        w.line("nullable(r0) = true  -- empty input would already match");
    }

    // Forward pass
    w.separator();
    w.line("FORWARD PASS (pDerivBC per residual, frontier flattened)");
    w.separator();

    for step in &trace.steps {
        w.line(&format!(
            "Step {} (char: '{}', position {})",
            step.position, step.character, step.position
        ));
        w.line(&format!("  frontier before ({} residual(s)):", step.before.len()));
        for (i, (r_i, bits)) in step.before.iter().enumerate() {
            w.line(&format!("    [{}] {:?}   bits-so-far = {}", i, r_i, bits_str(bits)));
        }
        w.line(&format!("  pDerivBC('{}', ·) applied to each:", step.character));
        for (i, (r_i, _)) in step.before.iter().enumerate() {
            w.line(&format!("    from [{}]:", i));
            render_pderiv_bc_expansion(r_i, step.character, w);
        }
        w.line(&format!("  frontier after ({} residual(s)):", step.after.len()));
        for (i, (r_i, bits)) in step.after.iter().enumerate() {
            w.line(&format!("    [{}] {:?}   bits = {}", i, r_i, bits_str(bits)));
        }
        if step.nullable {
            w.line("  a residual in this frontier is nullable  ✓  bits accumulating");
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
        if result.is_some() { "proceed to mkEpsBC + decode" } else { "no full parse tree exists" }
    ));

    if let Some(ref tree) = result {
        w.separator();
        w.line("MKEEPSBC + DECODE");
        w.separator();
        if let Some(ref bits) = trace.final_bits {
            w.line(&format!(
                "selected residual's accumulated bits ++ mkEpsBC(residual) = {}",
                bits_str(bits)
            ));
            w.blank();
            w.line(&format!("decode({}, {}):", regex_str, bits_str(bits)));
            w.line(&format!("  = {}", tree));
        }

        w.separator();
        w.line("RESULT");
        w.separator();
        if let Some(ref bits) = trace.final_bits {
            w.kv("Bits", &bits_str(bits));
        }
        w.kv("Parse tree", &format!("{}", tree));
        w.kv("Flattened",  &format!("{:?}", flatten(tree)));

    } else {
        // Partial recovery
        w.separator();
        w.line("PARTIAL RECOVERY");
        w.separator();

        if let Some(ref bits) = trace.bits_at_last_nullable {
            let last_idx = trace.last_nullable_idx.unwrap_or(0);
            let partial_str: String = input.chars().take(last_idx).collect();

            w.line(&format!(
                "Last nullable frontier: after position {}", last_idx
            ));
            w.line(&format!("Accumulated bits: {}", bits_str(bits)));
            w.blank();
            w.kv("Partial match", &format!("{:?}  (positions 1–{})", partial_str, last_idx));
        } else {
            w.line("No prefix matched.");
        }

        render_error_summary(w, input, r);
    }

    w.separator();
    w.line("END OF REPORT");
    w.separator();
}

fn render_pderiv_bc_expansion(r: &Regex, c: char, w: &mut ReportWriter) {
    use Regex::*;
    match r {
        Phi | Eps => w.line("      rule: pDerivBC(c, φ|ε) = []"),
        Lit(ch) => {
            if *ch == c {
                w.line(&format!(
                    "      rule: pDerivBC('{0}', '{0}') = [(ε, [])]", c
                ));
            } else {
                w.line(&format!(
                    "      rule: pDerivBC('{}', '{}') = []  [mismatch]", ch, c
                ));
            }
        }
        Alt(_, _) => {
            w.line(&format!(
                "      rule: pDerivBC('{0}', r1+r2) = tag(0, pDerivBC('{0}',r1)) ++ tag(1, pDerivBC('{0}',r2)), nub2",
                c
            ));
        }
        Seq(r1, _) => {
            if nullable(r1) {
                w.line(&format!(
                    "      rule: pDerivBC('{0}', r1·r2)  [nullable(r1)=true]", c
                ));
                w.line("           = nub2 $ [(r1'·r2, bs) | (r1',bs) <- pDerivBC(c,r1)]");
                w.line("                  ++ [(r2', mkEpsBC(r1) ++ bs) | (r2',bs) <- pDerivBC(c,r2)]");
            } else {
                w.line(&format!(
                    "      rule: pDerivBC('{0}', r1·r2)  [nullable(r1)=false]", c
                ));
                w.line("           = [(smartC(r1',r2), bs) | (r1',bs) <- pDerivBC(c,r1)]");
            }
        }
        Star(_) => {
            w.line(&format!(
                "      rule: pDerivBC('{0}', r*) = nub2 $ [(smartC(r',r*), 0:bs) | (r',bs) <- pDerivBC('{0}',r)]",
                c
            ));
        }
    }
    let results = pderiv_bc(r, c);
    if results.is_empty() {
        w.line("      -> []");
    } else {
        for (r_, bits) in &results {
            w.line(&format!("      -> ({:?}, {})", r_, bits_str(bits)));
        }
    }
}
