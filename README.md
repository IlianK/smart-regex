# Regex-Engine

A derivative-based regular expression engine in Rust. It implements matching and parse-tree construction with Brzozowski derivatives, and Antimirov partial derivatives plus a bit-coded parser optimization, with a shared diagnostics/tracing layer for inspecting each step.

## [Rust install](https://rust-lang.org/tools/install/)

```bash
rustc --version
cargo --version
```

---

## Build and Run

```bash
cargo clean && cargo build
cargo check
cargo run
```

`cargo run` should show:
```
Usage: regex-engine [OPTIONS] <COMMAND>
```
There are two base commands:
- **match**:  Boolean match only (returns true/false)
- **parse**:  Parsing with parse tree output


## Quickstart

```bash
# Match a string against a regex (default matcher: deriv)
cargo run -- match "a*" "aaa"

# Build a POSIX parse tree (default parser: recursive)
cargo run -- parse "(a+ab)(b+ε)" "ab"

# Add diagnostics: derivative steps
REGEX_DIAG=1 cargo run -- match "a*" "aab"
```

For the full command reference, see:

- [docs/CLI.md](docs/CLI.md): matcher/parser selection, diagnostics levels 1–3, env var reference
- [docs/EXAMPLES.md](docs/EXAMPLES.md): runnable demos (`examples/`)
- [docs/BENCHMARKS.md](docs/BENCHMARKS.md): Criterion benchmarks
- [docs/TESTING.md](docs/TESTING.md): test layout and `cargo test` invocations
