# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.4.3] - 2026-02-17

### Fixed

- Pipeline Summary in `bench` command now includes PKG and PBZX stages (previously only showed DMG open, extraction, and filesystem walk)

## [0.4.2] - 2026-02-16

### Changed

- Rust edition upgraded from 2021 to 2024
- Updated all workspace dependencies to latest versions

### Fixed

- Clippy fixes for Rust 2024 edition

## [0.4.1] - 2026-02-16

### Added

- CI pipeline: GitHub Actions workflow with formatting, clippy, cross-platform tests (Linux/macOS/Windows), feature-flag tests, and doc build

### Fixed

- Resolved all clippy warnings across the workspace (print_literal, manual_strip, derivable_impls, collapsible_if, and others)
- Fixed rustdoc warnings for unescaped HTML tags in doc comments (pbzx, hfsplus)
- Fixed formatting drift in apfs, xara, and other crates

## [0.4.0] - 2026-02-15

### Added

- Migrated CLI to clap 4 derive — auto-generated help, `--version`, short flags (`-n`, `-t`, `-d`), typo suggestions, and proper validation errors
- `--no-color` global flag to disable ANSI output for scripts and CI
- Automatic terminal detection: ANSI codes are suppressed when stdout is not a terminal (piping to files works cleanly)
- `--depth N` / `-d N` flag for all `tree` subcommands (previously hardcoded to 3)
- Hidden `completions` subcommand for bash/zsh/fish shell completion generation
- `benchmark` alias for the `bench` command

### Changed

- All argument parsing now uses clap derive structs — eliminates ~600 LOC of manual parsing, ~60 hardcoded usage strings, and ~68 `process::exit(1)` calls

### Dependencies

- Added `clap` 4 with derive feature
- Added `clap_complete` 4 for shell completions

## [0.3.2] - 2026-02-12

### Fixed

- DMG block type `0x80000008` now correctly reported as "XZ" instead of "LZVN" in `info` and `dmg info` output
- XZ-compressed DMGs (e.g. Google Chrome) now decompress correctly instead of producing all-zero data

### Changed

- Updated udif dependency to 0.3.0

## [0.3.1] - 2026-02-12

### Added

- Global `--in-memory` / `--temp-file` flags to control partition extraction mode
  - `--temp-file` (default): streams through temp file, low memory (~4 KB)
  - `--in-memory`: buffers partition in RAM, faster for small DMGs

### Changed

- `pkg` and `payload` commands now work on APFS DMGs (previously hard-coded to HFS+ only)
- `info` and `bench` commands refactored to use unified filesystem API (removes duplicated HFS+/APFS code paths)
- `bench` command now includes PKG and PBZX stages for APFS DMGs

## [0.3.0] - 2026-02-11

### Added

- `fs` subcommand: unified filesystem browsing (info, ls, tree, cat, stat, find) with HFS+/APFS auto-detection
- `apfs` subcommand for direct APFS volume access
- APFS support in `info` and `bench` commands

## [0.2.0] - 2026-02-11

### Changed

- Updated dependencies to 0.2.0

## [0.1.0] - 2026-02-10

### Added

- `info` command for DMG/HFS+/PKG summary
- `dmg` command for partition listing
- `hfs` command for filesystem browsing (list, tree, read)
- `pkg` command for PKG inspection
- `payload` command for PBZX payload extraction
- `bench` command for pipeline stage benchmarking
- Styled terminal output with table formatting
