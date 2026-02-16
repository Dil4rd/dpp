# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Quick Reference

```bash
cargo build --release              # Build all crates
cargo test                         # Run all tests (skips if fixtures missing)
cargo test -p <crate>              # Test single crate
cargo test -- --nocapture          # Show diagnostic output
cargo run -p dpp-tool -- <cmd>     # CLI tool (dmg, hfs, pkg, payload, info, bench)
```

## Pre-Commit Verification

**Run all four checks before considering any code change complete.** These mirror the CI pipeline and must all pass:

```bash
cargo fmt --all --check                                      # 1. Formatting
cargo clippy --workspace --all-targets -- -D warnings        # 2. Lints (zero warnings)
cargo test --workspace --exclude dpp-python                  # 3. Tests
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --exclude dpp-python  # 4. Docs
```

If a change touches `pbzx` or `dpp` with the `parallel` feature, also run:

```bash
cargo test -p pbzx --features parallel
cargo test -p dpp --features parallel
```

`dpp-python` is excluded because it requires Python headers; it is validated separately by the PyPI publish workflow.

## Coding Guidelines

- **Tests required:** Every code change must include or update relevant tests. Run all four pre-commit checks (see above) before considering work complete. If test fixtures are missing, verify the code compiles and any new unit tests pass.
- **Docs required:** Update `CHANGELOG.md` in **all** affected crates for user-visible changes. Update `README.md` if public API, features, or usage changes. Update `<crate>/docs/FORMATS.md` if binary format parsing changes.
- **Zero warnings policy:** Code must pass `cargo clippy -- -D warnings` and `RUSTDOCFLAGS="-D warnings" cargo doc` with no warnings. Fix warnings immediately rather than suppressing them (use `#[allow(...)]` only when the lint is a false positive, with a justifying comment).
- **No unsafe code** in hfsplus and apfs crates. Avoid `unsafe` elsewhere unless strictly necessary with a justifying comment.
- **Error handling:** Use `thiserror` derive in each crate's `error.rs`. Propagate errors with `?` — do not `unwrap()` or `expect()` in library code.
- **Generic I/O:** Keep reader types generic over `R: Read + Seek`. No trait objects — fully monomorphized.
- **Big-endian:** Apple formats are big-endian. Use `byteorder` for all binary reads/writes.

## Deeper Documentation

- **[Architecture](.claude/docs/ARCHITECTURE.md)** — Crate diagram, data flow, design patterns, crate responsibilities.
- **[Development](.claude/docs/DEVELOPMENT.md)** — Full build/test/run commands, test fixtures, workspace conventions, CI/CD.
