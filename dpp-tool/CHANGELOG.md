# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.3.0] - 2026-02-11

### Added

- `fs` subcommand: unified filesystem browsing (info, ls, tree, cat, stat, find) with HFS+/APFS auto-detection
- `apfs` subcommand for direct APFS volume access
- APFS support in `info` and `bench` commands

## [0.2.0] - 2026-02-11

### Changed

- Updated dependencies to 0.2.0

## [0.1.0] - 2026-02-10

### Added

- `info` command for DMG/HFS+/PKG summary
- `dmg` command for partition listing
- `hfs` command for filesystem browsing (list, tree, read)
- `pkg` command for PKG inspection
- `payload` command for PBZX payload extraction
- `bench` command for pipeline stage benchmarking
- Styled terminal output with table formatting
