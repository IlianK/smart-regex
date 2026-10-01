# External Pattern to Internal Regex

`src/frontend/` turns a pattern string into the crate's six-constructor
core `Regex`. 

For the CLI's own `match`/`parse` behavior, see [CLI.md](CLI.md)'s "External Pattern:
Anchoring" section.


## Entry points

`src/frontend/mod.rs` exposes three, differing only in what is applied
on top of the shared parse-and-translate core.

| Function | Wrapper stripped? | Flags applied | Padding | Used by |
|---|---|---|---|---|
| `parse_pattern` | no | none | none (full-string) | CLI `match`/`parse` (default) |
| `parse_dataset_pattern` | no | none | search (`Σ*` on unanchored sides) | CLI `match`/`parse --search` |
| `parse_pcre_rule` | yes (`/PATTERN/FLAGS`) | `i`, `s`, `A`, `E`; `R` recorded, `m`/`x` rejected | search | `src/data/` pipeline, `dataset_stats` |

`parse_dataset_pattern` is exactly `parse_pattern` plus padding: it
applies no other transformation, so `--search` cannot change what a
pattern means beyond adding padding. In particular, a `--search` pattern
that happens to look like `/foo/i` is parsed as a literal pattern of
those six characters, not as PCRE wrapper syntax.

`.` matches `\n` under both `parse_pattern` and `parse_dataset_pattern`.
The `strip_dot_newline` pass that excludes `\n` runs only in
`parse_pcre_rule`, where an `s` flag can be present or absent; the other
two entry points read no flags at all, so no such pass applies.


## PCRE flags (`parse_pcre_rule` only)

`PcreFlags` reads the flags from a `/PATTERN/FLAGS` wrapper. Only the
flags whose presence changes what the pattern denotes are recorded.
Unrecognized letters and Snort's buffer selectors like `U`, `H`, `P`, `C` —
are ignored, since they name which buffer the rule is tested against, not what
the pattern itself matches.

| Flag | Meaning | Handled as |
|---|---|---|
| `i` | case-insensitive | case-folded (`case_fold`) |
| `s` | dot-all | `.` includes `\n` (absent: excluded, `strip_dot_newline`) |
| `A` | anchored | left padding suppressed, like a leading `^` |
| `E` | dollar-endonly | `$` only at the true end (absent: also allows one trailing `\n`) |
| `R` | relative | recorded, not rejected; see faithfulness below |
| `m` | multiline | rejected: `^`/`$` would also match at newlines |
| `x` | extended | rejected: whitespace would stop being literal |


### Faithfulness

An accepted pattern is not necessarily a faithful one. `parse_pcre_rule`
can succeed on a pattern whose translated `Regex` is only an
approximation of what the source pattern means, because the frontend
does not have access to the surrounding rule (for `R`) or does not
enforce an anchor in every position it could appear (for a nested
anchor). `faithfulness_gaps` returns the specific approximations that
apply to a given pattern; `is_faithful` is true when `parse_pcre_rule`
accepts the pattern *and* `faithfulness_gaps` returns an empty list.

The dataset pipeline (`src/data/`) keeps only faithful patterns: a
pattern that is accepted but not faithful is skipped, the same as one
that fails to parse. 

The CLI's `match`/`parse` do not reject on
faithfulness and only warn for `NestedAnchor`. `Relative` is
not warned about, because `faithfulness_gaps` reads a `/PATTERN/R`
wrapper that `parse_pattern`/`parse_dataset_pattern` never strip.
A CLI pattern shaped like `/foo/R` is a literal, so no such gap applies.

| Gap | Cause |
|---|---|
| `Relative` | The `R` flag positions the match after the rule's previous `content:` match, which the frontend cannot see from the pattern string alone. The pattern is translated as an ordinary search pattern instead. |
| `NestedAnchor` | A `^` or `$` appears in the pattern but not at its own top-level edge, or not on every branch of its alternation. `translate` lowers it to `Eps` rather than enforcing it, so it has no effect on the translated regex. |


## Pipeline

```
pattern string
  -> strip_pcre_delimiters   parse_pcre_rule only; splits /PATTERN/FLAGS
  -> parse_ext_pattern       string -> ExtPat; rejects backreferences and lookaround
  -> case_fold               parse_pcre_rule only, if the i flag was set
  -> translate / translate_as_search   ExtPat -> Regex; rejects \b/\B
```

`ExtPat` is the surface AST: literals, `.`, classes, grouping,
quantifiers, bounds, alternation, anchors, and `\b`/`\B`. `translate`
lowers each to `Regex`'s six constructors; a class becomes an `Alt`
chain over the 98-character alphabet, and a bound unrolls into that many
copies.


## Search padding and anchoring

Padding is decided from where `^`/`$` sit *structurally* in the `ExtPat`
tree and not from the source text. `starts_with_carat`/`ends_with_dollar`
recurse through `Concat`'s first/last element and through `Group`, and
stop at every other constructor.

| Pattern | Anchored? | Effective shape |
|---|---|---|
| `^abc` | start | `abc · Σ*` |
| `abc$` | end | `Σ* · abc` |
| `^abc$` | both | `abc` |
| `abc` | neither | `Σ* · abc · Σ*` |
| `(^abc)` | start | `abc · Σ*` |
| `a^b` | neither | `Σ* · ab · Σ*` |
| `(a\|^b)c` | neither | `Σ* · (a\|b)c · Σ*` |
| `(a\|b)*` | neither | `Σ* · (a\|b)* · Σ*` |

An `ExtPat` built by hand is treated the same way: the anchor predicates
are structural, not syntactic. `\b`/`\B` parse into
`ExtPat::WordBoundary`, which `translate` rejects, so padding never
reaches them.

`ε` and `∅` participate in anchoring like any other atom. `^aε$` anchors
both sides and matches exactly `"a"`. `^∅$` anchors both sides and
matches nothing.


## Rejected Patterns

| Construct | Rejected by | Why there |
|---|---|---|
| Backreferences (`\1`-`\9`, `(?P=name)`), lookaround (`(?=`, `(?!`, `(?<=`, `(?<!`) | `parse_ext_pattern` | not regular; no `ExtPat` constructor exists for them, so there is nowhere to defer to |
| `\b`/`\B` | `translate` | parse succeeds into `ExtPat::WordBoundary` so it can be a descriptive error instead of falling into the `_ => Escape(c)` arm and silently becoming literal `b` |

