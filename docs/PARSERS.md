# Parsers 

The five `--parser` values, one example call each.
- Full flag/diagnostics reference: [docs/CLI.md](CLI.md). 
- Frontend/syntax reference: [docs/FRONTEND.md](FRONTEND.md)
---


## Standard Derivative-Based Recursive Parser

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser deriv_std_rec
```

Brzozowski derivatives, POSIX leftmost-longest, native recursion
(`mkEps`/`inject`). The default parser. Its recursion depth is bounded by
the input length; `examples/demo_crash.rs` binary-searches the point at
which a long enough input overflows the stack.


## Standard Derivative-Based Iterative Parser

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser deriv_std_loop
```

Identical algorithm to `deriv_std_rec` with the same `mkEps`/`inject` and
POSIX disambiguation, but with an explicit `Vec` in place of native
recursion. Both exist so the two can be compared directly:
`examples/demo_crash.rs` measures how much deeper the loop version
survives on the same input.


## Bitcoded Derivative-Based Parser

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser deriv_bc
```

The same POSIX leftmost-longest result, computed in a single fused
forward pass over a bit-annotated `ARegex`. `deriv_std_rec` and
`deriv_std_loop` compute the same answer in two passes: a forward pass
of successive derivatives, then a backward `inject` pass that
reconstructs the parse tree from those same intermediate expressions.
`deriv_bc` does both in one pass: each derivative step also records, as
bits attached to the expression itself, which alternative was taken or
whether a star took another iteration, so the tree can be read directly
off the final bits instead of replaying a backward pass.


## Standard Partial-Derivative-Based Parser

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser pderiv_std
```

Antimirov partial derivatives with leftmost priority, non-POSIX 
On an ambiguous input this may pick a different parse tree from
the three parsers above (Chapter 6), though it always agrees with them
on *whether* a string matches. The parse tree is reconstructed via
explicit injection closures rather than bit strings. 


## Bitcoded Partial-Derivative-Based Parser

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser pderiv_bc
```

Same Antimirov construction and same non-POSIX answers as `pderiv_std`,
verified byte-identical by `tests/test_pderiv_std.rs`. The two differ in
how the parse tree is reconstructed: `pderiv_std` carries explicit
injection closures alongside each residual, while `pderiv_bc` carries a
bit-vector alongside each residual recording which alternative was taken
or whether a star took another iteration, and reads the tree back out of
those bits in a single pass.


## Compare all parsers

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser all
```

Prints all five side by side, plus three agreement checks:

1. POSIX parsers agree with each other: `deriv_std_rec`, `deriv_std_loop`, `deriv_bc`.
2. Non-POSIX parsers agree with each other: `pderiv_std`, `pderiv_bc`.
3. POSIX vs. non-POSIX agree on tree shape for this input. Tree divergence allowed and expected on an ambiguous pattern. Whether it occurs depends on the pattern and input, but membership must always agree.