# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Enable `parallel` feature by default for multi-threaded PBZX XZ chunk decompression and DMG block decompression
- Updated dpp dependency to 0.3.5

## [0.1.4] - 2026-02-16

### Added

- Basic test suite for Python bindings (imports, error handling, CpioBuilder, DmgBuilder, context managers)

### Changed

- Rust edition upgraded from 2021 to 2024
- Updated dpp dependency to 0.3.4

## [0.1.3] - 2026-02-16

### Fixed

- Fix CI for pypi compilation and uplading
- Add `readme` field to `pyproject.toml` so PyPI displays the project description

## [0.1.2] - 2026-02-16

### Changed

- Upgrade pyo3 from 0.23 to 0.28.1 for Python 3.14 support

## [0.1.1] - 2026-02-16

### Changed

- Rename PyPI distribution from `dpp` to `dpp-py` (import name `import dpp` unchanged)

## [0.1.0] - 2026-02-15

### Added

- Pipeline API (`dpp.open()`, `DmgPipeline`, `FilesystemHandle`) for high-level DMG → filesystem → PKG workflow
- DMG access (`DmgArchive`, `DmgBuilder`) for low-level partition access and DMG creation
- PKG/XAR reading (`PkgReader`, `XarArchive`) for macOS package and XAR archive reading
- Payload reading and creation (`Archive`, `CpioBuilder`, `PbzxWriter`) for PBZX/CPIO archives
- Standalone filesystem access (`HfsVolume`, `ApfsVolume`) for raw partition images without DMG wrapper
- Frozen data types: `PartitionInfo`, `DirEntry`, `FileStat`, `VolumeInfo`, `WalkEntry`, `FileEntry`, `CompressionInfo`, `DmgStats`, `XarFile`, `ChunkInfo`, `ArchiveStats`
- Exception hierarchy: `DppError`, `IoError`, `InvalidFormatError`, `FileNotFoundError`, `DecompressionError`, `UnsupportedError`
- Context manager support (`with` statements) for all reader/writer classes
- Convenience functions: `find_packages()`, `extract_pkg_payload()`
- Extraction mode selection: `"temp_file"` (default, low memory) or `"in_memory"` (faster)
