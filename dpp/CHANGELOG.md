# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

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
