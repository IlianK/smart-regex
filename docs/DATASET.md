# Datasets

## Dataset Sources

### Samples

Few sample files picked from original corpus saved under `data/raw/_Samples/`,
one set per source, used by `demo_frontend_categories` to demonstrate
behaviours on real patterns.
Not part of the pipeline; `dataset_prepare` and `dataset_stats` read the
sources' own directories, not `_Samples`.

**Suricata/Snort** — exploit-signature style: hex-escaped literals
(`\x3a` for `:`, `\x28` for `(`), case-insensitive flags, alternation
over obfuscation variants.

```
/Authorization\x3a\s*Basic\s*[a-zA-Z0-9]{255,}==/i
/clsid\s*\x3a\s*\x7B?\s*f6d90f11-9c73-11d3-b32e-00c04f990bb4/si
/eval\x28(String\x2EfromCharCode\x28|[a-z,0-9]{1,20}\x28String\x2EfromCharCode\x28)/i
```

**SpamAssassin** — keyword and evasion detection, plus structural header
validation: drug-name matching, the letter-gap trick spammers use to
dodge keyword filters, and a MIME boundary/UUID format check.

```
/xan[ae]x/i
/l.{0,2}e.{0,2}v.{0,2}i.{0,2}t.{0,2}r.{0,2}a/i
/boundary="[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}"/
```

**RegexLib** — general-purpose validators, not security rules: email
address, US phone number, currency amount.

```
^[\w\.=-]+@[\w\.-]+\.[\w]{2,3}$
((\(\d{3}\) ?)|(\d{3}-))?\d{3}-\d{4}
^\$?\d{1,3}(,?\d{3})*(\.\d{1,2})?$
```

### Full corpus

How to download the full corpus per source, and where to put it.

**Snort/Suricata** — [Proofpoint Emerging Threats Open](https://rules.emergingthreats.net/).

```bash
curl -O https://rules.emergingthreats.net/open/snort-2.9.7.0/emerging.rules.tar.gz
tar xzf emerging.rules.tar.gz -C data/raw/et-full
# -> data/raw/et-full/rules/*.rules
```

Only the `snort-*` tier is downloadable without an account.
`extract_suricata` reads generic `pcre:"..."` / `content:"..."` fields,
so the `snort-*` tier is what this pipeline uses.

Per the ruleset's own header:
SIDs `1–3464` and `100000000–100000908` are GPLv2; SIDs `2000000–2799999`
are BSD-3-clause.

**SpamAssassin** — [github.com/apache/spamassassin](https://github.com/apache/spamassassin/tree/trunk/rules), `rules/` of `trunk`.

```bash
GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 \
  https://github.com/apache/spamassassin data/raw/spamassassin-full
# -> data/raw/spamassassin-full/rules/*.cf
```

Apache-2.0.
The source tarball on spamassassin.apache.org lacks the scored ruleset;
only the git repo has it.

**RegexLib** — [github.com/superhuman/rxxr2](https://github.com/superhuman/rxxr2), a mirror of an academic ReDoS tool that scraped regexlib.com itself.

```bash
GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 \
  https://github.com/superhuman/rxxr2 data/raw/rxxr2-source
# -> data/raw/rxxr2-source/data/input/regexlib-manual-processed.txt
```

MIT-licensed ("Copyright (c) 2016 University of Birmingham").

## Statistics

Run `dataset_stats` (all three sources by default) to print six tables:

| Table | Shows |
|---|---|
| Dataset coverage | per source: files, extracted, accepted / rejected, each with % |
| Accepted by match-intent reliability | `Direct` vs `Explicit ^...$` counts, each with % |
| Rejection reasons | per source: one column per cause (backreference, lookaround, word-boundary, anchor, flag-m, flag-x, other) |
| Anchor shape | per source: one column per shape (unparseable, none, fully-wrapped, start-only, end-only, nested) |
| PCRE flag distribution (all / accepted) | per flag: per-source count, total, effect |

Flags: `--source S` restrict to one source, `--verbose` list each
rejected pattern next to its cause, `--no-flags` suppress the two flag
tables.

## Preparing the dataset

```
cargo run --release --example dataset_prepare -- suricata data/raw/snort/*.rules
cargo run --release --example dataset_prepare -- spamassassin data/raw/spamAssassin/*.cf
cargo run --release --example dataset_prepare -- regexlib data/raw/regexLib/*.txt
```

Each run writes `data/processed/<source>/prepared.jsonl`, replacing the
file.
Each pattern contributes up to `--variants N` (default 5) positives,
plus up to `N` negatives.
Sample size:

```
wc -l data/processed/*/prepared.jsonl
```

## Pipeline

1. `extract` — one raw file's text to `ExtractedRule`s: pattern plus
   source-specific context (`content:` fields for Suricata, rule kind
   for SpamAssassin, description for RegexLib).
2. `prepare::core_regex_tagged` — parse and translate, with two size
   caps and one narrow anchor recovery.
   Rejected patterns stop here.
3. `generate` — one accepted pattern's `Regex` to `Candidate` inputs,
   sampled structurally or from real `content:` fields / a local message
   corpus. Nothing trusted yet.
4. `prepare::verify_all` — check every candidate against the real
   parser, discard what doesn't hold, write survivors as
   `PreparedCase` JSON lines tagged with their `MatchIntent`.
5. `run_pipeline` — drives 1–4 for one source; returns `PipelineReport`.
   `load_prepared_cases` reads the result back for benchmarks.

## Match-intent reliability tiers

A pattern wrapped in `^...$` at its own top level is an explicit
full-match statement; the anchors are provably redundant under
full-string matching, so `core_regex_tagged` strips them before
translation instead of rejecting.
Narrow on purpose: a lone `^` or `$`, or one nested in an `Or` branch or
under a quantifier, is left untouched and rejected by `translate`.

A pattern with no anchor is accepted too, benchmarked as a full-string
match — but that reading is this project's choice, not the source
tool's native convention.
Suricata and SpamAssassin search a buffer by default; RegexLib's
unanchored entries mix forgotten anchors (full-match intended) with
deliberate substring checks.

Every `PreparedCase` records which case applied:

| `MatchIntent` | Meaning |
|---|---|
| `ExplicitFullMatch` | Author's own `^...$` wrap, stripped before translation |
| `UnanchoredAmbiguous` | No anchor; this project's full-match reading |

Positives are sound under either tier: a string in the pattern's own
language is trivially found by a search over that string.
Negatives are not: `sample_negative_late` corrupts a positive's *last*
character, which can leave an earlier shorter match untouched.
`"aaa"` corrupted to `"aa☃"` is a full-match negative for `a+`, but a
false negative under search semantics (since `"aa"` alone still
matches).
Property of `UnanchoredAmbiguous` negatives, not the tier split.

Benchmarks (`bench_dataset.rs`, `demo_parser_agreement`) take the first
30 *distinct patterns* per source, then every verified input under those
30 (`BENCH_PATTERN_LIMIT` to change the cap).
Capping on rows would cover only a handful of patterns; capping on
patterns guarantees 30.