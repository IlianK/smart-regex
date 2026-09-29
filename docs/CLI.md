# CLI Reference

Full command reference for the `regex-engine` binary. See the [README](../README.md) for install/build and a quickstart.

**By default, `match`/`parse` require the entire input to match the
entire pattern**, and `^`/`$` do nothing: full-string matching is what
the core algorithm computes. Pass `--search` to pad unanchored sides
with `Σ*` instead (substring search), the same padding
`src/data/`'s dataset pipeline uses. 

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
cargo run -- match "a*" "aab" 
cargo run -- match "(a|ab)(b|ε)" "b" --matcher naive 
```

---

## Parser

```bash
# Default parser (deriv_std_rec), default diagnostics (off)
cargo run -- parse "(a|ab)(b|ε)" "ab"
cargo run -- parse "(a|b|ab)*"   "ab"

# Specific parser
cargo run -- parse "a*" "aaa" --parser deriv_std_rec
cargo run -- parse "a*" "aaa" --parser deriv_std_loop
cargo run -- parse "a*" "aaa" --parser deriv_bc

# Partial-derivative parsers compute GREEDY, not POSIX
cargo run -- parse "a*" "aaa" --parser pderiv_std
cargo run -- parse "a*" "aaa" --parser pderiv_bc 

# Compare all parsers side by side: 
# POSIX-proven ones checked for full agreement
# pderiv_bc shown alongside agreeing on membership (not tree shape) 
cargo run -- parse "a*"          "aaa" --parser all
cargo run -- parse "(a|ab)(b|ε)" "ab"  --parser all
```

---

## Parser with Diagnostics

Diagnostics are controlled by `--diag` (0-3) and work with all parsers.

### Level 1 - Basic (Regex, Input, Match, Tree / Error caret)

```bash
# Success
cargo run -- parse "a*" "aaa" --diag 1
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser deriv_std_loop --diag 1

# Failure (shows error position and caret)
cargo run -- parse "a*" "aab" --diag 1
cargo run -- parse "(a|ab)(b|ε)" "b" --parser deriv_std_loop --diag 1
```

### Level 2 - Verbose (+ time, step count, construction steps / bit trace)

```bash
# Standard success - shows mkEps(rN) and inject steps
cargo run -- parse "a*" "aaa" --diag 2
cargo run -- parse "a*" "aaa" --parser deriv_std_loop --diag 2

# Standard failure - shows partial tree recovery
cargo run -- parse "a*" "aab" --diag 2

# Bitcoded success - shows internalize, bit steps, mkEpsBC, decode
cargo run -- parse "a*" "aaa" --parser deriv_bc --diag 2

# Bitcoded failure - shows bits accumulated before failure
cargo run -- parse "a*" "aab" --parser deriv_bc --diag 2

# Paper examples
cargo run -- parse "(a|ab)(b|ε)" "ab" --diag 2
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser deriv_bc --diag 2
cargo run -- parse "(a|b|ab)*"   "ab" --diag 2
```

### Level 3 - Debug (full structural derivation trace, written to file or stdout)

Level 3 writes to `reports/report.txt` by default. Override with `--diag-report`.

```bash
# Standard success - full forward + backward pass trace
cargo run -- parse "a*" "aaa" --diag 3
cargo run -- parse "a*" "aaa" --parser deriv_std_loop --diag 3

# Standard failure - full forward trace + partial recovery + error summary
cargo run -- parse "a*" "aab" --diag 3

# Bitcoded success - internalize + all deriv_bc steps + mkEpsBC + decode
cargo run -- parse "a*" "aaa" --parser deriv_bc --diag 3

# Bitcoded failure
cargo run -- parse "a*" "aab" --parser deriv_bc --diag 3

# Custom filename inside reports/
cargo run -- parse "(a|ab)(b|ε)" "ab" --diag 3 --diag-report reports/paper_r1.txt
cargo run -- parse "(a|b|ab)*"   "ab" --diag 3 --diag-report reports/paper_r2.txt

# Confirm deriv_rec and deriv_std_loop produce identical derivation traces
# (diff will show two expected differences -- the "Mode:" label and the
# timing line -- and nothing else)
cargo run -- parse "a*" "aaa" --diag 3 --diag-report reports/rec.txt
cargo run -- parse "a*" "aaa" --parser deriv_std_loop --diag 3 --diag-report reports/loop.txt
diff reports/rec.txt reports/loop.txt

# Read directly
cat reports/report.txt
```

---

## Flags and Diagnostics Levels

| Flag | Values | Default | Use |
|---|---|---|---|
| `--parser` | `deriv_std_rec` `deriv_std_loop` `deriv_bc` `pderiv_std` `pderiv_bc` `all` | `deriv_std_rec` | Parser selection (`parse` only) |
| `--matcher` | `naive` `deriv` `pderiv` `all` | `deriv` | Matcher selection (`match` only) |
| `--diag`  | `0` `1` `2` `3` | `0` | Output verbosity level (`parse` only) |
| `--diag-report` | file path | unset (`reports/report.txt` if `--diag 3` with no path given) | Level 3 report destination (`parse` only) |
| `--search` | flag (present/absent) | absent | Pad unanchored sides with `Σ*` instead of requiring a full-string match; both `match` and `parse` |

### Verbosity levels

| Level | Name | On success | On failure |
|---|---|---|---|
| `0` | Off | `true` | `false` |
| `1` | Basic | Regex, Input, Match, Tree | + position, found, expected, caret |
| `2` | Verbose | + time, step count, construction steps / bit trace | + partial match recovery |
| `3` | Debug | + full structural derivation trace | + full trace to failure point; writes to `--diag-report` if set |

---

## External Pattern: Anchoring

Without `--search`, both `match` and `parse` parse their `regex`
argument with `frontend::parse_pattern`: the bare pattern text directly,
**not** wrapped in PCRE `/pattern/flags` delimiters, and **not** padded
into a search over `^`/`$`-unanchored substrings. `match`/`parse` ask
whether the *entire* input matches the *entire* pattern; `^`/`$` are
accepted but do nothing (`translate` maps both to `Eps`).

With `--search`, both instead parse with `frontend::parse_dataset_pattern`
(the same padding `src/data/`'s pipeline uses, minus its `/PATTERN/FLAGS`
wrapper-stripping and `i`-flag case-folding, which `--search` doesn't
add): whichever side of the pattern lacks a top-level `^`/`$` gets padded
with `Σ*`. Verified live:

```bash
$ cargo run -- match "abc" "xabcx"            # no --search: full string only
false
$ cargo run -- match "abc" "xabcx" --search   # unanchored: padded both sides
true
$ cargo run -- match '^abc$' "xabcx" --search # fully anchored: no padding either side
false
$ cargo run -- match '^abc' "abcx" --search   # start-anchored: padded on the end only
true
$ cargo run -- match '^abc' "xabc" --search
false
```

See [FRONTEND.md](FRONTEND.md)'s "Search padding and anchoring" section
for the full anchoring table (`(^abc)`, `a^b`, `(a|^b)c`, etc.) — every
row of it applies identically under `--search`.

### Supported syntax

```bash
# Literals, concatenation, alternation, grouping
cargo run -- match "cat|dog" "dog"
cargo run -- match "(a|b)c" "bc"
cargo run -- match "(?:a|b)c" "bc"      # non-capturing group

# Quantifiers: greedy and lazy (trailing ?)
cargo run -- match "a*"  "aaa"
cargo run -- match "a+"  "aaa"
cargo run -- match "a?"  "a"
cargo run -- match "a*?" "aaa"          # lazy star
cargo run -- match "a+?" "aaa"          # lazy plus

# Bounded repetition {m}, {m,}, {m,n}
cargo run -- match "a{2,4}" "aaa"

# Character classes, negation, ranges
cargo run -- match "[abc]"   "b"
cargo run -- match "[^abc]"  "d"
cargo run -- match "[a-z0-9]+" "a1b2"

# Shorthand classes: \d \D \w \W \s \S, and . (any character)
cargo run -- match '\d{3}-\d{4}' "555-1234"
cargo run -- match '\w+\s\w+' "hello world"
cargo run -- match "a.c" "abc"

# ^ and $: accepted, but a no-op under match/parse's default full-string
# semantics -- they only start mattering with --search (see below)
cargo run -- match "^abc$" "abc"
```

### Rejected outright, with a specific error

Three constructs describe languages, or depend on position, in a way
that isn't regular in the formal sense every parser depends on.
`parse_pattern` rejects all three with a
descriptive error rather than silently mis-parsing them:

```bash
$ cargo run -- match "(a)\1" "aa"
Regex parse error: backreference '\1' is not a regular-language construct -- unsupported

$ cargo run -- match "a(?=b)" "ab"
Regex parse error: lookahead '(?=...)' is not a regular-language construct -- unsupported

$ cargo run -- match '\bfoo\b' "foo"
Regex parse error: word boundary '\b'/'\B' is position-dependent, not a regular-language construct -- unsupported
```

Lookahead, negative lookahead, lookbehind, and negative lookbehind
(`(?=`, `(?!`, `(?<=`, `(?<!`) are all rejected the same way, at parse
time, before any translation is attempted. `\b`/`\B` parse successfully
(into an internal `WordBoundary` marker, so a literal `b` is never
silently substituted) but are rejected one step later, at translation.

### What this frontend still does *not* expose

`--search` covers the padding half of `parse_pcre_rule` (what
`src/data/`'s dataset pipeline and `examples/filter_dataset.rs` actually
use), not the rest: the `/PATTERN/FLAGS` wrapper and the `i`
case-insensitive flag are still specific to `parse_pcre_rule` and have no
CLI flag — there is no `cargo run -- match "/pattern/i" "input"` form.
See [DATASETS.md](DATASETS.md) for that path instead.
