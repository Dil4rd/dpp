# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `ApfsError::Unsupported` for images that use a feature the reader does not implement, keeping those cases distinct from `CorruptedData`
- `catalog::lookup_xattr` and `catalog::SYMLINK_XATTR_NAME` for reading an inode's extended attribute by name. Attributes stored as a data stream rather than embedded in the record are reported as `ApfsError::Unsupported`

### Changed

- **Breaking:** the `btree` module is no longer public. Its comparator contract fails silently when broken, so callers are served by `ApfsVolume` and the `catalog`, `omap` and `superblock` helpers, which own their comparators
- `btree_lookup` documents the ordering contract its comparator has to satisfy. Debug builds now panic when a comparator disagrees with a node's on-disk key order, instead of silently reporting a miss for a key that is present

### Fixed

- Reject file extents whose physical block number overflows the device address
  space instead of wrapping and reading from an unrelated offset

- Symlink targets are now read from the `com.apple.fs.symlink` extended attribute, where APFS stores them. `read_file` and `read_file_to` previously returned empty data for every symlink, and `stat` reported size 0, because symlink inodes carry no extents

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
