# Translation

`src/frontend/` parses an external pattern string as `ExtPat`, then translates into the six-constructor core `Regex`.

For the CLI's own `match` and `parse` behaviour, see [CLI.md](CLI.md).


## ExtPat vs Regex

`parse_ext_pattern` tries to parse an external pattern string into an `ExtPat`, a surface AST mirroring the constructs a real regex dialect (PCRE, POSIX) uses. Not every external pattern is supported. Three outcomes exist.

1. **Not parseable into ExtPat.** Rejected at parse with a named error. `translate` is never reached.
2. **Parsed into ExtPat, not lowerable to Regex.** Rejected at `translate`.
   - `^` and `$` (`Carat`, `Dollar`). The `ExtPat` nodes exist. `core_regex_tagged` strips a top-level `^...$` wrap and tags `ExplicitFullMatch`. `anchor_shape` classifies the anchor distribution. Any placement the strip does not remove reaches `translate` and is rejected there.
   - `\b` and `\B` (`WordBoundary`). The `ExtPat` nodes exist. No `Regex` lowering does.
3. **Parsed into ExtPat and lowered to Regex.** Accepted. Becomes one of `Regex`'s six constructors.


### Rejected

Hard `Err` from `parse_ext_pattern` or `translate`.

| Construct | Rejected by | Reason |
|---|---|---|
| Lookahead `(?=` `(?!`, lookbehind `(?<=` `(?<!` | `parse_ext_pattern` | Not regular. No `ExtPat` constructor exists. |
| Backreferences `\1`-`\9`, `(?P=name)` | `parse_ext_pattern` | Not regular. No `ExtPat` constructor. |
| POSIX classes `[[:alpha:]]` | `parse_ext_pattern` | A literal reading would consume `[`, `:`, and the class name. |
| Anchors `^`, `$` | `translate` | Recognised as `ExtPat::Carat` or `ExtPat::Dollar`. No `Regex` lowering exists. |
| Word boundary `\b`, `\B` | `translate` | Recognised as `ExtPat::WordBoundary`. Position-dependent, not regular-language. |
| Bounds above `MAX_BOUND = 1000` | `translate` | Rejected, not truncated. |
| Flags `m`, `x` | `parse_pcre_rule` (`PcreFlags::unsupported`) | `m` requires anchors. `x` makes whitespace non-literal. |


### Accepted

Lowered by `translate` to the six `Regex` constructors.

| Construct | Syntax | Notes |
|---|---|---|
| Literal character | `a`, `café` | Matches as a full `char`. Not restricted to the working alphabet. |
| Dot | `.` | One char from the 98-char alphabet (see Working alphabet below). Includes `\n` under `parse_pattern`. Excludes `\n` under `parse_pcre_rule` without `s`. |
| Control, hex, octal escape | `\n` `\t` `\r` `\xHH` `\0`-`\7` | Octal clamped to 255. |
| Shorthand class | `\d` `\D` `\w` `\W` `\s` `\S` | ASCII only. `\D`/`\W`/`\S` is the alphabet minus the positive set. |
| Bracket class | `[...]`, `[^...]`, ranges `[a-z]`, literal `]` and `-`, octal inside | ASCII only. Negation subtracts the listed chars from the alphabet. |
| Group | `(...)`, `(?:...)`, `(?<name>...)`, `(?P<name>...)` | Capture name parsed then dropped. |
| Alternation | `a\|b\|c` | (none) |
| Quantifier | `?` `+` `*` `{m,n}` `{m,}` `{m}` | `m,n ≤ 1000` (`MAX_BOUND`). A lazy `?` is accepted and discarded. |
| Explicit tokens | `ε` (Eps), `∅` (Phi) | (none) |
| Wrapper flags | `/.../i`, `/.../s` | Only `parse_pcre_rule`. `i` case-folds. `s` makes `.` include `\n`. |

Two lowerings expand to many nodes.

- A class (`.`, `\d`/`\w`/`\s` and their negations, `[...]`, `[^...]`) becomes an `Alt` chain over the 98-character working alphabet (see Working alphabet below).
- A bound `{m,n}` unrolls into `m` copies plus `n - m` optional copies, via `repeat_exact` and `optional_extra`. `MAX_BOUND = 1000` caps the unrolled size. A larger bound is rejected.


## Entry points

`src/frontend/mod.rs` exposes two, both full-string match entry points. The input string is matched as-is.

| Function | Wrapper stripped? | Flags applied | Used by |
|---|---|---|---|
| `parse_pattern` | no | none | CLI `match` and `parse` |
| `parse_pcre_rule` | yes (`/PATTERN/FLAGS`) | `i`, `s`. `m` and `x` rejected. | worked examples, test fixtures |

`.` matches `\n` under `parse_pattern`. The `strip_dot_newline` pass that excludes `\n` runs only in `parse_pcre_rule`, where an `s` flag can be present or absent. `parse_pattern` reads no flags, so no such pass applies.

The dataset pipeline (`src/data/`) uses its own `core_regex` and `core_regex_tagged` (`src/data/prepare.rs`). It repeats `parse_pcre_rule`'s steps plus two size caps and one narrow anchor recovery (see [DATASET.md](DATASET.md) under Match-intent reliability). That recovery is local to dataset preparation.


## PCRE flags

`parse_pcre_rule` only. `PcreFlags` reads the flags from a `/PATTERN/FLAGS` wrapper. Flags that change what the pattern denotes are recorded. Others are ignored. Unrecognised letters, Snort's buffer selectors (`U`, `H`, `P`, `C`, ...), `R` (relative), `A` (anchored), and `E` (dollar-endonly) all fall into that group.

| Flag | Meaning | Handled as |
|---|---|---|
| `i` | case-insensitive | case-folded (`case_fold`) |
| `s` | dot-all | `.` includes `\n`. Absent, excluded via `strip_dot_newline`. |
| `m` | multiline | Rejected. `^` and `$` would also match at newlines, and this frontend has no anchors. |
| `x` | extended | Rejected. Whitespace would stop being literal. |
| `R`, `A`, `E`, buffer selectors (`U`, `H`, `P`, `C`, ...) | ignored | Names a search or anchor concept this frontend does not have. |


## Pipeline

```
parse_pattern:                    parse_pcre_rule:
  pattern string                    /PATTERN/FLAGS
        │                                  │
        │                                  ▼
        │                         strip_pcre_delimiters
        │                                  │
        └──────────────┬───────────────────┘
                       ▼
              parse_ext_pattern          (rejects lookaround, backrefs, POSIX)
                       │
              ┌────────┴────────┐
              ▼                 ▼
    (parse_pattern)     (parse_pcre_rule)
              │                 │
              │          case_fold          (if /i)
              │                 │
              │          strip_dot_newline   (unless /s)
              │                 │
              └────────┬────────┘
                       ▼
                  translate              (rejects ^, $, \b, \B, bounds > 1000)
                       │
                       ▼
                     Regex
```


## Working alphabet

98 characters. `alphabet()` in `alphabet.rs` is printable ASCII `0x20`-`0x7E` (95 chars) plus `\t`, `\n`, `\r` (3 more).

Every class-shaped construct (`.`, `\d \D \w \W \s \S`, and any bracket class `[...]` or `[^...]`) lowers to an `Alt` chain over this alphabet.

**Consequences**:

- None of these constructs can match a character outside the alphabet, even under negation. `[^a]` matches every alphabet character except `a`. It does not match `é`, `€`, emoji, or any other non-ASCII character. Real PCRE and POSIX read a negated class as everything except the listed characters, non-ASCII included. Here it means any alphabet character except the listed ones.
- A literal character written directly in the pattern is unaffected. `Regex::Lit` compares the full `char`, so `café` still matches `"café"`. Only `.`, shorthand classes, and negated classes are alphabet-bounded.

For instance, `cargo run -- match "[^a]" "é"` prints `false`. So do `match "\W" "é"` and `match "." "é"`.


## Anchors

`^`, `$`, `\b`, and `\B` parse into `ExtPat::Carat`, `ExtPat::Dollar`, and `ExtPat::WordBoundary`. `translate` returns `Err` when it reaches one.

The rejection covers every placement. `translate` is structural recursion. Every compound `ExtPat` arm propagates a child's `Err` immediately. An anchor behind an optional quantifier, inside one branch of an alternation, or under a `Star` or a bound still rejects the whole pattern, even when taking that branch is not required for the pattern to match.

Each rejects with the message `anchor '^'/'$' is not supported by this frontend -- unsupported`.

```bash
cargo run -- parse "^?abc" "abc"        # optional, directly
cargo run -- parse "(^)?abc" "abc"      # optional, behind a group
cargo run -- parse "(^a|b)c" "bc"       # inside one alternation branch
cargo run -- parse "(^a)*b" "b"         # under a star
cargo run -- parse "(^a){0,1}b" "b"     # under an explicit bound
```

`ε` and `∅` are unaffected. `translate(ExtPat::Eps) = Regex::Eps` and `translate(ExtPat::Never) = Regex::Phi`.

Those rules cover the two nodes in isolation. `translate` recurses into every child. An anchor anywhere in a larger pattern still rejects the whole thing, even when the `ε` or `∅` branch is the one that would have matched.