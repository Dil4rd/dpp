# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.5.0] - 2026-09-19

### Fixed

- The writer put a constant, `0x0100000000000000`, in the header field that
  holds the chunk size, so it disagreed with the chunking it actually used and
  with every archive Apple writes. It now states the size in use

### Changed

- **Breaking:** `PbzxHeader::flags` is `PbzxHeader::chunk_size`, and
  `PbzxReader::flags` is `PbzxReader::chunk_size`. The field is the
  uncompressed chunk size, big-endian, not a flags word: on macOS 26.3
  `aa archive -a lzma -b` writes its argument there and nothing else — `1m`
  gives `0x100000`, `8m` `0x800000`, `512k` `0x80000` — and Apple's installer
  payloads carry `0x1000000`, the 16 MiB pbzx has always used
- **Breaking:** `PbzxWriter::flags` is removed. Setting it independently of
  `chunk_size` is what allowed the two to disagree

## [0.4.1] - 2026-09-10

### Changed

- Replaced `xz2` with the pure-Rust `lzma-rust2`, so the crate no longer builds
  or calls C. Streams written by `pack` stay readable by the C liblzma

## [0.4.0] - 2026-09-07

### Changed

- **Breaking:** `CpioHeader::filesize`, `CpioHeader::mtime`, `FileEntry::mtime` and
  `CpioEntry::mtime` widened from `u32` to `u64`

### Fixed

- Reject a zero-length chunk header that has data behind it. Apple's payloads end
  at EOF rather than with a marker, so treating one mid-stream as a clean end
  silently truncated the archive and reported success

- Read entry data in the format its own header declared. The streaming entry
  iterator always used the unpadded odc framing, so a newc or crc archive
  desynchronised by up to three bytes per entry and every later header was
  parsed from the wrong offset
- Keep odc `filesize` and `mtime` values above 32 bits instead of wrapping them.
  Both are 11 octal digits on the wire, a 33-bit range, and a wrapped filesize
  drives how many bytes the reader consumes

## [0.3.1] - 2026-04-12

### Changed

- Include `LICENSE` file in the published crate

## [0.3.0] - 2026-02-18

### Changed

- **Breaking:** `CpioReader::extract_all` and `Archive::extract_all` now return `ExtractStats` instead of `Vec<PathBuf>`
- Symlinks are now skipped during extraction instead of being created; the count is reported in `ExtractStats::symlinks_skipped`

### Added

- `ExtractStats` struct with `files`, `dirs`, `symlinks_skipped`, and `bytes` fields
- `CpioReader::extract_path(base_path, dest)` for path-filtered extraction — the base prefix is stripped so only the relative remainder appears under `dest`
- `Archive::extract_path(base_path, dest)` high-level path-filtered extraction

## [0.2.2] - 2026-02-16

### Changed

- Rust edition upgraded from 2021 to 2024

### Fixed

- Clippy fixes for Rust 2024 edition

## [0.2.1] - 2026-02-16

### Fixed

- Clippy warnings: `repeat().take()` → `repeat_n()`, needless borrow in example
- Rustdoc warning: escaped `Vec<u8>` in doc comment

## [0.2.0] - 2026-02-11

### Added

- Parallel XZ chunk decompression via `rayon` behind opt-in `parallel` feature flag
- `PbzxReader::decompress_parallel()` and `decompress_parallel_to()` methods
- `Archive::open()` and `Archive::from_reader()` automatically use parallel decompression when the `parallel` feature is enabled

## [0.1.0] - 2026-02-03

### Added

- PBZX archive reading with XZ stream decompression
- PBZX archive writing (`PbzxWriter`)
- CPIO parsing for odc (`070707`), newc (`070701`), and crc (`070702`) formats
- CPIO building for odc and newc formats (`CpioBuilder`)
- Seek-based header parsing for fast file listing
- Streaming extraction to filesystem
- Feature flags: `extract`, `list`, `pack`
