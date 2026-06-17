# Lessons

## 2026-06-17: Verify Clean Dependency Resolution

- Correction: `cargo check` passed in the working tree because an untracked `Cargo.lock` was present, but a clean checkout resolved newer transitive dependencies and failed under the installed Cargo version.
- Rule: For Rust binary crates, treat `Cargo.lock` as part of the build surface. Before declaring verification complete, test a clean checkout or otherwise confirm that required lockfiles are tracked.
