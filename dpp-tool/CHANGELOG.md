# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.5.4] - 2026-09-04

### Changed

- Inherits the format-crate fixes in this release: correct reads of sparse APFS
  files, and failure rather than silent truncation for HFS+ files whose extents
  end early
- Internal cleanup for the lints added in clippy 1.98

## [0.5.3] - 2026-04-12

### Changed

- Include `LICENSE` file in the published crate
- Bumped workspace dependencies (dpp 0.4.2, udif 0.3.4, hfsplus 0.2.4, apfs 0.2.4, xara 0.3.2, pbzx 0.3.1)

## [0.5.2] - 2026-02-21

### Fixed

- Use safe arithmetic, idiomatic string splitting, and justified expects

## [0.5.1] - 2026-02-18

### Changed

- Symlinks are now skipped during extraction (with a warning) instead of being created; extraction logic moved to library crates (`dpp`, `xara`, `pbzx`)

## [0.5.0] - 2026-02-17

### Added

- `extract` subcommand for `fs`, `hfs`, `apfs`, `pkg`, and `payload` layers — extract files and directories to a local directory
  - `dpp-tool fs extract <dmg> [path] -o <dir>` — auto-detect HFS+/APFS and extract
  - `dpp-tool hfs extract <dmg> [path] -o <dir>` — extract from HFS+ volume
  - `dpp-tool apfs extract <dmg> [path] -o <dir>` — extract from APFS volume
  - `dpp-tool pkg extract <dmg> <pkg_path> [path] -o <dir>` — extract XAR archive contents (optionally scoped to a subtree)
  - `dpp-tool payload extract <dmg> <pkg_path> <component> [path] -o <dir>` — extract CPIO payload contents (optionally scoped to a subtree)
- Path sanitization in all extract commands to prevent directory traversal attacks

## [0.4.3] - 2026-02-17

### Added

- `parallel` feature (enabled by default) for multi-threaded UDIF and PBZX decompression via rayon

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
