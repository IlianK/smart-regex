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
│   ├── demo_posix.rs               # POSIX parse tree demo (select via REGEX_PARSER)
│   ├── demo_crash.rs               # Stack overflow comparison: recursive vs. loop
│   └── demo_crash_worker.rs        # Worker process for demo_crash.rs
│
├── src/
│   ├── lib.rs                      # Library exports
│   ├── main.rs                     # CLI entry point
│   │
│   ├── types/                      # Core data types
│   │   ├── mod.rs
│   │   ├── regex.rs                # Regex enum (Phi, Eps, Lit, Seq, Alt, Star)
│   │   ├── aregex.rs               # ARegex enum (bit-annotated)
│   │   └── parse_tree.rs           # ParseTree enum + flatten
│   │
│   ├── regex/                      # Derivative algorithms
│   │   ├── mod.rs
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
│   │   ├── selection.rs            # Matcher selection (env / CLI)
│   │   ├── match_naive.rs          # Exponential recursive matcher
│   │   ├── match_deriv.rs          # Brzozowski derivative matcher
│   │   └── match_pderiv.rs         # Antimirov partial derivative matcher
│   │
│   ├── posix/                      # POSIX parse tree construction
│   │   ├── parser.rs               # parse_posix dispatch (reads REGEX_PARSER)
│   │   ├── selection.rs            # ParserType enum + from_env()
│   │   ├── standard/               # Standard algorithm  (Fig. 3)
│   │   │   ├── mk_eps.rs           # mkEps : Regex → ParseTree
│   │   │   ├── inject.rs           # inj   : Regex → char → ParseTree → ParseTree
│   │   │   └── parse.rs            # parse_recursive, parse_loop
│   │   └── bitcoded/               # Bit-coded optimisation  (Fig. 5)
│   │       ├── internalize.rs      # internalize, fuse
│   │       ├── mk_eps_bc.rs        # mkEpsBC : ARegex → Vec<bool>
│   │       ├── decode.rs           # decode  : Regex → Vec<bool> → ParseTree
│   │       └── parse.rs            # parse_bitcoded
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
# Default matcher (deriv)
cargo run -- match "a*" "aaa"

# Specific matcher
cargo run -- --matcher naive  match "a*" "aaa"
cargo run -- --matcher deriv  match "a*" "aaa"
cargo run -- --matcher pderiv match "a*" "aaa"

# Compare all three matchers side by side
cargo run -- --matcher all match "a*" "aaa"
```

### Parser

```bash
# Default parser (recursive)
cargo run -- parse "(a+ab)(b+ε)" "ab"
cargo run -- parse "(a+b+ab)*"   "ab"

# Specific parser
REGEX_PARSER=recursive  cargo run -- parse "a*" "aaa"
REGEX_PARSER=loop       cargo run -- parse "a*" "aaa"
REGEX_PARSER=bitcoded   cargo run -- parse "a*" "aaa"

# Compare all three parsers side by side
REGEX_PARSER=all cargo run -- parse "a*"           "aaa"
REGEX_PARSER=all cargo run -- parse "(a+ab)(b+ε)"  "ab"
```

---

## Examples

```bash
# Boolean matching demo (naive, deriv, pderiv)
cargo run --example demo_match

# POSIX parse tree demo - select parser via REGEX_PARSER
cargo run --example demo_posix
REGEX_PARSER=recursive  cargo run --example demo_posix
REGEX_PARSER=loop       cargo run --example demo_posix
REGEX_PARSER=bitcoded   cargo run --example demo_posix
REGEX_PARSER=all        cargo run --example demo_posix
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
- **Unit tests** live inside each `src/` file as `#[cfg(test)] mod tests { ... }`
- **Integration tests** live in `tests/` and use only the public library API


| Integration Test Files | Verifies ... |
|---|---|
| `tests/test_matchers.rs` | `match_naive = match_deriv = match_pderiv` for all inputs |
| `tests/test_posix_standard.rs` | `parse_recursive`, `parse_loop` correctness & equivalence, POSIX axioms A1/A2 |
| `tests/test_posix_bitcoded.rs` | `parse_bitcoded` = `parse_recursive` (Theorem 1), flatten |


### Run Tests 

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

# One test by name 
cargo test parse_recursive_paper_r1_on_ab
cargo test bitcoded_agrees_on_paper_r2_ab
cargo test mk_eps_bc_alt_prefers_left

# Unit tests for specific module 
cargo test --lib nullable::tests
cargo test --lib simplify::annotated::tests
cargo test --lib mk_eps_bc::tests
cargo test --lib decode::tests
cargo test --lib internalize::tests

# Print output even for passing tests 
cargo test -- --nocapture

# List all tests without running 
cargo test -- --list

# Run single-threaded 
cargo test -- --test-threads=1
```