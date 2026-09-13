# Architecture

Rust workspace with 8 crates forming a layered pipeline for cross-platform Apple file format extraction (no macOS required).

## Crate Diagram

```
dpp-tool (CLI binary)         dpp-python (Python bindings, PyO3 + maturin)
    ↓                              ↓
dpp (pipeline library — orchestrates + re-exports all below)
    ↓           ↓           ↓           ↓           ↓
udif (DMG)   hfsplus (HFS+)   apfs (APFS)   xara (XAR/PKG)   pbzx (PBZX/CPIO)
                  ↓            ↓
                   cmpfs (decmpfs)
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

- **udif** — UDIF/DMG reader and writer. Parses KOLY headers, MISH blocks, decompresses partitions (LZFSE, XZ, Zlib, Bzip2, Raw). CRC32 validation.
- **hfsplus** — HFS+/HFSX filesystem reader. B-tree catalog traversal, extent overflow, resource forks, extended attributes, Mac Roman→UTF-8 unicode. The Attributes B-tree is read on first use, so a damaged one does not fail `open`.
- **xara** — XAR archive and PKG installer parser. Reads XAR header + gzip-compressed TOC XML, extracts heap entries, understands product/component/flat packages.
- **pbzx** — PBZX archive reader/writer + CPIO parser. Chunked XZ decompression. Supports CPIO odc (070707), newc (070701), crc (070702 read-only).
- **apfs** — APFS filesystem reader. Fletcher-64 checksums, checkpoint scanning, B-tree traversal, object map resolution, catalog records, extended attributes (embedded and data-stream). The `btree` module is crate-private: its comparators must reproduce on-disk key ordering, so `catalog` and `omap` own them and expose record-level operations instead.
- **cmpfs** — decmpfs decoder, shared by `hfsplus` and `apfs`. Takes the `com.apple.decmpfs` attribute and, when the type calls for it, a resource fork; returns file contents. Filesystem agnostic — it never touches a volume, so both readers keep their own storage handling. Both re-export its `Header`, `Storage`, `XattrKind` and error type.

  It also owns `classify_xattr`, the one place that knows which attribute names
  are compression machinery. `list_xattrs` tags rather than filters: hiding
  would discard what the volume says, and the condition is per-file anyway —
  `com.apple.ResourceFork` is user data on an uncompressed file, which Darwin
  models the same way in `decmpfs_hides_xattr`.

- **dpp** — Pipeline library. Chains udif→hfsplus/apfs→xara→pbzx. Provides `DmgPipeline` (with `open_filesystem()` / `open_filesystem_with_mode()`), `FilesystemHandle` (unified HFS+/APFS access), unified types (`FsType`, `FsFileStat`, `FsVolumeInfo`, `FsDirEntry`, `FsWalkEntry`, `FsEntryKind`), `find_packages()`, `extract_pkg_payload()`.
- **dpp-tool** — CLI tool with subcommands for interactive exploration of each pipeline stage. The `fs` command auto-detects HFS+ or APFS; `hfs` and `apfs` commands target specific filesystems. Global `--in-memory` / `--temp-file` flags control extraction mode.
- **dpp-python** — Python bindings via PyO3 + maturin. Wraps the `dpp` crate API as a native Python extension module (`cdylib`). Provides `dpp.open()`, `DmgPipeline`, `FilesystemHandle`, `DmgArchive`, `DmgBuilder`, `PkgReader`, `XarArchive`, `Archive`, `CpioBuilder`, `PbzxWriter`, `HfsVolume`, `ApfsVolume`, and frozen data types. Exception hierarchy maps Rust errors to Python exceptions.
