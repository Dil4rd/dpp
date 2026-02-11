# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- APFS filesystem support: `ApfsHandle` for type-erased APFS volume access
- `DmgPipeline::open_apfs()` and `open_apfs_with_mode()` for APFS partition extraction
- `DmgPipeline::open_filesystem()` for auto-detecting HFS+ or APFS partitions
- `FilesystemHandle` unified enum dispatching to either `HfsHandle` or `ApfsHandle`
- Unified filesystem types: `FsDirEntry`, `FsWalkEntry`, `FsEntryKind`
- `DppError::Apfs`, `DppError::NoApfsPartition`, `DppError::NoFilesystemPartition` error variants

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
