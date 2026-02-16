# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.2.2] - 2026-02-16

### Changed

- Rust edition upgraded from 2021 to 2024

### Fixed

- Clippy fixes for Rust 2024 edition

## [0.2.1] - 2026-02-16

### Fixed

- Clippy warning: `collapsible_if` in package component detection

## [0.2.0] - 2026-02-11

### Added

- Self-contained unit tests for header parsing, TOC XML parsing, and XAR roundtrip

## [0.1.0] - 2026-02-10

### Added

- XAR archive reading (header, TOC, heap)
- macOS PKG installer parsing (product and component packages)
- Distribution XML parsing
- PackageInfo reading
- Streaming payload extraction
- Gzip and octet-stream heap decompression
