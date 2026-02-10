<div align="center">

# hfsplus

**Cross-platform Rust library for reading Apple HFS+ and HFSX filesystems**

![Version](https://img.shields.io/badge/version-0.1.0-green)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Platform](https://img.shields.io/badge/platform-windows%20%7C%20linux%20%7C%20macos-lightgrey)

Parse HFS+ / HFSX volumes from raw disk images on any platform — no kernel drivers or FUSE required.

**Pure Rust, zero unsafe** — works everywhere Rust compiles.

</div>

---

## Why hfsplus?

**hfsplus is the only pure-Rust library for reading HFS+ filesystems with B-tree traversal and extent overflow support.**

| Feature | **hfsplus** | hfsplus-rs | hfs-rs |
|---------|:-----------:|:----------:|:------:|
| HFS+ | ✓ | ✓ | ❌ |
| HFSX (case-sensitive) | ✓ | ❌ | ❌ |
| B-tree catalog | ✓ | ✓ | partial |
| Extent overflow | ✓ | ❌ | ❌ |
| Streaming reads | ✓ | ❌ | ❌ |
| Resource forks | ✓ | ❌ | ❌ |
| Unicode names | ✓ | ✓ | ❌ |
| Generic `Read+Seek` | ✓ | ❌ | ❌ |
| Zero dependencies\* | ✓ | ❌ | ❌ |

\* Only `byteorder` and `thiserror` — no compression, no FFI, no system libs.

> **Example:** macOS Kernel Debug Kit DMGs contain HFSX (case-sensitive HFS+) partitions.
> Most Rust HFS libraries can't read case-sensitive volumes — hfsplus handles both.

## Features

| | |
|---|---|
| **List directories** | Browse filesystem tree with names, sizes, timestamps |
| **Read files** | Extract file contents into memory or stream to a writer |
| **Streaming I/O** | `ForkReader` provides `Read+Seek` access without buffering |
| **File metadata** | BSD permissions, creation/modification dates, fork info |
| **Recursive walk** | Walk entire filesystem tree with full paths |
| **Path resolution** | Navigate by Unix-style paths (`/Library/Extensions/foo.kext`) |

### Format Support

| Format | Support | Description |
|--------|:-------:|-------------|
| HFS+ | ✓ | Standard Mac OS Extended |
| HFSX | ✓ | Case-sensitive variant |
| Legacy HFS | ❌ | Classic Mac OS (pre-1998) |
| APFS | ❌ | Apple File System (2017+) |

## Quick Start

### Open and Browse

```rust
use hfsplus::HfsVolume;
use std::fs::File;
use std::io::BufReader;

let file = File::open("partition.raw")?;
let mut vol = HfsVolume::open(BufReader::new(file))?;

// List root directory
for entry in vol.list_directory("/")? {
    println!("{:?} {:>12} {}", entry.kind, entry.size, entry.name);
}
```

### Read a File

```rust
// Read into memory
let data = vol.read_file("/System/Library/Kernels/kernel")?;

// Or stream to a writer (low memory)
let mut out = File::create("kernel")?;
vol.read_file_to("/System/Library/Kernels/kernel", &mut out)?;
```

### Walk Entire Filesystem

```rust
for entry in vol.walk()? {
    if entry.entry.kind == hfsplus::EntryKind::File {
        println!("{}: {} bytes", entry.path, entry.entry.size);
    }
}
```

### Streaming File Access

```rust
use std::io::Read;

// Open file for random-access Read+Seek without loading into memory
let mut reader = vol.open_file("/large-file.bin")?;
let mut buf = [0u8; 4096];
let n = reader.read(&mut buf)?;
```

### File Metadata

```rust
let stat = vol.stat("/Library/Extensions/AppleHDA.kext")?;
println!("Size: {} bytes", stat.size);
println!("Owner: {}", stat.permissions.owner_id);
println!("Mode: {:o}", stat.permissions.mode);
println!("Resource fork: {} bytes", stat.resource_fork_size);
```

## Documentation

| | |
|---|---|
| [Format Specification](docs/FORMATS.md) | HFS+ volume header, B-tree, and catalog structures |
| [Implementation Notes](docs/IMPLEMENTATION.md) | Tricky parts: Unicode, extent overflow, node parsing |

## Example Output

```
$ hfsplus-tool list partition.raw /

  Kind         Size  Name
──────────────────────────────────────────────────────
  Directory       0  .HFS+ Private Directory Data\r
  Directory       0  .Trashes
  Directory       0  Library
  Directory       0  System
  Directory       0  usr
  File        12288  .DS_Store
```

```
$ hfsplus-tool info partition.raw

HFS+ Volume Information
════════════════════════════════════════════════════════

  Signature:        HFSX (case-sensitive)
  Version:          5
  Block size:       4096 bytes
  Total blocks:     260608
  Free blocks:      6
  File count:       3,847
  Folder count:     612
  Created:          2024-01-15 08:23:41
```

## Alternatives

| Crate | HFS+ | HFSX | B-tree | Extents | Streaming | Generic R | Platform |
|-------|:----:|:----:|:------:|:-------:|:---------:|:---------:|----------|
| **hfsplus** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | All |
| [hfsplus-rs](https://github.com/penguin359/hfsplus-rs) | ✓ | ❌ | ✓ | ❌ | ❌ | ❌ | All |
| [hfs-rs](https://sr.ht/~az1/hfs-rs/) | ❌ | ❌ | partial | ❌ | ❌ | ❌ | Unix |
| [hfsfuse](https://github.com/0x09/hfsfuse) | ✓ | ✓ | ✓ | ✓ | ✓ | N/A | Unix (C) |

**Choose hfsplus if you need:**
- Pure Rust with no FFI or system dependencies
- HFSX (case-sensitive) volume support
- Extent overflow file handling for large/fragmented files
- Generic `Read+Seek` interface (works with files, memory, network streams)
- Integration with the `dpp` pipeline for DMG → HFS+ workflows

**Choose hfsfuse if you need:**
- FUSE mounting (kernel-level filesystem access)
- HFS+ compression support (zlib, lzvn, lzfse)
- Extended attributes and hard link support

## Next Steps

- [ ] **Write support** — create and modify HFS+ volumes
- [ ] **HFS+ compression** — decompress transparent compression (zlib, lzvn, lzfse)
- [ ] **Extended attributes** — read xattr data from the attributes B-tree
- [ ] **Hard links** — resolve directory and file hard links
- [ ] **Journal parsing** — read the HFS+ journal for recovery scenarios
- [ ] **APFS support** — read Apple File System containers (separate crate likely)
- [ ] **Allocation bitmap** — validate filesystem consistency

## License

MIT
