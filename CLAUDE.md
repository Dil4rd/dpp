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

If a change touches a format parser, also run the fixture tests. They are `#[ignore]`d, so neither `cargo test` nor CI runs them — a green pipeline says nothing about whether real images still parse:

```bash
cargo test -p apfs -- --ignored     # likewise hfsplus, udif, dpp
```

`dpp-python` is excluded from checks 3 and 4 because they run without Python headers. It is **not** excluded from clippy — do not add `--exclude dpp-python` to check 2. Its exhaustive `ApfsError` match is what catches a new error variant that has no Python mapping.

The bindings are covered by the `python` CI job, which builds the extension module with maturin, installs the wheel and runs `dpp-python/tests` on the oldest and newest supported Python. To reproduce it locally:

```bash
python3 -m venv /tmp/dppvenv && /tmp/dppvenv/bin/pip install maturin pytest
/tmp/dppvenv/bin/maturin build --out /tmp/dppdist --manifest-path dpp-python/Cargo.toml
/tmp/dppvenv/bin/pip install --no-index --find-links /tmp/dppdist dpp-py
/tmp/dppvenv/bin/pytest dpp-python/tests
RUSTDOCFLAGS="-D warnings" cargo doc -p dpp-python --no-deps   # needs the headers
```

Do not rely on the PyPI publish workflow for verification. It builds wheels and publishes them; it runs no tests, only fires on push to `main`, and only when the version differs from what is on PyPI.

The toolchain is pinned in `rust-toolchain.toml`, so these commands use the same compiler as CI.

## Coding Guidelines

See **[Coding](.claude/docs/CODING.md)** for full coding standards, safety rules, and error handling conventions.

## Deeper Documentation

- **[Architecture](.claude/docs/ARCHITECTURE.md)** — Crate diagram, data flow, design patterns, crate responsibilities.
- **[Coding](.claude/docs/CODING.md)** — Coding standards, safety rules, error handling conventions.
- **[Development](.claude/docs/DEVELOPMENT.md)** — Full build/test/run commands, test fixtures, workspace conventions, CI/CD.
