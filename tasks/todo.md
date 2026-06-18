# Todo

- [x] Inspect current binary naming and paste behavior
- [x] Add smart stdin detection to choose copy vs paste
- [x] Rename built binary target to pbi
- [x] Verify build/tests and inspect git diff
- [x] Commit scoped changes

## Review

- `cargo check` passed.
- `cargo test` passed with 2 unit tests.
- `cargo build --bin pbi` produced `target/debug/pbi`.

## Follow-up: Clean Cargo Check Failure

- [x] Reproduce the clean-checkout compile failure.
- [x] Identify missing committed `Cargo.lock` as the dependency-resolution gap.
- [x] Verify a clean checkout with `Cargo.lock`.
- [x] Commit the scoped lockfile fix.

## Follow-up: Lowercase Binary Name

- [x] Update binary target, docs, and error prefix to `pbi`.
- [x] Verify lowercase build target.
- [x] Commit the scoped lowercase rename.

## Follow-up: README Icon

- [x] Create a cutesy project icon asset.
- [x] Add the icon to the README.
- [x] Verify the asset and README reference.
- [x] Commit the scoped icon update.

## Follow-up: Cargo Run Warnings

- [x] Reproduce/check the cargo warning path without exposing pasteboard output.
- [x] Remove the deprecated `Error::description` override.
- [x] Declare the legacy `cargo-clippy` cfg value expected by `objc` macros.
- [x] Verify warning-clean cargo checks and run.
- [x] Commit the scoped warning cleanup.
