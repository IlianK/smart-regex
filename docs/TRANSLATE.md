# External Pattern to Internal Regex

`src/frontend/` turns a pattern string into the crate's six-constructor
core `Regex`.

For the CLI's own `match`/`parse` behavior, see [CLI.md](CLI.md).


## Entry points

`src/frontend/mod.rs` exposes two, both full-string match only --
neither pads, searches, or otherwise changes what the translated pattern
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
size caps and one additional, narrow anchor recovery — see
[DATASET.md](DATASET.md)'s "Match-intent reliability tiers". That
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
pattern string
  -> strip_pcre_delimiters   parse_pcre_rule only; splits /PATTERN/FLAGS
  -> parse_ext_pattern       string -> ExtPat; rejects backreferences and lookaround
  -> case_fold               parse_pcre_rule only, if the i flag was set
  -> strip_dot_newline       parse_pcre_rule only, if the s flag was not set
  -> translate               ExtPat -> Regex; rejects ^, $, \b, \B wherever they appear
```

`ExtPat` is the surface AST: literals, `.`, classes, grouping,
quantifiers, bounds, alternation, anchors, and `\b`/`\B`. `translate`
lowers each to `Regex`'s six constructors; a class becomes an `Alt` chain
over the 98-character alphabet, and a bound unrolls into that many
copies. Anchors and word-boundary assertions have no lowering: `translate`
returns `Err` the moment it visits one, regardless of where in the
pattern it sits (see "Anchors are rejected" below).

Because of the `Alt`-chain lowering, `.`, the shorthand classes
(`\d \D \w \W \s \S`), and any negated class `[^...]` can only ever match
a character from that 98-character alphabet (printable ASCII plus
tab/newline/CR, `alphabet.rs`) -- none of them matches a character
outside it, even though real PCRE/POSIX semantics define a negated class
as everything except the characters listed. A literal character written
directly in the pattern is unaffected (`Regex::Lit` compares the full
`char`, not just the working alphabet), so `café` still matches `"café"`;
it is specifically `.`, a shorthand class, or negation that is bounded
this way. Verified: `cargo run -- match "[^a]" "é"`, `match "\W" "é"`,
and `match "." "é"` all print `false`.


## Anchors are rejected

`^`, `$`, `\b`, and `\B` all parse successfully -- into `ExtPat::Carat`,
`ExtPat::Dollar`, and `ExtPat::WordBoundary` -- and are rejected one
stage later, by `translate`, rather than being desugared into an
equivalent `Regex`. Every entry point in `src/frontend/` performs
full-string matching only: no padding is inserted, and no substring
search is performed.

Rejection is total, not positional: `translate` is structural recursion,
and every compound `ExtPat` arm propagates a child's `Err` immediately
(`?` in every case except `Group`/`GroupNonMarking`, which tail-call and
so propagate it for free). An anchor buried behind an optional
quantifier, inside one branch of an alternation, or under a `Star`/bound
still reaches `translate` and still rejects the whole pattern, even
though taking that branch is not required for the pattern to match at
all.

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
in the pattern, since neither ever contains one.


## Rejected Patterns

| Construct | Rejected by | Why there |
|---|---|---|
| Backreferences (`\1`-`\9`, `(?P=name)`), lookaround (`(?=`, `(?!`, `(?<=`, `(?<!`) | `parse_ext_pattern` | not regular; no `ExtPat` constructor exists for them, so there is nowhere to defer to |
| `^`, `$` | `translate` | parse succeeds into `ExtPat::Carat`/`ExtPat::Dollar` so rejection can name the construct instead of silently mismatching it |
| `\b`, `\B` | `translate` | parse succeeds into `ExtPat::WordBoundary`, same reason; both are position-dependent, not regular-language, constructs |