# External engine comparison (`regex` crate + RE2)

The default build has no external regex-engine dependency. 

Comparison with Rust's `regex` crate and Google's RE2 is opt-in via `external-engines`, 
since RE2 requires its system C++ library.


## Setup

RE2 is integrated through a small C++ shim (`csrc/re2_shim.{h,cpp}`).
```bash
    # Debian/Ubuntu
    sudo apt install libre2-dev pkg-config

    # macOS
    brew install re2 pkg-config

    cargo build --features external-engines
    cargo test --features external-engines --lib external::
    cargo bench --bench bench_external --features external-engines
```

## Benchmark

`bench_external.rs` uses the same corpus sample as `bench_dataset.rs`: one
shared `Vec<Entry>` per category, with every parser, matcher, and external
engine processing the same `(pattern, input)` rows.

The external engines are:

- `rust_regex`: Rust's `regex` crate, leftmost-first.
- `re2_perl`: RE2's default leftmost-first mode.
- `re2_posix`: RE2 with `posix_syntax` + `longest_match`, i.e. POSIX
  leftmost-longest.

`re2_posix` rejects roughly half the corpus because POSIX ERE does not support
Perl extensions such as `(?:...)` and lazy quantifiers. Rejected patterns are
skipped inside that engine's timed loop, so all engines iterate the same corpus
slice but `re2_posix` performs roughly half as many actual `is_match` calls.

Therefore, re2_posix wall-clock totals are not directly comparable to the other engines. It attempts is_match only for the subset of patterns it accepts, so its total time partly reflects fewer match operations. Dividing by accepted-entry count gives a useful per-accepted-entry figure, but does not eliminate differences in the workloads being measured.


## Parser vs. matcher comparisons

The benchmark has two distinct Criterion groups:

| Group | Internal side | External side | Purpose |
|---|---|---|---|
| `external_best` / `neutral` / `worst` | Five parsers, each building a full `ParseTree` | `regex`/RE2 `is_match` | Tree construction vs. boolean matching |
| `external_matcher_best` / `neutral` / `worst` | `match_deriv`, `match_pderiv` | Same `is_match` calls | Boolean matching vs. boolean matching |

The second group is the meaningful speed comparison. `regex` and RE2 parse and
compile patterns once, then their match APIs return only boolean/span/capture
information. They do not construct this project's `ParseTree`.




- The three frontends largely accept the same language on the corpus, providing
  an agreement smoke test for `src/frontend/`.
- RE2's POSIX mode changes disambiguation policy: the
  `posix_mode_prefers_longest_match` test confirms that `a|ab` on `"ab"` yields
  `(0,1)` in leftmost-first mode and `(0,2)` in POSIX leftmost-longest mode.

