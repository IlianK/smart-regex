# Tests

- **Unit tests** - `#[cfg(test)] mod tests` inside each `src/` file, test private functions
- **Integration tests** - `tests/` directory, public API only


```bash
# All tests (unit + integration)
cargo test

# Unit tests only (src/)
cargo test --lib

# Integration tests only (tests/)
cargo test --tests

# One integration file
cargo test --test test_matchers
cargo test --test test_deriv
cargo test --test test_deriv_bc
cargo test --test test_pderiv_bc

# One test by name (substring match)
cargo test parse_recursive_paper_r1_on_ab
cargo test parse_recursive_paper_r2_on_ab
cargo test bitcoded_agrees_on_paper_r2_ab
cargo test loop_traced_inject_steps_are_in_forward_order
cargo test recursive_traced_agrees_with_loop_traced

# Unit tests for a specific module
cargo test --lib nullable::tests
cargo test --lib simplify::annotated::tests
cargo test --lib mk_eps_bc::tests
cargo test --lib decode::tests
cargo test --lib internalize::tests
cargo test --lib parse::tests

# Print output even for passing tests
cargo test -- --nocapture

# List all tests without running
cargo test -- --list
```
