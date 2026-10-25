# Parsers

Five `--parser` values, one example call each.

See [docs/CLI.md](CLI.md) for full flags and diagnostics.
See [docs/TRANSLATE.md](TRANSLATE.md) for frontend and syntax reference.

## Standard Derivative Recursive

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser deriv_std_rec
```

POSIX leftmost-longest. Brzozowski derivatives, native recursion, `mkEps` plus `inject`. The default parser. Recursion depth is bounded by input length, so a long enough input overflows the stack.

## Standard Derivative Iterative

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser deriv_std_loop
```

POSIX leftmost-longest. Same algorithm and same `mkEps` plus `inject` as `deriv_std_rec`, but with an explicit `Vec` in place of native recursion.

## Bitcoded Derivative

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser deriv_bc
```

POSIX leftmost-longest. Single fused forward pass over a bit-annotated `ARegex`.

`deriv_std_rec` and `deriv_std_loop` use two passes. First a forward pass of successive derivatives, then a backward `inject` pass that rebuilds the parse tree from those same intermediate expressions.

`deriv_bc` does both in one pass. Each derivative step records, as bits attached to the expression itself, which alternative was taken or whether a star took another iteration. The tree reads directly off the final bits. No backward replay.

## Standard Partial Derivative

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser pderiv_std
```

Non-POSIX leftmost. Antimirov partial derivatives.

On an ambiguous input this may pick a different parse tree from the three POSIX parsers above (Chapter 6). Membership always agrees.

Tree rebuilt via explicit injection closures, not bit strings.

## Bitcoded Partial Derivative

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser pderiv_bc
```

Non-POSIX leftmost. Same Antimirov construction and same answers as `pderiv_std`.

Differs only in tree reconstruction. `pderiv_std` carries explicit injection closures alongside each residual. `pderiv_bc` carries a bit-vector alongside each residual, recording which alternative was taken or whether a star took another iteration, then reads the tree back from those bits in a single pass.

## Compare All Parsers

```bash
cargo run -- parse "(a|ab)(b|ε)" "ab" --parser all
```

Prints all five side by side plus three agreement checks.

1. POSIX internal agreement among `deriv_std_rec`, `deriv_std_loop`, `deriv_bc`.
2. Non-POSIX internal agreement between `pderiv_std`, `pderiv_bc`.
3. POSIX vs Non-POSIX tree shape on this input. Divergence allowed and expected on an ambiguous pattern. Membership always agrees.