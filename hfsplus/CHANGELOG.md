# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-02-10

### Added

- HFS+ and HFSX (case-sensitive) volume parsing
- B-tree catalog traversal
- Extent overflow file handling
- `ForkReader` with `Read + Seek` streaming I/O
- Unicode filename decomposition
- Directory listing, file reading, recursive walk
- Path resolution (Unix-style paths)
- File metadata (BSD permissions, timestamps, fork info)
