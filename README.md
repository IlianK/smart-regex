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
├── benches/
│   └── matcher_bench.rs              # Unchanged
├── src/
│   ├── lib.rs                        # Module declarations
│   ├── basic/                        # Three fundamental approaches
│   │   ├── mod.rs
│   │   ├── regex.rs                  # Regex enum + constructors
│   │   ├── naive.rs                  # match_naive
│   │   ├── brzozowski.rs             # deriv, nullable, match_deriv
│   │   ├── antimirov.rs              # pderiv, match_pderiv
│   │   └── common.rs                 # simplify, smart_seq
│   ├── posix/                        # POSIX parse tree construction
│   │   ├── mod.rs
│   │   ├── parse_tree.rs             # ParseTree enum + flatten
│   │   ├── mk_eps.rs                 # mk_eps function
│   │   ├── inject.rs                 # inject function
│   │   └── parser.rs                 # parse_posix, match_posix
│   └── main.rs                       # Demo application
├── tests/
│   ├── basic_tests.rs                # Tests for basic matchers
│   └── posix_tests.rs                # Tests for POSIX parser
├── Cargo.toml
├── Cargo.lock
└── README.md
```

## How to start
### Build and Run
```bash
cargo build
cargo run
cargo check
```

### Run Tests
```bash
# Run all
cargo test

# Run Tests Modules
cargo test --test mk_eps_tests -- --nocapture --test-threads=1
cargo test --test flatten_tests -- --nocapture --test-threads=1
cargo test --test paper_tests -- --nocapture --test-threads=1
cargo test --test posix_tests -- --nocapture --test-threads=1

# Run specific test 
cargo test test_parse_posix_star
cargo test test_ab_star

# Run with debug 
cargo test --test posix_tests -- --nocapture --test-threads=1
REGEX_DEBUG=1 cargo test --test posix_tests test_parse_posix_paper_example_verbose -- --nocapture
```

### Run Benchmarks
```bash
# All benchmarks
cargo bench

# Run specific benchmarks
cargo bench -- pathological
cargo bench -- benign
```


