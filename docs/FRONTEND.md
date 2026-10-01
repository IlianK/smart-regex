# Frontend

`src/frontend/` turns an external pattern *string* into the crate's
six-constructor core `Regex` (Chapter 7). This is a library-API
reference for that module; for the CLI's own `match`/`parse` usage and
exactly what syntax they accept, see [CLI.md](CLI.md)'s "Frontend: What
the `regex` Argument Accepts" section instead.

## Entry points

`src/frontend/mod.rs` exposes three, differing only in how much gets
applied on top of the shared parse-and-translate core:

| Function | Wrapper stripped? | Case-folds `i`? | Padding | Used by |
|---|---|---|---|---|
| `parse_pattern` | no | no | none (full-string) | CLI `match`/`parse` (default) |
| `parse_dataset_pattern` | no | no | search (`Sigma*` on unanchored sides) | CLI `match`/`parse --search` |
| `parse_pcre_rule` | yes (`/PATTERN/FLAGS`) | yes | search | `src/data/` pipeline, `filter_dataset` |

All three share the same first two steps: `parse_ext_pattern` (pattern
string to `ExtPat` AST) and `translate`/`translate_as_search` (`ExtPat`
to `Regex`). A pattern rejected by `parse_ext_pattern` is rejected
identically by all three entry points; there is no leniency specific to
any one of them.

## Pipeline

```
pattern string
  -> strip_pcre_delimiters   (parse_pcre_rule only: /PATTERN/FLAGS -> PATTERN, i-flag)
  -> parse_ext_pattern       (string -> ExtPat AST; rejects backreferences and lookaround here)
  -> case_fold               (parse_pcre_rule only, if the i flag was set)
  -> translate / translate_as_search   (ExtPat -> Regex; rejects \b/\B here)
```

`ExtPat` (`src/frontend/ext_pattern.rs`) is the surface AST: literals,
`.`, character classes (`[abc]`, `[^abc]`, ranges, `\d \D \w \W \s \S`),
grouping (`(...)`, `(?:...)`), quantifiers (`*`, `+`, `?`, each with a
lazy `?` variant), bounded repetition (`{m,n}`), alternation (`|`),
anchors (`^`, `$`), and `\b`/`\B` word-boundary markers. `translate`
(`src/frontend/translate.rs`) lowers each of these to `Regex`'s six
constructors (`Eps`, `Phi`, `Lit`, `Seq`, `Alt`, `Star`); a character
class becomes an explicit `Alt` chain over `alphabet.rs`'s 98-character
working alphabet (Section 8.6's "Character classes in place of literal
alternation" discusses the cost of that decision), and a bound unrolls
into that many literal copies.

## Search padding and anchoring (`parse_dataset_pattern`/`parse_pcre_rule`)

This section is about the two search-padding entry points. The CLI uses
`parse_pattern` (no padding, ever) by default, and only switches to
`parse_dataset_pattern` when `--search` is passed (see CLI.md); without
that flag, `cargo run -- match "abc" "xabcx"` prints `false` -- the
whole input must equal the pattern exactly, so an unanchored pattern is
never implicitly a substring search unless `--search` says so.

`translate_as_search` decides padding from where `^`/`$` sit
*structurally* in the `ExtPat` tree, not from where they appear in the
source text: `starts_with_carat`/`ends_with_dollar`
(`src/frontend/ext_pattern.rs`) recurse into a top-level `Concat`'s
first/last element and into `Group`/`GroupNonMarking`, but stop at every
other constructor, including `Or`. The "Effective shape" column below is
what `parse_dataset_pattern` (or `parse_pcre_rule`, which shares the
same `translate_as_search`) actually produces:

| Pattern | Anchored? | Effective shape | Why |
|---|---|---|---|
| `^abc` | start | `abc · Σ*` | Top-level `Carat` is the first element of the `Concat`; padding on the right only. |
| `abc$` | end | `Σ* · abc` | Top-level `Dollar` is the last element of the `Concat`; padding on the left only. |
| `^abc$` | both | `abc` | Fully anchored; no padding on either side. |
| `abc` | neither | `Σ* · abc · Σ*` | No top-level anchors; padded on both sides. |
| `(^abc)` | start | `abc · Σ*` | `starts_with_carat` recurses through `Group`/`GroupNonMarking`; padding on the right only. |
| `a^b` | neither | `Σ* · ab · Σ*` | `starts_with_carat` sees `Concat([Char('a'), Carat, ...])`, whose first element is not `Carat`. The internal `^` translates to `Eps`, so the pattern is effectively `ab`, padded on both sides. |
| `(a\|^b)c` | neither | `Σ* · (a\|b)c · Σ*` | `starts_with_carat` on `Or` is `false` -- it only recurses into `Group`, not `Or`. The `^` inside the left branch of the `Or` translates to `Eps`. |
| `(a\|b)*` | neither | `Σ* · (a\|b)* · Σ*` | The top-level node is `Star`, not `Carat`; padded on both sides. |

An `ExtPat` built by hand is treated the same way -- `starts_with_carat`
and `ends_with_dollar` are structural, not syntactic.

`\b`/`\B` are **not** anchors in this sense: they parse into
`ExtPat::WordBoundary`, which `translate` rejects (below), so padding
never reaches them.

`ε` (`ExtPat::Empty`) and `∅` (`ExtPat::Never`) participate in anchoring
like any other atom, not specially: `^aε$` anchors on both sides and
matches exactly `"a"` (`ε` translates to `Eps`, the identity element
`nullable`/`deriv` treat `Seq` against); `^∅$` anchors on both sides and
matches nothing (`∅` translates to `Phi`; `Regex::seq` doesn't simplify
at construction, so the tree still literally contains a `Seq` node, but
`nullable`/`deriv` treat any `Seq` with a `Phi` side exactly as they
would treat `Phi` itself).

## What's accepted, and what's rejected where

Two rejections happen at different stages, not both at parse time:

- **Backreferences** (`\1`-`\9`, `(?P=name)`) and **lookaround**
  (`(?=`, `(?!`, `(?<=`, `(?<!`) are rejected by `parse_ext_pattern`
  itself, before any `ExtPat` value exists for them: both describe
  languages that are not regular in the formal sense this entire crate
  is built on (Section 7's frontend chapter), so there is no `ExtPat`
  constructor for either, and no fallback path that could silently
  mis-parse one into something weaker.
- **`\b`/`\B`** parse successfully, into `ExtPat::WordBoundary`, and are
  rejected one step later, by `translate`. This two-step shape exists
  because `parse_ext_pattern`'s escape handling has a `_ => Escape(c)`
  fallback for any other one-letter escape; without a dedicated
  `WordBoundary` variant, `\b` would fall into that arm and silently
  become the *literal character* `b` instead of an error. Giving it its
  own `ExtPat` variant turns a silent miscompile into a caught,
  descriptive rejection.

Both are recorded by cause, not just as a pass/fail count, wherever a
whole corpus is run through the frontend: `examples/filter_dataset.rs`
and Section 8.3's rejection-cause table.
