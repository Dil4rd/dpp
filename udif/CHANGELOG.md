# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.3.2] - 2026-02-16

### Changed

- Rust edition upgraded from 2021 to 2024
- Renamed test fixture `Kernel_Debug_Kit_26.3_build_25D5087f.dmg` → `kdk.dmg`

### Fixed

- Clippy fixes for Rust 2024 edition
- Removed `#[allow(dead_code)]` by dropping unused `options` field from `DmgReader`

## [0.3.1] - 2026-02-16

### Fixed

- Clippy warnings: `manual_strip`, `manual_div_ceil`, `op_ref`, `needless_range_loop`

## [0.3.0] - 2026-02-12

### Fixed

- Block type `0x80000008` is XZ (LZMA2) compressed, not LZVN. Renamed `BlockType::Lzvn` to `BlockType::Xz` and added proper XZ decompression via the `xz2` crate. This fixes DMGs like Google Chrome that use XZ compression.

### Changed

- `CompressionInfo::lzvn_blocks` renamed to `xz_blocks`
- Added `xz2` dependency for XZ decompression

## [0.2.0] - 2026-02-11

### Changed

- Fixture-dependent DMG tests now use `#[ignore]` instead of silent path-exists guards

## [0.1.0] - 2026-02-04

### Added

- DMG/UDIF reading with partition listing and data extraction
- DMG creation via `DmgBuilder`
- Compression support: LZFSE, XZ, Zlib, Bzip2, Raw
- Streaming decompression for partition extraction
- CRC32 checksum verification on read and write
- Cross-platform support (Windows, Linux, macOS)
