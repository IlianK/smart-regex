# Diagnostics

`--diag 0|1|2|3` controls output verbosity for `parse` and, for 0 and 1, `match`.

Full command reference in [docs/CLI.md](CLI.md) and [docs/PARSERS.md](PARSERS.md).



## Level 0

Default. No `--diag` flag. Prints the boolean result and nothing else.

**Match**

```bash
cargo run -- parse "a*" "aaa"
true
```

**No match**

```bash
cargo run -- parse "a*" "aab"
false
```



## Level 1

Every parser produces this output. Match first, then no match, then the ambiguous-input tree comparison.

**Match**

```bash
cargo run -- parse "a*" "aaa" --diag 1
Regex:  a*
Input:  "aaa"
Match:  true
Tree:   [a, a, a]
```

**No match**

```bash
cargo run -- parse "a*" "aab" --diag 1
Regex:  a*
Input:  "aab"
Match:  false
Error:  position 3: found 'b', expected 'a' or end of input
  aab
    ^
```

### Tree comparison on an ambiguous input

The `Tree` value may differ between POSIX and Non-POSIX parsers.

**POSIX**

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --diag 1 --parser deriv_std_rec
Tree:   (Right (a, b), Right ())        # longest match wins
```

**Non-POSIX**

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --diag 1 --parser pderiv_bc
Tree:   (Left a, Left b)                # leftmost alternative wins

cargo run -- parse "(a|ab)(b|ε)" "ab" --diag 1 --parser pderiv_std
Tree:   (Left a, Left b)                # same answer as pderiv_bc
```



## Level 2

Each parser family shows its own construction mechanism. Match and no match cases are separated per family.

### deriv_std_rec and deriv_std_loop

**Match**

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
```

**No match**

```bash
cargo run -- parse "a*" "aab" --diag 2 --parser deriv_std_rec
Match:  false
Steps:  4 derivative expressions computed (2 successful, 1 failed)
Error:  position 3: found 'b', expected 'a' or end of input
  aab
    ^

Partial match: "aa"  (positions 1-2)
Partial tree:  [a, a]  (recovered from last nullable derivative)
```

### deriv_bc

**Match**

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
```

**No match**

```bash
cargo run -- parse "a*" "aab" --diag 2 --parser deriv_bc
Match:  false
Partial match: "aa"  (positions 1-2)
Bits so far:   [001]
Last nullable: [false, false]@([]@'a')*  (after step 2)
```

The trace holds one bit-annotated expression per step. `internalize` runs once before step 1. `decode` runs once after the last step.

### pderiv_bc

**Match**

```bash
cargo run -- parse "a*" "aaa" --diag 2 --parser pderiv_bc
Policy: Non-POSIX
Steps:  3 pDerivBC steps computed (frontier size at each step: 1 -> 1 -> 1)

Frontier construction:
  step 0: (Star(Lit('a')), [])
  step 1 ('a'): (Star(Lit('a')), [0])
  step 2 ('a'): (Star(Lit('a')), [00])
  step 3 ('a'): (Star(Lit('a')), [000])
  selected (first nullable, priority order) -> bits: [0001]
  decode  -> [a, a, a]
```

**No match**

```bash
cargo run -- parse "a*" "aab" --diag 2 --parser pderiv_bc
Match:  false
Steps:  3 pDerivBC steps computed (2 with a nullable residual)
Partial match: "aa"  (positions 1-2)
Bits so far:   [001]
```

### pderiv_std

**Match**

```bash
cargo run -- parse "a*" "aaa" --diag 2 --parser pderiv_std
Policy: Non-POSIX
Steps:  3 pderiv_std steps computed (frontier size at each step: 1 -> 1 -> 1)

Frontier construction:
  step 0: Star(Lit('a'))
  step 1 ('a'): Seq(Eps, Star(Lit('a')))
  step 2 ('a'): Seq(Eps, Star(Lit('a')))
  step 3 ('a'): Seq(Eps, Star(Lit('a')))
  selected (first nullable, priority order) -> [a, a, a]
```

**No match**

```bash
cargo run -- parse "a*" "aab" --diag 2 --parser pderiv_std
Match:  false
Partial match: "aa"  (positions 1-2)
Tree so far:   [a, a]
```

Frontier entries carry an `Rc<dyn Fn>` injection closure. The closure has no printable form. The trace shows residual regexes only.

The trace shows the `ParseTree` directly at nullable points. No separate decode step.



## Level 3

Level 3 writes a full structural trace to a report file. Same four parsers as Level 2.

```bash
cargo run -- parse "(a+b+ab)*" "ab" --diag 3 --parser deriv_bc
cat reports/diag/report.txt
```

Every parser shares this header.

```
-------------------------------====
REGEX ENGINE DEBUG REPORT
-------------------------------====
Timestamp:  ...
Mode:       <parser-specific label>
Regex:      ...
Input:      ...
Result:     MATCH | NO MATCH
Policy:     POSIX | Non-POSIX
```

Below the header, sections run in this order (shared by all four parsers).

1. TIMING                                    (all four)
2. INTERNALIZE / INITIAL FRONTIER            (deriv_bc / pderiv_bc, pderiv_std)
3. FORWARD PASS                              (all four, label varies)
4. NULLABILITY CHECK / SELECTION             (deriv_std, deriv_bc / pderiv_bc, pderiv_std)
5. BACKWARD PASS / MKEPSBC + DECODE          (deriv_std / deriv_bc, pderiv_bc)
6. RESULT or PARTIAL RECOVERY                (all four)
7. ERROR SUMMARY                             (all four)

At Level 3, `deriv_std_loop` and `deriv_std_rec` produce identical reports
(excluding `Mode` and `Parse time`).

```bash
cargo run -- parse "a*" "aaa" --diag 3 --diag-report reports/diag/rec.txt
cargo run -- parse "a*" "aaa" --parser deriv_std_loop --diag 3 --diag-report reports/diag/loop.txt
diff reports/diag/rec.txt reports/diag/loop.txt
```