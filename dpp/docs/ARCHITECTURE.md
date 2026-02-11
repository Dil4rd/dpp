# Architecture

Design decisions and patterns in the dpp pipeline.

## Pipeline Design

The dpp crate chains five independent libraries into a single pipeline:

```
DmgPipeline::open(path)        →  udif::DmgArchive
    ↓
pipeline.open_filesystem()      →  FilesystemHandle (auto-detects HFS+ or APFS)
  ├─ pipeline.open_hfs()        →  HfsHandle  (hfsplus::HfsVolume)
  └─ pipeline.open_apfs()       →  ApfsHandle (apfs::ApfsVolume)
    ↓
fs.open_pkg(path)               →  xara::PkgReader
    ↓
pkg.payload(component)          →  pbzx::Archive
    ↓
archive.extract_all(dest)       →  files on disk
```

Each stage is independently usable — you can stop at any level. The `open_filesystem()` method tries HFS+ first, then falls back to APFS.

## Type Erasure

The main design challenge is that `HfsVolume<R>` and `ApfsVolume<R>` are generic over the reader type, but handles need to be returned from methods that choose the reader type at runtime.

We solve this with an enum + dispatch macro pattern, used for both `HfsHandle` and `ApfsHandle`:

```rust
enum HfsHandleInner {
    File(HfsVolume<BufReader<std::fs::File>>),
    Memory(HfsVolume<Cursor<Vec<u8>>>),
}

macro_rules! dispatch {
    ($self:expr, $method:ident $(, $arg:expr)*) => {
        match &mut $self.inner {
            HfsHandleInner::File(vol) => vol.$method($($arg),*),
            HfsHandleInner::Memory(vol) => vol.$method($($arg),*),
        }
    };
}
```

This avoids `dyn Read + Seek` (which requires object safety) and keeps everything monomorphized for performance. The same pattern is used for `ApfsHandleInner` with a `dispatch_apfs!` macro.

## Unified Filesystem Layer

`FilesystemHandle` sits above both handle types and provides a filesystem-agnostic API:

```rust
enum FilesystemHandle {
    Hfs(HfsHandle),
    Apfs(ApfsHandle),
}
```

Unified types bridge the two filesystem APIs:

| Unified type | HFS+ source | APFS source |
|---|---|---|
| `FsEntryKind` | `hfsplus::EntryKind` | `apfs::EntryKind` |
| `FsDirEntry` | `hfsplus::DirEntry` | `apfs::DirEntry` |
| `FsWalkEntry` | `hfsplus::WalkEntry` | `apfs::WalkEntry` |
| `FsFileStat` | `hfsplus::FileStat` | `apfs::FileStat` |
| `FsVolumeInfo` | `hfsplus::VolumeHeader` | `apfs::VolumeInfo` |

`FsFileStat` and `FsVolumeInfo` carry common fields directly and filesystem-specific fields as `Option`s (e.g., `nlink` for APFS, `data_fork_extents` for HFS+). `FsType` discriminates which filesystem produced the data.

## Extraction Modes

| Mode | How it works | Memory | Speed |
|------|-------------|--------|-------|
| `TempFile` | Streams decompressed partition to a temp file, then opens it | ~4 KB buffer | Normal |
| `InMemory` | Decompresses entire partition into a `Vec<u8>` | Partition size | Faster |

TempFile is the default and recommended for production use. InMemory is useful for small DMGs (< 500 MB) or when you know the partition is small.

## Error Propagation

`DppError` wraps all five crate error types using `#[from]`:

```rust
pub enum DppError {
    Io(std::io::Error),
    Dmg(udif::DppError),
    Hfs(hfsplus::HfsPlusError),
    Apfs(apfs::ApfsError),
    Xar(xara::XarError),
    Pbzx(pbzx::PbzxError),
    FileNotFound(String),
    NoHfsPartition,
    NoApfsPartition,
    NoFilesystemPartition,
}
```

This gives callers a single error type for the entire pipeline while preserving the original error for debugging. `NoFilesystemPartition` is returned by `open_filesystem()` when neither HFS+ nor APFS is found.

## Streaming Design

The pipeline supports two streaming strategies to minimize memory usage. Both work identically for HFS+ and APFS volumes (and through `FilesystemHandle`):

### PKG extraction via temp file

```rust
fs.open_pkg_streaming("/path.pkg")
```

This streams the `.pkg` file from the volume to a temp file, then opens the XAR archive from that temp file. Peak memory: temp file handle + XAR TOC.

### PKG extraction in memory

```rust
fs.open_pkg("/path.pkg")
```

This reads the entire `.pkg` into memory, then opens the XAR archive from a `Cursor<Vec<u8>>`. Peak memory: full PKG size. Faster for small packages.
