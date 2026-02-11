# Development

## Build & Test Commands

```bash
cargo build --release              # Build all crates
cargo build -p <crate>             # Build single crate (pbzx, udif, hfsplus, xara, apfs, dpp, dpp-tool)
cargo test                         # Run all tests (skips gracefully if fixtures missing)
cargo test -p dpp                  # Run integration tests only
cargo test <test_name>             # Run a single test by name
cargo test -- --nocapture          # Show eprintln diagnostic output
cargo run -p dpp-tool -- <cmd>     # Run CLI tool (subcommands: dmg, hfs, pkg, payload, info, bench)
cargo run -p pbzx --example pbzx-tool --release -- <file>   # Run pbzx example
cargo run -p udif --example udif-tool --release -- <cmd>    # Run udif example
cargo bench -p apfs                # Run APFS benchmarks (criterion)
```

## Test Fixtures

Test fixtures live in `tests/` (large binary files: DMGs, raw partitions, PBZX payloads). Tests skip with `eprintln!` warning when fixtures are absent — they do not fail.

## Workspace Conventions

- All crates use **edition 2021**, **MIT license**.
- Apple formats are **big-endian** — `byteorder` is used throughout.
- Each crate has its own `error.rs` with `thiserror`-derived error types.
- Detailed format documentation lives in `<crate>/docs/FORMATS.md`.

## Feature Flags

- **pbzx:** `extract`, `list`, `pack` — all enabled by default.
- **udif:** `extract`, `list`, `create` — all enabled by default.

## CI/CD

`.github/workflows/publish.yml` — publishes to crates.io on `v*` tags using `CARGO_REGISTRY_TOKEN` secret.
