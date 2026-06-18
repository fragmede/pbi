# Lessons

## 2026-06-17: Verify Clean Dependency Resolution

- Correction: `cargo check` passed in the working tree because an untracked `Cargo.lock` was present, but a clean checkout resolved newer transitive dependencies and failed under the installed Cargo version.
- Rule: For Rust binary crates, treat `Cargo.lock` as part of the build surface. Before declaring verification complete, test a clean checkout or otherwise confirm that required lockfiles are tracked.

## 2026-06-18: Terminal Capability Detection Needs Overrides

- Correction: Sixel support existed in the installed binary, but a user-overridden `TERM` hid the terminal capability signal and made auto-detection report no supported protocol.
- Rule: When runtime behavior depends on terminal environment variables, add debug output for the exact decision inputs and provide an explicit user override instead of relying only on inferred terminal metadata.
