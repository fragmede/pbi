# Todo

- [x] Inspect current binary naming and paste behavior
- [x] Add smart stdin detection to choose copy vs paste
- [x] Rename built binary target to PBI
- [x] Verify build/tests and inspect git diff
- [x] Commit scoped changes

## Review

- `cargo check` passed.
- `cargo test` passed with 2 unit tests.
- `cargo build --bin PBI` produced `target/debug/PBI`.

## Follow-up: Clean Cargo Check Failure

- [x] Reproduce the clean-checkout compile failure.
- [x] Identify missing committed `Cargo.lock` as the dependency-resolution gap.
- [x] Verify a clean checkout with `Cargo.lock`.
- [x] Commit the scoped lockfile fix.
