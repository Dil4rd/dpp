# Architecture

Design decisions and patterns in the dpp pipeline.

## Pipeline Design

The dpp crate chains four independent libraries into a single pipeline:

```
DmgPipeline::open(path)      →  udif::DmgArchive
    ↓
pipeline.open_hfs()           →  hfsplus::HfsVolume (via temp file or memory)
    ↓
hfs.open_pkg(path)            →  xara::PkgReader
    ↓
pkg.payload(component)        →  pbzx::Archive
    ↓
archive.extract_all(dest)     →  files on disk
```

Each stage is independently usable — you can stop at any level.

## Type Erasure

The main design challenge is that `HfsVolume<R>` is generic over the reader type, but `HfsHandle` needs to be returned from methods that choose the reader type at runtime.

We solve this with an enum + dispatch macro:

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

This avoids `dyn Read + Seek` (which requires object safety) and keeps everything monomorphized for performance.

## Extraction Modes

| Mode | How it works | Memory | Speed |
|------|-------------|--------|-------|
| `TempFile` | Streams decompressed partition to a temp file, then opens it | ~4 KB buffer | Normal |
| `InMemory` | Decompresses entire partition into a `Vec<u8>` | Partition size | Faster |

TempFile is the default and recommended for production use. InMemory is useful for small DMGs (< 500 MB) or when you know the partition is small.

## Error Propagation

`DppError` wraps all four crate error types using `#[from]`:

```rust
pub enum DppError {
    Io(std::io::Error),
    Dmg(udif::DppError),
    Hfs(hfsplus::HfsPlusError),
    Xar(xara::XarError),
    Pbzx(pbzx::PbzxError),
    FileNotFound(String),
    NoHfsPartition,
}
```

This gives callers a single error type for the entire pipeline while preserving the original error for debugging.

## Streaming Design

The pipeline supports two streaming strategies to minimize memory usage:

### PKG extraction via temp file

```rust
hfs.open_pkg_streaming("/path.pkg")
```

This streams the `.pkg` file from the HFS+ volume to a temp file, then opens the XAR archive from that temp file. Peak memory: temp file handle + XAR TOC.

### PKG extraction in memory

```rust
hfs.open_pkg("/path.pkg")
```

This reads the entire `.pkg` into memory, then opens the XAR archive from a `Cursor<Vec<u8>>`. Peak memory: full PKG size. Faster for small packages.
