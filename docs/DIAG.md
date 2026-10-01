# Diagnostics 

`--diag 0|1|2|3` controls output verbosity for `parse` (and, for 0/1,
`match`). This doc shows real, verified output for every level, both
success and failure, and states plainly where the output is identical
across parsers and where it genuinely differs. Full command reference:
[docs/CLI.md](CLI.md). Parser-by-parser reference:
[docs/PARSERS.md](PARSERS.md).

**One asymmetry worth knowing up front:** `match` sets exit code 1 on a
failed match, at every `--diag` level. `parse` does not -- it always
exits 0, regardless of whether the input matched, at every level. This
is existing, verified behavior (checked by running `echo $?` after a
failing `parse` at diag 0), not something this doc introduces or a
Level-specific quirk -- if you're scripting against exit codes, only
`match`'s is meaningful.

---

## Level 0 (default, no `--diag` flag): identical across every parser

Prints the boolean result and nothing else.

```
$ regex-engine parse "a*" "aaa"
true

$ regex-engine parse "a*" "aab"
false
```

Same one-line shape for all five parsers -- there's nothing here that
*could* differ except the value itself.

---

## Level 1 (`--diag 1`): same fields for every parser, tree *value* can differ

```
$ regex-engine parse "a*" "aaa" --diag 1
Regex:  a*
Input:  "aaa"
Match:  true
Tree:   [a, a, a]

$ regex-engine parse "a*" "aab" --diag 1
Regex:  a*
Input:  "aab"
Match:  false
Error:  position 3: found 'b', expected 'a' or end of input
  aab
    ^
```

Every parser (`deriv_rec`/`deriv_loop`/`deriv_bc`/`pderiv_bc`/
`pderiv_standard`) produces this exact field layout at Level 1 -- it
dispatches generically through `ParserType::parser()`, with no
parser-specific code in the renderer at all.

**What genuinely differs at this level: the `Tree` *value*, on an
ambiguous input, between POSIX and GREEDY.** Membership (`Match:`) never
differs; which parse tree wins can:

```
$ regex-engine parse "(a|ab)(b|ε)" "ab" --diag 1 --parser deriv_rec
Tree:   (Right (a, b), Right ())        # POSIX: longest match wins

$ regex-engine parse "(a|ab)(b|ε)" "ab" --diag 1 --parser pderiv_bc
Tree:   (Left a, Left b)                # GREEDY: leftmost alternative wins

$ regex-engine parse "(a|ab)(b|ε)" "ab" --diag 1 --parser pderiv_standard
Tree:   (Left a, Left b)                # same GREEDY answer as pderiv_bc, always
```

`deriv_rec`/`deriv_loop`/`deriv_bc` always agree with each other (proven
POSIX-equivalent). `pderiv_bc`/`pderiv_standard` always agree with each
other (verified byte-identical, 20,000-case fuzzer) and always agree
with the POSIX three on *whether* something matches -- only, as above,
possibly on *which* tree.

---

## Level 2 (`--diag 2`): four distinct shapes, one per construction family

This is where real structural differences start -- each family shows
its own construction mechanism.

### `deriv_rec` / `deriv_loop` -- single expression, mkEps + inject

```
$ regex-engine parse "a*" "aaa" --diag 2 --parser deriv_rec
Regex:  a*
Input:  "aaa"
Policy: POSIX
Match:  true
Tree:   [a, a, a]
Time:   0.028ms
Steps:  4 derivative expressions computed

Construction steps:
  mkEps(r3) → Right Right ((), [])
  inject(a*, 'a', Right Right ((), [])) → Right ((), [a])
  inject(a*, 'a', Right ((), [a])) → ((), [a, a])
  inject(a*, 'a', ((), [a, a])) → [a, a, a]
```

```
$ regex-engine parse "a*" "aab" --diag 2 --parser deriv_rec
Match:  false
Steps:  4 derivative expressions computed (2 successful, 1 failed)
Error:  position 3: found 'b', expected 'a' or end of input
  aab
    ^

Partial match: "aa"  (positions 1–2)
Partial tree:  [a, a]  (recovered from last nullable derivative)
```

`deriv_loop` is byte-identical to `deriv_rec` at this level except the
`Time:` line (real measured time, always differs run to run) -- verified
by diffing both outputs directly.

### `deriv_bc` -- single fused pass over a bit-annotated expression

```
$ regex-engine parse "a*" "aaa" --diag 2 --parser deriv_bc
Steps:  3 derivative steps computed

Bit construction:
  internalize(a*) → []@([]@'a')*
  step 1 ('a'): deriv_bc + simp → [false]@([]@'a')*
  step 2 ('a'): deriv_bc + simp → [false, false]@([]@'a')*
  step 3 ('a'): deriv_bc + simp → [false, false, false]@([]@'a')*
  mkEpsBC → bits: [0001]
  decode  → [a, a, a]
```

```
$ regex-engine parse "a*" "aab" --diag 2 --parser deriv_bc
Match:  false
Partial match: "aa"  (positions 1–2)
Bits so far:   [001]
Last nullable: [false, false]@([]@'a')*  (after step 2)
```

Note: no separate "expressions" list the way `deriv_rec`/`deriv_loop`
have -- one bit-annotated expression per step, `internalize` up front,
`decode` at the end.

### `pderiv_bc` -- a whole *frontier* per step, with bits

```
$ regex-engine parse "a*" "aaa" --diag 2 --parser pderiv_bc
Policy: GREEDY
Steps:  3 pDerivBC steps computed (frontier size at each step: 1 -> 1 -> 1)

Frontier construction:
  step 0: (Star(Lit('a')), [])
  step 1 ('a'): (Star(Lit('a')), [0])
  step 2 ('a'): (Star(Lit('a')), [00])
  step 3 ('a'): (Star(Lit('a')), [000])
  selected (first nullable, priority order) -> bits: [0001]
  decode  -> [a, a, a]
```

```
$ regex-engine parse "a*" "aab" --diag 2 --parser pderiv_bc
Match:  false
Steps:  3 pDerivBC steps computed (2 with a nullable residual)
Partial match: "aa"  (positions 1–2)
Bits so far:   [001]
```

### `pderiv_standard` -- same frontier idea, no bits (not printable)

```
$ regex-engine parse "a*" "aaa" --diag 2 --parser pderiv_standard
Policy: GREEDY
Steps:  3 pderiv_standard steps computed (frontier size at each step: 1 -> 1 -> 1)

Frontier construction (residuals only -- injections aren't printable):
  step 0: Star(Lit('a'))
  step 1 ('a'): Seq(Eps, Star(Lit('a')))
  step 2 ('a'): Seq(Eps, Star(Lit('a')))
  step 3 ('a'): Seq(Eps, Star(Lit('a')))
  selected (first nullable, priority order) -> [a, a, a]
```

```
$ regex-engine parse "a*" "aab" --diag 2 --parser pderiv_standard
Match:  false
Partial match: "aa"  (positions 1–2)
Tree so far:   [a, a]
```

The one structural difference from `pderiv_bc`: frontier entries here
carry an `Rc<dyn Fn>` injection closure internally, which has no
printable form, so the trace shows residual regexes only -- and, in
exchange, shows the actual `ParseTree` at nullable points directly
(already computed, no separate decode step), instead of `pderiv_bc`'s
bits.

---

## Level 3 (`--diag 3`): the same four shapes, in much more detail

Same four families as Level 2, each with its own report structure
(section headings shown below; full multi-page dumps are in
`docs/CLI.md`'s Level 3 section, not repeated here):

| Family | Sections |
|---|---|
| `deriv_rec`/`deriv_loop` | TIMING → FORWARD PASS (Derivatives) → NULLABILITY CHECK → BACKWARD PASS (mkEps + inject) → RESULT *or* PARTIAL RECOVERY → ERROR SUMMARY |
| `deriv_bc` | TIMING → INTERNALIZE → FORWARD PASS (deriv_bc + simp) → NULLABILITY CHECK → MKEPSBC + DECODE → RESULT *or* PARTIAL RECOVERY → ERROR SUMMARY |
| `pderiv_bc` | TIMING → INITIAL FRONTIER → FORWARD PASS (pDerivBC per residual) → SELECTION → MKEPSBC + DECODE → RESULT *or* PARTIAL RECOVERY → ERROR SUMMARY |
| `pderiv_standard` | TIMING → INITIAL FRONTIER → FORWARD PASS (pderiv_tree per residual) → SELECTION → RESULT *or* PARTIAL RECOVERY → ERROR SUMMARY |

Every family's report header is otherwise identical in shape:

```
REGEX ENGINE DEBUG REPORT
Timestamp:  ...
Mode:       <parser-specific label>
Regex:      ...
Input:      ...
Result:     MATCH | NO MATCH
Policy:     POSIX | GREEDY
```

`deriv_loop` vs. `deriv_rec` at Level 3: identical except the `Mode:`
label and the `Parse time:` line -- confirmed by diffing full reports
for both against the same input (`docs/CLI.md`'s own verification
recipe). `pderiv_bc` has one section `pderiv_standard` doesn't
(`MKEPSBC + DECODE`) since `pderiv_standard` never has bits to decode --
its `SELECTION` section produces the final tree directly.

---
