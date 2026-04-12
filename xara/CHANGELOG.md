# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.3.2] - 2026-04-12

### Changed

- Include `LICENSE` file in the published crate

## [0.3.1] - 2026-02-21

### Fixed

- Use safe string slicing in `strip_base_prefix`

## [0.3.0] - 2026-02-18

### Added

- `extract` feature flag (default-on) for filesystem extraction
- `XarArchive::extract_all(dest)` extracts all entries to a directory
- `XarArchive::extract_path(base_path, dest)` extracts entries under a path prefix — the base prefix is stripped from output paths
- `ExtractStats` struct with `files`, `dirs`, `symlinks_skipped`, and `bytes` fields
- `XarError::InvalidPath` variant for path traversal detection
- Symlinks are skipped during extraction (counted in `ExtractStats::symlinks_skipped`)

## [0.2.2] - 2026-02-16

### Changed

- Rust edition upgraded from 2021 to 2024

### Fixed

- Clippy fixes for Rust 2024 edition

## [0.2.1] - 2026-02-16

### Fixed

- Clippy warning: `collapsible_if` in package component detection

## [0.2.0] - 2026-02-11

### Added

- Self-contained unit tests for header parsing, TOC XML parsing, and XAR roundtrip

## [0.1.0] - 2026-02-10

### Added

- XAR archive reading (header, TOC, heap)
- macOS PKG installer parsing (product and component packages)
- Distribution XML parsing
- PackageInfo reading
- Streaming payload extraction
- Gzip and octet-stream heap decompression
