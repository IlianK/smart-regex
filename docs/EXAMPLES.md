# Examples (Demo)

Runnable demos under `examples/`. These are standalone library-API demos, **not** the `regex-engine` CLI binary -- they read the `REGEX_*` environment variables below directly, independent of the CLI's
`--matcher`/`--parser`/`--diag`/`--diag-report` flags (see [CLI.md](CLI.md)).

---

## Matching demo

```bash
# All three matchers side by side (no parsing / diagnostics)
cargo run --example demo_match
```

## POSIX parsing demo

```bash
# Default (deriv_rec, diagnostics off)
cargo run --example demo_posix

# Select parser
REGEX_PARSER=deriv_rec  cargo run --example demo_posix
REGEX_PARSER=deriv_loop cargo run --example demo_posix
REGEX_PARSER=deriv_bc   cargo run --example demo_posix

# All parsers side by side (comparison table, ignores REGEX_DIAG)
REGEX_PARSER=all cargo run --example demo_posix

# Level 1
REGEX_DIAG=1 cargo run --example demo_posix
REGEX_DIAG=1 REGEX_PARSER=deriv_loop cargo run --example demo_posix
REGEX_DIAG=1 REGEX_PARSER=deriv_bc   cargo run --example demo_posix

# Level 2
REGEX_DIAG=2 cargo run --example demo_posix
REGEX_DIAG=2 REGEX_PARSER=deriv_loop cargo run --example demo_posix
REGEX_DIAG=2 REGEX_PARSER=deriv_bc   cargo run --example demo_posix

# Level 3 with report file
REGEX_DIAG=3 cargo run --example demo_posix
REGEX_DIAG=3 REGEX_DIAG_REPORT=reports/demo.txt cargo run --example demo_posix
```

## Crash demo (stack overflow: recursive vs. loop)

```bash
# TODO: doc difference for symbols / compiler behaviour
cargo build --example demo_crash_worker
cargo build --example demo_crash_worker --release

cargo run --example demo_crash
cargo run --example demo_crash --release
```
