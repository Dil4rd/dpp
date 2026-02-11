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

## Coding Guidelines

- **Tests required:** Every code change must include or update relevant tests. Run `cargo test` before considering work complete. If test fixtures are missing, verify the code compiles and any new unit tests pass.
- **Docs required:** Update `CHANGELOG.md` in affected crates for user-visible changes. Update `<crate>/docs/FORMATS.md` if binary format parsing changes.
- **No unsafe code** in hfsplus and apfs crates. Avoid `unsafe` elsewhere unless strictly necessary with a justifying comment.
- **Error handling:** Use `thiserror` derive in each crate's `error.rs`. Propagate errors with `?` — do not `unwrap()` or `expect()` in library code.
- **Generic I/O:** Keep reader types generic over `R: Read + Seek`. No trait objects — fully monomorphized.
- **Big-endian:** Apple formats are big-endian. Use `byteorder` for all binary reads/writes.

## Deeper Documentation

- **[Architecture](.claude/docs/ARCHITECTURE.md)** — Crate diagram, data flow, design patterns, crate responsibilities.
- **[Development](.claude/docs/DEVELOPMENT.md)** — Full build/test/run commands, test fixtures, workspace conventions, CI/CD.
