# Regex-Engine

## Rust install

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

rustc --version
cargo --version
```

---

## Project Structure

```
regex-engine/
├── Cargo.toml
├── Cargo.lock
├── README.md
│
├── benches/
│   ├── bench_match.rs              # Benchmarks: naive, deriv, pderiv
│   └── bench_posix.rs              # Benchmarks: recursive, loop, bitcoded
│
├── examples/
│   ├── demo_match.rs               # Boolean matching demo (all 3 matchers)
│   ├── demo_posix.rs               # POSIX parse tree demo (REGEX_PARSER + REGEX_DIAG)
│   ├── demo_crash.rs               # Stack overflow comparison: recursive vs. loop
│   └── demo_crash_worker.rs        # Worker process for demo_crash.rs
│
├── src/
│   ├── lib.rs                      # Library exports
│   ├── main.rs                     # CLI entry point
│   ├── trace.rs                    # Shared trace structs (ParseTrace, BitTrace)
│   │
│   ├── types/                      # Core data types
│   │   ├── mod.rs
│   │   ├── regex.rs                # Regex enum (Phi, Eps, Lit, Seq, Alt, Star)
│   │   ├── aregex.rs               # ARegex enum (bit-annotated)
│   │   └── parse_tree.rs           # ParseTree enum + flatten
│   │
│   ├── regex/                      # Derivative algorithms
│   │   ├── nullable/
│   │   │   ├── standard.rs         # nullable : Regex → bool
│   │   │   └── annotated.rs        # nullable_bc, is_phi : ARegex → bool
│   │   ├── simplify/
│   │   │   ├── standard.rs         # simplify, smart_seq : Regex → Regex
│   │   │   └── annotated.rs        # simp : ARegex → ARegex  (Fig. 6)
│   │   ├── brzozowski/
│   │   │   ├── standard.rs         # deriv : Regex → Regex
│   │   │   └── annotated.rs        # deriv_bc : ARegex → ARegex  (Fig. 5)
│   │   └── antimirov/
│   │       ├── standard.rs         # pderiv : Regex → HashSet<Regex>
│   │       └── annotated.rs        # pderiv_bc (future work)
│   │
│   ├── matchers/                   # Boolean matchers (no parse tree)
│   │   ├── selection.rs            # MatcherType enum + from_env()
│   │   ├── match_naive.rs          # Exponential recursive matcher
│   │   ├── match_deriv.rs          # Brzozowski derivative matcher
│   │   └── match_pderiv.rs         # Antimirov partial derivative matcher
│   │
│   ├── posix/                      # POSIX parse tree construction
│   │   ├── parser.rs               # parse_posix dispatch (reads REGEX_PARSER)
│   │   ├── selection.rs            # ParserType enum + from_env()
│   │   ├── standard/               # Standard algorithm (Fig. 3)
│   │   │   ├── mk_eps.rs           # mkEps : Regex → ParseTree
│   │   │   ├── inject.rs           # inj   : Regex → char → ParseTree → ParseTree
│   │   │   └── parse.rs            # parse_recursive, parse_loop
│   │   │                           # parse_recursive_traced, parse_loop_traced
│   │   └── bitcoded/               # Bit-coded optimisation (Fig. 5)
│   │       ├── internalize.rs      # internalize, fuse
│   │       ├── mk_eps_bc.rs        # mkEpsBC : ARegex → Vec<bool>
│   │       ├── decode.rs           # decode  : Regex → Vec<bool> → ParseTree
│   │       └── parse.rs            # parse_bitcoded, parse_bitcoded_traced
│   │
│   ├── diagnostics/                # Output verbosity (REGEX_DIAG 0–3)
│   │   ├── mod.rs                  # DiagLevel, DiagConfig, run_parser, run_matcher
│   │   ├── trace.rs                # Re-exports from crate::trace
│   │   ├── replay.rs               # Error position finder, partial tree recovery
│   │   ├── level1.rs               # Basic formatter
│   │   ├── level2.rs               # Verbose formatter
│   │   ├── level3.rs               # Debug formatter (full derivation trace)
│   │   └── report.rs               # ReportWriter (stdout or file)
│   │
│   └── cli/
│       ├── input.rs                # Parse user regex string → Regex
│       ├── matcher.rs              # CLI matcher dispatch
│       ├── parser.rs               # CLI parser dispatch
│       └── mod.rs
│
└── tests/                          # Integration tests (public API only)
    ├── common/
    │   └── mod.rs                  # Shared helpers: paper_r1, paper_r2, assert_*
    ├── test_matchers.rs            # Agreement: match_naive = match_deriv = match_pderiv
    ├── test_posix_standard.rs      # parse_recursive, parse_loop, POSIX ordering axioms
    └── test_posix_bitcoded.rs      # parse_bitcoded agreement with parse_recursive
```

---

## Build and Run

```bash
cargo clean && cargo build
cargo check
cargo run
```

---

## CLI

### Matcher

```bash
# Default matcher (deriv), default diagnostics (off)
cargo run -- match "a*" "aaa"

# Specific matcher
cargo run -- --matcher naive  match "a*" "aaa"
cargo run -- --matcher deriv  match "a*" "aaa"
cargo run -- --matcher pderiv match "a*" "aaa"

# Compare all three matchers side by side
cargo run -- --matcher all match "a*" "aaa"

# Matcher with diagnostics (Level 1 adds error caret on failure)
REGEX_DIAG=1 cargo run -- match "a*" "aab"
REGEX_DIAG=1 cargo run -- --matcher naive match "(a+ab)(b+ε)" "b"
```

### Parser

```bash
# Default parser (recursive), default diagnostics (off)
cargo run -- parse "(a+ab)(b+ε)" "ab"
cargo run -- parse "(a+b+ab)*"   "ab"

# Specific parser
REGEX_PARSER=recursive  cargo run -- parse "a*" "aaa"
REGEX_PARSER=loop       cargo run -- parse "a*" "aaa"
REGEX_PARSER=bitcoded   cargo run -- parse "a*" "aaa"

# Compare all three parsers side by side
REGEX_PARSER=all cargo run -- parse "a*"          "aaa"
REGEX_PARSER=all cargo run -- parse "(a+ab)(b+ε)" "ab"
```

### Parser with Diagnostics

Diagnostics are controlled by `REGEX_DIAG` (0–3) and work with all parsers.

#### Level 1 — Basic (Regex, Input, Match, Tree / Error caret)

```bash
# Success
REGEX_DIAG=1 cargo run -- parse "a*" "aaa"
REGEX_DIAG=1 REGEX_PARSER=loop     cargo run -- parse "(a+ab)(b+ε)" "ab"
REGEX_DIAG=1 REGEX_PARSER=bitcoded cargo run -- parse "a*" "aaa"

# Failure (shows error position and caret)
REGEX_DIAG=1 cargo run -- parse "a*" "aab"
REGEX_DIAG=1 REGEX_PARSER=loop     cargo run -- parse "(a+ab)(b+ε)" "b"
REGEX_DIAG=1 REGEX_PARSER=bitcoded cargo run -- parse "a*" "aab"
```

#### Level 2 — Verbose (+ time, step count, construction steps / bit trace)

```bash
# Standard success — shows mkEps(rN) and inject steps
REGEX_DIAG=2 cargo run -- parse "a*" "aaa"
REGEX_DIAG=2 REGEX_PARSER=loop cargo run -- parse "a*" "aaa"

# Standard failure — shows partial tree recovery
REGEX_DIAG=2 cargo run -- parse "a*" "aab"

# Bitcoded success — shows internalize, bit steps, mkEpsBC, decode
REGEX_DIAG=2 REGEX_PARSER=bitcoded cargo run -- parse "a*" "aaa"

# Bitcoded failure — shows bits accumulated before failure
REGEX_DIAG=2 REGEX_PARSER=bitcoded cargo run -- parse "a*" "aab"

# Paper examples
REGEX_DIAG=2 cargo run -- parse "(a+ab)(b+ε)" "ab"
REGEX_DIAG=2 REGEX_PARSER=bitcoded cargo run -- parse "(a+ab)(b+ε)" "ab"
REGEX_DIAG=2 cargo run -- parse "(a+b+ab)*" "ab"
```

#### Level 3 — Debug (full structural derivation trace, written to file or stdout)

Level 3 writes to `reports/report.txt` by default. Override with `REGEX_DIAG_REPORT`.


```bash
# Standard success — full forward + backward pass trace
REGEX_DIAG=3 cargo run -- parse "a*" "aaa"
REGEX_DIAG=3 REGEX_PARSER=loop cargo run -- parse "a*" "aaa"

# Standard failure — full forward trace + partial recovery + error summary
REGEX_DIAG=3 cargo run -- parse "a*" "aab"

# Bitcoded success — internalize + all deriv_bc steps + mkEpsBC + decode
REGEX_DIAG=3 REGEX_PARSER=bitcoded cargo run -- parse "a*" "aaa"

# Bitcoded failure
REGEX_DIAG=3 REGEX_PARSER=bitcoded cargo run -- parse "a*" "aab"

# Custom filename inside reports/
REGEX_DIAG=3 REGEX_DIAG_REPORT=reports/paper_r1.txt cargo run -- parse "(a+ab)(b+ε)" "ab"
REGEX_DIAG=3 REGEX_DIAG_REPORT=reports/paper_r2.txt cargo run -- parse "(a+b+ab)*" "ab"

# Confirm recursive and loop produce identical traces
REGEX_DIAG=3 REGEX_DIAG_REPORT=reports/rec.txt  cargo run -- parse "a*" "aaa"
REGEX_DIAG=3 REGEX_DIAG_REPORT=reports/loop.txt REGEX_PARSER=loop cargo run -- parse "a*" "aaa"
diff reports/rec.txt reports/loop.txt

# Read directly
cat report.txt
```

---

## Examples (Demo)

### Matching demo

```bash
# All three matchers side by side — no diagnostics (table format)
cargo run --example demo_match
```

### POSIX parsing demo

```bash
# Default (recursive, diagnostics off)
cargo run --example demo_posix

# Select parser
REGEX_PARSER=recursive  cargo run --example demo_posix
REGEX_PARSER=loop       cargo run --example demo_posix
REGEX_PARSER=bitcoded   cargo run --example demo_posix

# All parsers side by side (comparison table, ignores REGEX_DIAG)
REGEX_PARSER=all cargo run --example demo_posix

# With diagnostics — same env vars as CLI, same output format
REGEX_DIAG=1 cargo run --example demo_posix
REGEX_DIAG=1 REGEX_PARSER=loop     cargo run --example demo_posix
REGEX_DIAG=1 REGEX_PARSER=bitcoded cargo run --example demo_posix

REGEX_DIAG=2 cargo run --example demo_posix
REGEX_DIAG=2 REGEX_PARSER=loop     cargo run --example demo_posix
REGEX_DIAG=2 REGEX_PARSER=bitcoded cargo run --example demo_posix

# Level 3 with report file
REGEX_DIAG=3 cargo run --example demo_posix
REGEX_DIAG=3 REGEX_DIAG_REPORT=reports/demo.txt cargo run --example demo_posix
```

### Crash demo (stack overflow: recursive vs. loop)

```bash
cargo build --example demo_crash_worker
cargo build --example demo_crash_worker --release

cargo run --example demo_crash
cargo run --example demo_crash --release
```

---

## Benchmarks

```bash
# Run all benchmarks (HTML report: target/criterion/reports/)
cargo bench
cargo bench -- --verbose

# Specific benchmark file
cargo bench --bench bench_match
cargo bench --bench bench_posix

# Specific group within a file
cargo bench --bench bench_posix -- scaling_a_star
```

---

## Tests

Two-tier layout per Rust Book ch. 11-03:

- **Unit tests** — `#[cfg(test)] mod tests` inside each `src/` file, test private functions
- **Integration tests** — `tests/` directory, public API only

| Integration test file | What it verifies |
|---|---|
| `test_matchers.rs` | `match_naive = match_deriv = match_pderiv` for all inputs |
| `test_posix_standard.rs` | `parse_recursive` correctness, `parse_loop` equivalence, POSIX axioms A1/A2 |
| `test_posix_bitcoded.rs` | `parse_bitcoded = parse_recursive` (Theorem 1), flatten round-trip |

```bash
# All tests (unit + integration)
cargo test

# Unit tests only (src/)
cargo test --lib

# Integration tests only (tests/)
cargo test --tests

# One integration file
cargo test --test test_matchers
cargo test --test test_posix_standard
cargo test --test test_posix_bitcoded

# One test by name (substring match)
cargo test parse_recursive_paper_r1_on_ab
cargo test bitcoded_agrees_on_paper_r2_ab
cargo test loop_traced_inject_steps_are_in_forward_order
cargo test recursive_traced_agrees_with_loop_traced

# Unit tests for a specific module
cargo test --lib nullable::tests
cargo test --lib simplify::annotated::tests
cargo test --lib mk_eps_bc::tests
cargo test --lib decode::tests
cargo test --lib internalize::tests
cargo test --lib parse::tests

# Print output even for passing tests
cargo test -- --nocapture

# List all tests without running
cargo test -- --list
```

---

## Diagnostics Environment Variables

| Variable | Values | Default | Effect |
|---|---|---|---|
| `REGEX_DIAG` | `0` `1` `2` `3` | `0` | Output verbosity level |
| `REGEX_PARSER` | `recursive` `loop` `bitcoded` `all` | `recursive` | Parser selection |
| `REGEX_MATCHER` | `naive` `deriv` `pderiv` | `deriv` | Matcher selection |
| `REGEX_DIAG_REPORT` | file path | unset | Level 3 report destination (stdout if unset) |

### Verbosity levels

| Level | Name | On success | On failure |
|---|---|---|---|
| `0` | Off | `true` | `false` |
| `1` | Basic | Regex, Input, Match, Tree | + position, found, expected, caret |
| `2` | Verbose | + time, step count, construction steps / bit trace | + partial match recovery |
| `3` | Debug | + full structural derivation trace | + full trace to failure point; writes to `REGEX_DIAG_REPORT` if set |