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

## Follow-up: Sixel Terminal Images

- [x] Preserve existing copy/paste and file-output behavior while adding terminal protocol selection.
- [x] Add a Sixel output path for iTerm-compatible terminals.
- [x] Share image conversion logic between terminal protocols.
- [x] Add focused tests for terminal protocol detection.
- [x] Update README terminal display documentation.
- [x] Run formatting, checks, and tests.
- [x] Commit the scoped Sixel support changes.

### Review

- `cargo fmt -- --check` passed.
- `cargo check` passed.
- `cargo test` passed with 14 unit tests.

## Follow-up: SVG Clipboard Support

- [x] Add SVG pasteboard read/write support.
- [x] Add SVG detection tests.
- [x] Verify checks/tests.
- [x] Commit the scoped SVG support.

## Follow-up: Debug SVG HTML Clipboard

- [x] Inspect the current GitHub SVG pasteboard payload.
- [x] Add `--debug` pasteboard diagnostics.
- [x] Resolve SVGs referenced by HTML clipboard fragments.
- [x] Add focused parser tests.
- [x] Verify checks/tests and live clipboard behavior.
- [x] Commit the scoped debug and HTML-SVG fix.

## Follow-up: Publish Dry Run Yanked Dependencies

- [x] Trace why `bytemuck` and `rgb` are locked to yanked versions.
- [x] Update dependency constraints or lockfile with the smallest safe change.
- [x] Verify checks/tests and `cargo publish --dry-run`.
- [x] Document results and commit the scoped dependency fix.

### Review

- `cargo fmt -- --check` passed.
- `cargo check` passed.
- `cargo test` passed with 18 unit tests.
- `cargo publish --dry-run --allow-dirty` passed before commit and no longer warned about yanked `bytemuck` or `rgb` versions.

## Follow-up: crates.io iTerm Sixel Detection

- [x] Inspect the published `pbi v0.1.0` source and installed binary for Sixel support.
- [x] Fix terminal protocol selection so iTerm prefers Sixel even if Kitty environment markers leak in.
- [x] Bump crate metadata for a publishable patch release.
- [x] Verify formatting, tests, install behavior, and publish dry run.
- [x] Commit the scoped fix.

### Review

- The crates.io `pbi v0.1.0` source and installed binary already contain Sixel support.
- The protocol detector incorrectly preferred Kitty when `KITTY_WINDOW_ID` existed, even if `TERM_PROGRAM` identified iTerm.
- `cargo fmt -- --check` passed.
- `cargo check --locked` passed.
- `cargo test --locked` passed with 19 unit tests.
- `cargo install --path . --root /tmp/pbi-local-0.1.1 --force --locked` passed.
- `cargo publish --dry-run --allow-dirty` passed for `pbi v0.1.1`.

## Follow-up: Installed 0.1.1 Still Misses Sixel

- [x] Confirm crates.io advertises `pbi v0.1.1` and inspect installed binary lookup.
- [x] Reproduce the installed binary's terminal protocol decision.
- [x] Add runtime diagnostics for terminal protocol detection.
- [x] Verify the published-install path.
- [x] Commit the scoped fix.

### Review

- With `TERM=xterm-256color` and no `TERM_PROGRAM` or `KITTY_WINDOW_ID`, `pbi --debug` reproduced `terminal_protocol=None`.
- Added `PBI_IMAGE_PROTOCOL=sixel` and `PBI_IMAGE_PROTOCOL=kitty` overrides.
- `--debug` now prints `terminal_protocol`, `PBI_IMAGE_PROTOCOL`, `TERM`, `TERM_PROGRAM`, and `KITTY_WINDOW_ID`.
- The unsupported-terminal message now points at `PBI_IMAGE_PROTOCOL=sixel`.
- Bumped the crate to `0.1.2` for the next crates.io publish.
- `cargo fmt -- --check` passed.
- `cargo check --locked` passed.
- `cargo test --locked` passed with 22 unit tests.
- `cargo install --path . --root /tmp/pbi-local-0.1.2 --force --locked` passed.
- `cargo publish --dry-run --allow-dirty` passed for `pbi v0.1.2`.
- Runtime PTY verification showed `terminal_protocol=Some(Sixel)` with `PBI_IMAGE_PROTOCOL=sixel` despite `TERM=xterm-256color`.

## Follow-up: Help Flag

- [x] Add a `--help`/`-h` code path that exits before clipboard access.
- [x] Add focused argument parser tests.
- [x] Update README usage documentation.
- [x] Verify formatting, checks, and tests.
- [x] Commit the scoped help flag changes.

### Review

- `cargo fmt -- --check` passed.
- `cargo check --locked` passed.
- `cargo test --locked` passed with 24 unit tests.
- `cargo run --locked -- --help` printed usage text and exited successfully.
