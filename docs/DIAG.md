# Diagnostics 

`--diag 0|1|2|3` controls output verbosity for `parse` (and, for 0/1 `match`). 

Full command reference:
- [docs/CLI.md](CLI.md).
- [docs/PARSERS.md](PARSERS.md).


## Level 0 (default, no `--diag` flag)

Prints the boolean result and nothing else.

```bash
cargo run -- parse "a*" "aaa"
true

cargo run -- parse "a*" "aab"
false
```


## Level 1 (`--diag 1`)
Every parser (`deriv_std_rec`/`deriv_std_loop`/`deriv_bc`/`pderiv_bc`/
`pderiv_std`) produces this output at Level 1:
```bash
cargo run -- parse "a*" "aaa" --diag 1
Regex:  a*
Input:  "aaa"
Match:  true
Tree:   [a, a, a]

cargo run -- parse "a*" "aab" --diag 1
Regex:  a*
Input:  "aab"
Match:  false
Error:  position 3: found 'b', expected 'a' or end of input
  aab
    ^
```

The `Tree` on an ambiguous input, between POSIX and GREEDY.** 
Membership (`Match:`) never differs.

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --diag 1 --parser deriv_std_rec
Tree:   (Right (a, b), Right ())        # POSIX: longest match wins

cargo run -- parse "(a|ab)(b|ε)" "ab" --diag 1 --parser pderiv_bc
Tree:   (Left a, Left b)                # GREEDY: leftmost alternative wins

cargo run -- parse "(a|ab)(b|ε)" "ab" --diag 1 --parser pderiv_std
Tree:   (Left a, Left b)                # same GREEDY answer as pderiv_bc, always
```


## Level 2 (`--diag 2`)

At this level structural differences start, since each family 
has to show its own construction mechanism.


### `deriv_std_rec` / `deriv_std_loop`

```bash
cargo run -- parse "a*" "aaa" --diag 2 --parser deriv_std_rec
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

cargo run -- parse "a*" "aab" --diag 2 --parser deriv_std_rec
Match:  false
Steps:  4 derivative expressions computed (2 successful, 1 failed)
Error:  position 3: found 'b', expected 'a' or end of input
  aab
    ^

Partial match: "aa"  (positions 1–2)
Partial tree:  [a, a]  (recovered from last nullable derivative)
```


### `deriv_bc`

```bash
cargo run -- parse "a*" "aaa" --diag 2 --parser deriv_bc
Steps:  3 derivative steps computed

Bit construction:
  internalize(a*) → []@([]@'a')*
  step 1 ('a'): deriv_bc + simp → [false]@([]@'a')*
  step 2 ('a'): deriv_bc + simp → [false, false]@([]@'a')*
  step 3 ('a'): deriv_bc + simp → [false, false, false]@([]@'a')*
  mkEpsBC → bits: [0001]
  decode  → [a, a, a]

cargo run -- parse "a*" "aab" --diag 2 --parser deriv_bc
Match:  false
Partial match: "aa"  (positions 1–2)
Bits so far:   [001]
Last nullable: [false, false]@([]@'a')*  (after step 2)
```

Here there is no separate "expressions".
Only one bit-annotated expression per step, `internalize` up front, `decode` at the end.


### `pderiv_bc`

```bash
cargo run -- parse "a*" "aaa" --diag 2 --parser pderiv_bc
Policy: GREEDY
Steps:  3 pDerivBC steps computed (frontier size at each step: 1 -> 1 -> 1)

Frontier construction:
  step 0: (Star(Lit('a')), [])
  step 1 ('a'): (Star(Lit('a')), [0])
  step 2 ('a'): (Star(Lit('a')), [00])
  step 3 ('a'): (Star(Lit('a')), [000])
  selected (first nullable, priority order) -> bits: [0001]
  decode  -> [a, a, a]

cargo run -- parse "a*" "aab" --diag 2 --parser pderiv_bc
Match:  false
Steps:  3 pDerivBC steps computed (2 with a nullable residual)
Partial match: "aa"  (positions 1–2)
Bits so far:   [001]
```


### `pderiv_std`

```bash
cargo run -- parse "a*" "aaa" --diag 2 --parser pderiv_std
Policy: GREEDY
Steps:  3 pderiv_std steps computed (frontier size at each step: 1 -> 1 -> 1)

Frontier construction: 
  step 0: Star(Lit('a'))
  step 1 ('a'): Seq(Eps, Star(Lit('a')))
  step 2 ('a'): Seq(Eps, Star(Lit('a')))
  step 3 ('a'): Seq(Eps, Star(Lit('a')))
  selected (first nullable, priority order) -> [a, a, a]

cargo run -- parse "a*" "aab" --diag 2 --parser pderiv_std
Match:  false
Partial match: "aa"  (positions 1–2)
Tree so far:   [a, a]
```

The one structural difference from `pderiv_bc`: frontier entries here
carry an `Rc<dyn Fn>` injection closure internally, which has no
printable form, so the trace shows residual regexes only.

In exchange, shows the actual `ParseTree` at nullable points directly
(already computed, no separate decode step), instead of `pderiv_bc`'s
bits.


## Level 3 (`--diag 3`)

Same four approaches as Level 2, but each writing a full structural trace to
a report file instead of stdout.

```bash
cargo run -- parse "(a+b+ab)*" "ab" --diag 3 --parser deriv_bc
cat reports/report.txt
```

The header is the same for every family:

```
-------------------------------====
REGEX ENGINE DEBUG REPORT
-------------------------------====
Timestamp:  ...
Mode:       <parser-specific label>
Regex:      ...
Input:      ...
Result:     MATCH | NO MATCH
Policy:     POSIX | GREEDY
```

Below the header, sections run in this order per family.

**`deriv_std_rec`, `deriv_std_loop`**

1. TIMING
2. FORWARD PASS (Derivatives)
3. NULLABILITY CHECK
4. BACKWARD PASS (mkEps + inject)
5. RESULT *or* PARTIAL RECOVERY
6. ERROR SUMMARY

**`deriv_bc`**

1. TIMING
2. INTERNALIZE
3. FORWARD PASS (deriv_bc + simp)
4. NULLABILITY CHECK
5. MKEPSBC + DECODE
6. RESULT *or* PARTIAL RECOVERY
7. ERROR SUMMARY

**`pderiv_bc`**

1. TIMING
2. INITIAL FRONTIER
3. FORWARD PASS (pDerivBC per residual)
4. SELECTION
5. MKEPSBC + DECODE
6. RESULT *or* PARTIAL RECOVERY
7. ERROR SUMMARY

**`pderiv_std`**

1. TIMING
2. INITIAL FRONTIER
3. FORWARD PASS (pderiv_tree per residual)
4. SELECTION
5. RESULT *or* PARTIAL RECOVERY
6. ERROR SUMMARY
