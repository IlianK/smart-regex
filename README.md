# Regex-Engine

A derivative-based regular expression engine in Rust. 

It implements matching
and parse-tree construction with **Brzozowski derivatives** and **Antimirov partial
derivatives**, plus a **bit-coded parser optimization** based on
[this paper by Sulzmann and Lu](https://www.researchgate.net/publication/268173400_POSIX_Regular_Expression_Parsing_with_Derivatives),
with a shared diagnostics/tracing layer for inspecting each step.

## Rust install

```bash
rustc --version # 1.98.0
cargo --version # 1.98.0
```

See <https://rust-lang.org/tools/install/> for installation.

---

## Build and run

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

- **`match`**: Boolean match only (returns true/false).
- **`parse`**: Parsing with parse-tree output.

---

## Quickstart

```bash
# Match a string against a regex (default matcher: deriv)
cargo run -- match "a*" "aaa"

# Build a POSIX parse tree (default parser: deriv_std_rec)
cargo run -- parse "(a|ab)(b|ε)" "ab"

# Add diagnostics: derivative steps with --diag=1/2/3
cargo run -- parse "(a|ab)(b|ε)" "ab" --diag=2

# Parse regex with all parseres
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser all
```

---

## Documentation

| File | Contents |
|---|---|
| [docs/CLI.md](docs/CLI.md) | matcher/parser selection, diagnostics levels 1–3, environment-variable reference |
| [docs/EXAMPLES.md](docs/EXAMPLES.md) | runnable demos under `examples/` |
| [docs/DIAG.md](docs/DIAG.md) | `--diag 1/2/3` output examples for every parser, on both successful and failing inputs |
| [docs/FRONTEND.md](docs/FRONTEND.md) | pattern string → `ExtPat` → `Regex`, what the frontend accepts, how anchors and search padding work |
| [docs/PARSERS.md](docs/PARSERS.md) | the four (+ one) `--parser` values, one call each, with pointers into the parser source |
| [docs/DATASETS.md](docs/DATASETS.md) | the real-world regex corpus: sources, preparation pipeline, output format, and safety guards |
| [docs/CATEGORIES.md](docs/CATEGORIES.md) | how Best/Neutral/Worst inputs are generated and verified, one section per generator |
| [docs/BENCHMARKS.md](docs/BENCHMARKS.md) | Criterion benchmarks: `bench_match`, `bench_parse`, `bench_dataset`, `bench_external` |
| [docs/TESTING.md](docs/TESTING.md) | test layout and `cargo test` invocations |
| [docs/EXTERNAL_ENGINES.md](docs/EXTERNAL_ENGINES.md) | comparing against Rust's `regex` crate and Google's RE2, installing RE2, what the comparison does |