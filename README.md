# Regex-Engine

A derivative-based regular expression engine in Rust. It implements matching and parse-tree construction with Brzozowski derivatives, and Antimirov partial derivatives plus a bit-coded parser optimization based on [this paper by Sulzmann and Lu](https://www.researchgate.net/publication/268173400_POSIX_Regular_Expression_Parsing_with_Derivatives), with a shared diagnostics/tracing layer for inspecting each step.

## [Rust install](https://rust-lang.org/tools/install/)

```bash
rustc --version # 1.98.0
cargo --version # 1.98.0
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
cargo run -- parse "(a|ab)(b|ε)" "ab"

# Add diagnostics: derivative steps with --diag=1/2/3
cargo run -- parse "(a|ab)(b|ε)" "ab" --diag=2
```

For the full command reference, see:

- [docs/CLI.md](docs/CLI.md): matcher/parser selection, diagnostics levels 1–3, env var reference
- [docs/EXAMPLES.md](docs/EXAMPLES.md): runnable demos (`examples/`)

- [docs/PARSERS.md](docs/PARSERS.md): the five `--parser` values, one call each
- [docs/parsers/DERIV_STD.md](docs/parsers/DERIV_STD.md): POSIX derivatives, plain Regex (Fig. 3)
- [docs/parsers/DERIV_BC.md](docs/parsers/DERIV_BC.md): POSIX derivatives, bit-coded ARegex (Fig. 4-6)
- [docs/parsers/PDERIV_STD.md](docs/parsers/PDERIV_STD.md): Greedy partial derivatives, injection closures (no reference)
- [docs/parsers/PDERIV_BC.md](docs/parsers/PDERIV_BC.md): Greedy partial derivatives, bit-coded, plain Regex + `Vec<bool>`

- [docs/testing/DATASET.md](docs/testing/DATASETS.md): the real-world regex corpus and how it's triaged
- [docs/testing/BENCHMARKS.md](docs/testing/BENCHMARKS.md): Criterion benchmarks
- [docs/testing/TESTING.md](docs/testing/TESTING.md): test layout and `cargo test` invocations

- [docs/DIAG.md](docs/DIAG.md): `--diag 1/2/3` output examples, every parser, success and failure
- [docs/FRONTEND.md](docs/FRONTEND.md): pattern string → `ExtPat` → `Regex`
- [docs/SUBSTRING_SEARCH.md](docs/SUBSTRING_SEARCH.md): substring search vs. full-string match

