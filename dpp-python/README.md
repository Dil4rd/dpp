<div align="center">

# dpp

**Python bindings for the Apple DMG extraction pipeline**

[![PyPI](https://img.shields.io/pypi/v/dpp.svg)](https://pypi.org/project/dpp/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Python](https://img.shields.io/badge/python-3.9%2B-blue)
![Platform](https://img.shields.io/badge/platform-windows%20%7C%20linux%20%7C%20macos-lightgrey)

Open macOS `.dmg` disk images from Python — browse HFS+/APFS filesystems, extract `.pkg` installers, and unpack payloads.

**Native Rust speed** — powered by PyO3 bindings to the `dpp` Rust pipeline.

</div>

---

## Why dpp?

**dpp is the only Python library that handles the entire Apple package extraction pipeline natively.**

Without dpp, extracting files from a macOS `.dmg` in Python requires shelling out to multiple command-line tools — most of which only work on macOS.

| Feature | **dpp** | hdiutil + shell | dmglib (Python) |
|---------|:-------:|:---------------:|:---------------:|
| Single API | ✓ | ❌ (4+ tools) | partial |
| DMG read/write | ✓ | ✓ | read only |
| HFS+ support | ✓ | ✓ | ❌ |
| APFS support | ✓ | ✓ | ❌ |
| PKG extraction | ✓ | ✓ (xar) | ❌ |
| PBZX/CPIO | ✓ | ✓ (cpio) | ❌ |
| Cross-platform | ✓ | macOS only | partial |
| Native speed | ✓ | ✓ | ❌ |

## Features

| | |
|---|---|
| **Open DMG** | Parse UDIF disk images with LZFSE/XZ/Zlib/Bzip2 compression |
| **Browse HFS+/APFS** | Auto-detect and navigate filesystems inside the DMG |
| **Extract PKG** | Open `.pkg` installers found on the volume |
| **Unpack PBZX** | Decompress XZ payloads and parse CPIO archives |
| **Create DMG** | Build DMG files with selectable compression |
| **Create CPIO/PBZX** | Build CPIO archives and PBZX payloads |
| **Context managers** | All reader/writer classes support `with` statements |
| **Exception hierarchy** | Structured errors that map to specific failure modes |

### Pipeline

```
                 ┌─────────┐
             ┌──▶│  HFS+   │──┐
┌─────────┐  │   │ (volume)│  │   ┌─────────┐    ┌─────────┐
│  UDIF   │──┤   └─────────┘  ├──▶│   XAR   │───▶│  PBZX   │
│  (DMG)  │  │   ┌─────────┐  │   │  (PKG)  │    │ (files) │
└─────────┘  └──▶│  APFS   │──┘   └─────────┘    └─────────┘
                 │ (volume)│
                 └─────────┘
```

## Installation

```bash
pip install dpp
```

### From source (development)

```bash
cd dpp-python
pip install maturin
maturin develop
```

## Quick Start

### Browse a DMG (auto-detect filesystem)

```python
import dpp

with dpp.open("installer.dmg") as dmg:
    # List partitions
    for p in dmg.partitions:
        print(p.name, p.partition_type, p.size)

    # Open filesystem (auto-detects HFS+/APFS)
    with dmg.filesystem() as fs:
        print(fs.fs_type)       # "hfsplus" or "apfs"

        # Directory listing
        for entry in fs.list_directory("/"):
            print(entry.name, entry.kind, entry.size)

        # Read file contents
        data = fs.read_file("/some/file.txt")

        # File metadata
        stat = fs.stat("/some/file.txt")
        print(stat.size, stat.mode, stat.uid)

        # Walk entire filesystem
        for entry in fs.walk():
            print(entry.path, entry.kind)
```

### Choose Extraction Mode

```python
# In-memory mode: faster for small DMGs
with dpp.open("small.dmg") as dmg:
    fs = dmg.filesystem(mode="in_memory")

# Temp-file mode (default): low memory for large DMGs
with dpp.open("large.dmg") as dmg:
    fs = dmg.filesystem(mode="temp_file")
```

### Extract a PKG Payload

```python
with dpp.open("installer.dmg") as dmg:
    with dmg.filesystem() as fs:
        with fs.open_pkg("/path/to/package.pkg") as pkg:
            print(pkg.components)

            with pkg.payload("com.example.pkg") as payload:
                for f in payload.list():
                    print(f.path, f.size)
                data = payload.extract_file("./usr/bin/tool")
```

### One-Call Extraction

```python
# Find all .pkg files in a DMG
packages = dpp.find_packages("image.dmg")

# Extract a specific component payload in one call
archive = dpp.extract_pkg_payload(
    "image.dmg",
    "/path/to/installer.pkg",
    "com.apple.pkg.KDK",
)
for entry in archive.list():
    print(entry.path, entry.size)
```

### Create DMG Files

```python
builder = dpp.DmgBuilder()
builder.compression = "zlib"     # "raw", "zlib", "bzip2", "lzfse"
builder.compression_level = 6
builder.add_partition("disk image", partition_data)
builder.build("output.dmg")
```

### Create CPIO/PBZX Archives

```python
# Build CPIO content
cpio = dpp.CpioBuilder()
cpio.add_directory("./usr/bin", mode=0o755)
cpio.add_file("./usr/bin/hello", b"#!/bin/sh\necho hello\n", mode=0o755)
cpio.add_symlink("./usr/bin/hi", "./usr/bin/hello")
cpio_data = cpio.finish()

# Write PBZX archive
writer = dpp.PbzxWriter("output.pbzx", compression_level=6)
writer.write_cpio(cpio_data)
writer.finish()
```

### Low-Level DMG Access

```python
with dpp.DmgArchive.open("file.dmg") as archive:
    print(archive.stats)
    print(archive.compression_info)
    data = archive.extract_partition(0)
    archive.extract_partition_to(0, "/tmp/output.bin")
```

### Standalone Filesystem Access

```python
# Read raw partition images directly (no DMG wrapper)
with dpp.HfsVolume.open("partition.img") as vol:
    entries = vol.list_directory("/")
    data = vol.read_file("/some/file")

with dpp.ApfsVolume.open("apfs_partition.img") as vol:
    entries = vol.list_directory("/")
```

## Documentation

| | |
|---|---|
| [API Reference](#api-reference) | Full class and method documentation |
| [Rust Library](../dpp/) | Underlying Rust pipeline API |
| [CLI Tool](../dpp-tool/) | `dpp-tool` for interactive DMG exploration |

## API Reference

### Top-level Functions

| Function | Description |
|----------|-------------|
| `dpp.open(path)` | Open a DMG file, returns `DmgPipeline` |
| `dpp.find_packages(path)` | Find all .pkg files inside a DMG |
| `dpp.extract_pkg_payload(dmg, pkg, component)` | Extract a PKG payload in one call |

### Pipeline Classes

**`DmgPipeline`** — High-level entry point. Context manager.

| Property/Method | Description |
|----------------|-------------|
| `partitions` | List of `PartitionInfo` |
| `filesystem(mode=None)` | Open filesystem, returns `FilesystemHandle`. Mode: `"temp_file"` (default) or `"in_memory"` |

**`FilesystemHandle`** — Unified HFS+/APFS volume. Context manager.

| Property/Method | Description |
|----------------|-------------|
| `fs_type` | `"hfsplus"` or `"apfs"` |
| `volume_info` | `VolumeInfo` metadata |
| `list_directory(path)` | List entries, returns `list[DirEntry]` |
| `read_file(path)` | Read file contents, returns `bytes` |
| `stat(path)` | File metadata, returns `FileStat` |
| `walk()` | Walk all entries, returns `list[WalkEntry]` |
| `exists(path)` | Check if path exists |
| `open_pkg(path, streaming=False)` | Open a .pkg file, returns `PkgReader` |

### DMG Classes

**`DmgArchive`** — Lower-level DMG access. Context manager.

| Property/Method | Description |
|----------------|-------------|
| `stats` | `DmgStats` |
| `compression_info` | `CompressionInfo` |
| `partitions` | List of `PartitionInfo` |
| `extract_partition(id)` | Extract by ID, returns `bytes` |
| `extract_partition_by_name(name)` | Extract by name, returns `bytes` |
| `extract_partition_to(id, path)` | Extract to file |
| `extract_main_partition()` | Extract main partition, returns `bytes` |

**`DmgBuilder`** — Create DMG files.

| Property/Method | Description |
|----------------|-------------|
| `compression` | `"raw"`, `"zlib"`, `"bzip2"`, or `"lzfse"` |
| `compression_level` | 0–9 |
| `add_partition(name, data)` | Add partition data |
| `build(path)` | Write DMG to disk |

### PKG/XAR Classes

**`PkgReader`** — macOS package reader. Context manager.

| Property/Method | Description |
|----------------|-------------|
| `is_product_package` | Whether this is a distribution package |
| `components` | List of component names |
| `distribution()` | Distribution XML (if product package) |
| `package_info(component)` | PackageInfo XML |
| `payload(component)` | Extract payload, returns `Archive` |
| `payload_bytes(component)` | Raw payload bytes |
| `list_files()` | List all XAR file paths |

**`XarArchive`** — XAR archive reader. Context manager.

| Property/Method | Description |
|----------------|-------------|
| `files` | List of `XarFile` |
| `find(path)` | Find file by path |
| `read_file(index)` | Read file by index, returns `bytes` |

### Payload Classes

**`Archive`** — PBZX/CPIO payload reader. Context manager.

| Property/Method | Description |
|----------------|-------------|
| `list()` | List entries, returns `list[FileEntry]` |
| `extract_file(path)` | Extract file, returns `bytes` |
| `extract_all(dest)` | Extract all to directory |
| `decompressed_size` | Size of decompressed CPIO data |
| `cpio_data()` | Raw CPIO bytes |

**`CpioBuilder`** — Create CPIO archives.

| Method | Description |
|--------|-------------|
| `add_file(path, content, mode=0o644)` | Add a file |
| `add_directory(path, mode=0o755)` | Add a directory |
| `add_symlink(path, target, mode=0o777)` | Add a symlink |
| `finish()` | Finalize, returns `bytes` |

**`PbzxWriter`** — Create PBZX archives.

| Method | Description |
|--------|-------------|
| `write_cpio(data)` | Write CPIO data |
| `total_written` | Bytes written so far |
| `finish()` | Finalize archive |

### Filesystem Classes

**`HfsVolume`** / **`ApfsVolume`** — Standalone volume readers for raw partition images. Context managers.

| Method | Description |
|--------|-------------|
| `list_directory(path)` | List entries |
| `read_file(path)` | Read file, returns `bytes` |
| `stat(path)` | File metadata |
| `walk()` | Walk all entries |
| `exists(path)` | Check path existence |

### Data Types

All data types are immutable (frozen) Python objects with `__repr__`.

| Type | Fields |
|------|--------|
| `PartitionInfo` | `name`, `id`, `sectors`, `size`, `compressed_size`, `partition_type` |
| `DirEntry` | `name`, `kind`, `size` |
| `FileStat` | `fs_type`, `id`, `kind`, `size`, `uid`, `gid`, `mode`, `create_time`, `modify_time`, `nlink`, `data_fork_extents`, `resource_fork_size` |
| `VolumeInfo` | `fs_type`, `block_size`, `file_count`, `directory_count`, `name`, `symlink_count`, `total_blocks`, `free_blocks`, `version`, `is_hfsx` |
| `WalkEntry` | `path`, `name`, `kind`, `size` |
| `FileEntry` | `path`, `size`, `mode`, `mtime`, `uid`, `gid`, `is_dir`, `is_symlink`, `link_target` |
| `CompressionInfo` | `zero_fill_blocks`, `raw_blocks`, `zlib_blocks`, `bzip2_blocks`, `lzfse_blocks`, `xz_blocks`, `adc_blocks` |
| `DmgStats` | `version`, `sector_count`, `partition_count`, `total_uncompressed`, `total_compressed`, `data_fork_length`, `compression_ratio`, `space_savings` |
| `XarFile` | `id`, `name`, `path`, `file_type`, `size`, `compressed_size` |
| `ChunkInfo` | `index`, `offset`, `compressed_size`, `uncompressed_size`, `is_compressed`, `compression_ratio` |
| `ArchiveStats` | `chunk_count`, `compressed_size`, `uncompressed_size`, `file_count`, `directory_count`, `total_file_size`, `compression_ratio`, `space_savings` |

### Exceptions

```
DppError (base)
├── IoError               # I/O errors
├── InvalidFormatError    # bad magic, corrupt data, invalid headers
├── FileNotFoundError     # file or partition not found
├── DecompressionError    # decompression failures
└── UnsupportedError      # unsupported features/formats
```

## Example Output

```python
>>> import dpp
>>> with dpp.open("Kernel_Debug_Kit.dmg") as dmg:
...     for p in dmg.partitions:
...         print(p)
PartitionInfo(name="MBR : 0", id=-1, size=512, type="Other")
PartitionInfo(name="Primary GPT Header : 1", id=0, size=512, type="Other")
PartitionInfo(name="Apple_HFSX : 3", id=2, size=1069593600, type="Hfsx")
```

```python
>>> with dpp.open("Kernel_Debug_Kit.dmg") as dmg:
...     with dmg.filesystem() as fs:
...         print(fs.fs_type)
...         for e in fs.list_directory("/"):
...             print(e)
hfsplus
DirEntry(name="Library", kind="directory", size=0)
DirEntry(name=".DS_Store", kind="file", size=6148)
```

```python
>>> builder = dpp.DmgBuilder()
>>> builder.compression = "zlib"
>>> builder.add_partition("test", b"\x00" * 4096)
>>> builder.build("/tmp/test.dmg")
>>> with dpp.DmgArchive.open("/tmp/test.dmg") as a:
...     print(a.stats)
DmgStats(partitions=1, uncompressed=4096, compressed=30, ratio=0.01)
```

## Alternatives

| Approach | DMG | HFS+ | APFS | PKG | PBZX | Cross-platform | Language |
|----------|:---:|:----:|:----:|:---:|:----:|:--------------:|:--------:|
| **dpp** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | Python (Rust native) |
| `hdiutil` + subprocess | ✓ | ✓ | ✓ | ✓ | ✓ | macOS only | Python + shell |
| dmglib | partial | ❌ | ❌ | ❌ | ❌ | ✓ | Python |
| **dpp** Rust library | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | Rust |

**Choose dpp if you need:**
- End-to-end DMG → files extraction from Python on any platform
- Native speed without subprocess overhead
- Structured API instead of shell pipelines
- Both read and write capabilities

## Next Steps

- [ ] **Type stubs** — `.pyi` stub files for full IDE autocompletion
- [ ] **Async support** — async wrappers for I/O-heavy operations
- [ ] **Iterator protocol** — lazy iteration for `walk()` and `list()`
- [ ] **Progress callbacks** — report extraction progress
- [ ] **Wheels** — pre-built wheels for common platforms

## License

MIT
