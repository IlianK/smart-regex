# CLI Reference

Full command reference for the `regex-engine` binary. See the [README](../README.md) for install/build and a quickstart.


---

## Matcher

```bash
# Default matcher (deriv), default diagnostics (off)
cargo run -- match "a*" "aaa"

# Specific matcher
cargo run -- match "a*" "aaa" --matcher naive
cargo run -- match "a*" "aaa" --matcher deriv
cargo run -- match "a*" "aaa" --matcher pderiv

# Compare all three matchers side by side
cargo run -- match "a*" "aaa" --matcher all

# Matcher with diagnostics (only adds error info on failure)
cargo run -- match "a*" "aab" --diag 1
cargo run -- match "(a+ab)(b+ε)" "b" --matcher naive --diag 1
```

---

## Parser

```bash
# Default parser (deriv_rec), default diagnostics (off)
cargo run -- parse "(a+ab)(b+ε)" "ab"
cargo run -- parse "(a+b+ab)*"   "ab"

# Specific parser
cargo run -- parse "a*" "aaa" --parser deriv_rec
cargo run -- parse "a*" "aaa" --parser deriv_loop
cargo run -- parse "a*" "aaa" --parser deriv_bc

# Bit-coded partial-derivative parser computes GREEDY
cargo run -- parse "a*" "aaa" --parser pderiv
cargo run -- parse "a*" "aaa" --parser pderiv_bc 

# Compare all parsers side by side: 
# POSIX-proven ones checked for full agreement
# pderiv_bc shown alongside agreeing on membership (not tree shape) 
cargo run -- parse "a*"          "aaa" --parser all
cargo run -- parse "(a+ab)(b+ε)" "ab"  --parser all
```

---

## Parser with Diagnostics

Diagnostics are controlled by `--diag` (0-3) and work with all parsers.

### Level 1 - Basic (Regex, Input, Match, Tree / Error caret)

```bash
# Success
cargo run -- parse "a*" "aaa" --diag 1
cargo run -- parse "(a+ab)(b+ε)" "ab" --parser deriv_loop --diag 1

# Failure (shows error position and caret)
cargo run -- parse "a*" "aab" --diag 1
cargo run -- parse "(a+ab)(b+ε)" "b" --parser deriv_loop --diag 1
```

### Level 2 - Verbose (+ time, step count, construction steps / bit trace)

```bash
# Standard success - shows mkEps(rN) and inject steps
cargo run -- parse "a*" "aaa" --diag 2
cargo run -- parse "a*" "aaa" --parser deriv_loop --diag 2

# Standard failure - shows partial tree recovery
cargo run -- parse "a*" "aab" --diag 2

# Bitcoded success - shows internalize, bit steps, mkEpsBC, decode
cargo run -- parse "a*" "aaa" --parser deriv_bc --diag 2

# Bitcoded failure - shows bits accumulated before failure
cargo run -- parse "a*" "aab" --parser deriv_bc --diag 2

# Paper examples
cargo run -- parse "(a+ab)(b+ε)" "ab" --diag 2
cargo run -- parse "(a+ab)(b+ε)" "ab" --parser deriv_bc --diag 2
cargo run -- parse "(a+b+ab)*"   "ab" --diag 2
```

### Level 3 - Debug (full structural derivation trace, written to file or stdout)

Level 3 writes to `reports/report.txt` by default. Override with `--diag-report`.

```bash
# Standard success - full forward + backward pass trace
cargo run -- parse "a*" "aaa" --diag 3
cargo run -- parse "a*" "aaa" --parser deriv_loop --diag 3

# Standard failure - full forward trace + partial recovery + error summary
cargo run -- parse "a*" "aab" --diag 3

# Bitcoded success - internalize + all deriv_bc steps + mkEpsBC + decode
cargo run -- parse "a*" "aaa" --parser deriv_bc --diag 3

# Bitcoded failure
cargo run -- parse "a*" "aab" --parser deriv_bc --diag 3

# Custom filename inside reports/
cargo run -- parse "(a+ab)(b+ε)" "ab" --diag 3 --diag-report reports/paper_r1.txt
cargo run -- parse "(a+b+ab)*"   "ab" --diag 3 --diag-report reports/paper_r2.txt

# Confirm deriv_rec and deriv_loop produce identical derivation traces
# (diff will show two expected differences -- the "Mode:" label and the
# timing line -- and nothing else)
cargo run -- parse "a*" "aaa" --diag 3 --diag-report reports/rec.txt
cargo run -- parse "a*" "aaa" --parser deriv_loop --diag 3 --diag-report reports/loop.txt
diff reports/rec.txt reports/loop.txt

# Read directly
cat reports/report.txt
```

---

## Flags and Diagnostics Levels

| Flag | Command | Values | Default | Effect |
|---|---|---|---|---|
| `--matcher` | `match` | `naive` `deriv` `pderiv` `all` | `deriv` | Matcher selection |
| `--parser` | `parse` | `deriv_rec` `deriv_loop` `deriv_bc` `pderiv` `pderiv_bc` `all` | `deriv_rec` | Parser selection |
| `--diag` | both | `0` `1` `2` `3` | `0` | Output verbosity level |
| `--diag-report` | `parse` | file path | unset (`reports/report.txt` if `--diag 3` with no path given) | Level 3 report destination |

### Verbosity levels

| Level | Name | On success | On failure |
|---|---|---|---|
| `0` | Off | `true` | `false` |
| `1` | Basic | Regex, Input, Match, Tree | + position, found, expected, caret |
| `2` | Verbose | + time, step count, construction steps / bit trace | + partial match recovery |
| `3` | Debug | + full structural derivation trace | + full trace to failure point; writes to `--diag-report` if set |
