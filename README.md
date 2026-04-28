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



## Build and run
```bash
# Build everything
cargo build

# Run the demo
cargo run

# Run all tests
cargo test

# Run specific test modules
cargo test --test basic_tests
cargo test --test posix_tests

# Run specific test 
cargo test test_parse_posix_star
cargo test test_ab_star

# Run benchmarks
cargo bench

# Run specific benchmarks
cargo bench -- pathological
cargo bench -- benign

# Check for errors 
cargo check
```


