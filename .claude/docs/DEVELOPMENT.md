# Development

## Build & Test Commands

```bash
cargo build --release              # Build all crates
cargo build -p <crate>             # Build single crate (pbzx, udif, hfsplus, xara, apfs, dpp, dpp-tool)
cargo test                         # Run all tests except the #[ignore]d fixture ones
cargo test -p dpp                  # Run integration tests only
cargo test <test_name>             # Run a single test by name
cargo test -- --nocapture          # Show eprintln diagnostic output
cargo test -p apfs -- --ignored    # Run the fixture tests (needs tests/, see below)
cargo run -p dpp-tool -- <cmd>     # Run CLI tool (dmg, fs, hfs, apfs, pkg, payload, info, bench)
cargo run -p dpp-tool -- --in-memory fs info <dmg>  # In-memory extraction mode
cargo run -p pbzx --example pbzx-tool --release -- <file>   # Run pbzx example
cargo run -p udif --example udif-tool --release -- <cmd>    # Run udif example
cargo bench -p apfs                # Run APFS benchmarks (criterion)

# Python bindings (requires maturin + Python 3.9+)
cd dpp-python && maturin develop            # Build and install into current venv
cd dpp-python && maturin develop --release   # Release-mode build
cd dpp-python && python -m pytest tests/     # Run Python tests
python -c "import dpp; print(dir(dpp))"      # Quick smoke test
```

## Test Fixtures

Test fixtures live in `tests/` (large binary files: DMGs, raw partitions, PBZX payloads). The directory is gitignored, so the fixtures exist only on a maintainer's machine.

Every test that needs one is marked `#[ignore]` — around 25 across `apfs`, `hfsplus`, `udif` and `dpp`. That has two consequences worth knowing:

- **`cargo test` does not run them, and neither does CI.** The only tests that exercise the parsers against real images run locally, on request, via `cargo test -p <crate> -- --ignored`. Run them before calling parser work finished; a green CI says nothing about whether an image still parses.
- **They fail rather than skip when fixtures are absent.** They `.unwrap()` on `File::open`, so `--ignored` on a machine without `tests/` panics. Do not add `--include-ignored` to CI without changing that.

This gap is why a comparator bug that broke 13 of 15 symlinks in `tests/appfs.raw` passed every check. Synthetic tests that construct their own input are the only kind CI rewards, so prefer adding both: a unit test CI can run, and an `#[ignore]`d one that proves the behaviour against a real image.

## Workspace Conventions

- All crates use **edition 2024**, **MIT license**.
- The toolchain is pinned in `rust-toolchain.toml`. Bump it deliberately, in its own commit, fixing any newly-introduced lints there — otherwise a Rust release turns CI red on code nobody touched, and it lands on whoever opens the next pull request.
- Apple formats are **big-endian** — `byteorder` is used throughout.
- Each crate has its own `error.rs` with `thiserror`-derived error types.
- Detailed format documentation lives in `<crate>/docs/FORMATS.md`.

## Feature Flags

- **pbzx:** `extract`, `list`, `pack` — all enabled by default.
- **udif:** `extract`, `list`, `create` — all enabled by default.

## CI/CD

`.github/workflows/ci.yml` — fmt, clippy, tests on three OSes, the `parallel` feature, and docs. Runs on `main` and `dev`, for pushes and pull requests.

`.github/workflows/publish.yml` — publishes to crates.io **on every push to `main`**, using the `CARGO_REGISTRY_TOKEN` secret. There is no tag step: merging to `main` publishes.

## Release Flow

Work lands on `dev`, which merges to `main` only when a release is intended.

- Crates being changed carry a `-dev` suffix on `dev` (`0.3.0-dev`), so a tree that differs from what was published never claims the published version number. Internal dependency pins carry it too.
- The release commit strips every `-dev` and dates the `[Unreleased]` changelog sections. It must be the **last** commit before merging to `main`, because the merge publishes immediately.
- `publish.yml` refuses to run if any `Cargo.toml` still contains a `-dev` version, so a mis-ordered merge fails instead of burning a version number on crates.io permanently.
