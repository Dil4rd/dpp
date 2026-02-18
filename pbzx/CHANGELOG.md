# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.3.0] - 2026-02-18

### Changed

- **Breaking:** `CpioReader::extract_all` and `Archive::extract_all` now return `ExtractStats` instead of `Vec<PathBuf>`
- Symlinks are now skipped during extraction instead of being created; the count is reported in `ExtractStats::symlinks_skipped`

### Added

- `ExtractStats` struct with `files`, `dirs`, `symlinks_skipped`, and `bytes` fields
- `CpioReader::extract_path(base_path, dest)` for path-filtered extraction — the base prefix is stripped so only the relative remainder appears under `dest`
- `Archive::extract_path(base_path, dest)` high-level path-filtered extraction

## [0.2.2] - 2026-02-16

### Changed

- Rust edition upgraded from 2021 to 2024

### Fixed

- Clippy fixes for Rust 2024 edition

## [0.2.1] - 2026-02-16

### Fixed

- Clippy warnings: `repeat().take()` → `repeat_n()`, needless borrow in example
- Rustdoc warning: escaped `Vec<u8>` in doc comment

## [0.2.0] - 2026-02-11

### Added

- Parallel XZ chunk decompression via `rayon` behind opt-in `parallel` feature flag
- `PbzxReader::decompress_parallel()` and `decompress_parallel_to()` methods
- `Archive::open()` and `Archive::from_reader()` automatically use parallel decompression when the `parallel` feature is enabled

## [0.1.0] - 2026-02-03

### Added

- PBZX archive reading with XZ stream decompression
- PBZX archive writing (`PbzxWriter`)
- CPIO parsing for odc (`070707`), newc (`070701`), and crc (`070702`) formats
- CPIO building for odc and newc formats (`CpioBuilder`)
- Seek-based header parsing for fast file listing
- Streaming extraction to filesystem
- Feature flags: `extract`, `list`, `pack`
