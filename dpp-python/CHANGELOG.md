# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.3.0] - 2026-09-04

### Added

- `XarFile.link` exposes XAR symbolic-link targets

### Changed

- Map `ApfsError::Unsupported` to `UnsupportedError`, alongside the existing unsupported-version and unsupported-encoding mappings

### Fixed

- `FileEntry.mtime` no longer wraps for CPIO timestamps above 32 bits
- Upgraded `pyo3` from 0.28.1 to 0.29, which patches two advisories against the
  0.28 line. Neither is fixed anywhere in 0.28, so the upgrade is the only
  remedy:
  - [RUSTSEC-2026-0176](https://rustsec.org/advisories/RUSTSEC-2026-0176.html)
    (high) — out-of-bounds read in `nth` / `nth_back` for `PyList` and
    `PyTuple` iterators, from unchecked arithmetic ahead of the bounds check
  - [GHSA-chgr-c6px-7xpp](https://github.com/advisories/GHSA-chgr-c6px-7xpp)
    (moderate) — `PyCFunction::new_closure` did not require `Sync` on closures
    that Python may call from any thread

  These bindings use neither API, so the practical exposure was nil, but the
  wheels statically link `pyo3` and 0.28.1 is additionally yanked on crates.io.
  No binding code changed: none of the 0.28 to 0.29 breaking changes apply
  here, and 0.29 keeps CPython 3.9 support and needs Rust 1.83, below the
  pinned 1.98

## [0.2.1] - 2026-04-12

### Changed

- Include `LICENSE` file in the published wheel
- Bumped dpp dependency to 0.4.2

## [0.2.0] - 2026-02-18

### Added

- New `ExtractStats` data type returned from extraction methods (fields: `files`, `dirs`, `symlinks_skipped`, `bytes`)
- `Archive.extract_path(base_path, dest)` — extract files under a specific path, with the base prefix stripped from output paths
- `XarArchive.extract_all(dest)` and `XarArchive.extract_path(base_path, dest)` — extraction methods for XAR archives
- `FilesystemHandle.extract_all(dest)` and `FilesystemHandle.extract_path(base_path, dest)` — extraction methods for HFS+/APFS filesystems
- Error handling for `InvalidPath` and `NoEntriesFound` error variants

### Changed

- **Breaking:** `Archive.extract_all(dest)` now returns `ExtractStats` instead of `list[str]`

## [0.1.5] - 2026-02-17

### Changed

- Enable `parallel` feature by default for multi-threaded PBZX XZ chunk decompression and DMG block decompression
- Release the Python GIL during expensive Rust operations (DMG open/extract, filesystem read/walk, PKG payload decompression, DMG creation) to allow other Python threads to run concurrently
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
