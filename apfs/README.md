<div align="center">

# apfs

**Cross-platform Rust library for reading Apple File System (APFS) containers**

![Version](https://img.shields.io/badge/version-0.1.0-green)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Platform](https://img.shields.io/badge/platform-windows%20%7C%20linux%20%7C%20macos-lightgrey)

Parse APFS volumes from raw disk images on any platform — no kernel drivers or FUSE required.

**Pure Rust, zero unsafe** — works everywhere Rust compiles.

</div>

---

## Why apfs?

**apfs is a standalone pure-Rust library for reading APFS filesystems with full B-tree traversal and object map resolution.**

| Feature | **apfs** | fal-backend-apfs | apfs-fuse | libfsapfs |
|---------|:--------:|:----------------:|:---------:|:---------:|
| Pure Rust | ✓ | ✓ | ❌ (C++) | ❌ (C) |
| Standalone | ✓ | ❌ (fal ecosystem) | ❌ (FUSE) | ❌ (system lib) |
| Generic `Read+Seek` | ✓ | ❌ | ❌ | ❌ |
| Streaming reads | ✓ | ❌ | ✓ | ✓ |
| Container parsing | ✓ | ✓ | ✓ | ✓ |
| Object map resolution | ✓ | ✓ | ✓ | ✓ |
| Catalog B-tree | ✓ | ✓ | ✓ | ✓ |
| Checkpoint scanning | ✓ | partial | ✓ | ✓ |
| Fletcher-64 checksums | ✓ | ✓ | ✓ | ✓ |
| Encryption | ❌ | ❌ | ✓ | ✓ |
| Compression | ❌ | ❌ | ✓ | ✓ |
| Permissive license | MIT | MIT | GPL-2.0 | LGPL-3.0 |

\* Only `byteorder` and `thiserror` — no compression, no FFI, no system libs.

## Features

| | |
|---|---|
| **List directories** | Browse filesystem tree with names, sizes, timestamps |
| **Read files** | Extract file contents into memory or stream to a writer |
| **Streaming I/O** | `ApfsForkReader` provides `Read+Seek` access without buffering |
| **File metadata** | BSD permissions, creation/modification dates, inode info |
| **Recursive walk** | Walk entire filesystem tree with full paths |
| **Path resolution** | Navigate by Unix-style paths (`/Applications/Upscayl.app/Contents/Info.plist`) |
| **Checksums** | Fletcher-64 verification on all on-disk objects |
| **Checkpoint scanning** | Finds latest valid container superblock |

### Format Support

| Feature | Support | Notes |
|---------|:-------:|-------|
| Read-only volumes | ✓ | Full directory listing, file reading, metadata |
| Multiple volumes | First only | Reads the first non-empty volume in the container |
| Encryption | ❌ | Encrypted volumes not supported |
| Snapshots | ❌ | Snapshot browsing not supported |
| Clones | ❌ | Clone resolution not supported |
| Compression | ❌ | Compressed extents not supported |

## Quick Start

### Open and Browse

```rust
use apfs::ApfsVolume;
use std::fs::File;
use std::io::BufReader;

let file = File::open("container.raw")?;
let mut vol = ApfsVolume::open(BufReader::new(file))?;

// Volume info
let info = vol.volume_info();
println!("{}: {} files, {} dirs", info.name, info.num_files, info.num_directories);

// List root directory
for entry in vol.list_directory("/")? {
    println!("{:?} {:>12} {}", entry.kind, entry.size, entry.name);
}
```

### Read a File

```rust
// Read into memory
let data = vol.read_file("/Applications/Upscayl.app/Contents/Info.plist")?;

// Or stream to a writer (low memory)
let mut out = File::create("Info.plist")?;
vol.read_file_to("/Applications/Upscayl.app/Contents/Info.plist", &mut out)?;
```

### Walk Entire Filesystem

```rust
for entry in vol.walk()? {
    if entry.entry.kind == apfs::EntryKind::File {
        println!("{}: {} bytes", entry.path, entry.entry.size);
    }
}
```

### Streaming File Access

```rust
use std::io::Read;

let mut reader = vol.open_file("/large-file.bin")?;
let mut buf = [0u8; 4096];
let n = reader.read(&mut buf)?;
```

### File Metadata

```rust
let stat = vol.stat("/.DS_Store")?;
println!("Size: {} bytes", stat.size);
println!("Owner: {}:{}", stat.uid, stat.gid);
println!("Mode: 0o{:o}", stat.mode);
```

## Architecture

```
Container (NXSB)
  ├── Checkpoint descriptor area → latest valid superblock
  ├── Container OMAP → resolves volume OIDs to physical blocks
  └── Volume (APSB)
        ├── Volume OMAP → resolves catalog OIDs to physical blocks
        └── Catalog B-tree (virtual, keyed by OID then type)
              ├── Inodes (type 3) — file/directory metadata
              ├── Xattrs (type 4) — extended attributes
              ├── File extents (type 8) — physical data locations
              └── Directory records (type 9) — name → inode mapping
```

### Modules

| Module | Description |
|--------|-------------|
| `fletcher` | Fletcher-64 checksum computation and verification |
| `object` | 32-byte object header parsing, type constants |
| `superblock` | Container (NXSB) and volume (APSB) superblock parsing, checkpoint scanning |
| `omap` | Object Map B-tree lookup — virtual OID to physical block |
| `btree` | Generic APFS B-tree node parsing, search, and range scan |
| `catalog` | Catalog record types: inodes, directory records, file extents, path resolution |
| `extents` | File data reading from physical extents, `ApfsForkReader` |

## Limitations

- **Read-only** — no write support
- **No encryption** — cannot read FileVault or per-file encrypted volumes
- **No compression** — transparent compression (lzvn, lzfse, zlib) not decompressed
- **No snapshots** — snapshot browsing not implemented
- **Single volume** — reads only the first volume in a multi-volume container
- **No extended attributes** — xattr values not exposed through the public API

## Next Steps

- [ ] Pipeline integration with `dpp` for DMG → APFS workflows
- [ ] Encryption support (FileVault, per-file)
- [ ] Compressed extent decompression (lzvn, lzfse, zlib)
- [ ] Snapshot browsing
- [ ] Multi-volume support
- [ ] Extended attribute access

## License

MIT
