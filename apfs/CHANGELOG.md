# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

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
