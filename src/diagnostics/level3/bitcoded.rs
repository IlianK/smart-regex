//! Level 3 rendering for the bit-coded Brzozowski-derivative parser.

use std::time::Instant;

use crate::types::{Regex, ARegex, flatten};
use crate::parsers::bitcoded::parse_bitcoded_traced;
use crate::regex::nullable::annotated::nullable_bc;
use crate::diagnostics::format::bits_str;
use crate::diagnostics::report::ReportWriter;
use super::{timestamp, render_error_summary};

pub fn render(regex_str: &str, r: &Regex, input: &str, w: &mut ReportWriter) {
    let start = Instant::now();
    let (result, trace) = parse_bitcoded_traced(input, r);
    let elapsed = start.elapsed();

    let result_label = if result.is_some() { "MATCH" } else { "NO MATCH" };

    // Header
    w.separator();
    w.line("REGEX ENGINE DEBUG REPORT");
    w.separator();
    w.kv("Timestamp",        &timestamp());
    w.kv("Mode",             "POSIX Bitcoded");
    w.kv("Regex",            regex_str);
    w.kv("Input",            &format!("{:?}", input));
    w.kv("Result",           result_label);

    // Timing
    w.separator();
    w.line("TIMING");
    w.separator();
    w.kv("Parse time",       &format!("{:.3}ms", elapsed.as_secs_f64() * 1000.0));
    w.kv("Derivative steps", &format!("{}", trace.bit_steps.len()));

    // Internalize
    w.separator();
    w.line("INTERNALIZE");
    w.separator();
    w.line(&format!("internalize({})", regex_str));
    render_internalize_expansion(r, w);
    w.line(&format!("ri0 = {}", trace.internalized));

    // Forward pass
    w.separator();
    w.line("FORWARD PASS (deriv_bc + simp)");
    w.separator();

    for step in &trace.bit_steps {
        w.line(&format!(
            "Step {} (char: '{}', position {})",
            step.position, step.character, step.position
        ));
        w.line(&format!("  ri{} = {}", step.position - 1, step.before));
        w.line(&format!("  deriv_bc(ri{}, '{}'):", step.position - 1, step.character));
        render_deriv_bc_expansion(&step.before, step.character, w);
        w.line(&format!("  simp → {}", step.after));
        w.line(&format!("  ri{} = {}", step.position, step.after));
        if step.nullable {
            w.line(&format!("  nullable_bc(ri{}) = true  ✓  bits accumulating", step.position));
        } else {
            w.line(&format!("  nullable_bc(ri{}) = false  ✗", step.position));
        }
        w.blank();
    }

    // Nullability check
    w.separator();
    w.line("NULLABILITY CHECK");
    w.separator();
    let final_step = trace.bit_steps.len();
    w.line(&format!(
        "nullable_bc(ri{}) = {} → {}",
        final_step,
        result.is_some(),
        if result.is_some() { "proceed to mkEpsBC + decode" } else { "no full parse tree exists" }
    ));

    // MkEpsBC + decode
    if let Some(ref tree) = result {
        w.separator();
        w.line("MKEEPSBC + DECODE");
        w.separator();
        if let Some(ref bits) = trace.final_bits {
            w.line(&format!("mkEpsBC(ri{}) = {}", final_step, bits_str(bits)));
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

            if last_idx > 0 {
                let last_ri = &trace.bit_steps[last_idx - 1].after;
                w.line(&format!(
                    "Last nullable expression: ri{} = {}  (after position {})",
                    last_idx, last_ri, last_idx
                ));
                w.line(&format!("Accumulated bits: {}", bits_str(bits)));
                w.blank();
                w.line(&format!("mkEpsBC(ri{}) would give: {}", last_idx, bits_str(bits)));
                // Cannot decode partial bits - final expression is not nullable,
                // so no valid complete parse tree exists for the accumulated bits.
                w.line("Decoding not possible: final expression is not nullable.");
            }
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

fn render_deriv_bc_expansion(ri: &ARegex, c: char, w: &mut ReportWriter) {
    use ARegex::*;
    match ri {
        Phi    => w.line("    rule: Phi \\ c = Phi"),
        Eps(_) => w.line("    rule: (bs@ε) \\ c = Phi  [Eps has no derivative]"),
        Lit(_, ch) => {
            if *ch == c {
                w.line(&format!(
                    "    rule: (bs@'{}') \\ '{}' = Eps(bs)  [literal match, bits preserved]",
                    ch, c
                ));
            } else {
                w.line(&format!(
                    "    rule: (bs@'{}') \\ '{}' = Phi  [literal mismatch]",
                    ch, c
                ));
            }
        }
        Alt(_, _r1, _r2) => {
            w.line(&format!(
                "    rule: (bs@(ri1 ⊕ ri2)) \\ '{}' = bs@(ri1\\{0} ⊕ ri2\\{0})", c
            ));
        }
        Seq(_, r1, _r2) => {
            if nullable_bc(r1) {
                w.line(&format!(
                    "    rule: (bs@ri1·ri2) \\ '{}'  [nullable_bc(ri1)=true]", c
                ));
                w.line(&format!(
                    "         = bs@((ri1\\'{}'·ri2) ⊕ (fuse (mkEpsBC ri1) (ri2\\'{}')))",
                    c, c
                ));
            } else {
                w.line(&format!(
                    "    rule: (bs@ri1·ri2) \\ '{}'  [nullable_bc(ri1)=false]", c
                ));
                w.line(&format!(
                    "         = bs@(ri1\\'{}'·ri2)", c
                ));
            }
        }
        Star(_, _r1) => {
            w.line(&format!(
                "    rule: (bs@ri*) \\ '{}' = bs@(fuse [0] (ri\\'{}') · ([]@ri*))", c, c
            ));
        }
    }
}

fn render_internalize_expansion(r: &Regex, w: &mut ReportWriter) {
    use Regex::*;
    match r {
        Phi         => w.line("  rule: internalize(∅) = ∅"),
        Eps         => w.line("  rule: internalize(ε) = []@ε"),
        Lit(c)      => w.line(&format!("  rule: internalize('{}') = []@'{}'", c, c)),
        Alt(r1, r2) => {
            w.line("  rule: internalize(r1 + r2)");
            w.line("       = []@(fuse [0] (internalize r1) ⊕ fuse [1] (internalize r2))");
            w.line(&format!("  left:  fuse [0] (internalize {:?}) → [false]@...", r1));
            w.line(&format!("  right: fuse [1] (internalize {:?}) → [true]@...", r2));
        }
        Seq(_r1, _r2) => {
            w.line("  rule: internalize(r1 · r2) = []@(internalize r1)(internalize r2)");
        }
        Star(r1) => {
            w.line("  rule: internalize(r*) = []@(internalize r)*");
            w.line(&format!("  inner: internalize({:?}) → []@...", r1));
        }
    }
}
