# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

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
