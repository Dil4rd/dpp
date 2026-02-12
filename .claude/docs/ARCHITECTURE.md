# Architecture

Rust workspace with 7 crates forming a layered pipeline for cross-platform Apple file format extraction (no macOS required).

## Crate Diagram

```
dpp-tool (CLI binary)
    ↓
dpp (pipeline library — orchestrates + re-exports all below)
    ↓           ↓           ↓           ↓           ↓
udif (DMG)   hfsplus (HFS+)   apfs (APFS)   xara (XAR/PKG)   pbzx (PBZX/CPIO)
```

## Data Flow

DMG → decompress partition → mount HFS+ or APFS filesystem → find .pkg → parse XAR archive → extract PBZX payload → decompress CPIO → individual files.

## Key Design Patterns

- **Generic I/O:** All filesystem/archive types are generic over `R: Read + Seek` (works with files, memory cursors, network streams). No trait objects — fully monomorphized.
- **Type erasure via macro:** `dpp/src/pipeline.rs` uses a `dispatch!` macro on `HfsHandle` to avoid `dyn` while supporting both `TempFile` and `InMemory` extraction modes.
- **Extraction modes:** `ExtractMode::TempFile` (default, low memory ~4KB) vs `ExtractMode::InMemory` (fast, loads full partition into `Vec<u8>`). All `open_*()` methods have `_with_mode()` variants. CLI exposes this via `--temp-file` / `--in-memory` global flags.
- **Error aggregation:** `dpp::DppError` uses `#[from]` to unify errors from all four sub-crates plus `io::Error`.
- **Zero unsafe code** in hfsplus and apfs crates.

## Crate Responsibilities

- **udif** — UDIF/DMG reader and writer. Parses KOLY headers, MISH blocks, decompresses partitions (LZFSE, LZVN, Zlib, Bzip2, Raw). CRC32 validation.
- **hfsplus** — HFS+/HFSX filesystem reader. B-tree catalog traversal, extent overflow, resource forks, Mac Roman→UTF-8 unicode.
- **xara** — XAR archive and PKG installer parser. Reads XAR header + gzip-compressed TOC XML, extracts heap entries, understands product/component/flat packages.
- **pbzx** — PBZX archive reader/writer + CPIO parser. Chunked XZ decompression. Supports CPIO odc (070707), newc (070701), crc (070702 read-only).
- **apfs** — APFS filesystem reader. Fletcher-64 checksums, checkpoint scanning, B-tree traversal, object map resolution, catalog records.
- **dpp** — Pipeline library. Chains udif→hfsplus/apfs→xara→pbzx. Provides `DmgPipeline` (with `open_filesystem()` / `open_filesystem_with_mode()`), `FilesystemHandle` (unified HFS+/APFS access), unified types (`FsType`, `FsFileStat`, `FsVolumeInfo`, `FsDirEntry`, `FsWalkEntry`, `FsEntryKind`), `find_packages()`, `extract_pkg_payload()`.
- **dpp-tool** — CLI tool with subcommands for interactive exploration of each pipeline stage. The `fs` command auto-detects HFS+ or APFS; `hfs` and `apfs` commands target specific filesystems. Global `--in-memory` / `--temp-file` flags control extraction mode.
