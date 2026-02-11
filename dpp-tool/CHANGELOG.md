# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `apfs` subcommand with `info`, `ls`, `tree`, `cat`, `stat`, `find` operations
- APFS support in `info` command — shows volume info and file listing for APFS DMGs
- APFS support in `bench` command — benchmarks APFS extraction and walk stages
- README.md with install instructions, command reference, and examples

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
