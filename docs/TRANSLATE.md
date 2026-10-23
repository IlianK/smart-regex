# External Pattern to Internal Regex

`src/frontend/` parses an external pattern string as `ExtPat` and then turns it into the crate's six-constructor core `Regex`.

For the CLI's own `match`/`parse` behavior, see [CLI.md](CLI.md).


## Two-stage design: `ExtPat` vs `Regex`

`parse_ext_pattern` produces `ExtPat`, a **surface AST** that mirrors the
constructs a real regex dialect (PCRE, POSIX) uses.

The reason `ExtPat` is a separate AST from the internal `Regex` is that
`ExtPat` can *represent* constructs the internal `Regex` cannot. The
frontend deliberately splits recognition from lowering:

- `parse_ext_pattern` produces `ExtPat` nodes. For constructs with no
  `ExtPat` variant at all -- lookaround, backreferences, POSIX classes --
  it cannot build a node, so it returns `Err` immediately: **rejected at
  parse**.
- For `^`, `$`, `\b`, `\B` there *is* an `ExtPat` variant (`Carat`,
  `Dollar`, `WordBoundary`), so `parse_ext_pattern` succeeds and hands
  the node to `translate`. `translate` has no `Regex` lowering for those
  variants, so it returns `Err` with a message naming the construct:
  **rejected at lower**.
- `translate` lowers every remaining `ExtPat` arm to one of `Regex`'s
  six constructors. This two-stage split exists so that anchors and word
  boundaries get a *named* error ("anchor `^` is not supported") instead
  of a generic parse failure -- the `ExtPat` variants exist for
  diagnosis, not for use.

This split is what makes errors useful ("`^` is not supported by this
frontend") instead of generic ("unexpected character `^`"), and it
keeps the parser honest about what PCRE actually allows while the
lowering stage enforces what the matcher actually supports.

### Rejected -- hard `Err` from `parse_ext_pattern` or `translate`

| Construct | Rejected by | Reason |
|---|---|---|
| Lookahead `(?=` `(?!`, lookbehind `(?<=` `(?<!` | `parse_ext_pattern` | not regular; no `ExtPat` constructor exists |
| Backreferences `\1`-`\9`, `(?P=name)` | `parse_ext_pattern` | not regular; no `ExtPat` constructor |
| POSIX classes `[[:alpha:]]` | `parse_ext_pattern` | explicitly rejected to avoid silent misread as literal `[`, `:`, ... |
| Anchors `^`, `$` | `translate` | recognised as `ExtPat::Carat`/`Dollar`; no `Regex` lowering exists |
| Word boundary `\b`, `\B` | `translate` | recognised as `ExtPat::WordBoundary`; position-dependent, not regular-language |
| Bounds > 1000 | `translate` (`MAX_BOUND`) | unrolling would be prohibitive |
| Flags `m`, `x` | `parse_pcre_rule` (`PcreFlags::unsupported`) | `m` needs anchors; `x` changes whitespace significance |

**Consequence for stress-test datasets:** every pattern fed through
`parse_pattern` or `parse_pcre_rule` must avoid the "Rejected" column --
otherwise the harness records a compile error, not a timing datum. In
particular, do **not** wrap patterns in `^...$`; matching is
full-string-only by construction, so anchors are unnecessary and cause
a hard rejection.

### Accepted -- lowered by `translate` to the six `Regex` constructors

| Construct | Syntax | Notes |
|---|---|---|
| Literal character | `a`, `café` | matched as a full `char`; not restricted to the working alphabet |
| Dot | `.` | matches one char from the 98-char alphabet (see "Working alphabet" below); includes `\n` under `parse_pattern`, excludes `\n` under `parse_pcre_rule` without `s` |
| Control / hex / octal escape | `\n` `\t` `\r` `\xHH` `\0`-`\7` | octal clamped to 255 |
| Shorthand class | `\d` `\D` `\w` `\W` `\s` `\S` | ASCII only; `\D`/`\W`/`\S` = alphabet minus the positive set |
| Bracket class | `[...]`, `[^...]`, ranges `[a-z]`, literal `]` and `-`, octal inside | ASCII only; negation = alphabet minus listed set |
| Group | `(...)`, `(?:...)`, `(?<name>...)`, `(?P<name>...)` | capture name parsed then dropped |
| Alternation | `a\|b\|c` | -- |
| Quantifier | `?` `+` `*` `{m,n}` `{m,}` `{m}` | `m,n ≤ 1000` (`MAX_BOUND`); lazy `?` accepted and discarded |
| Explicit tokens | `ε` (Eps), `∅` (Phi) | -- |
| Wrapper flags | `/.../i`, `/.../s` | only `parse_pcre_rule`; `i` case-folds, `s` makes `.` include `\n` |

Two lowerings are worth noting for size:

- A class (`.`, `\d`/`\w`/`\s` and their negations, `[...]`, `[^...]`)
  becomes an `Alt` chain over the 98-character working alphabet
  (see "Working alphabet" below).
- A bound `{m,n}` unrolls into `m` copies plus `n - m` optional copies
  (`repeat_exact` / `optional_extra`). Bounds are capped at
  `MAX_BOUND = 1000`: a bound exceeding the cap is rejected, not
  silently truncated.


## Entry points

`src/frontend/mod.rs` exposes two, both full-string match entrypoints.

Neither pads, searches, or otherwise changes what the translated pattern
is matched against.

| Function | Wrapper stripped? | Flags applied | Used by |
|---|---|---|---|
| `parse_pattern` | no | none | CLI `match`/`parse` |
| `parse_pcre_rule` | yes (`/PATTERN/FLAGS`) | `i`, `s`; `m`/`x` rejected | worked examples, test fixtures |

`.` matches `\n` under `parse_pattern`. The `strip_dot_newline` pass that
excludes `\n` runs only in `parse_pcre_rule`, where an `s` flag can be
present or absent; `parse_pattern` reads no flags at all, so no such pass
applies to it.

The dataset pipeline (`src/data/`) does **not** call either of these
directly. It has its own `core_regex`/`core_regex_tagged`
(`src/data/prepare.rs`), which repeats `parse_pcre_rule`'s steps plus two
size caps and one additional, narrow anchor recovery (see
[DATASET.md](DATASET.md)'s "Match-intent reliability"). That
recovery is specific to dataset preparation and never changes what
`translate`, `parse_pattern`, or `parse_pcre_rule` themselves do.


## PCRE flags (`parse_pcre_rule` only)

`PcreFlags` reads the flags from a `/PATTERN/FLAGS` wrapper. Only the
flags whose presence changes what the pattern denotes are recorded.
Unrecognized letters and Snort's buffer selectors like `U`, `H`, `P`, `C`
are ignored, since they name which buffer the rule is tested against, not
what the pattern itself matches. So are `R` (relative), `A` (anchored),
and `E` (dollar-endonly): each positions or tunes a search or an anchor
this frontend has no concept of, so each is ignored like an unrecognized
buffer selector.

| Flag | Meaning | Handled as |
|---|---|---|
| `i` | case-insensitive | case-folded (`case_fold`) |
| `s` | dot-all | `.` includes `\n` (absent: excluded, `strip_dot_newline`) |
| `m` | multiline | rejected: `^`/`$` would also match at newlines, and this frontend has no `^`/`$` |
| `x` | extended | rejected: whitespace would stop being literal |
| `R`, `A`, `E`, buffer selectors (`U`, `H`, `P`, `C`, ...) | ignored | positions or tunes something (a search, an anchor) this frontend has no concept of |


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

## Working alphabet (98 characters)

`alphabet()` in `alphabet.rs` is printable ASCII `0x20`-`0x7E` (95 chars)
plus `\t`, `\n`, `\r` (3 more) = **98 characters**. Every class-shaped
construct -- `.`, `\d \D \w \W \s \S`, and any bracket class `[...]` or
`[^...]` -- lowers to an `Alt` chain over this alphabet. Consequence:

- None of these constructs can match a character **outside** the
  alphabet, even under negation. A `[^a]` class matches *every alphabet
  character except `a`*; it does not match `é`, `€`, emoji, or any other
  non-ASCII character, contrary to real PCRE/POSIX semantics where a
  negated class means "everything except the listed characters".
- A **literal** character written directly in the pattern is unaffected:
  `Regex::Lit` compares the full `char`, so `café` still matches `"café"`.
  Only `.`, shorthand classes, and negated classes are alphabet-bounded.

Verified: `cargo run -- match "[^a]" "é"`, `match "\W" "é"`, and
`match "." "é"` all print `false`.


## Anchors are rejected everywhere they appear

`^`, `$`, `\b`, and `\B` parse into `ExtPat::Carat`, `ExtPat::Dollar`,
and `ExtPat::WordBoundary` -- but `translate` returns `Err` the moment it
reaches one. The rejection is **total, not positional**: `translate` is
structural recursion, and every compound `ExtPat` arm propagates a
child's `Err` immediately (`?` in every case except `Group` /
`GroupNonMarking`, which tail-call and so propagate it for free). An
anchor buried behind an optional quantifier, inside one branch of an
alternation, or under a `Star`/bound still rejects the whole pattern --
even though taking that branch is not required for the pattern to match
at all.

Verified against the built CLI, each rejecting with the same message
(`anchor '^'/'$' is not supported by this frontend -- unsupported`):

```bash
cargo run -- parse "^?abc" "abc"        # optional, directly
cargo run -- parse "(^)?abc" "abc"      # optional, behind a group
cargo run -- parse "(^a|b)c" "bc"       # inside one alternation branch
cargo run -- parse "(^a)*b" "b"         # under a star
cargo run -- parse "(^a){0,1}b" "b"     # under an explicit bound
```

`ε` and `∅` are unaffected: `translate(ExtPat::Eps) = Regex::Eps` and
`translate(ExtPat::Never) = Regex::Phi` regardless of anchors elsewhere
in the pattern, since neither ever contains one. (The same is *not* true
in reverse -- `translate` recurses into every child, so an anchor
**anywhere** in a larger pattern still rejects the whole thing, even if
the `ε` or `∅` branch is the one that would have matched.)