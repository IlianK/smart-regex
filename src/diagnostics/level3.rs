//! Level 3 — Debug diagnostics. Full structural derivation trace.
//!
//! Output goes to REGEX_DIAG_REPORT file if set, otherwise stdout.
//! Standard mode uses the parser selected by REGEX_PARSER (recursive or loop).

use std::time::Instant;

use crate::types::{Regex, ARegex, flatten};
use crate::posix::selection::ParserType;
use crate::posix::standard::{parse_loop_traced, parse_recursive_traced};
use crate::posix::bitcoded::parse_bitcoded_traced;
use crate::regex::nullable::standard::nullable;
use crate::regex::nullable::annotated::nullable_bc;
use crate::diagnostics::DiagConfig;
use crate::diagnostics::replay::{find_failure, caret_lines, partial_tree_standard};
use crate::diagnostics::report::ReportWriter;

// ============================================================================
// Entry point
// ============================================================================

pub fn run_parser(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    let mut w = ReportWriter::new(config.report_path.as_deref());

    if config.is_bitcoded() {
        render_bitcoded(regex_str, r, input, &mut w);
    } else {
        render_standard(regex_str, r, input, config, &mut w);
    }

    w.flush();
}

// ============================================================================
// Standard trace render
// ============================================================================

fn render_standard(
    regex_str: &str,
    r: &Regex,
    input: &str,
    config: &DiagConfig,
    w: &mut ReportWriter,
) {
    let chars: Vec<char> = input.chars().collect();
    let start = Instant::now();

    // Respect REGEX_PARSER — use the traced variant matching the selected parser
    let (result, trace) = match config.parser_type {
        ParserType::Recursive => parse_recursive_traced(input, r),
        _                     => parse_loop_traced(input, r),
    };

    let elapsed = start.elapsed();

    let parser_label = match config.parser_type {
        ParserType::Recursive => "POSIX Standard (recursive parser)",
        ParserType::Loop      => "POSIX Standard (loop parser)",
        ParserType::Bitcoded  => unreachable!(),
    };

    let result_label = if result.is_some() { "MATCH" } else { "NO MATCH" };

    // ── Header ───────────────────────────────────────────────────────────────
    w.separator();
    w.line("REGEX ENGINE DEBUG REPORT");
    w.separator();
    w.kv("Timestamp", &timestamp());
    w.kv("Mode",      parser_label);
    w.kv("Regex",     regex_str);
    w.kv("Input",     &format!("{:?}", input));
    w.kv("Result",    result_label);

    // ── Timing ───────────────────────────────────────────────────────────────
    w.separator();
    w.line("TIMING");
    w.separator();
    w.kv("Parse time",         &format!("{:.3}ms", elapsed.as_secs_f64() * 1000.0));
    w.kv("Expressions stored", &format!(
        "{} ({})",
        trace.expression_count(),
        expression_range_label(trace.expression_count())
    ));

    // ── Forward pass ─────────────────────────────────────────────────────────
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

    // ── Nullability check ────────────────────────────────────────────────────
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
        // ── Backward pass ────────────────────────────────────────────────────
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

        // ── Result ───────────────────────────────────────────────────────────
        w.separator();
        w.line("RESULT");
        w.separator();
        w.kv("Parse tree", &format!("{}", tree));
        w.kv("Flattened",  &format!("{:?}", flatten(tree)));

    } else {
        // ── Partial recovery ─────────────────────────────────────────────────
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

        // ── Error summary ────────────────────────────────────────────────────
        w.separator();
        w.line("ERROR SUMMARY");
        w.separator();
        let info = find_failure(input, r);
        w.kv("Failure at position", &format!("{} (1-indexed)", info.position));
        if info.found == '\0' {
            w.kv("Found",    "end of input");
        } else {
            w.kv("Found",    &format!("'{}'", info.found));
        }
        w.kv("Expected", &info.expected);
        // caret_lines already produces "  input\n    ^" — no extra wrapping needed
        w.line(&caret_lines(input, info.position));
    }

    w.separator();
    w.line("END OF REPORT");
    w.separator();
}

// ============================================================================
// Bitcoded trace render
// ============================================================================

fn render_bitcoded(regex_str: &str, r: &Regex, input: &str, w: &mut ReportWriter) {
    let start = Instant::now();
    let (result, trace) = parse_bitcoded_traced(input, r);
    let elapsed = start.elapsed();

    let result_label = if result.is_some() { "MATCH" } else { "NO MATCH" };

    w.separator();
    w.line("REGEX ENGINE DEBUG REPORT");
    w.separator();
    w.kv("Timestamp",        &timestamp());
    w.kv("Mode",             "POSIX Bitcoded");
    w.kv("Regex",            regex_str);
    w.kv("Input",            &format!("{:?}", input));
    w.kv("Result",           result_label);

    w.separator();
    w.line("TIMING");
    w.separator();
    w.kv("Parse time",       &format!("{:.3}ms", elapsed.as_secs_f64() * 1000.0));
    w.kv("Derivative steps", &format!("{}", trace.bit_steps.len()));

    w.separator();
    w.line("INTERNALIZE");
    w.separator();
    w.line(&format!("internalize({})", regex_str));
    render_internalize_expansion(r, w);
    w.line(&format!("ri0 = {}", trace.internalized));

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

    if let Some(ref tree) = result {
        w.separator();
        w.line("MKEEPSBC + DECODE");
        w.separator();
        if let Some(ref bits) = trace.final_bits {
            let bits_str = bits.iter()
                .map(|b| if *b { "1" } else { "0" })
                .collect::<Vec<_>>()
                .join(",");
            w.line(&format!("mkEpsBC(ri{}) = [{}]", final_step, bits_str));
            w.blank();
            w.line(&format!("decode({}, [{}]):", regex_str, bits_str));
            w.line(&format!("  = {}", tree));
        }

        w.separator();
        w.line("RESULT");
        w.separator();
        if let Some(ref bits) = trace.final_bits {
            let bits_str = bits.iter()
                .map(|b| if *b { "1" } else { "0" })
                .collect::<Vec<_>>()
                .join(",");
            w.kv("Bits",       &format!("[{}]", bits_str));
        }
        w.kv("Parse tree", &format!("{}", tree));
        w.kv("Flattened",  &format!("{:?}", flatten(tree)));

    } else {
        w.separator();
        w.line("PARTIAL RECOVERY");
        w.separator();

        if let Some(ref bits) = trace.bits_at_last_nullable {
            let bits_str = bits.iter()
                .map(|b| if *b { "1" } else { "0" })
                .collect::<Vec<_>>()
                .join(",");
            let last_idx = trace.last_nullable_idx.unwrap_or(0);
            let partial_str: String = input.chars().take(last_idx).collect();

            if last_idx > 0 {
                let last_ri = &trace.bit_steps[last_idx - 1].after;
                w.line(&format!(
                    "Last nullable expression: ri{} = {}  (after position {})",
                    last_idx, last_ri, last_idx
                ));
                w.line(&format!("Accumulated bits: [{}]", bits_str));
                w.blank();
                w.line(&format!("mkEpsBC(ri{}) would give: [{}]", last_idx, bits_str));
                w.line(&format!("This would decode to: ... (partial match {:?})", partial_str));
            }
            w.blank();
            w.kv("Partial match", &format!("{:?}  (positions 1–{})", partial_str, last_idx));
        } else {
            w.line("No prefix matched.");
        }

        w.separator();
        w.line("ERROR SUMMARY");
        w.separator();
        let info = find_failure(input, r);
        w.kv("Failure at position", &format!("{} (1-indexed)", info.position));
        if info.found == '\0' {
            w.kv("Found",    "end of input");
        } else {
            w.kv("Found",    &format!("'{}'", info.found));
        }
        w.kv("Expected", &info.expected);
        // caret_lines already produces "  input\n    ^" — no extra wrapping needed
        w.line(&caret_lines(input, info.position));
    }

    w.separator();
    w.line("END OF REPORT");
    w.separator();
}

// ============================================================================
// Structural expansion helpers
// ============================================================================

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
            w.line(&format!("  ∂({:?}, '{}') — see left branch", r1, c));
            w.line(&format!("  ∂({:?}, '{}') — see right branch", r2, c));
        }
        Seq(r1, _r2) => {
            if nullable(r1) {
                w.line(&format!("  rule: ∂(r1·r2, '{}')  [nullable(r1)=true]", c));
                w.line("       = ∂(r1, c)·r2  +  ∂(r2, c)");
            } else {
                w.line(&format!("  rule: ∂(r1·r2, '{}')  [nullable(r1)=false]", c));
                w.line("       = ∂(r1, c)·r2");
            }
        }
        Star(r1) => {
            w.line(&format!("  rule: ∂(r*, '{}') = ∂(r, '{}') · r*", c, c));
            w.line(&format!("  ∂({:?}, '{}'):", r1, c));
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
                w.line(&format!("    rule: (bs@ri1·ri2) \\ '{}'  [nullable_bc(ri1)=true]", c));
                w.line("         = bs@(ri1\\'c'·ri2  ⊕  fuse(mkEpsBC(ri1), ri2\\'c'))");
            } else {
                w.line(&format!("    rule: (bs@ri1·ri2) \\ '{}'  [nullable_bc(ri1)=false]", c));
                w.line("         = bs@(ri1\\'c'·ri2)");
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

// ============================================================================
// Utility
// ============================================================================

fn timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn expression_range_label(count: usize) -> String {
    if count == 0 { return "none".to_string(); }
    let labels: Vec<String> = (0..count).map(|i| format!("r{}", i)).collect();
    labels.join(" → ")
}