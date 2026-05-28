# Regex-Engine

## Rust install

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

rustc --version  
cargo --version  
```

## Project Structure
```
regex-engine/
├── Cargo.toml
├── Cargo.lock
├── README.md
│
├── benches/
│   ├── bench_match.rs          # Basic matchers (naive, deriv, pderiv)
│   ├── bench_posix.rs          # POSIX parsers (recursive, loop, bitcoded)
│   └── bench_crash.rs          # Stack overflow comparison (loop vs. recursive)
│
├── examples/
│   ├── demo_match.rs           # Basic matching demo
│   └── demo_posix.rs           # POSIX parsing demo 
│
├── src/
│   ├── lib.rs                  # Library exports
│   ├── main.rs                 # CLI 
│   │
│   ├── types/                  # Core data types
│   │   ├── mod.rs
│   │   ├── regex.rs            # Regex enum
│   │   ├── aregex.rs           # ARegex enum
│   │   └── parse_tree.rs       # ParseTree enum + flatten
│   │
│   ├── regex/                  # Derivative algorithms
│   │   ├── mod.rs
│   │   │
│   │   ├── nullable/           # Nullability checks
│   │   │   ├── mod.rs
│   │   │   ├── standard.rs     # nullable (Regex → bool)
│   │   │   └── annotated.rs    # nullable_bc, is_phi (ARegex → bool)
│   │   │
│   │   ├── simplify/           # Simplification rules
│   │   │   ├── mod.rs
│   │   │   ├── standard.rs     # simplify, smart_seq (Regex → Regex)
│   │   │   └── annotated.rs    # simp (ARegex → ARegex)
│   │   │
│   │   ├── brzozowski/         # Brzozowski derivatives
│   │   │   ├── mod.rs
│   │   │   ├── standard.rs     # deriv (Regex → Regex)
│   │   │   └── annotated.rs    # deriv_bc (ARegex → ARegex)
│   │   │
│   │   └── antimirov/          # Antimirov partial derivatives
│   │       ├── mod.rs
│   │       ├── standard.rs     # pderiv (Regex → HashSet<Regex>)
│   │       └── annotated.rs    # pderiv_bc 
│   │
│   ├── matchers/               # Boolean matchers
│   │   ├── mod.rs
│   │   ├── selection.rs        # Matcher selection
│   │   ├── match_naive.rs
│   │   ├── match_deriv.rs
│   │   └── match_pderiv.rs
│   │
│   ├── posix/                  # POSIX parsers
│   │   ├── mod.rs
│   │   ├── parser.rs           # Parser selection
│   │   ├── selection.rs
│   │   │
│   │   ├── standard/           # Standard POSIX (Regex → ParseTree)
│   │   │   ├── mod.rs
│   │   │   ├── mk_eps.rs
│   │   │   ├── inject.rs
│   │   │   └── parse.rs        # parse_recursive, parse_loop
│   │   │
│   │   └── bitcoded/           # Bit-coded POSIX (ARegex → ParseTree)
│   │       ├── mod.rs
│   │       ├── internalize.rs
│   │       ├── mk_eps_bc.rs
│   │       ├── decode.rs
│   │       └── parse.rs        # parse_bitcoded
│   │
│   └── cli/           
│       ├── input.rs
│       ├── matcher.rs
│       ├── parser.rs
│       └── mod.rs
│
├── tests/                      
```

## How to start
### Build and Run
```bash
cargo clean && cargo build
cargo run
cargo check
```

---


### Run CLI 

#### Matcher
```bash
# Match command (default: deriv)
cargo run -- match "a*" "aaa"

# Use specific matcher
cargo run -- --matcher naive match "a*" "aaa"
cargo run -- --matcher deriv match "a*" "aaa"
cargo run -- --matcher pderiv match "a*" "aaa"

# Compare all matchers
cargo run -- --matcher all match "a*" "aaa"
```


#### Parser
```bash
# Parse command (default: recursive)
cargo run -- parse "(a|ab)(b|ε)" "ab"
cargo run -- parse "(a+ab)(b+ε)" "ab"
cargo run -- parse "(a+b+ab)*" "ab"

# Use specific parser
REGEX_PARSER=recursive cargo run -- parse "a*" "aaa"
REGEX_PARSER=loop cargo run -- parse "a*" "aaa"
REGEX_PARSER=bitcoded cargo run -- parse "a*" "aaa"

# Compare all parsers
REGEX_PARSER=all cargo run -- parse "a*" "aaa"
REGEX_PARSER=all cargo run -- parse "(a+ab)(b+ε)" "ab"
```


---


### Run Demo
```bash
# Run demo examples (default: recursive)
cargo run --example demo_match
cargo run --example demo_posix

# Run with specific 
REGEX_PARSER=recursive cargo run --example demo_posix
REGEX_PARSER=loop cargo run --example demo_posix
REGEX_PARSER=bitcoded cargo run --example demo_posix

# Run with all
REGEX_PARSER=all cargo run --example demo_posix
```


### Run Crash Demo
```bash
# Build the worker
cargo build --example demo_crash_worker
cargo build --example demo_crash_worker --release

# Run crash demo
cargo run --example demo_crash
cargo run --example demo_crash --release
```

---


### Run Benches
```bash
# Run all benches
cargo bench
cargo bench -- --verbose   # HTML report in target/criterion/reports

# Run specific bench test 
cargo bench --bench bench_match
cargo bench --bench bench_posix

# Run specific benchmark group within a bench test
cargo bench --bench bench_posix -- scaling_a_star
```


---



### Run Tests 
```bash
# Run all
cargo test

```


