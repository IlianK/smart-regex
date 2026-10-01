# External Engine Integration (`regex` crate + RE2)

This crate's default build has zero dependency on any other regex engine --
`cargo build`/`cargo test`/`cargo bench` never touch Rust's `regex` crate or
Google's RE2. Comparing against them is opt-in, behind the `external-engines`
Cargo feature, because RE2 needs its system C++ library at build time and
this project should stay buildable without it. This document describes
what's wired up, how to run it, and what it does and does not establish.

## Setup

RE2 has no pure-Rust binding published on crates.io; this integrates it
directly via a small C shim over RE2's C++ API (RE2 doesn't offer a C API of
its own). That means, unlike the pure-Rust `regex` crate, RE2 needs its
development package installed before the feature will build:

```bash
# Debian/Ubuntu
sudo apt install libre2-dev pkg-config

# macOS
brew install re2 pkg-config
```

`pkg-config` (the tool, already present on most systems) must be able to
find `re2.pc` (bundled with the packages above):

```bash
pkg-config --exists re2 && echo "found" || echo "not found"
```

With that in place:

```bash
cargo build  --features external-engines
cargo test   --features external-engines --lib external::
cargo bench  --bench bench_external --features external-engines
```

Without the feature, none of this is touched -- `build.rs` checks
`CARGO_FEATURE_EXTERNAL_ENGINES` and returns immediately if it's unset, so a
plain `cargo build` never invokes pkg-config or a C++ compiler.

## What's actually integrated

| Piece | File(s) | Role |
|---|---|---|
| Cargo feature | `Cargo.toml` (`external-engines`) | Gates the `regex` crate dependency, `src/external/`, and `benches/bench_external.rs` (`required-features`) |
| Build script | `build.rs` | Compiles the RE2 shim and links `libre2` via pkg-config, only when the feature is on |
| C shim | `csrc/re2_shim.h`, `csrc/re2_shim.cpp` | A ~15-function `extern "C"` surface over `RE2`/`re2::StringPiece`, the minimum this project needs |
| Rust FFI wrapper | `src/external/re2.rs` | Safe `Re2` struct: `new`, `is_match`, `full_match`, `find` |
| `regex`-crate wrapper | `src/external/rust_regex.rs` | Thin `RustRegex` struct with the same three-method shape, for a symmetric call site in the bench |
| Comparison bench | `benches/bench_external.rs` | Runs all three engines against the same corpus sample as `bench_dataset.rs` |

Rust's `regex` crate is a normal, optional Cargo dependency
(`regex = { version = "1", optional = true }`) -- no FFI needed. RE2 is the
one that needed the shim, since Google only publishes a C++ API for it.

### Why a C shim instead of an existing crate

At the time this was written there was no actively-maintained, crates.io-published
RE2 binding suited to this use (a safe wrapper over the small subset of RE2
this project actually needs: compile a pattern, run it in either matching
mode, get a boolean or a match span). Writing the ~60-line shim directly
keeps the dependency surface small and auditable rather than pulling in a
general-purpose binding crate for four function calls.

### The shim's API

```c
Re2Handle *re2_new(const char *pattern, size_t pattern_len,
                    int posix, int case_insensitive, char **error_out);
void re2_free(Re2Handle *handle);
void re2_free_error(char *error);
int re2_partial_match(const Re2Handle *handle, const char *text, size_t text_len);
int re2_full_match(const Re2Handle *handle, const char *text, size_t text_len);
int re2_partial_match_span(const Re2Handle *handle, const char *text, size_t text_len,
                            size_t *match_start, size_t *match_end);
```

`Re2Handle` wraps a single `RE2` instance (`csrc/re2_shim.cpp`); `re2_new`
sets `RE2::Options` before compiling:

- `posix` -> `set_posix_syntax(true)` + `set_longest_match(true)`: RE2's
  POSIX leftmost-longest mode instead of its default leftmost-first
  (Perl-like) mode. This is the actual disambiguation-policy switch this
  project cares about, in the same sense `deriv_std`/`deriv_bc` (POSIX) vs
  `pderiv_std`/`pderiv_bc` (Greedy) are a policy switch here (see
  [docs/PARSERS.md](PARSERS.md)).
- When `posix` is set, also `set_perl_classes(true)` and
  `set_word_boundary(true)`, so `\s`/`\d`/`\w`/`\b` still parse. Without
  this, `posix_syntax(true)` restricts patterns to bare POSIX ERE escape
  syntax, which conflates a *syntax-dialect* difference with the
  disambiguation-policy one this project wants to isolate -- see the
  "verified limitation" below for what it does not fix.
- `case_insensitive` -> `set_case_sensitive(false)`, exposed as a separate
  option because RE2's `(?i)` inline group is rejected outright in
  `posix_syntax` mode (there is no way to write it into the pattern text
  itself in that mode).
- `set_log_errors(false)` always: RE2 logs syntax/execution errors to
  stderr by default on top of returning them from `error()`; since callers
  here already surface `error()` through `error_out`, the internal logging
  is turned off to avoid the same message printing twice.

## Matching-semantics mapping

This project's parsers, and the corpus they run against, work with two
distinct match semantics (see [docs/SUBSTRING_SEARCH.md](SUBSTRING_SEARCH.md)):
whole-string membership (`parse_pattern`, what the CLI uses) and unanchored
substring search (`parse_dataset_pattern`/`parse_pcre_rule`, what the
dataset corpus uses, since real IDS/spam-filter/validator rules fire on
seeing a match anywhere in the input). Both wrappers expose the matching
external-engine equivalent directly, rather than re-deriving it from a
padded pattern the way this project's own frontend does:

| This project | `regex` crate | RE2 |
|---|---|---|
| `parse_pattern` (anchored, whole-string) | `RustRegex::full_match` (`Regex::find`, checked against the whole span) | `Re2::full_match` (`RE2::FullMatch`) |
| `parse_dataset_pattern`/`parse_pcre_rule` (unanchored search) | `RustRegex::is_match` (`Regex::is_match`) | `Re2::is_match` (`RE2::PartialMatch`) |

`Re2::find` additionally returns the matched byte span (`RE2::Match` with
`nsubmatch = 1`, which fills submatch 0 -- the overall match -- even
without a capturing group in the pattern). This exists because the two
booleans above **cannot** distinguish leftmost-first from POSIX
leftmost-longest: both modes accept exactly the same language, so
`is_match`/`full_match` always agree between them regardless of `posix`.
Only the match span differs -- `src/external/re2.rs`'s
`posix_mode_prefers_longest_match` test demonstrates this directly on
`a|ab` against `"ab"`: leftmost-first reports span `(0, 1)` ("a"), POSIX
leftmost-longest reports `(0, 2)` ("ab").

## What `bench_external.rs` runs

Same corpus sample as `bench_dataset.rs`
(`data/corpus_patterns.txt`, `CORPUS_SAMPLE_SIZE = 60` patterns), same two
probe strings. Three groups:

- `external_corpus_sample_sweep` -- times all five internal parsers plus
  `rust_regex`, `re2_perl` (leftmost-first), and `re2_posix`
  (leftmost-longest) over the sample, using each engine's unanchored-search
  entry point (`is_match`/`Option::is_some()`), matching the corpus's own
  search semantics.
- `external_matcher_sample_sweep` -- the fairer comparison for
  membership-only questions: `regex`/RE2's `is_match` builds no parse
  tree, so this times this crate's own tree-free Boolean matchers
  (`match_deriv`, `match_pderiv`, Chapter 3 of the thesis) against them
  instead of the full parsers above. Only two matchers, not five, since
  membership never distinguishes POSIX from Greedy -- `match_deriv` alone
  stands in for both `deriv_std`/`deriv_bc`, `match_pderiv` alone for both
  `pderiv_std`/`pderiv_bc`. `match_naive` (the unsimplified baseline) is
  deliberately excluded: it hangs on the very first corpus pattern against
  a non-empty probe, verified directly rather than assumed -- the same
  class of blowup `parse_recursive` hits on `(a+aa)*`
  (`docs/DERIV_BC.md`), just triggered here by a realistic pattern rather
  than a synthetic adversarial one.
- `external_agreement_smoke` -- not a timing benchmark. Once, it compares
  this crate's own membership decision (`parse_loop`) against `regex` and
  RE2's on the same sample and reports how often they agree, to stderr.
  POSIX and Greedy always agree with each other on plain membership (a
  yes/no question depends only on the language a pattern denotes, not on
  which policy resolves an ambiguous match) -- so this checks that three
  independently-written frontends parse the corpus's PCRE-ish syntax into
  the *same language*, not a POSIX-vs-Greedy disambiguation comparison.
  Comparing disambiguation directly would require comparing match spans
  across all three engines on inputs where a pattern is genuinely
  ambiguous, which this bench does not currently do.

## A verified limitation: RE2's true POSIX mode rejects most of this corpus

Running the bench against the current corpus sample:

```
bench_external: 60 patterns loaded (0 rejected by `regex`, 0 rejected by
RE2 perl-mode, 33 rejected by RE2 posix-mode -- each excluded from that
engine's own bench only)
```

`regex` and RE2's default mode compile the entire sample; RE2's POSIX mode
rejects roughly half. The cause, confirmed by inspecting the rejected
patterns and RE2's own error messages ("no argument for repetition
operator: ?"): POSIX extended regular expression syntax has no
non-capturing group (`(?:...)`) and no lazy quantifiers (`*?`, `+?`, ...) --
both are Perl/PCRE extensions RE2 disables in `posix_syntax` mode, and
neither has an `Options` flag to re-enable individually the way
`perl_classes`/`word_boundary` cover escape shorthands. Real-world rules in
this corpus use `(?:...)` and lazy quantifiers freely, so a large fraction
simply cannot be expressed in strict POSIX ERE syntax at all -- this is a
syntax-dialect gap, not a disambiguation-policy difference, and rewriting
`(?:...)` to `(...)` is not a safe automatic fix (it changes capture-group
numbering, and this project's frontend and the two external engines would
then disagree on what group indices mean).

This is a genuine finding about comparing "POSIX" as RE2 defines it against
a Perl-syntax corpus, not a bug in the shim -- and it does not affect
`re2_perl` (leftmost-first) or `regex`, both of which parse Perl-style
syntax and both of which are unaffected. A cleaner future comparison would
either restrict the POSIX side to a sub-corpus of patterns that are valid
POSIX EREs to begin with, or accept this rejection rate as an honest
property of the comparison, as this document does.

## What this integration does and does not establish

Does:
- That this project's internal parsers can be benchmarked side-by-side
  with `regex` and RE2 on the same real-world corpus and the same
  matching semantics, with the setup fully reproducible via
  `cargo bench --bench bench_external --features external-engines`.
- That the `regex` crate and RE2 (in both its modes) largely accept the
  same *language* on this corpus as this project's own frontend does
  (the agreement-smoke check), which is itself a useful cross-validation
  of `src/frontend/`'s translation.
- A working, tested demonstration that RE2's `posix`/`longest_match`
  option actually changes disambiguation (the match-span test above), not
  just that it exists.

Does not:
- A timed, apples-to-apples performance comparison of this project's
  parsers against production-grade engines under equivalent optimization
  effort -- `regex` and RE2 are both mature, heavily-optimized C++/Rust
  codebases; this project's parsers are a thesis implementation of an
  academic algorithm. Raw wall-clock numbers from `bench_external` should
  be read with that gap in mind, not as a claim of competitiveness.
  [docs/testing/BENCHMARKS.md](testing/BENCHMARKS.md) covers running it;
  Section "Standardized Benchmarking" of the thesis's evaluation chapter
  names `rebar` as a concrete next step for a more rigorous version of
  this comparison.
- A POSIX-vs-Greedy disambiguation comparison across engines -- as
  explained above, that needs match-span comparison on genuinely ambiguous
  inputs, which `bench_external.rs` does not yet do.
- Submatch/capture-group equivalence -- neither wrapper exposes RE2's or
  `regex`'s capture groups; only whole-match membership and the overall
  match span are compared.

## Extending this

To add a probe input, corpus size, or a third matching mode, edit
`benches/bench_external.rs` directly -- it mirrors `bench_dataset.rs`'s
structure deliberately, so the two stay easy to compare side by side. To
expose more of RE2's API, add the function to `csrc/re2_shim.{h,cpp}` and
its `extern "C"` declaration plus safe wrapper method to
`src/external/re2.rs`; keep new shim functions to the same minimal,
opaque-handle shape as the existing ones.
