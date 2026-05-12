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
│   └── matcher_bench.rs              
├── src/
│   ├── basic/                        # Naive, Deriv, Par-Deriv
│   │   ├── mod.rs
│   │   ├── regex.rs                  # Regex enum + constructors
│   │   ├── naive.rs                  # match_naive
│   │   ├── brzozowski.rs             # deriv, nullable, match_deriv
│   │   ├── antimirov.rs              # pderiv, match_pderiv
│   │   └── common.rs                 # simplify, smart_seq
│   │
│   ├── posix/                        # POSIX parse tree 
│   │   ├── mod.rs
│   │   ├── parse_tree.rs             # ParseTree enum + flatten
│   │   ├── mk_eps.rs                 # mk_eps function
│   │   ├── inject.rs                 # inject function
│   │   └── parser.rs                 # parse_posix, match_posix
│   │
│   ├── lib.rs                        
│   ├── demo.rs                       
│   └── main.rs                       
│   
├── tests/                            # Tests
│   ├── basic_tests.rs                
│   ├── flatten_tests.rs               
│   ├── mk_eps_tests.rs               
│   ├── paper_tests.rs    
│   ├── compare_test.rs               # loop vs recursive parse              
│   └── posix_tests.rs  
│                
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
cargo test --test compare_tests -- --nocapture --test-threads=1

# Run specific test 
cargo test --test paper_tests test_paper_example_epsilon_alt_star -- --nocapture
```

Run with `REGEX_DEBUG=1` for detailled derivation and injection steps.

Run with `REGEX_USE_LOOP=1` to use loops in forward and backwards pass instead of recursion (recursion is default).

```bash
REGEX_USE_LOOP=1 REGEX_DEBUG=1 cargo test --test paper_tests test_paper_example_epsilon_alt_star -- --nocapture
```


### Run Benchmarks
```bash
# Run benchmarks
cargo bench

# Run only matcher_bench (naive, deriv, pderiv)
cargo bench --bench basic_bench

# Run only posix_bench (recursive vs loop)
cargo bench --bench posix_bench

# Run specific benchmark group within posix_bench
cargo bench --bench posix_bench -- scaling_a_star

# Generate HTML report (in target/criterion/reports)
cargo bench -- --verbose
```

### Run Crash Test
```bash
# Build worker first
cargo build --bin crash_demo_worker
cargo build --bin crash_demo_worker --release

# DEBUG mode
cargo build --bin crash_demo
cargo run --bin crash_demo

# RELEASE mode
cargo build --bin crash_demo --release
cargo run --bin crash_demo --release
```