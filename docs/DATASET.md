# Datasets

## Dataset Sources

### Samples

Few sample files picked from original corpus saved under `data/raw/_Samples/`,
one set per source, used by `demo_samples_anchored` / `demo_samples_unanchored` /
`demo_frontend_categories` to demonstrate behaviours on real patterns.
Not part of the pipeline;
`dataset_prepare` and `dataset_stats` read the sources' own directories, not `_Samples`.

**Suricata/Snort**: Exploit-signature style: hex-escaped literals (`\x3a`
for `:`, `\x28` for `(`), case-insensitive flags, alternation over
obfuscation variants.

```
/Authorization\x3a\s*Basic\s*[a-zA-Z0-9]{255,}==/i
/clsid\s*\x3a\s*\x7B?\s*f6d90f11-9c73-11d3-b32e-00c04f990bb4/si
/eval\x28(String\x2EfromCharCode\x28|[a-z,0-9]{1,20}\x28String\x2EfromCharCode\x28)/i
```

**SpamAssassin**: Keyword and evasion detection, plus structural header
validation: simple drug-name matching, the letter-gap trick spammers use
to dodge keyword filters, and a MIME boundary/UUID format check.

```
/xan[ae]x/i
/l.{0,2}e.{0,2}v.{0,2}i.{0,2}t.{0,2}r.{0,2}a/i
/boundary="[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}"/
```

**RegexLib**: General-purpose validators, not security rules: email
address, US phone number, currency amount.

```
^[\w\.=-]+@[\w\.-]+\.[\w]{2,3}$
((\(\d{3}\) ?)|(\d{3}-))?\d{3}-\d{4}
^\$?\d{1,3}(,?\d{3})*(\.\d{1,2})?$
```


### Full corpus

How to download the full corpus per source, and where to put it:

**Snort/Suricata** — [Proofpoint Emerging Threats Open](https://rules.emergingthreats.net/).

```bash
curl -O https://rules.emergingthreats.net/open/snort-2.9.7.0/emerging.rules.tar.gz
tar xzf emerging.rules.tar.gz -C data/raw/et-full
# -> data/raw/et-full/rules/*.rules
```

Only the `snort-*` tier is downloadable without an account; the
`suricata-*` tier is not open. `extract_suricata` reads generic
`pcre:"..."` / `content:"..."` fields and works with either, so the
`snort-*` tier is what this pipeline uses. 

Per the ruleset's own header:
SIDs `1–3464` and `100000000–100000908` are GPLv2; SIDs `2000000–2799999`
are BSD-3-clause.


**SpamAssassin** — [github.com/apache/spamassassin](https://github.com/apache/spamassassin/tree/trunk/rules), `rules/` of `trunk`.

```bash
GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 \
  https://github.com/apache/spamassassin data/raw/spamassassin-full
# -> data/raw/spamassassin-full/rules/*.cf
```

The source tarball on spamassassin.apache.org does *not* include the
scored ruleset, only the git repo has it checked in. Apache-2.0.


**RegexLib** — [github.com/superhuman/rxxr2](https://github.com/superhuman/rxxr2), a mirror of an academic ReDoS tool that scraped regexlib.com itself.

```bash
GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 \
  https://github.com/superhuman/rxxr2 data/raw/rxxr2-source
# -> data/raw/rxxr2-source/data/input/regexlib-manual-processed.txt
```

MIT-licensed ("Copyright (c) 2016 University of Birmingham")


## Statistics

Run `dataset_stats` (all three sources at once by default) to see, per source:

- extraction counts: rules read, patterns extracted
- acceptance: how many patterns the frontend accepts versus rejects
- rejection causes: backreference / lookaround / word-boundary / flag-m / flag-x / other
- faithful vs. approx split: how many accepted patterns denote exactly what the source pattern means (`faithful`) versus carrying a known simplification (`approx` — `R`, or a nested anchor)
- PCRE flag distribution: which flags appear in the corpus, and what each is treated as (honoured / ignored / rejected)
- anchor usage: `^ only`, `$ only`, `^...$`, unanchored

Useful flags: `--source S` restrict to one source, `--verbose` list each rejected pattern next to its cause, `--no-flags` / `--no-anchors` / `--no-caveats` suppress the corresponding table. The `Faithful` row is exactly the set `dataset_prepare` writes to `data/processed/` (the raw corpus also contains `approx` patterns, which `dataset_prepare` skips, but can be included to expand dataset size when compromising on faithfullness).


## Preparing the dataset

```
cargo run --release --example dataset_prepare -- snort data/raw/snort/*.rules
cargo run --release --example dataset_prepare -- spamassassin data/raw/spamAssassin/*.cf
cargo run --release --example dataset_prepare -- regexlib data/raw/regexLib/*.txt
```

Each run writes to `data/processed/<source>/prepared.jsonl`, replacing the file (not appending). Each pattern contributes up to `--variants N` (default 5) independently verified inputs per category.
Sample size:

```
wc -l data/processed/*/prepared.jsonl
```

## Pipeline

1. `extract`: Read one raw rule file's text and produce `ExtractedRule` values: the pattern string plus source-specific context (`content:` fields for Suricata, rule kind for SpamAssassin, description for RegexLib).
2. `generate`: Turn one `ExtractedRule` into `Candidate` inputs, sampled either from the pattern's own structure or (where available) from real `content:` fields or a local message corpus. Nothing trusted yet.
3. `prepare`: Verify every candidate against the real parser, discard what doesn't hold up, and write survivors as `PreparedCase` JSON lines. Holds the safety caps (`MAX_CORE_DEPTH`, `MAX_CORE_NODES`, `MAX_AREGEX_NODES`) that let the pipeline run unattended over a full real corpus.
4. `mod::run_pipeline`: Drives 1 → 2 → 3 for one source and returns a `PipelineReport`; `mod::load_prepared_cases` reads the result back for benchmarks.


## Best/Neutral/Worst

Each pattern contributes up to `--variants` (default 5) distinct, verified
inputs per category.
- **Best**: shortest positive, ≤ 1 `Star` iteration. Baseline cost.
- **Neutral**: ordinary-length positive, 2–4 `Star` iterations. SpamAssassin also samples real matching messages from a local corpus dir (unused by the tracked sample).
- **Worst**: heavily-iterated positive (6 `Star` iterations), or a negative built by corrupting a positive's last character to an out-of-alphabet marker. Worst negatives must fail late (≥ 2/3 of the input consumed) or they are discarded. Suricata also assembles candidates from the rule's own `content:` fields.

Verification checks only that the claimed match/non-match outcome matches
the parser's result, and not that the input makes sense or is representative
of the pattern's real usage.

Benchmarks (`bench_dataset.rs`, `bench_external.rs`) take the first 30
*distinct patterns* per category, not the first 30 rows, and then every
verified input belonging to those 30. Rows per pattern are uneven: one
pattern can contribute up to `3 × --variants` rows, while a short pattern
like `a` contributes 1. Capping on rows would cover only a handful of
distinct patterns; capping on patterns guarantees 30.


See [CATEGORIES.md](CATEGORIES.md) for the full generation and verification
specification.
