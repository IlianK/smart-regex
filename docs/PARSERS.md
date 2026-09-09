# Parsers Quick Reference

The five `--parser` values, one example call each. Each has its own
detailed writeup with code references:
[DERIV_STD.md](DERIV_STD.md), [DERIV_BC.md](DERIV_BC.md),
[PDERIV_STD.md](PDERIV_STD.md), [PDERIV_BC.md](PDERIV_BC.md).
- Full flag/diagnostics reference: [docs/CLI.md](CLI.md). 
- Frontend/syntax reference: [docs/FRONTEND.md](FRONTEND.md),
  [docs/SUBSTRING_SEARCH.md](SUBSTRING_SEARCH.md).

---

## Derivative Based

### `deriv_std_rec` -- POSIX, standard recursive

```bash
cargo run -- parse "a*" "aaa" --parser deriv_std_rec
```

Brzozowski derivatives, POSIX leftmost-longest, native recursion
(`mkEps`/`inject`, Fig. 3). The default parser (same as omitting
`--parser` entirely).

### `deriv_std_loop` -- POSIX, standard iterative

```bash
cargo run -- parse "a*" "aaa" --parser deriv_std_loop
```

Identical algorithm and answer to `deriv_std_rec` -- same `mkEps`/`inject`,
same POSIX disambiguation -- with an explicit `Vec` in place of native
recursion. Verified to produce identical derivation traces
(`docs/CLI.md`'s Level 3 diff check).

### `deriv_bc` -- POSIX, bit-coded

```bash
cargo run -- parse "a*" "aaa" --parser deriv_bc
```

Same POSIX answer again, computed via a single fused forward pass over
a bit-annotated `ARegex` instead of `deriv_std_rec`/`deriv_std_loop`'s two-pass
(forward derivative, backward inject) structure.



## Partial Derivative Based

### pderiv_std -- Greedy, standard

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser pderiv_std
```

The same Antimirov construction and the same Greedy answer as
`pderiv_bc` -- verified byte-identical to it on every input
(`tests/test_pderiv_std.rs`'s 20,000-case fuzzer) -- built via
explicit `ParseTree` injection closures instead of bit strings. No
Haskell reference gives this construction; it's this project's own.


### `pderiv_bc` -- Greedy, bit-coded

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser pderiv_bc
```

Antimirov partial derivatives, bit-coded. **Greedy leftmost priority,
not POSIX** -- may pick a different parse tree than the three parsers
above on an ambiguous input (Chapter 6), though it always agrees with
them on *whether* a string matches.

---

## Compare all parsers

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser all
```

Prints all five side by side, plus three agreement checks: the three
POSIX parsers checked for full mutual agreement, `pderiv_bc`/
`pderiv_std` checked against the POSIX parsers on membership only
(tree divergence on ambiguous input is expected there, not a bug), and
`pderiv_bc`/`pderiv_std` checked against *each other* for **exact**
tree equality (since both compute the identical Greedy policy, any
disagreement between those two specifically would be a real bug).


## Add `--diag`

Every value above accepts `--diag 1|2|3` for increasing detail (regex/
input/match/tree and error caret at 1, construction steps at 2, full
derivation trace at 3) -- see `docs/CLI.md` for the complete reference,
including `--diag-report` for redirecting Level 3 output to a file.
