# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.2.4] - 2026-04-12

### Changed

- Include `LICENSE` file in the published crate

## [0.2.3] - 2026-02-21

### Fixed

- Use checked arithmetic and fallible indexing in APFS xfield parsing to prevent panics on malformed images

## [0.2.2] - 2026-02-16

### Changed

- Rust edition upgraded from 2021 to 2024

### Fixed

- Clippy fixes for Rust 2024 edition
- Removed `#[allow(clippy::too_many_arguments)]` by refactoring B-tree traversal parameters into `BTreeParams` struct

## [0.2.1] - 2026-02-16

### Fixed

- Clippy warnings: `empty_line_after_doc_comments`, `unnecessary_cast`, `too_many_arguments` allow
- Formatting drift in benchmark and source files

## [0.2.0] - 2026-02-11

### Changed

- Fixture-dependent tests now use `#[ignore]` instead of silent path-exists guards

### Added

- Self-contained unit tests for Fletcher-64 checksum, superblock magic validation,
  DrecVal parsing, and FileExtentVal length masking

## [0.1.0] - 2026-02-10

### Added

- APFS container and volume superblock parsing
- Fletcher-64 checksum verification
- Checkpoint descriptor scanning
- Object Map B-tree resolution
- Catalog B-tree traversal (inodes, directory records, file extents)
- `ApfsForkReader` with `Read + Seek` streaming I/O
- Directory listing, file reading, recursive walk
- Path resolution (Unix-style paths)
