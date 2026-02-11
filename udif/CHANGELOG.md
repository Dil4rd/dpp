# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.2.0] - 2026-02-11

### Changed

- Fixture-dependent DMG tests now use `#[ignore]` instead of silent path-exists guards

## [0.1.0] - 2026-02-04

### Added

- DMG/UDIF reading with partition listing and data extraction
- DMG creation via `DmgBuilder`
- Compression support: LZFSE, LZVN, Zlib, Bzip2, Raw
- Streaming decompression for partition extraction
- CRC32 checksum verification on read and write
- Cross-platform support (Windows, Linux, macOS)
