# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.4.2] - 2026-04-12

### Changed

- Include `LICENSE` file in the published crate
- Bumped udif to 0.3.4, pbzx to 0.3.1, hfsplus to 0.2.4, apfs to 0.2.4, xara to 0.3.2

## [0.4.1] - 2026-02-21

### Fixed

- Use safe string slicing in `strip_base_prefix`

## [0.4.0] - 2026-02-18

### Added

- `extract` feature flag (default-on) for filesystem extraction
- `FilesystemHandle::extract_all(dest)` extracts all entries to a directory
- `FilesystemHandle::extract_path(base_path, dest)` extracts entries under a path prefix — the base prefix is stripped from output paths
- `ExtractStats` struct with `files`, `dirs`, `symlinks_skipped`, and `bytes` fields
- `DppError::InvalidPath` and `DppError::NoEntriesFound` variants
- Symlinks are skipped during extraction (counted in `ExtractStats::symlinks_skipped`)

## [0.3.5] - 2026-02-17

### Changed

- `parallel` feature now enables both `pbzx/parallel` (XZ chunk decompression) and `udif/parallel` (DMG block decompression) for full parallel extraction
- Updated udif dependency to 0.3.3

## [0.3.4] - 2026-02-16

### Added

- Synthetic full-pipeline integration tests (DMG → HFS+ → PKG → PBZX → CPIO) that run without external fixtures
- Tests for `find_packages()`, filesystem auto-detection, and `.pkg` payload extraction

### Changed

- Rust edition upgraded from 2021 to 2024
- Renamed test fixtures: `Kernel_Debug_Kit_26.3_build_25D5087f.dmg` → `kdk.dmg`, `upscayl-2.15.0-mac.dmg` → `upscayl.dmg`, `kdk.raw` → `hfsp.raw`, `Payload` → `payload.bin`
- Updated all workspace dependencies to latest versions

### Fixed

- Clippy fixes for Rust 2024 edition

## [0.3.3] - 2026-02-16

### Fixed

- Clippy warnings: `derivable_impls` for `ExtractMode`, `large_enum_variant` by boxing `HfsHandle`

## [0.3.2] - 2026-02-12

### Fixed

- XZ-compressed DMGs (block type `0x80000008`) now decompress correctly via udif 0.3.0

### Changed

- Updated udif dependency to 0.3.0

## [0.3.1] - 2026-02-12

### Added

- `DmgPipeline::open_filesystem_with_mode()` for explicit extraction mode control on the unified filesystem API

### Changed

- `open_filesystem()` now uses partition metadata for upfront HFS+/APFS detection instead of try-then-fallback
- `find_packages()` and `extract_pkg_payload()` now use `open_filesystem()` and support APFS DMGs

## [0.3.0] - 2026-02-11

### Added

- APFS filesystem support via `ApfsHandle`
- Auto-detecting `open_filesystem()` for HFS+ and APFS partitions
- Unified `FilesystemHandle` dispatching to either filesystem
- Unified metadata types: `FsFileStat`, `FsVolumeInfo`, `FsDirEntry`, `FsWalkEntry`

## [0.2.0] - 2026-02-11

### Changed

- Fixture-dependent integration tests now use `#[ignore]` instead of silent path-exists guards

### Added

- Self-contained unit tests for `ExtractMode` default and `DppError` display
- Updated pbzx, udif, xara and hfsplug to 0.2.0
- `parallel` feature flag forwarded from `pbzx` for parallel XZ chunk decompression

## [0.1.0] - 2026-02-10

### Added

- `DmgPipeline` API for end-to-end DMG extraction
- Automatic PKG discovery within DMG images
- One-call payload extraction (`extract_pkg_payload`)
- TempFile and InMemory extraction modes
- Unified error handling across all pipeline stages
