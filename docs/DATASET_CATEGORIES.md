# Best / Neutral / Worst

Reference for `src/data/generate.rs` (candidate generation) and
`src/data/prepare.rs` (verification). See [DATASETS.md](DATASETS.md) for
sources and download instructions, [BENCHMARKS.md](BENCHMARKS.md) for how
a category is selected at bench time.

Throughout, `N` is `--variants`, default 5. Every pattern contributes up
to `N` verified inputs per category, never padded with duplicates. 

A pattern with fewer than `N` distinct samples available contributes
however many actually exist.


## The three categories

- **Best**: Short, unambiguous match. Baseline cost, no padding or
  ambiguity inflation.
- **Neutral**: Ordinary-length match, no engineered pathology.
- **Worst**: Heavily-iterated match (genuine ambiguity or length
  cost), or a negative that only fails after consuming most of the
  input.


## Pipeline order

1. `extract.rs` reads one raw source file and returns `ExtractedRule`
   values: the pattern text, plus source-specific context (Suricata
   `content:` fields, SpamAssassin rule kind, RegexLib description).
2. For each `ExtractedRule`:
   1. `prepare::core_regex` parses and translates the pattern. The
      pattern is rejected here if it is structurally too deep or too
      large to translate safely (see "Safety bounds" below).
   2. `generate.rs` produces `Candidate` values from the translated
      pattern, together with the source-specific extras.
   3. `prepare::verify_all` checks each candidate against the real
      parser. Candidates whose claimed outcome does not hold are
      discarded
   4. The surviving candidates are appended to
      `data/processed/<source>/prepared.jsonl`.
3. Until step 2.3 completes, a candidate produced by `generate.rs` is not trusted.
   Verification establishes that the outcome matches the parser's own
   result. It does not establish that the input makes sense / is representative of
   how the pattern is used in practice.


## Best: `generate_best`

1. Draw `4 × N` samples, each walking the pattern's structure with at
   most 1 iteration per `Star` (`max_star_iters = 1`).
2. Deduplicate by exact text.
3. Sort the distinct survivors by length, ascending.
4. Keep the shortest `N`.
5. If fewer than `N` distinct samples exist (e.g. pattern `a` only ever
   matches `"a"`), keep however many actually exist.


## Neutral: `generate_neutral`

1. Draw up to `3 × N` samples, stopping early once `N` distinct ones are
   found.
2. Each draw independently picks its iteration count, 2 to 4 per `Star`
   — not a fixed number, so repeated draws diversify rather than
   collapsing to the same string.
3. Deduplicate by exact text; keep up to `N`.


## Worst: `generate_worst_structural`

Two independent sub-lists, each capped at `N`, concatenated:

1. **Positives** (ambiguity or length cost):
   1. Draw up to `3 × N` samples with `max_star_iters = 6`.
   2. Deduplicate by exact text; keep up to `N`.
2. **Negatives** (late-failing):
   1. Draw up to `3 × N` fresh positives, same `max_star_iters = 6`.
   2. Corrupt each one's last character to an out-of-alphabet marker
      (`U+2603`), guaranteed not to match the pattern.
   3. Deduplicate by exact text; keep up to `N`.
3. Total Worst candidates per pattern before verification: up to `2 × N`.


## Source-specific extras

**Suricata only**: `generate_from_content_fields`:

1. Join the rule's own `content:` field bytes, in rule order.
2. Between two fields, insert a short filler (first 3 characters of a
   fresh `max_star_iters = 1` structural sample).
3. Repeat up to `3 × N` times, varying only the filler; dedupe; keep up
   to `N`.
4. Category: Worst. Provenance: `ContentDerived`.
5. A rule with 0 or 1 `content:` fields has no filler to vary, so it
   converges on exactly 1 candidate regardless of `N`.


**SpamAssassin only, and only with `--corpus-dir <path>`** —
`generate_from_corpus_dir`:

1. Shuffle the files in the given message directory.
2. Take the first `N` files.
3. Keep only the ones that actually match the rule (via
   `matches_bounded`, not a plain parser call — see "Safety bounds").
4. Category: Neutral. Provenance: `RealCorpus`.
5. Without `--corpus-dir`, this step is skipped entirely; SpamAssassin
   falls back to structural generation like every other source.


## Verification: `prepare::verify_candidate`

Every candidate goes through this before it is trusted, regardless of
which generator produced it.

1. Parse the pattern's real search-padded form (`parse_pcre_rule`) — the
   exact `Regex` a benchmark would run against.
2. Step it against the candidate text via `matches_bounded`.
   `None` (too expensive to verify safely) discards the candidate.
3. Compare the result to the candidate's own `claimed_match`. Mismatch
   discards the candidate.
4. For a non-match:
   1. Measure how many characters were consumed before the match became
      structurally impossible (`failed_after_chars`, against the *core*,
      non-padded pattern — see below).
   2. If the category is Worst and fewer than 2/3 of the input was
      consumed, discard: not a genuine worst case, whatever it claimed.
5. Survivors become `PreparedCase` records: source, pattern, input,
   category, provenance, verified outcome, and (for a non-match) how far
   it got.

`failed_after_chars` deliberately measures against the *core*, non-padded
pattern, not the search-padded one: against a padded regex the leading
`Σ*` would swallow the whole prefix and make the measurement meaningless.


## Safety bounds

Three caps enforced by `prepare`, intended to let the pipeline
run unattended over a full real corpus:

1. `core_regex` rejects a pattern before translating it if its estimated
   nesting depth exceeds `2_000`. Catches a pathological character-class
   range (e.g. `[\--–]`, `-` to en dash, ~8,000 code points) before the
   translation that would expand it is attempted.
2. `core_regex` also rejects a translated pattern with more than
   `20_000` nodes, as a coarser second check.
3. `matches_bounded` / `failed_after_chars` step the pattern's
   bit-coded form one character at a time, checking node count after
   every step. Stepping stops (`None`, candidate discarded) if the
   annotated expression passes `100_000` nodes. This was added after
   running the pipeline against a real RegexLib HTML-tag pattern whose
   annotated form passed a million nodes within ~50 characters despite
   per-step simplification.


## Determinism

`dataset_prepare` seeds its RNG.  Given the same
input files and the same `--variants`, a re-run produces the same
`prepared.jsonl`.